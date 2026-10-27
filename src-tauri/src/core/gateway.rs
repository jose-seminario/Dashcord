use crate::core::models::{FrontendChannel, FrontendGuild, FrontendReadyPayload, UserPayload};
use crate::evasion::SuperProperties;
use futures_util::{FutureExt, SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message as WsMessage;

const GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";
const CHROME_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.9168 Chrome/128.0.6613.186 Electron/32.2.2 Safari/537.36";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Estado compartido accesible por comando IPC directo (fallback a prueba de fallos)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatewaySnapshot {
    pub status: String, // "idle" | "connecting" | "ready" | "error"
    pub ready_payload: Option<FrontendReadyPayload>,
    pub error_message: Option<String>,
    pub recent_messages: Vec<serde_json::Value>,
}

static SHARED_SNAPSHOT: OnceLock<std::sync::Mutex<GatewaySnapshot>> = OnceLock::new();
static ACTIVE_ABORT_HANDLE: OnceLock<std::sync::Mutex<Option<tokio::task::AbortHandle>>> =
    OnceLock::new();
static LAST_CONNECT_ATTEMPT: OnceLock<std::sync::Mutex<Option<(String, Instant)>>> =
    OnceLock::new();

fn snapshot_lock() -> &'static std::sync::Mutex<GatewaySnapshot> {
    SHARED_SNAPSHOT.get_or_init(|| {
        std::sync::Mutex::new(GatewaySnapshot {
            status: "idle".to_string(),
            ready_payload: None,
            error_message: None,
            recent_messages: Vec::new(),
        })
    })
}

fn abort_lock() -> &'static std::sync::Mutex<Option<tokio::task::AbortHandle>> {
    ACTIVE_ABORT_HANDLE.get_or_init(|| std::sync::Mutex::new(None))
}

fn attempt_lock() -> &'static std::sync::Mutex<Option<(String, Instant)>> {
    LAST_CONNECT_ATTEMPT.get_or_init(|| std::sync::Mutex::new(None))
}

/// Emite un evento al frontend usando simultáneamente:
/// 1) `app.emit` global (Tauri v2 Emitter)
/// 2) `window.emit` directo a la ventana "main"
/// 3) `window.eval` despachando un CustomEvent DOM nativo en el WebView
fn emit_ipc_event<S: Serialize + Clone>(app: &tauri::AppHandle, event_name: &str, payload: &S) {
    match app.emit(event_name, payload.clone()) {
        Ok(_) => println!("[IPC] Evento '{}' emitido globalmente (app.emit OK)", event_name),
        Err(e) => eprintln!("[IPC WARN] Error en app.emit('{}'): {}", event_name, e),
    }

    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.emit(event_name, payload.clone());

        if let Ok(json_str) = serde_json::to_string(payload) {
            let js_code = format!(
                "window.dispatchEvent(new CustomEvent('dashcord:{}', {{ detail: {} }}));",
                event_name, json_str
            );
            let _ = main_win.eval(&js_code);
        }
    }
    let _ = std::io::stdout().flush();
}

/// Generador de jitter pseudoaleatorio basado en nanosegundos y xorshift
/// Devuelve un factor entre 0.05 y 0.95
fn random_jitter() -> f64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(424242);
    let mut x = (nanos ^ (nanos >> 32)) as u64;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    let val = (x % 10000) as f64 / 10000.0;
    0.05 + 0.90 * val
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayState {
    pub session_id: Option<String>,
    pub sequence: Option<u64>,
    pub resume_gateway_url: Option<String>,
    pub heartbeat_interval_ms: u64,
    pub is_dry_run: bool,
}

impl Default for GatewayState {
    fn default() -> Self {
        Self {
            session_id: None,
            sequence: None,
            resume_gateway_url: None,
            heartbeat_interval_ms: 41250,
            is_dry_run: false,
        }
    }
}

pub struct GatewayManager {
    pub state: GatewayState,
}

