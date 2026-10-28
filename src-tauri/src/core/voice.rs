use crate::core::audio::{self, VoiceAudioEngine};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message as WsMessage;

const CHROME_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.9168 Chrome/128.0.6613.186 Electron/32.2.2 Safari/537.36";
const VOICE_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const UDP_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(3);

/// Estado público de la conexión de voz (sin exponer secret_key al frontend)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VoiceConnectionStatus {
    pub state: String, // "disconnected" | "connecting" | "connected" | "error"
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub endpoint: Option<String>,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub ssrc: Option<u32>,
    pub encryption_mode: Option<String>,
    pub has_secret_key: bool,
    #[serde(default)]
    pub self_mute: bool,
    #[serde(default)]
    pub self_deaf: bool,
    #[serde(default)]
    pub speaking: bool,
    pub error: Option<String>,
}

/// Estado interno completo de la sesión de voz (incluyendo secret_key para Fases B y C)
#[derive(Debug, Clone, Default)]
pub struct VoiceInternalSession {
    pub status: VoiceConnectionStatus,
    pub secret_key: Vec<u8>,
}

static VOICE_SESSION: OnceLock<std::sync::Mutex<VoiceInternalSession>> = OnceLock::new();
static VOICE_ABORT_HANDLE: OnceLock<std::sync::Mutex<Option<tokio::task::AbortHandle>>> =
    OnceLock::new();

fn voice_session_lock() -> &'static std::sync::Mutex<VoiceInternalSession> {
    VOICE_SESSION.get_or_init(|| {
        std::sync::Mutex::new(VoiceInternalSession {
            status: VoiceConnectionStatus {
                state: "disconnected".to_string(),
                ..Default::default()
            },
            secret_key: Vec::new(),
        })
    })
}

fn voice_abort_lock() -> &'static std::sync::Mutex<Option<tokio::task::AbortHandle>> {
    VOICE_ABORT_HANDLE.get_or_init(|| std::sync::Mutex::new(None))
}

fn emit_voice_status(app: &tauri::AppHandle, status: &VoiceConnectionStatus) {
    let _ = app.emit("voice-connection-status", status.clone());
    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.emit("voice-connection-status", status.clone());
        if let Ok(json_str) = serde_json::to_string(status) {
            let js_code = format!(
                "window.dispatchEvent(new CustomEvent('dashcord:voice-connection-status', {{ detail: {} }}));",
                json_str
            );
            let _ = main_win.eval(&js_code);
        }
    }
    let _ = std::io::stdout().flush();
}

/// Abre el socket UDP asíncrono hacia el servidor de voz de Discord y ejecuta
/// el paquete de IP Discovery de 74 bytes (Voice v4), devolviendo el mismo `Arc<UdpSocket>`
/// para reutilizarlo en la recepción y envío de paquetes RTP (Fases B y C).
async fn open_udp_and_discover_ip(
    server_ip: &str,
    server_port: u16,
    ssrc: u32,
) -> Result<(Arc<UdpSocket>, String, u16), String> {
    let target_addr = format!("{}:{}", server_ip, server_port);

    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("No se pudo abrir socket UDP local: {}", e))?;

    socket
        .connect(&target_addr)
        .await
        .map_err(|e| format!("No se pudo conectar socket UDP a {}: {}", target_addr, e))?;

    let shared_socket = Arc::new(socket);

    // Construir paquete de 74 bytes:
    // [0..2] = 0x0001 (Request)
    // [2..4] = 0x0046 (70 bytes de payload)
    // [4..8] = SSRC en Big-Endian
    // [8..74] = 66 bytes en cero
    let mut packet = [0u8; 74];
    packet[0..2].copy_from_slice(&1u16.to_be_bytes());
    packet[2..4].copy_from_slice(&70u16.to_be_bytes());
    packet[4..8].copy_from_slice(&ssrc.to_be_bytes());

    if let Err(e) = shared_socket.send(&packet).await {
        println!("[VOICE] Aviso enviando paquete UDP IP Discovery: {}", e);
        return Ok((shared_socket, server_ip.to_string(), server_port));
    }

    let mut recv_buf = [0u8; 74];
    match timeout(UDP_DISCOVERY_TIMEOUT, shared_socket.recv(&mut recv_buf)).await {
        Ok(Ok(len)) if len >= 74 => {
            let ip_bytes = &recv_buf[8..72];
            let null_pos = ip_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(ip_bytes.len());
            let external_ip = String::from_utf8_lossy(&ip_bytes[..null_pos]).to_string();
            let external_port = u16::from_be_bytes([recv_buf[72], recv_buf[73]]);
            Ok((shared_socket, external_ip, external_port))
        }
        _ => {
            println!(
                "[VOICE] Aviso en UDP IP Discovery (usando {}:{} de respaldo)",
                server_ip, server_port
            );
            Ok((shared_socket, server_ip.to_string(), server_port))
        }
    }
}

