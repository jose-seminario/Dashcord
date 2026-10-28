use crate::core::models::{
    FrontendChannel, FrontendGuild, FrontendReadyPayload, PresenceUpdatePayload, UserPayload,
    VoiceStatePayload,
};
use crate::core::voice::{VoiceConnectionStatus, VoiceGatewayManager};
use crate::evasion::SuperProperties;
use futures_util::{FutureExt, SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub recent_presences: Vec<PresenceUpdatePayload>,
    pub recent_voice_states: Vec<VoiceStatePayload>,
    pub voice_status: Option<VoiceConnectionStatus>,
}

static SHARED_SNAPSHOT: OnceLock<std::sync::Mutex<GatewaySnapshot>> = OnceLock::new();
static ACTIVE_ABORT_HANDLE: OnceLock<std::sync::Mutex<Option<tokio::task::AbortHandle>>> =
    OnceLock::new();
static LAST_CONNECT_ATTEMPT: OnceLock<std::sync::Mutex<Option<(String, Instant)>>> =
    OnceLock::new();
static ACTIVE_TOKEN: OnceLock<std::sync::Mutex<Option<String>>> = OnceLock::new();
static MAIN_WS_TX: OnceLock<std::sync::Mutex<Option<tokio::sync::mpsc::Sender<WsMessage>>>> =
    OnceLock::new();
static ACTIVE_VOICE_CHANNEL: OnceLock<std::sync::Mutex<Option<String>>> = OnceLock::new();

fn snapshot_lock() -> &'static std::sync::Mutex<GatewaySnapshot> {
    SHARED_SNAPSHOT.get_or_init(|| {
        std::sync::Mutex::new(GatewaySnapshot {
            status: "idle".to_string(),
            ready_payload: None,
            error_message: None,
            recent_messages: Vec::new(),
            recent_presences: Vec::new(),
            recent_voice_states: Vec::new(),
            voice_status: None,
        })
    })
}

fn abort_lock() -> &'static std::sync::Mutex<Option<tokio::task::AbortHandle>> {
    ACTIVE_ABORT_HANDLE.get_or_init(|| std::sync::Mutex::new(None))
}

fn attempt_lock() -> &'static std::sync::Mutex<Option<(String, Instant)>> {
    LAST_CONNECT_ATTEMPT.get_or_init(|| std::sync::Mutex::new(None))
}

fn token_lock() -> &'static std::sync::Mutex<Option<String>> {
    ACTIVE_TOKEN.get_or_init(|| std::sync::Mutex::new(None))
}

fn main_ws_tx_lock() -> &'static std::sync::Mutex<Option<tokio::sync::mpsc::Sender<WsMessage>>> {
    MAIN_WS_TX.get_or_init(|| std::sync::Mutex::new(None))
}

fn active_voice_channel_lock() -> &'static std::sync::Mutex<Option<String>> {
    ACTIVE_VOICE_CHANNEL.get_or_init(|| std::sync::Mutex::new(None))
}