impl GatewayManager {
    pub fn new(is_dry_run: bool) -> Self {
        Self {
            state: GatewayState {
                is_dry_run,
                ..Default::default()
            },
        }
    }

    /// Devuelve una copia instantánea del estado actual del Gateway para consulta directa desde React
    pub fn get_snapshot() -> GatewaySnapshot {
        if let Ok(mut guard) = snapshot_lock().lock() {
            let snap = guard.clone();
            // Limpiar cola de mensajes recientes una vez entregados al frontend
            guard.recent_messages.clear();
            snap
        } else {
            GatewaySnapshot::default()
        }
    }

    /// Mantiene compatibilidad con las pruebas unitarias del backend
    pub async fn start_session(&mut self) -> Result<String, String> {
        if self.state.is_dry_run {
            println!("[DRY-RUN] Passive mode active. No outbound network sockets created.");
            println!("[DRY-RUN] Simulated READY received (session_id: dry_run_session_001)");
            let _ = std::io::stdout().flush();
            self.state.session_id = Some("dry_run_session_001".to_string());
            return Ok("dry_run_session_001".to_string());
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok("session_active".to_string())
    }

    /// Inicia la conexión real al Gateway v10 asegurando una única instancia activa
    pub async fn start_gateway(app: tauri::AppHandle, raw_token: String, is_dry_run: bool) {
        let clean_token = raw_token
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim()
            .to_string();

        // Evitar ejecuciones duplicadas en ráfaga (< 3 segundos con el mismo token)
        if let Ok(mut attempt_guard) = attempt_lock().lock() {
            if let Some((ref prev_token, prev_instant)) = *attempt_guard {
                if prev_token == &clean_token && prev_instant.elapsed() < Duration::from_secs(3) {
                    println!(
                        "[GATEWAY] Ignorando llamada duplicada a start_gateway ({}ms desde la anterior).",
                        prev_instant.elapsed().as_millis()
                    );
                    let _ = std::io::stdout().flush();

                    // Si ya teníamos el payload READY listo, re-emitirlo inmediatamente al frontend
                    if let Ok(snap) = snapshot_lock().lock() {
                        if snap.status == "ready" {
                            if let Some(ref ready_data) = snap.ready_payload {
                                emit_ipc_event(&app, "gateway-ready", ready_data);
                            }
                        }
                    }
                    return;
                }
            }
            *attempt_guard = Some((clean_token.clone(), Instant::now()));
        }

        // Si había una tarea anterior del Gateway corriendo, abortarla limpiamente antes de abrir otra
        if let Ok(mut abort_guard) = abort_lock().lock() {
            if let Some(prev_handle) = abort_guard.take() {
                println!("[GATEWAY] Abortando sesión anterior del Gateway antes de reconectar...");
                prev_handle.abort();
            }
        }

        // Actualizar snapshot a "connecting"
        if let Ok(mut snap) = snapshot_lock().lock() {
            snap.status = "connecting".to_string();
            snap.error_message = None;
        }

        let join_handle = tokio::spawn(async move {
            let app_clone = app.clone();
            let result = AssertUnwindSafe(Self::connect_and_listen(app, clean_token, is_dry_run))
                .catch_unwind()
                .await;

            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    eprintln!("[GATEWAY ERROR] Detenido con error: {}", e);
                    let _ = std::io::stderr().flush();
                    if let Ok(mut snap) = snapshot_lock().lock() {
                        snap.status = "error".to_string();
                        snap.error_message = Some(e.clone());
                    }
                    let err_payload = serde_json::json!({ "message": e });
                    emit_ipc_event(&app_clone, "gateway-error", &err_payload);
                }
                Err(panic_payload) => {
                    let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                        (*s).to_string()
                    } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "Fallo crítico inesperado en tarea del Gateway".to_string()
                    };
                    let full_err = format!("Error interno del Gateway: {}", panic_msg);
                    eprintln!("[GATEWAY PANIC] {}", full_err);
                    let _ = std::io::stderr().flush();
                    if let Ok(mut snap) = snapshot_lock().lock() {
                        snap.status = "error".to_string();
                        snap.error_message = Some(full_err.clone());
                    }
                    let err_payload = serde_json::json!({ "message": full_err });
                    emit_ipc_event(&app_clone, "gateway-error", &err_payload);
                }
            }
        });

        if let Ok(mut abort_guard) = abort_lock().lock() {
            *abort_guard = Some(join_handle.abort_handle());
        }
    }

    async fn connect_and_listen(
        app: tauri::AppHandle,
        token: String,
        is_dry_run: bool,
    ) -> Result<(), String> {
        if token.is_empty() {
            return Err("El token proporcionado está vacío.".to_string());
        }

        if is_dry_run {
            println!("[DRY-RUN] Starting DashCord in dry-run mode");
            println!("[DRY-RUN] Passive mode active. No outbound network sockets created.");
            println!("[DRY-RUN] Simulated READY received");
            println!("[GATEWAY] Conectado exitosamente");
            let _ = std::io::stdout().flush();

            let mock_payload = FrontendReadyPayload {
                user: UserPayload {
                    id: "usr_me".to_string(),
                    username: "DashOperator".to_string(),
                    discriminator: "0001".to_string(),
                    global_name: Some("GhostProtocol".to_string()),
                    avatar: None,
                    status: "online".to_string(),
                },
                guilds: vec![FrontendGuild {
                    id: "guild_1".to_string(),
                    name: "Cyberpunk Syndicate".to_string(),
                    icon: None,
                    unread_count: 1,
                    channels: vec![
                        FrontendChannel {
                            id: "c_general".to_string(),
                            guild_id: Some("guild_1".to_string()),
                            name: "general-chat".to_string(),
                            channel_type: "text".to_string(),
                            topic: Some("Canal principal de DashCord".to_string()),
                            unread: false,
                        },
                        FrontendChannel {
                            id: "c_dev".to_string(),
                            guild_id: Some("guild_1".to_string()),
                            name: "rust-dev".to_string(),
                            channel_type: "text".to_string(),
                            topic: None,
                            unread: false,
                        },
                    ],
                }],
            };

            if let Ok(mut snap) = snapshot_lock().lock() {
                snap.status = "ready".to_string();
                snap.ready_payload = Some(mock_payload.clone());
                snap.error_message = None;
            }
            emit_ipc_event(&app, "gateway-ready", &mock_payload);
            return Ok(());
        }

        println!("[NET] Connecting to {}", GATEWAY_URL);
        eprintln!("[NET] Connecting to {}", GATEWAY_URL);
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();

        // 1. Construir la solicitud HTTP para el handshake con cabeceras de navegador Chrome
        let mut request = GATEWAY_URL
            .into_client_request()
            .map_err(|e| format!("Error creando solicitud WebSocket: {}", e))?;

        request
            .headers_mut()
            .insert("User-Agent", HeaderValue::from_static(CHROME_USER_AGENT));
        request.headers_mut().insert(
            "Origin",
            HeaderValue::from_static("https://discord.com"),
        );
        request.headers_mut().insert(
            "Accept-Language",
            HeaderValue::from_static("es-ES,es;q=0.9,en;q=0.8"),
        );
        request
            .headers_mut()
            .insert("Cache-Control", HeaderValue::from_static("no-cache"));
        request
            .headers_mut()
            .insert("Pragma", HeaderValue::from_static("no-cache"));

        // Configuración sin límite de tamaño de frame/mensaje para soportar payloads READY masivos
        let mut ws_config = WebSocketConfig::default();
        ws_config.max_message_size = None;
        ws_config.max_frame_size = None;

        // 2. Establecer handshake WebSocket con timeout de 10s
        let connect_res = timeout(
            HANDSHAKE_TIMEOUT,
            connect_async_with_config(request, Some(ws_config), false),
        )
        .await;

        let (ws_stream, _) = match connect_res {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                let err = format!("Error en handshake con Discord Gateway: {}", e);
                eprintln!("[GATEWAY ERROR] {}", err);
                let _ = std::io::stderr().flush();
                return Err(err);
            }
            Err(_) => {
                let err =
                    "Tiempo de espera agotado al conectar con Discord Gateway (10s)".to_string();
                eprintln!("[GATEWAY ERROR] {}", err);
                let _ = std::io::stderr().flush();
                return Err(err);
            }
        };

        println!("[GATEWAY] Conexión establecida, esperando HELLO...");
        eprintln!("[GATEWAY] Conexión establecida, esperando HELLO...");
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();

        let (mut write_half, mut read_half) = ws_stream.split();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<WsMessage>(32);

        // 3. Tarea para enviar mensajes WebSocket de manera thread-safe
        let writer_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = write_half.send(msg).await {
                    eprintln!("[GATEWAY] Error escribiendo en socket: {}", e);
                    let _ = std::io::stderr().flush();
                    break;
                }
            }
        });

        let seq = Arc::new(Mutex::new(None::<u64>));
        let mut heartbeat_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut ready_received = false;
        let mut error_emitted = false;

        // 4. Bucle principal de lectura del Gateway
        while let Some(msg_result) = read_half.next().await {
            let msg = match msg_result {
                Ok(m) => m,
                Err(e) => {
                    let err_msg = format!("Conexión interrumpida con el Gateway: {}", e);
                    eprintln!("[GATEWAY ERROR] {}", err_msg);
                    let _ = std::io::stderr().flush();
                    if !ready_received {
                        if let Ok(mut snap) = snapshot_lock().lock() {
                            snap.status = "error".to_string();
                            snap.error_message = Some(err_msg.clone());
                        }
                        let payload = serde_json::json!({ "message": err_msg });
                        emit_ipc_event(&app, "gateway-error", &payload);
                        error_emitted = true;
                    }
                    break;
                }
            };

            // Responder a tramas Ping del servidor
            if let WsMessage::Ping(payload) = msg {
                let _ = tx.send(WsMessage::Pong(payload)).await;
                continue;
            }

            // Manejo de Close Frame
            if let WsMessage::Close(ref frame) = msg {
                let err_msg = if let Some(cf) = frame {
                    let code: u16 = cf.code.into();
                    let reason = cf.reason.to_string();
                    if code == 4004 {
                        "Token inválido o expirado".to_string()
                    } else {
                        format!("Conexión cerrada por Discord (código {}: {})", code, reason)
                    }
                } else {
                    "El Gateway cerró la conexión inesperadamente.".to_string()
                };

                println!("[GATEWAY ERROR] {}", err_msg);
                eprintln!("[GATEWAY ERROR] {}", err_msg);
                let _ = std::io::stderr().flush();

                if let Ok(mut snap) = snapshot_lock().lock() {
                    snap.status = "error".to_string();
                    snap.error_message = Some(err_msg.clone());
                }
                let payload = serde_json::json!({ "message": err_msg });
                emit_ipc_event(&app, "gateway-error", &payload);
                error_emitted = true;
                break;
            }

            if let WsMessage::Text(raw_text) = msg {
                let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw_text) else {
                    continue;
                };

                // Actualizar número de secuencia si viene en el payload
                if let Some(s) = val["s"].as_u64() {
                    *seq.lock().await = Some(s);
                }

                let op = val["op"].as_u64().unwrap_or(0);

                match op {
                    // Opcode 10: HELLO
                    10 => {
                        let heartbeat_interval = val["d"]["heartbeat_interval"]
                            .as_u64()
                            .unwrap_or(41250);

                        let jitter = random_jitter();
                        let first_delay_ms = ((heartbeat_interval as f64) * jitter) as u64;
                        println!(
                            "[GATEWAY] HELLO recibido. Intervalo: {}ms, Primer latido con jitter en: {}ms. Enviando IDENTIFY...",
                            heartbeat_interval,
                            first_delay_ms
                        );
                        let _ = std::io::stdout().flush();

                        if let Some(t) = heartbeat_task.take() {
                            t.abort();
                        }

                        let hb_tx = tx.clone();
                        let hb_seq = seq.clone();
                        let interval_dur = Duration::from_millis(heartbeat_interval);
                        let first_delay_dur = Duration::from_millis(first_delay_ms);

                        heartbeat_task = Some(tokio::spawn(async move {
                            tokio::time::sleep(first_delay_dur).await;
                            loop {
                                let current_s = *hb_seq.lock().await;
                                let hb_msg = serde_json::json!({
                                    "op": 1,
                                    "d": current_s
                                });
                                if hb_tx
                                    .send(WsMessage::Text(hb_msg.to_string().into()))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                                tokio::time::sleep(interval_dur).await;
                            }
                        }));

                        let super_props = SuperProperties::default();
                        let identify_msg = serde_json::json!({
                            "op": 2,
                            "d": {
                                "token": token,
                                "capabilities": 30717,
                                "properties": {
                                    "os": super_props.os,
                                    "browser": super_props.browser,
                                    "release_channel": super_props.release_channel,
                                    "client_version": super_props.client_version,
                                    "os_version": super_props.os_version,
                                    "os_arch": super_props.os_arch,
                                    "system_locale": super_props.system_locale,
                                    "client_build_number": super_props.client_build_number,
                                    "client_event_source": super_props.client_event_source,
                                    "design_id": super_props.design_id
                                },
                                "presence": {
                                    "status": "online",
                                    "since": 0,
                                    "activities": [],
                                    "afk": false
                                },
                                "compress": false
                            }
                        });

                        let _ = tx
                            .send(WsMessage::Text(identify_msg.to_string().into()))
                            .await;
                    }

                    // Opcode 1: Heartbeat solicitado por el servidor
                    1 => {
                        let current_s = *seq.lock().await;
                        let hb_reply = serde_json::json!({
                            "op": 1,
                            "d": current_s
                        });
                        let _ = tx.send(WsMessage::Text(hb_reply.to_string().into())).await;
                    }

                    // Opcode 11: Heartbeat ACK
                    11 => {
                        tracing::debug!("[GATEWAY] Heartbeat ACK recibido");
                    }

                    // Opcode 9: Sesión inválida / Token no autorizado
                    9 => {
                        let err_msg = "Token inválido o expirado".to_string();
                        println!("[GATEWAY ERROR] Opcode 9 (Invalid Session): {}", err_msg);
                        eprintln!("[GATEWAY ERROR] Opcode 9 (Invalid Session): {}", err_msg);
                        let _ = std::io::stderr().flush();
                        if let Ok(mut snap) = snapshot_lock().lock() {
                            snap.status = "error".to_string();
                            snap.error_message = Some(err_msg.clone());
                        }
                        let payload = serde_json::json!({ "message": err_msg });
                        emit_ipc_event(&app, "gateway-error", &payload);
                        error_emitted = true;
                        break;
                    }

                    // Opcode 7: Reconexión solicitada
                    7 => {
                        println!("[GATEWAY] El servidor solicita reconexión (Opcode 7)");
                        break;
                    }

                    // Opcode 0: Dispatch
                    0 => {
                        let event_type = val["t"].as_str().unwrap_or("");

                        if event_type == "READY" {
                            ready_received = true;

                            println!("[GATEWAY] Conectado exitosamente");
                            eprintln!("[GATEWAY] Conectado exitosamente");
                            let _ = std::io::stdout().flush();
                            let _ = std::io::stderr().flush();
                            tracing::info!("[GATEWAY] Conectado exitosamente");

                            let d = &val["d"];
                            let u = &d["user"];

                            let current_user = UserPayload {
                                id: u["id"].as_str().unwrap_or("").to_string(),
                                username: u["username"].as_str().unwrap_or("").to_string(),
                                discriminator: u["discriminator"]
                                    .as_str()
                                    .unwrap_or("0000")
                                    .to_string(),
                                global_name: u["global_name"].as_str().map(|s| s.to_string()),
                                avatar: u["avatar"].as_str().map(|s| s.to_string()),
                                status: "online".to_string(),
                            };

                            let mut guilds_list = Vec::new();
                            if let Some(guilds_raw) = d["guilds"].as_array() {
                                for g in guilds_raw {
                                    let props = if g["properties"].is_object() {
                                        &g["properties"]
                                    } else {
                                        g
                                    };

                                    let guild_id = g["id"]
                                        .as_str()
                                        .or_else(|| props["id"].as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let guild_name = props["name"]
                                        .as_str()
                                        .or_else(|| g["name"].as_str())
                                        .unwrap_or("Servidor")
                                        .to_string();
                                    let guild_icon = props["icon"]
                                        .as_str()
                                        .or_else(|| g["icon"].as_str())
                                        .map(|s| s.to_string());

                                    let mut channels = Vec::new();
                                    if let Some(ch_raw) = g["channels"].as_array() {
                                        for ch in ch_raw {
                                            let ch_type_num = ch["type"].as_u64().unwrap_or(0);
                                            if ch_type_num == 4 {
                                                continue;
                                            }

                                            let ch_type = match ch_type_num {
                                                2 | 13 => "voice",
                                                5 => "announcement",
                                                _ => "text",
                                            };

                                            channels.push(FrontendChannel {
                                                id: ch["id"].as_str().unwrap_or("").to_string(),
                                                guild_id: Some(guild_id.clone()),
                                                name: ch["name"]
                                                    .as_str()
                                                    .unwrap_or("canal")
                                                    .to_string(),
                                                channel_type: ch_type.to_string(),
                                                topic: ch["topic"].as_str().map(|s| s.to_string()),
                                                unread: false,
                                            });
                                        }
                                    }

                                    guilds_list.push(FrontendGuild {
                                        id: guild_id,
                                        name: guild_name,
                                        icon: guild_icon,
                                        unread_count: 0,
                                        channels,
                                    });
                                }
                            }

                            println!(
                                "[GATEWAY] Usuario: {} | Servidores cargados: {}",
                                current_user.username,
                                guilds_list.len()
                            );
                            let _ = std::io::stdout().flush();

                            let ready_payload = FrontendReadyPayload {
                                user: current_user,
                                guilds: guilds_list,
                            };

                            // Guardar en el snapshot compartido ANTES de emitir el evento
                            if let Ok(mut snap) = snapshot_lock().lock() {
                                snap.status = "ready".to_string();
                                snap.ready_payload = Some(ready_payload.clone());
                                snap.error_message = None;
                            }

                            emit_ipc_event(&app, "gateway-ready", &ready_payload);
                        } else if event_type == "MESSAGE_CREATE" {
                            let d = val["d"].clone();
                            if let Ok(mut snap) = snapshot_lock().lock() {
                                snap.recent_messages.push(d.clone());
                            }
                            emit_ipc_event(&app, "gateway-message-create", &d);
                        }
                    }

                    _ => {}
                }
            }
        }

        if !ready_received && !error_emitted {
            let err_msg =
                "La conexión con el Gateway se cerró antes de recibir READY. Verifica tu token."
                    .to_string();
            if let Ok(mut snap) = snapshot_lock().lock() {
                snap.status = "error".to_string();
                snap.error_message = Some(err_msg.clone());
            }
            let payload = serde_json::json!({ "message": err_msg });
            emit_ipc_event(&app, "gateway-error", &payload);
        }

        if let Some(t) = heartbeat_task {
            t.abort();
        }
        writer_task.abort();
        println!("[GATEWAY] Conexión cerrada limpiamente");
        let _ = std::io::stdout().flush();
        Ok(())
    }
}