/// Selecciona el modo de cifrado más ligero soportado por el nodo de voz,
/// priorizando `xsalsa20_poly1305_lite`.
fn select_encryption_mode(modes: Option<&Vec<serde_json::Value>>) -> String {
    let preferred_order = [
        "xsalsa20_poly1305_lite",
        "xsalsa20_poly1305_lite_rtpsize",
        "xsalsa20_poly1305_suffix",
        "xsalsa20_poly1305",
        "aead_xchacha20_poly1305_rtpsize",
        "aead_aes256_gcm_rtpsize",
    ];

    if let Some(arr) = modes {
        let available: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
        for pref in preferred_order {
            if available.contains(&pref) {
                return pref.to_string();
            }
        }
        if let Some(first) = available.first() {
            return (*first).to_string();
        }
    }

    "xsalsa20_poly1305_lite".to_string()
}

pub struct VoiceGatewayManager;

impl VoiceGatewayManager {
    pub fn get_status() -> VoiceConnectionStatus {
        voice_session_lock()
            .lock()
            .map(|g| g.status.clone())
            .unwrap_or_default()
    }

    pub fn set_mute_deaf(app: &tauri::AppHandle, muted: bool, deafened: bool) {
        let effective_mute = muted || deafened;
        VoiceAudioEngine::set_mute_deaf(effective_mute, deafened);

        let mut updated_opt = None;
        if let Ok(mut sess) = voice_session_lock().lock() {
            sess.status.self_mute = effective_mute;
            sess.status.self_deaf = deafened;
            if effective_mute {
                sess.status.speaking = false;
            }
            updated_opt = Some(sess.status.clone());
        }

        if let Some(st) = updated_opt {
            emit_voice_status(app, &st);
        }
    }

    pub fn disconnect(app: &tauri::AppHandle) {
        VoiceAudioEngine::stop();

        if let Ok(mut abort_guard) = voice_abort_lock().lock() {
            if let Some(handle) = abort_guard.take() {
                println!("[VOICE] Cerrando sesión activa del Voice Gateway...");
                handle.abort();
            }
        }

        let disconnected_status = VoiceConnectionStatus {
            state: "disconnected".to_string(),
            self_mute: VoiceAudioEngine::is_muted(),
            self_deaf: VoiceAudioEngine::is_deafened(),
            speaking: false,
            ..Default::default()
        };

        if let Ok(mut sess) = voice_session_lock().lock() {
            sess.status = disconnected_status.clone();
            sess.secret_key.clear();
        }

        emit_voice_status(app, &disconnected_status);
    }