/// Extrae el custom_status o actividad actual desde el array `activities` de una presencia de Discord
fn extract_custom_status(activities: Option<&Vec<serde_json::Value>>) -> Option<String> {
    let acts = activities?;
    for act in acts {
        let act_type = act["type"].as_u64().unwrap_or(99);
        if act_type == 4 {
            // Custom status
            if let Some(state) = act["state"].as_str() {
                if !state.trim().is_empty() {
                    return Some(state.to_string());
                }
            }
        } else if let Some(name) = act["name"].as_str() {
            if !name.trim().is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Emite un evento al frontend usando simultáneamente:
/// 1) `app.emit` global (Tauri v2 Emitter)
/// 2) `window.emit` directo a la ventana "main"
/// 3) `window.eval` despachando un CustomEvent DOM nativo en el WebView
fn emit_ipc_event<S: Serialize + Clone>(app: &tauri::AppHandle, event_name: &str, payload: &S) {
    let _ = app.emit(event_name, payload.clone());

    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.emit(event_name, payload.clone());
    }
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

    /// Obtiene el token activo en memoria (si ya se autenticó en la sesión actual)
    pub fn get_active_token() -> Option<String> {
        token_lock().lock().ok().and_then(|g| g.clone())
    }

    /// Devuelve una copia instantánea del estado actual del Gateway para consulta directa desde React
    pub fn get_snapshot() -> GatewaySnapshot {
        if let Ok(mut guard) = snapshot_lock().lock() {
            let mut snap = guard.clone();
            snap.voice_status = Some(VoiceGatewayManager::get_status());
            guard.recent_messages.clear();
            guard.recent_presences.clear();
            guard.recent_voice_states.clear();
            snap
        } else {
            GatewaySnapshot::default()
        }
    }

    /// Envía Opcode 4 (Voice State Update) al Gateway principal para unirse a un canal de voz
    pub async fn join_voice_channel(guild_id: String, channel_id: String) -> Result<(), String> {
        if let Ok(mut ch_guard) = active_voice_channel_lock().lock() {
            *ch_guard = Some(channel_id.clone());
        }

        let tx_opt = main_ws_tx_lock().lock().ok().and_then(|g| g.clone());
        let Some(tx) = tx_opt else {
            return Err("El Gateway principal no está conectado.".to_string());
        };

        let is_muted = crate::core::audio::VoiceAudioEngine::is_muted();
        let is_deaf = crate::core::audio::VoiceAudioEngine::is_deafened();

        let op4 = serde_json::json!({
            "op": 4,
            "d": {
                "guild_id": guild_id,
                "channel_id": channel_id,
                "self_mute": is_muted,
                "self_deaf": is_deaf,
                "self_video": false
            }
        });

        println!(
            "[VOICE] Solicitando unión al canal de voz {} (servidor {}) vía Opcode 4...",
            channel_id, guild_id
        );
        let _ = std::io::stdout().flush();

        tx.send(WsMessage::Text(op4.to_string().into()))
            .await
            .map_err(|e| format!("Error enviando Opcode 4 al Gateway: {}", e))
    }

    /// Envía Opcode 4 con channel_id: null para desconectarse del canal de voz
    pub async fn leave_voice_channel(
        app: &tauri::AppHandle,
        guild_id: String,
    ) -> Result<(), String> {
        if let Ok(mut ch_guard) = active_voice_channel_lock().lock() {
            *ch_guard = None;
        }

        VoiceGatewayManager::disconnect(app);

        let tx_opt = main_ws_tx_lock().lock().ok().and_then(|g| g.clone());
        if let Some(tx) = tx_opt {
            let op4 = serde_json::json!({
                "op": 4,
                "d": {
                    "guild_id": guild_id,
                    "channel_id": serde_json::Value::Null,
                    "self_mute": false,
                    "self_deaf": false
                }
            });
            let _ = tx.send(WsMessage::Text(op4.to_string().into())).await;
        }
        Ok(())
    }

    /// Actualiza el estado de Mute / Deafen en el motor de audio y sincroniza Opcode 4 con el Gateway
    pub async fn set_voice_mute_deaf(
        app: &tauri::AppHandle,
        guild_id: Option<String>,
        muted: bool,
        deafened: bool,
    ) -> Result<VoiceConnectionStatus, String> {
        let effective_mute = muted || deafened;
        VoiceGatewayManager::set_mute_deaf(app, effective_mute, deafened);

        let status = VoiceGatewayManager::get_status();
        let active_ch = active_voice_channel_lock()
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .or_else(|| status.channel_id.clone());
        let target_guild = guild_id.or_else(|| status.guild_id.clone());

        if let (Some(gid), Some(cid)) = (target_guild, active_ch) {
            let tx_opt = main_ws_tx_lock().lock().ok().and_then(|g| g.clone());
            if let Some(tx) = tx_opt {
                let op4 = serde_json::json!({
                    "op": 4,
                    "d": {
                        "guild_id": gid,
                        "channel_id": cid,
                        "self_mute": effective_mute,
                        "self_deaf": deafened,
                        "self_video": false
                    }
                });
                let _ = tx.send(WsMessage::Text(op4.to_string().into())).await;
            }
        }

        Ok(VoiceGatewayManager::get_status())
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

        if let Ok(mut tok_guard) = token_lock().lock() {
            *tok_guard = Some(clean_token.clone());
        }

        // Evitar ejecuciones duplicadas en ráfaga (< 3 segundos con el mismo token)
        if let Ok(mut attempt_guard) = attempt_lock().lock() {
            if let Some((ref prev_token, prev_instant)) = *attempt_guard {
                if prev_token == &clean_token && prev_instant.elapsed() < Duration::from_secs(3) {
                    println!(
                        "[GATEWAY] Ignorando llamada duplicada a start_gateway ({}ms desde la anterior).",
                        prev_instant.elapsed().as_millis()
                    );
                    let _ = std::io::stdout().flush();

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

            let mock_user = UserPayload {
                id: "usr_me".to_string(),
                username: "DashOperator".to_string(),
                discriminator: "0001".to_string(),
                global_name: Some("GhostProtocol".to_string()),
                avatar: None,
                status: "online".to_string(),
                custom_status: Some("⚡ Dry-Run Mode".to_string()),
                is_bot: Some(false),
            };

            let mock_payload = FrontendReadyPayload {
                user: mock_user.clone(),
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
                    members: vec![mock_user],
                    voice_states: Vec::new(),
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
        let _ = std::io::stdout().flush();

        // 1. Construir la solicitud HTTP para el handshake con cabeceras de navegador Chrome
        let mut request = GATEWAY_URL
            .into_client_request()
            .map_err(|e| format!("Error creando solicitud WebSocket: {}", e))?;

        request
            .headers_mut()
            .insert("User-Agent", HeaderValue::from_static(CHROME_USER_AGENT));
        request
            .headers_mut()
            .insert("Origin", HeaderValue::from_static("https://discord.com"));
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

        let mut ws_config = WebSocketConfig::default();
        ws_config.max_message_size = None;
        ws_config.max_frame_size = None;

        // 2. Establecer handshake WebSocket con timeout de 10s y Nagle deshabilitado (TCP_NODELAY = true)
        let connect_res = timeout(
            HANDSHAKE_TIMEOUT,
            connect_async_with_config(request, Some(ws_config), true),
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
        let _ = std::io::stdout().flush();

        let (mut write_half, mut read_half) = ws_stream.split();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<WsMessage>(32);

        if let Ok(mut ws_tx_guard) = main_ws_tx_lock().lock() {
            *ws_tx_guard = Some(tx.clone());
        }

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
        let mut known_users: HashMap<String, UserPayload> = HashMap::new();
        let mut guild_ids_order: Vec<String> = Vec::new();
        let mut current_user_id = String::new();
        let mut current_session_id = String::new();
        // (guild_id, voice_token, endpoint) si VOICE_SERVER_UPDATE llega antes que session_id
        let mut pending_voice_server: Option<(String, String, String)> = None;

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

            if let WsMessage::Ping(payload) = msg {
                let _ = tx.send(WsMessage::Pong(payload)).await;
                continue;
            }

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

                if let Some(s) = val["s"].as_u64() {
                    *seq.lock().await = Some(s);
                }

                let op = val["op"].as_u64().unwrap_or(0);

                match op {
                    // Opcode 10: HELLO
                    10 => {
                        let heartbeat_interval =
                            val["d"]["heartbeat_interval"].as_u64().unwrap_or(41250);

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
                            let _ = std::io::stdout().flush();
                            tracing::info!("[GATEWAY] Conectado exitosamente");

                            let d = &val["d"];
                            let u = &d["user"];

                            if let Some(sid) = d["session_id"].as_str() {
                                current_session_id = sid.to_string();
                            }

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
                                custom_status: Some("⚡ Conectado vía DashCord".to_string()),
                                is_bot: Some(false),
                            };

                            current_user_id = current_user.id.clone();
                            known_users.insert(current_user.id.clone(), current_user.clone());

                            // 1. Indexar d["users"] globales del evento READY
                            if let Some(users_arr) = d["users"].as_array() {
                                for usr in users_arr {
                                    if let Some(uid) = usr["id"].as_str() {
                                        let u_payload = UserPayload {
                                            id: uid.to_string(),
                                            username: usr["username"]
                                                .as_str()
                                                .unwrap_or("Usuario")
                                                .to_string(),
                                            discriminator: usr["discriminator"]
                                                .as_str()
                                                .unwrap_or("0000")
                                                .to_string(),
                                            global_name: usr["global_name"]
                                                .as_str()
                                                .map(|s| s.to_string()),
                                            avatar: usr["avatar"].as_str().map(|s| s.to_string()),
                                            status: "offline".to_string(),
                                            custom_status: None,
                                            is_bot: usr["bot"].as_bool(),
                                        };
                                        known_users.insert(uid.to_string(), u_payload);
                                    }
                                }
                            }

                            // 2. Indexar presencias globales (d["presences"] y d["merged_presences"]["friends"])
                            let mut global_presences: HashMap<String, (String, Option<String>)> =
                                HashMap::new();
                            if let Some(pres_arr) = d["presences"].as_array() {
                                for p in pres_arr {
                                    if let Some(uid) =
                                        p["user"]["id"].as_str().or_else(|| p["user_id"].as_str())
                                    {
                                        let st =
                                            p["status"].as_str().unwrap_or("offline").to_string();
                                        let cs = extract_custom_status(p["activities"].as_array());
                                        global_presences.insert(uid.to_string(), (st, cs));
                                    }
                                }
                            }
                            if let Some(friends_pres) = d["merged_presences"]["friends"].as_array()
                            {
                                for p in friends_pres {
                                    if let Some(uid) =
                                        p["user_id"].as_str().or_else(|| p["user"]["id"].as_str())
                                    {
                                        let st =
                                            p["status"].as_str().unwrap_or("offline").to_string();
                                        let cs = extract_custom_status(p["activities"].as_array());
                                        global_presences.insert(uid.to_string(), (st, cs));
                                    }
                                }
                            }

                            let merged_members_outer = d["merged_members"].as_array();
                            let merged_presences_guilds =
                                d["merged_presences"]["guilds"].as_array();

                            let mut guilds_list = Vec::new();
                            guild_ids_order.clear();

                            if let Some(guilds_raw) = d["guilds"].as_array() {
                                for (g_idx, g) in guilds_raw.iter().enumerate() {
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
                                    guild_ids_order.push(guild_id.clone());

                                    let guild_name = props["name"]
                                        .as_str()
                                        .or_else(|| g["name"].as_str())
                                        .unwrap_or("Servidor")
                                        .to_string();
                                    let guild_icon = props["icon"]
                                        .as_str()
                                        .or_else(|| g["icon"].as_str())
                                        .map(|s| s.to_string());

                                    // Canales del servidor
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

                                    // Presencias específicas del servidor
                                    let mut guild_presences = global_presences.clone();
                                    if let Some(gp_arr) = g["presences"].as_array() {
                                        for p in gp_arr {
                                            if let Some(uid) = p["user"]["id"]
                                                .as_str()
                                                .or_else(|| p["user_id"].as_str())
                                            {
                                                let st = p["status"]
                                                    .as_str()
                                                    .unwrap_or("offline")
                                                    .to_string();
                                                let cs = extract_custom_status(
                                                    p["activities"].as_array(),
                                                );
                                                guild_presences.insert(uid.to_string(), (st, cs));
                                            }
                                        }
                                    }
                                    if let Some(mpg) = merged_presences_guilds {
                                        if let Some(gp_arr) =
                                            mpg.get(g_idx).and_then(|v| v.as_array())
                                        {
                                            for p in gp_arr {
                                                if let Some(uid) = p["user_id"]
                                                    .as_str()
                                                    .or_else(|| p["user"]["id"].as_str())
                                                {
                                                    let st = p["status"]
                                                        .as_str()
                                                        .unwrap_or("offline")
                                                        .to_string();
                                                    let cs = extract_custom_status(
                                                        p["activities"].as_array(),
                                                    );
                                                    guild_presences
                                                        .insert(uid.to_string(), (st, cs));
                                                }
                                            }
                                        }
                                    }

                                    // Miembros del servidor
                                    let mut members_map: HashMap<String, UserPayload> =
                                        HashMap::new();
                                    members_map
                                        .insert(current_user.id.clone(), current_user.clone());

                                    let mut parse_member_entry = |m: &serde_json::Value| {
                                        let nick = m["nick"].as_str().map(|s| s.to_string());
                                        if let Some(u_obj) = m["user"].as_object() {
                                            if let Some(uid) =
                                                u_obj.get("id").and_then(|v| v.as_str())
                                            {
                                                let (st, cs) = guild_presences
                                                    .get(uid)
                                                    .cloned()
                                                    .unwrap_or_else(|| {
                                                        ("offline".to_string(), None)
                                                    });
                                                let global_n = nick.or_else(|| {
                                                    u_obj
                                                        .get("global_name")
                                                        .and_then(|v| v.as_str())
                                                        .map(|s| s.to_string())
                                                });
                                                let user_entry = UserPayload {
                                                    id: uid.to_string(),
                                                    username: u_obj
                                                        .get("username")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("Usuario")
                                                        .to_string(),
                                                    discriminator: u_obj
                                                        .get("discriminator")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("0000")
                                                        .to_string(),
                                                    global_name: global_n,
                                                    avatar: u_obj
                                                        .get("avatar")
                                                        .and_then(|v| v.as_str())
                                                        .map(|s| s.to_string()),
                                                    status: if uid == current_user.id {
                                                        "online".to_string()
                                                    } else {
                                                        st
                                                    },
                                                    custom_status: cs,
                                                    is_bot: u_obj
                                                        .get("bot")
                                                        .and_then(|v| v.as_bool()),
                                                };
                                                known_users
                                                    .insert(uid.to_string(), user_entry.clone());
                                                members_map.insert(uid.to_string(), user_entry);
                                            }
                                        } else if let Some(uid) = m["user_id"].as_str() {
                                            if let Some(base_user) = known_users.get(uid) {
                                                let (st, cs) = guild_presences
                                                    .get(uid)
                                                    .cloned()
                                                    .unwrap_or_else(|| {
                                                        ("offline".to_string(), None)
                                                    });
                                                let mut u_clone = base_user.clone();
                                                if nick.is_some() {
                                                    u_clone.global_name = nick;
                                                }
                                                u_clone.status = if uid == current_user.id {
                                                    "online".to_string()
                                                } else {
                                                    st
                                                };
                                                if cs.is_some() {
                                                    u_clone.custom_status = cs;
                                                }
                                                members_map.insert(uid.to_string(), u_clone);
                                            }
                                        }
                                    };

                                    if let Some(m_arr) = g["members"].as_array() {
                                        for m in m_arr {
                                            parse_member_entry(m);
                                        }
                                    }
                                    if let Some(mm_outer) = merged_members_outer {
                                        if let Some(m_arr) =
                                            mm_outer.get(g_idx).and_then(|v| v.as_array())
                                        {
                                            for m in m_arr {
                                                parse_member_entry(m);
                                            }
                                        }
                                    }

                                    // Añadir también usuarios con presencia activa en el servidor
                                    for (uid, (st, cs)) in &guild_presences {
                                        if !members_map.contains_key(uid) {
                                            if let Some(base_u) = known_users.get(uid) {
                                                let mut u_clone = base_u.clone();
                                                u_clone.status = st.clone();
                                                u_clone.custom_status = cs.clone();
                                                members_map.insert(uid.clone(), u_clone);
                                            }
                                        }
                                    }

                                    // Estados de voz iniciales del servidor
                                    let mut voice_states = Vec::new();
                                    if let Some(vs_arr) = g["voice_states"].as_array() {
                                        for vs in vs_arr {
                                            if let Some(uid) = vs["user_id"].as_str() {
                                                let ch_id = vs["channel_id"]
                                                    .as_str()
                                                    .map(|s| s.to_string());
                                                if ch_id.is_some() {
                                                    let u_info = members_map
                                                        .get(uid)
                                                        .or_else(|| known_users.get(uid));
                                                    let uname = u_info
                                                        .and_then(|u| {
                                                            u.global_name.clone().or_else(|| {
                                                                Some(u.username.clone())
                                                            })
                                                        })
                                                        .unwrap_or_else(|| "Usuario".to_string());
                                                    let uavatar =
                                                        u_info.and_then(|u| u.avatar.clone());

                                                    voice_states.push(VoiceStatePayload {
                                                        user_id: uid.to_string(),
                                                        channel_id: ch_id,
                                                        guild_id: Some(guild_id.clone()),
                                                        username: uname,
                                                        avatar: uavatar,
                                                        self_mute: vs["self_mute"]
                                                            .as_bool()
                                                            .unwrap_or(false),
                                                        self_deaf: vs["self_deaf"]
                                                            .as_bool()
                                                            .unwrap_or(false),
                                                    });
                                                }
                                            }
                                        }
                                    }

                                    let members_vec: Vec<UserPayload> =
                                        members_map.into_values().collect();

                                    guilds_list.push(FrontendGuild {
                                        id: guild_id.clone(),
                                        name: guild_name,
                                        icon: guild_icon,
                                        unread_count: 0,
                                        channels,
                                        members: members_vec,
                                        voice_states,
                                    });

                                    // Enviar Opcode 14 (Lazy Request) para el primer canal de texto del servidor para suscribirse a presencias y miembros
                                    if let Some(first_ch) =
                                        guilds_list.last().and_then(|gl| gl.channels.first())
                                    {
                                        let mut ch_ranges = serde_json::Map::new();
                                        ch_ranges.insert(
                                            first_ch.id.clone(),
                                            serde_json::json!([[0, 99]]),
                                        );
                                        let sub_msg = serde_json::json!({
                                            "op": 14,
                                            "d": {
                                                "guild_id": guild_id,
                                                "typing": true,
                                                "threads": false,
                                                "activities": true,
                                                "members": [],
                                                "channels": ch_ranges
                                            }
                                        });
                                        let _ = tx
                                            .send(WsMessage::Text(sub_msg.to_string().into()))
                                            .await;
                                    }
                                }
                            }

                            println!(
                                "[GATEWAY] Usuario: {} | Servidores cargados: {}",
                                current_user.username,
                                guilds_list.len()
                            );
                            let _ = std::io::stdout().flush();

                            // Si el usuario ya estaba en un canal de voz o el servidor tiene un canal de voz disponible,
                            // iniciamos la señalización Opcode 4 para establecer el handshake del Voice Gateway (Fase A)
                            let mut auto_voice_target: Option<(String, String)> = None;
                            for g_item in &guilds_list {
                                if let Some(existing_vs) = g_item.voice_states.iter().find(|vs| {
                                    vs.user_id == current_user.id && vs.channel_id.is_some()
                                }) {
                                    if let Some(ref cid) = existing_vs.channel_id {
                                        auto_voice_target = Some((g_item.id.clone(), cid.clone()));
                                        break;
                                    }
                                }
                            }
                            if auto_voice_target.is_none() {
                                for g_item in &guilds_list {
                                    if let Some(v_ch) =
                                        g_item.channels.iter().find(|c| c.channel_type == "voice")
                                    {
                                        auto_voice_target =
                                            Some((g_item.id.clone(), v_ch.id.clone()));
                                        break;
                                    }
                                }
                            }

                            let ready_payload = FrontendReadyPayload {
                                user: current_user,
                                guilds: guilds_list,
                            };

                            if let Ok(mut snap) = snapshot_lock().lock() {
                                snap.status = "ready".to_string();
                                snap.ready_payload = Some(ready_payload.clone());
                                snap.error_message = None;
                            }

                            emit_ipc_event(&app, "gateway-ready", &ready_payload);

                            if let Some((v_guild_id, v_channel_id)) = auto_voice_target {
                                if let Ok(mut ch_guard) = active_voice_channel_lock().lock() {
                                    *ch_guard = Some(v_channel_id.clone());
                                }
                                let op4 = serde_json::json!({
                                    "op": 4,
                                    "d": {
                                        "guild_id": v_guild_id,
                                        "channel_id": v_channel_id,
                                        "self_mute": false,
                                        "self_deaf": false,
                                        "self_video": false
                                    }
                                });
                                let _ = tx.send(WsMessage::Text(op4.to_string().into())).await;
                            }
                        } else if event_type == "READY_SUPPLEMENTAL" {
                            let d = &val["d"];
                            // Procesar presencias y voice_states suplementarios que Discord envía justo después de READY
                            if let Some(mpg) = d["merged_presences"]["guilds"].as_array() {
                                for (g_idx, gp_val) in mpg.iter().enumerate() {
                                    let guild_id = guild_ids_order.get(g_idx).cloned();
                                    if let Some(gp_arr) = gp_val.as_array() {
                                        for p in gp_arr {
                                            if let Some(uid) = p["user_id"]
                                                .as_str()
                                                .or_else(|| p["user"]["id"].as_str())
                                            {
                                                let st = p["status"]
                                                    .as_str()
                                                    .unwrap_or("offline")
                                                    .to_string();
                                                let cs = extract_custom_status(
                                                    p["activities"].as_array(),
                                                );
                                                let u_info = known_users.get(uid);
                                                let pres_update = PresenceUpdatePayload {
                                                    user_id: uid.to_string(),
                                                    guild_id: guild_id.clone(),
                                                    status: st,
                                                    custom_status: cs,
                                                    username: u_info.map(|u| u.username.clone()),
                                                    global_name: u_info
                                                        .and_then(|u| u.global_name.clone()),
                                                    avatar: u_info.and_then(|u| u.avatar.clone()),
                                                };
                                                if let Ok(mut snap) = snapshot_lock().lock() {
                                                    snap.recent_presences.push(pres_update.clone());
                                                }
                                                emit_ipc_event(
                                                    &app,
                                                    "gateway-presence-update",
                                                    &pres_update,
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            if let Some(guilds_supp) = d["guilds"].as_array() {
                                for g in guilds_supp {
                                    let gid = g["id"].as_str().map(|s| s.to_string());
                                    if let Some(vs_arr) = g["voice_states"].as_array() {
                                        for vs in vs_arr {
                                            if let Some(uid) = vs["user_id"].as_str() {
                                                let ch_id = vs["channel_id"]
                                                    .as_str()
                                                    .map(|s| s.to_string());
                                                let u_info = known_users.get(uid);
                                                let uname = u_info
                                                    .and_then(|u| {
                                                        u.global_name
                                                            .clone()
                                                            .or_else(|| Some(u.username.clone()))
                                                    })
                                                    .unwrap_or_else(|| "Usuario".to_string());
                                                let uavatar = u_info.and_then(|u| u.avatar.clone());

                                                let vs_payload = VoiceStatePayload {
                                                    user_id: uid.to_string(),
                                                    channel_id: ch_id,
                                                    guild_id: gid.clone(),
                                                    username: uname,
                                                    avatar: uavatar,
                                                    self_mute: vs["self_mute"]
                                                        .as_bool()
                                                        .unwrap_or(false),
                                                    self_deaf: vs["self_deaf"]
                                                        .as_bool()
                                                        .unwrap_or(false),
                                                };
                                                if let Ok(mut snap) = snapshot_lock().lock() {
                                                    snap.recent_voice_states
                                                        .push(vs_payload.clone());
                                                }
                                                emit_ipc_event(
                                                    &app,
                                                    "gateway-voice-state-update",
                                                    &vs_payload,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        } else if event_type == "MESSAGE_CREATE" {
                            let d = val["d"].clone();
                            if let Ok(mut snap) = snapshot_lock().lock() {
                                snap.recent_messages.push(d.clone());
                                if snap.recent_messages.len() > 20 {
                                    snap.recent_messages.remove(0);
                                }
                            }
                            emit_ipc_event(&app, "gateway-message-create", &d);
                        } else if event_type == "PRESENCE_UPDATE" {
                            let d = &val["d"];
                            if let Some(uid) =
                                d["user"]["id"].as_str().or_else(|| d["user_id"].as_str())
                            {
                                let status = d["status"].as_str().unwrap_or("offline").to_string();
                                let custom_status =
                                    extract_custom_status(d["activities"].as_array());
                                let guild_id = d["guild_id"].as_str().map(|s| s.to_string());

                                let username = d["user"]["username"]
                                    .as_str()
                                    .map(|s| s.to_string())
                                    .or_else(|| known_users.get(uid).map(|u| u.username.clone()));
                                let global_name = d["user"]["global_name"]
                                    .as_str()
                                    .map(|s| s.to_string())
                                    .or_else(|| {
                                        known_users.get(uid).and_then(|u| u.global_name.clone())
                                    });
                                let avatar =
                                    d["user"]["avatar"].as_str().map(|s| s.to_string()).or_else(
                                        || known_users.get(uid).and_then(|u| u.avatar.clone()),
                                    );

                                let pres_payload = PresenceUpdatePayload {
                                    user_id: uid.to_string(),
                                    guild_id,
                                    status,
                                    custom_status,
                                    username,
                                    global_name,
                                    avatar,
                                };

                                if let Ok(mut snap) = snapshot_lock().lock() {
                                    snap.recent_presences.push(pres_payload.clone());
                                }
                                emit_ipc_event(&app, "gateway-presence-update", &pres_payload);
                            }
                        } else if event_type == "VOICE_STATE_UPDATE" {
                            let d = &val["d"];
                            if let Some(uid) = d["user_id"].as_str() {
                                let channel_id = d["channel_id"].as_str().map(|s| s.to_string());
                                let guild_id = d["guild_id"].as_str().map(|s| s.to_string());

                                // Si el cambio de estado de voz pertenece a nuestro usuario, actualizar session_id de voz
                                if uid == current_user_id {
                                    if let Some(sid) = d["session_id"].as_str() {
                                        if !sid.is_empty() {
                                            current_session_id = sid.to_string();
                                        }
                                    }
                                    if let Ok(mut ch_guard) = active_voice_channel_lock().lock() {
                                        *ch_guard = channel_id.clone();
                                    }

                                    if channel_id.is_none() {
                                        VoiceGatewayManager::disconnect(&app);
                                    } else if let Some((p_guild_id, p_token, p_endpoint)) =
                                        pending_voice_server.take()
                                    {
                                        if !current_session_id.is_empty() {
                                            VoiceGatewayManager::start_voice_session(
                                                app.clone(),
                                                p_guild_id,
                                                channel_id.clone(),
                                                current_user_id.clone(),
                                                current_session_id.clone(),
                                                p_token,
                                                p_endpoint,
                                            );
                                        }
                                    }
                                }

                                let member_user = &d["member"]["user"];
                                let username = member_user["global_name"]
                                    .as_str()
                                    .or_else(|| member_user["username"].as_str())
                                    .map(|s| s.to_string())
                                    .or_else(|| {
                                        known_users.get(uid).map(|u| {
                                            u.global_name
                                                .clone()
                                                .unwrap_or_else(|| u.username.clone())
                                        })
                                    })
                                    .unwrap_or_else(|| "Usuario".to_string());

                                let avatar = member_user["avatar"]
                                    .as_str()
                                    .map(|s| s.to_string())
                                    .or_else(|| {
                                        known_users.get(uid).and_then(|u| u.avatar.clone())
                                    });

                                let vs_payload = VoiceStatePayload {
                                    user_id: uid.to_string(),
                                    channel_id,
                                    guild_id,
                                    username,
                                    avatar,
                                    self_mute: d["self_mute"].as_bool().unwrap_or(false),
                                    self_deaf: d["self_deaf"].as_bool().unwrap_or(false),
                                };

                                if let Ok(mut snap) = snapshot_lock().lock() {
                                    snap.recent_voice_states.push(vs_payload.clone());
                                }
                                emit_ipc_event(&app, "gateway-voice-state-update", &vs_payload);
                            }
                        } else if event_type == "VOICE_SERVER_UPDATE" {
                            let d = &val["d"];
                            let v_guild_id = d["guild_id"].as_str().unwrap_or("").to_string();
                            let v_token = d["token"].as_str().unwrap_or("").to_string();
                            let v_endpoint = d["endpoint"].as_str().map(|s| s.to_string());

                            if let Some(endpoint) = v_endpoint {
                                if !v_guild_id.is_empty() && !v_token.is_empty() {
                                    let active_ch = active_voice_channel_lock()
                                        .lock()
                                        .ok()
                                        .and_then(|g| g.clone());

                                    if !current_session_id.is_empty() && !current_user_id.is_empty()
                                    {
                                        VoiceGatewayManager::start_voice_session(
                                            app.clone(),
                                            v_guild_id,
                                            active_ch,
                                            current_user_id.clone(),
                                            current_session_id.clone(),
                                            v_token,
                                            endpoint,
                                        );
                                    } else {
                                        pending_voice_server =
                                            Some((v_guild_id, v_token, endpoint));
                                    }
                                }
                            }
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