    /// Inicia una nueva conexión WebSocket al Voice Gateway v4 de Discord
    pub fn start_voice_session(
        app: tauri::AppHandle,
        guild_id: String,
        channel_id: Option<String>,
        user_id: String,
        session_id: String,
        voice_token: String,
        endpoint: String,
    ) {
        VoiceAudioEngine::stop();

        if let Ok(mut abort_guard) = voice_abort_lock().lock() {
            if let Some(prev_handle) = abort_guard.take() {
                prev_handle.abort();
            }
        }

        let connecting_status = VoiceConnectionStatus {
            state: "connecting".to_string(),
            guild_id: Some(guild_id.clone()),
            channel_id: channel_id.clone(),
            endpoint: Some(endpoint.clone()),
            ip: None,
            port: None,
            ssrc: None,
            encryption_mode: None,
            has_secret_key: false,
            self_mute: VoiceAudioEngine::is_muted(),
            self_deaf: VoiceAudioEngine::is_deafened(),
            speaking: false,
            error: None,
        };

        if let Ok(mut sess) = voice_session_lock().lock() {
            sess.status = connecting_status.clone();
            sess.secret_key.clear();
        }
        emit_voice_status(&app, &connecting_status);

        let join_handle = tokio::spawn(async move {
            let app_clone = app.clone();
            if let Err(err) = Self::run_voice_ws(
                app,
                guild_id.clone(),
                channel_id.clone(),
                user_id,
                session_id,
                voice_token,
                endpoint.clone(),
            )
            .await
            {
                eprintln!("[VOICE ERROR] {}", err);
                let _ = std::io::stderr().flush();
                VoiceAudioEngine::stop();

                let err_status = VoiceConnectionStatus {
                    state: "error".to_string(),
                    guild_id: Some(guild_id),
                    channel_id,
                    endpoint: Some(endpoint),
                    ip: None,
                    port: None,
                    ssrc: None,
                    encryption_mode: None,
                    has_secret_key: false,
                    self_mute: VoiceAudioEngine::is_muted(),
                    self_deaf: VoiceAudioEngine::is_deafened(),
                    speaking: false,
                    error: Some(err),
                };

                if let Ok(mut sess) = voice_session_lock().lock() {
                    sess.status = err_status.clone();
                }
                emit_voice_status(&app_clone, &err_status);
            }
        });

        if let Ok(mut abort_guard) = voice_abort_lock().lock() {
            *abort_guard = Some(join_handle.abort_handle());
        }
    }

    async fn run_voice_ws(
        app: tauri::AppHandle,
        guild_id: String,
        channel_id: Option<String>,
        user_id: String,
        session_id: String,
        voice_token: String,
        endpoint: String,
    ) -> Result<(), String> {
        let clean_host = endpoint
            .trim()
            .trim_end_matches(":80")
            .trim_end_matches(":443");
        let ws_url = format!("wss://{}/?v=4", clean_host);

        println!("[VOICE] Conectando al Voice Gateway: {}", ws_url);
        let _ = std::io::stdout().flush();

        let mut request = ws_url
            .clone()
            .into_client_request()
            .map_err(|e| format!("Error creando request de Voice Gateway: {}", e))?;

        request
            .headers_mut()
            .insert("User-Agent", HeaderValue::from_static(CHROME_USER_AGENT));
        request
            .headers_mut()
            .insert("Origin", HeaderValue::from_static("https://discord.com"));

        let mut ws_config = WebSocketConfig::default();
        ws_config.max_message_size = None;
        ws_config.max_frame_size = None;

        let connect_res = timeout(
            VOICE_HANDSHAKE_TIMEOUT,
            connect_async_with_config(request, Some(ws_config), true),
        )
        .await;

        let (ws_stream, _) = match connect_res {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                return Err(format!(
                    "Fallo en handshake WebSocket con Voice Gateway ({}): {}",
                    ws_url, e
                ));
            }
            Err(_) => {
                return Err(format!(
                    "Tiempo de espera agotado conectando al Voice Gateway ({})",
                    ws_url
                ));
            }
        };

        let (mut write_half, mut read_half) = ws_stream.split();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<WsMessage>(64);

        let writer_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if write_half.send(msg).await.is_err() {
                    break;
                }
            }
        });

        let my_user_id_u64: u64 = user_id.parse().unwrap_or(0);
        let my_channel_id_u64: u64 = channel_id.as_deref().unwrap_or("").parse().unwrap_or(0);

        // Enviar Opcode 0 (IDENTIFY) inmediatamente tras abrir el WebSocket de voz
        let identify_payload = serde_json::json!({
            "op": 0,
            "d": {
                "server_id": guild_id,
                "user_id": user_id,
                "session_id": session_id,
                "token": voice_token,
                "max_dave_protocol_version": 1
            }
        });
        let _ = tx
            .send(WsMessage::Text(identify_payload.to_string().into()))
            .await;

        let mut heartbeat_task: Option<tokio::task::JoinHandle<()>> = None;
        let current_ip_port: Arc<Mutex<Option<(String, u16, u32)>>> = Arc::new(Mutex::new(None));
        let mut active_udp_socket: Option<Arc<UdpSocket>> = None;

        while let Some(msg_result) = read_half.next().await {
            let msg = match msg_result {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("[VOICE] Socket cerrado con error: {}", e);
                    break;
                }
            };

            if let WsMessage::Ping(payload) = msg {
                let _ = tx.send(WsMessage::Pong(payload)).await;
                continue;
            }

            if let WsMessage::Close(frame) = msg {
                if let Some(cf) = frame {
                    let code: u16 = cf.code.into();
                    if code == 4017 {
                        eprintln!(
                            "[VOICE] DAVE handshake falló (código 4017: Discord requiere protocolo DAVE E2EE)"
                        );
                    } else {
                        println!(
                            "[VOICE] Voice Gateway cerró conexión (código {}: {})",
                            code, cf.reason
                        );
                    }
                }
                break;
            }

            if let WsMessage::Text(ref raw_text) = msg {
                let Ok(val) = serde_json::from_str::<serde_json::Value>(raw_text) else {
                    continue;
                };

                let op = val["op"].as_u64().unwrap_or(999);

                match op {
                    // Opcode 8: HELLO (Contiene heartbeat_interval para el Voice Gateway)
                    8 => {
                        let hb_interval_ms = val["d"]["heartbeat_interval"]
                            .as_f64()
                            .map(|v| v as u64)
                            .or_else(|| val["d"]["heartbeat_interval"].as_u64())
                            .unwrap_or(13750);

                        if let Some(t) = heartbeat_task.take() {
                            t.abort();
                        }

                        let hb_tx = tx.clone();
                        let interval_dur = Duration::from_millis(hb_interval_ms);
                        let first_delay_dur =
                            Duration::from_millis((hb_interval_ms as f64 * 0.25) as u64);

                        heartbeat_task = Some(tokio::spawn(async move {
                            tokio::time::sleep(first_delay_dur).await;
                            loop {
                                let nonce = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .map(|d| d.as_millis() as u64)
                                    .unwrap_or(1000);
                                let hb_msg = serde_json::json!({
                                    "op": 3,
                                    "d": nonce
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
                    }

                    // Opcode 2: READY (Entrega IP, puerto UDP, SSRC y modos de cifrado)
                    2 => {
                        let d = &val["d"];
                        let ssrc = d["ssrc"].as_u64().unwrap_or(0) as u32;
                        let ip = d["ip"].as_str().unwrap_or("0.0.0.0").to_string();
                        let port = d["port"].as_u64().unwrap_or(0) as u16;
                        let selected_mode = select_encryption_mode(d["modes"].as_array());

                        println!(
                            "[VOICE] Conexión al Voice Gateway establecida. IP: {}, Puerto: {}",
                            ip, port
                        );
                        let _ = std::io::stdout().flush();

                        *current_ip_port.lock().await = Some((ip.clone(), port, ssrc));

                        let ready_status = VoiceConnectionStatus {
                            state: "connected".to_string(),
                            guild_id: Some(guild_id.clone()),
                            channel_id: channel_id.clone(),
                            endpoint: Some(endpoint.clone()),
                            ip: Some(ip.clone()),
                            port: Some(port),
                            ssrc: Some(ssrc),
                            encryption_mode: Some(selected_mode.clone()),
                            has_secret_key: false,
                            self_mute: VoiceAudioEngine::is_muted(),
                            self_deaf: VoiceAudioEngine::is_deafened(),
                            speaking: false,
                            error: None,
                        };
                        if let Ok(mut sess) = voice_session_lock().lock() {
                            sess.status = ready_status.clone();
                        }
                        emit_voice_status(&app, &ready_status);

                        // Abrir socket UDP persistente y ejecutar IP Discovery de 74 bytes
                        let (udp_sock, client_ip, client_port) =
                            open_udp_and_discover_ip(&ip, port, ssrc).await?;

                        println!(
                            "[VOICE] Socket UDP listo -> {}:{} (SSRC: {})",
                            client_ip, client_port, ssrc
                        );
                        let _ = std::io::stdout().flush();

                        active_udp_socket = Some(udp_sock);

                        // Enviar Opcode 1: SELECT_PROTOCOL (preferiblemente xsalsa20_poly1305_lite)
                        let select_protocol_payload = serde_json::json!({
                            "op": 1,
                            "d": {
                                "protocol": "udp",
                                "data": {
                                    "address": client_ip,
                                    "port": client_port,
                                    "mode": selected_mode
                                }
                            }
                        });

                        println!(
                            "[VOICE] Enviando SELECT_PROTOCOL con modo de cifrado: {}",
                            selected_mode
                        );
                        let _ = std::io::stdout().flush();

                        let _ = tx
                            .send(WsMessage::Text(select_protocol_payload.to_string().into()))
                            .await;
                    }

                    // Opcode 4: SESSION_DESCRIPTION (Entrega secret_key de 32 bytes y confirma modo)
                    4 => {
                        let d = &val["d"];
                        let mode = d["mode"]
                            .as_str()
                            .unwrap_or("xsalsa20_poly1305_lite")
                            .to_string();

                        let mut secret_key: Vec<u8> = Vec::new();
                        if let Some(key_arr) = d["secret_key"].as_array() {
                            for byte_val in key_arr {
                                if let Some(b) = byte_val.as_u64() {
                                    secret_key.push(b as u8);
                                }
                            }
                        }

                        println!(
                            "[VOICE] SESSION_DESCRIPTION recibido. Cifrado: {} | secret_key: {} bytes listos.",
                            mode,
                            secret_key.len()
                        );
                        let _ = std::io::stdout().flush();

                        let info_opt = current_ip_port.lock().await.clone();
                        let (ip_opt, port_opt, ssrc_opt) = match info_opt {
                            Some((i, p, s)) => (Some(i), Some(p), Some(s)),
                            None => (None, None, None),
                        };

                        let full_status = VoiceConnectionStatus {
                            state: "connected".to_string(),
                            guild_id: Some(guild_id.clone()),
                            channel_id: channel_id.clone(),
                            endpoint: Some(endpoint.clone()),
                            ip: ip_opt,
                            port: port_opt,
                            ssrc: ssrc_opt,
                            encryption_mode: Some(mode.clone()),
                            has_secret_key: !secret_key.is_empty(),
                            self_mute: VoiceAudioEngine::is_muted(),
                            self_deaf: VoiceAudioEngine::is_deafened(),
                            speaking: false,
                            error: None,
                        };

                        if let Ok(mut sess) = voice_session_lock().lock() {
                            sess.status = full_status.clone();
                            sess.secret_key = secret_key.clone();
                        }
                        emit_voice_status(&app, &full_status);

                        // Iniciar el motor de audio bidireccional (Fases B y C: Recepción + Envío con nnnoiseless, Opus y XSalsa20Poly1305)
                        if let (Some(ref udp_sock), Some(ssrc_val)) = (&active_udp_socket, ssrc_opt)
                        {
                            let app_for_speaking = app.clone();
                            VoiceAudioEngine::start(
                                udp_sock.clone(),
                                secret_key,
                                mode,
                                ssrc_val,
                                tx.clone(),
                                move |is_speaking| {
                                    let mut updated_status = None;
                                    if let Ok(mut sess) = voice_session_lock().lock() {
                                        if sess.status.speaking != is_speaking {
                                            sess.status.speaking = is_speaking;
                                            updated_status = Some(sess.status.clone());
                                        }
                                    }
                                    if let Some(st) = updated_status {
                                        emit_voice_status(&app_for_speaking, &st);
                                    }
                                },
                            );
                        }
                    }

                    // Opcode 5: SPEAKING (Mapeo SSRC -> User ID para descifrado DAVE)
                    5 => {
                        let d = &val["d"];
                        if let (Some(ssrc_val), Some(uid_str)) =
                            (d["ssrc"].as_u64(), d["user_id"].as_str())
                        {
                            if let Ok(uid_num) = uid_str.parse::<u64>() {
                                audio::ssrc_map_lock()
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .insert(ssrc_val as u32, uid_num);
                            }
                        }
                    }

                    // Opcode 6: HEARTBEAT_ACK
                    6 => {
                        tracing::debug!("[VOICE] Heartbeat ACK recibido");
                    }

                    // Opcode 21: DAVE Prepare Transition
                    21 => {
                        let transition_id = val["d"]["transition_id"].as_u64().unwrap_or(0) as u16;
                        println!("[VOICE DAVE] Preparando transición DAVE: id {}", transition_id);
                        let ready_payload = serde_json::json!({
                            "op": 23,
                            "d": {
                                "transition_id": transition_id
                            }
                        });
                        let _ = tx.send(WsMessage::Text(ready_payload.to_string().into())).await;
                    }

                    // Opcode 22: DAVE Execute Transition
                    22 => {
                        let transition_id = val["d"]["transition_id"].as_u64().unwrap_or(0) as u16;
                        println!("[VOICE DAVE] Ejecutando transición DAVE: id {}", transition_id);
                    }

                    _ => {}
                }
            }

            if let WsMessage::Binary(ref bin_data) = msg {
                let data: &[u8] = bin_data.as_ref();
                if data.is_empty() {
                    continue;
                }

                // Las tramas del servidor suelen tener 2 bytes de sequence_number + 1 byte de opcode
                let (op, payload) = if data.len() >= 3 && data[2] >= 25 && data[2] <= 31 {
                    (data[2], &data[3..])
                } else if data[0] >= 25 && data[0] <= 31 {
                    (data[0], &data[1..])
                } else {
                    (0, &data[..])
                };

                match op {
                    // Opcode 25: DAVE MLS External Sender Package
                    25 => {
                        println!(
                            "[VOICE DAVE] Recibido Opcode 25 (External Sender Package, {} bytes)",
                            payload.len()
                        );
                        let tx_dave = tx.clone();
                        let payload_vec = payload.to_vec();
                        let uid = my_user_id_u64;
                        let cid = my_channel_id_u64;

                        tokio::spawn(async move {
                            let out_opt = {
                                let mut guard = audio::dave_session_lock()
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner());
                                if guard.is_none() {
                                    if let Ok(sess) = davey::DaveSession::new(
                                        std::num::NonZeroU16::new(1).unwrap(),
                                        uid,
                                        cid,
                                        None,
                                    ) {
                                        *guard = Some(sess);
                                    }
                                }

                                if let Some(session) = guard.as_mut() {
                                    if let Err(e) = session.set_external_sender(&payload_vec) {
                                        eprintln!(
                                            "[VOICE DAVE] Error configurando external sender: {:?}",
                                            e
                                        );
                                        None
                                    } else {
                                        println!(
                                            "[VOICE DAVE] External sender configurado. Generando Key Package..."
                                        );
                                        match session.create_key_package() {
                                            Ok(key_pkg) => {
                                                println!(
                                                    "[VOICE DAVE] Enviando Opcode 26 (Key Package, {} bytes)...",
                                                    key_pkg.len()
                                                );
                                                let mut out = Vec::with_capacity(1 + key_pkg.len());
                                                out.push(26u8); // Opcode 26: DAVE MLS Key Package
                                                out.extend_from_slice(&key_pkg);
                                                Some(out)
                                            }
                                            Err(e) => {
                                                eprintln!("[VOICE DAVE] Error creando Key Package: {:?}", e);
                                                None
                                            }
                                        }
                                    }
                                } else {
                                    None
                                }
                            };

                            if let Some(out) = out_opt {
                                let _ = tx_dave.send(WsMessage::Binary(out.into())).await;
                            }
                        });
                    }

                    // Opcode 27: DAVE MLS Proposals
                    27 => {
                        println!(
                            "[VOICE DAVE] Recibido Opcode 27 (Proposals, {} bytes)",
                            payload.len()
                        );
                        if !payload.is_empty() {
                            let tx_dave = tx.clone();
                            let op_type_byte = payload[0];
                            let proposals_data = payload[1..].to_vec();
                            let op_type = if op_type_byte == 0 {
                                davey::ProposalsOperationType::APPEND
                            } else {
                                davey::ProposalsOperationType::REVOKE
                            };

                            tokio::spawn(async move {
                                let out_opt = {
                                    let mut guard = audio::dave_session_lock()
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner());
                                    if let Some(session) = guard.as_mut() {
                                        match session.process_proposals(op_type, &proposals_data, None) {
                                            Ok(Some(commit_welcome)) => {
                                                println!(
                                                    "[VOICE DAVE] Commit generado tras proposals. Enviando Opcode 28..."
                                                );
                                                let mut out = Vec::with_capacity(
                                                    1 + commit_welcome.commit.len()
                                                        + commit_welcome
                                                            .welcome
                                                            .as_ref()
                                                            .map(|w| w.len())
                                                            .unwrap_or(0),
                                                );
                                                out.push(28u8); // Opcode 28: DAVE MLS Commit Welcome
                                                out.extend_from_slice(&commit_welcome.commit);
                                                if let Some(welcome) = &commit_welcome.welcome {
                                                    out.extend_from_slice(welcome);
                                                }
                                                Some(out)
                                            }
                                            Ok(None) => {
                                                println!(
                                                    "[VOICE DAVE] Proposals procesadas, sin commit requerido."
                                                );
                                                None
                                            }
                                            Err(e) => {
                                                eprintln!("[VOICE DAVE] Error procesando proposals: {:?}", e);
                                                None
                                            }
                                        }
                                    } else {
                                        None
                                    }
                                };

                                if let Some(out) = out_opt {
                                    let _ = tx_dave.send(WsMessage::Binary(out.into())).await;
                                }
                            });
                        }
                    }

                    // Opcode 29: DAVE MLS Announce Commit Transition
                    29 => {
                        println!(
                            "[VOICE DAVE] Recibido Opcode 29 (Announce Commit Transition, {} bytes)",
                            payload.len()
                        );
                        if payload.len() >= 2 {
                            let transition_id = u16::from_be_bytes([payload[0], payload[1]]);
                            let commit_data = payload[2..].to_vec();
                            let tx_dave = tx.clone();

                            tokio::spawn(async move {
                                let ready_opt = {
                                    let mut guard = audio::dave_session_lock()
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner());
                                    if let Some(session) = guard.as_mut() {
                                        match session.process_commit(&commit_data) {
                                            Ok(()) => {
                                                println!(
                                                    "[VOICE DAVE] Commit aplicado. Sesión lista: {}",
                                                    session.is_ready()
                                                );
                                                // Enviar Opcode 23: DAVE_TRANSITION_READY
                                                let ready_payload = serde_json::json!({
                                                    "op": 23,
                                                    "d": {
                                                        "transition_id": transition_id
                                                    }
                                                });
                                                Some(ready_payload.to_string())
                                            }
                                            Err(e) => {
                                                eprintln!("[VOICE DAVE] Error procesando commit: {:?}", e);
                                                None
                                            }
                                        }
                                    } else {
                                        None
                                    }
                                };

                                if let Some(ready_str) = ready_opt {
                                    let _ = tx_dave.send(WsMessage::Text(ready_str.into())).await;
                                }
                            });
                        }
                    }

                    // Opcode 30: DAVE MLS Welcome
                    30 => {
                        println!(
                            "[VOICE DAVE] Recibido Opcode 30 (Welcome, {} bytes)",
                            payload.len()
                        );
                        if payload.len() >= 2 {
                            let transition_id = u16::from_be_bytes([payload[0], payload[1]]);
                            let welcome_data = payload[2..].to_vec();
                            let tx_dave = tx.clone();

                            tokio::spawn(async move {
                                let ready_opt = {
                                    let mut guard = audio::dave_session_lock()
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner());
                                    if let Some(session) = guard.as_mut() {
                                        match session.process_welcome(&welcome_data) {
                                            Ok(()) => {
                                                println!(
                                                    "[VOICE DAVE] Welcome aplicado. Sesión lista: {}",
                                                    session.is_ready()
                                                );
                                                // Enviar Opcode 23: DAVE_TRANSITION_READY
                                                let ready_payload = serde_json::json!({
                                                    "op": 23,
                                                    "d": {
                                                        "transition_id": transition_id
                                                    }
                                                });
                                                Some(ready_payload.to_string())
                                            }
                                            Err(e) => {
                                                eprintln!("[VOICE DAVE] Error procesando welcome: {:?}", e);
                                                None
                                            }
                                        }
                                    } else {
                                        None
                                    }
                                };

                                if let Some(ready_str) = ready_opt {
                                    let _ = tx_dave.send(WsMessage::Text(ready_str.into())).await;
                                }
                            });
                        }
                    }

                    _ => {
                        tracing::debug!("[VOICE] Binario no manejado: op {}", op);
                    }
                }
            }
        }

        VoiceAudioEngine::stop();
        if let Some(t) = heartbeat_task {
            t.abort();
        }
        writer_task.abort();
        Ok(())
    }
}
