use chacha20poly1305::{
    aead::{AeadInPlace, KeyInit},
    Key as ChaChaKey, XChaCha20Poly1305, XNonce as ChaChaXNonce,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use nnnoiseless::DenoiseState;
use opus::{
    Application as OpusApp, Bitrate as OpusBitrate, Channels as OpusChannels,
    Decoder as OpusDecoder, Encoder as OpusEncoder, Signal as OpusSignal,
};
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use xsalsa20poly1305::{Key as SalsaKey, Nonce as SalsaNonce, XSalsa20Poly1305};

const TARGET_SAMPLE_RATE: u32 = 48_000;
const OPUS_FRAME_SAMPLES_MONO: usize = 960; // 20ms @ 48kHz
const OPUS_FRAME_SAMPLES_STEREO: usize = 960 * 2; // 20ms @ 48kHz interleaved stereo
const DENOISE_FRAME_SAMPLES: usize = DenoiseState::FRAME_SIZE; // 480 samples (10ms @ 48kHz)
const MAX_JITTER_SAMPLES: usize = OPUS_FRAME_SAMPLES_STEREO * 8; // ~160ms max buffer
const PREBUFFER_SAMPLES: usize = OPUS_FRAME_SAMPLES_STEREO * 2; // ~40ms pre-buffer
const OPUS_SILENCE_FRAME: [u8; 3] = [0xF8, 0xFF, 0xFE];

static IS_MUTED: AtomicBool = AtomicBool::new(false);
static IS_DEAFENED: AtomicBool = AtomicBool::new(false);
static IS_SPEAKING: AtomicBool = AtomicBool::new(false);

pub struct AudioPipelineHandle {
    stop_flag: Arc<AtomicBool>,
    rx_task: tokio::task::AbortHandle,
}

static ACTIVE_AUDIO_PIPELINE: OnceLock<std::sync::Mutex<Option<AudioPipelineHandle>>> =
    OnceLock::new();

fn pipeline_lock() -> &'static std::sync::Mutex<Option<AudioPipelineHandle>> {
    ACTIVE_AUDIO_PIPELINE.get_or_init(|| std::sync::Mutex::new(None))
}

static DAVE_SESSION: OnceLock<std::sync::Mutex<Option<davey::DaveSession>>> = OnceLock::new();
static SSRC_TO_USER_ID: OnceLock<std::sync::Mutex<HashMap<u32, u64>>> = OnceLock::new();

pub fn dave_session_lock() -> &'static std::sync::Mutex<Option<davey::DaveSession>> {
    DAVE_SESSION.get_or_init(|| std::sync::Mutex::new(None))
}

pub fn ssrc_map_lock() -> &'static std::sync::Mutex<HashMap<u32, u64>> {
    SSRC_TO_USER_ID.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Buffer de reproducción con protección básica contra Jitter y subdesbordamiento (underrun)
struct PlaybackJitterBuffer {
    samples: VecDeque<f32>, // Muestras estéreo intercaladas (L, R, L, R...) a 48kHz
    primed: bool,
}

impl PlaybackJitterBuffer {
    fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(MAX_JITTER_SAMPLES * 2),
            primed: false,
        }
    }

    fn push_stereo_frame(&mut self, frame: &[f32]) {
        // Si el buffer acumuló demasiada latencia (> 160ms), descartar muestras viejas
        if self.samples.len() + frame.len() > MAX_JITTER_SAMPLES {
            let excess = (self.samples.len() + frame.len()) - (MAX_JITTER_SAMPLES / 2);
            let drain_count = (excess / 2) * 2; // Mantener alineación par (L/R)
            self.samples.drain(..drain_count.min(self.samples.len()));
        }
        self.samples.extend(frame.iter().copied());
        if self.samples.len() >= PREBUFFER_SAMPLES {
            self.primed = true;
        }
    }

    fn pop_stereo_pair(&mut self) -> (f32, f32) {
        if !self.primed {
            return (0.0, 0.0);
        }
        if self.samples.len() < 2 {
            self.primed = false;
            return (0.0, 0.0);
        }
        let l = self.samples.pop_front().unwrap_or(0.0);
        let r = self.samples.pop_front().unwrap_or(0.0);
        (l, r)
    }
}

/// Estado de decodificación Opus por usuario remoto (identificado por SSRC)
struct PeerDecoderState {
    decoder: OpusDecoder,
    last_seq: Option<u16>,
    pcm_buf: Vec<f32>,
}

impl PeerDecoderState {
    fn new() -> Result<Self, String> {
        let decoder = OpusDecoder::new(TARGET_SAMPLE_RATE, OpusChannels::Stereo)
            .map_err(|e| format!("Error creando decodificador Opus: {:?}", e))?;
        Ok(Self {
            decoder,
            last_seq: None,
            pcm_buf: vec![0.0f32; OPUS_FRAME_SAMPLES_STEREO],
        })
    }

    /// Verifica si el número de secuencia RTP es más reciente que el anterior (ignorando paquetes viejos/duplicados)
    fn accept_sequence(&mut self, seq: u16) -> bool {
        if let Some(prev) = self.last_seq {
            let diff = seq.wrapping_sub(prev);
            // Si diff == 0 (duplicado) o diff > 32768 (paquete viejo fuera de orden), ignorarlo
            if diff == 0 || diff > 32768 {
                return false;
            }
        }
        self.last_seq = Some(seq);
        true
    }
}

/// Cifra un frame Opus en un paquete RTP listo para enviar por UDP según el modo negociado
fn encrypt_rtp_opus_packet(
    mode: &str,
    secret_key: &[u8; 32],
    rtp_header: &[u8; 12],
    opus_payload: &[u8],
    nonce_counter: u32,
) -> Result<Vec<u8>, String> {
    if mode.contains("xchacha20") {
        let key = ChaChaKey::from_slice(secret_key);
        let cipher = XChaCha20Poly1305::new(key);

        let mut nonce_bytes = [0u8; 24];
        let counter_be = nonce_counter.to_be_bytes();
        nonce_bytes[0..4].copy_from_slice(&counter_be);
        let nonce = ChaChaXNonce::from_slice(&nonce_bytes);

        let mut buffer = opus_payload.to_vec();
        cipher
            .encrypt_in_place(nonce, rtp_header, &mut buffer)
            .map_err(|_| "Error cifrando con XChaCha20Poly1305".to_string())?;

        let mut packet = Vec::with_capacity(12 + buffer.len() + 4);
        packet.extend_from_slice(rtp_header);
        packet.extend_from_slice(&buffer);
        packet.extend_from_slice(&counter_be);
        return Ok(packet);
    }

    let key = SalsaKey::from_slice(secret_key);
    let cipher = XSalsa20Poly1305::new(key);

    if mode == "xsalsa20_poly1305" {
        let mut nonce_bytes = [0u8; 24];
        nonce_bytes[0..12].copy_from_slice(rtp_header);
        let nonce = SalsaNonce::from_slice(&nonce_bytes);

        let mut buffer = opus_payload.to_vec();
        cipher
            .encrypt_in_place(nonce, b"", &mut buffer)
            .map_err(|_| "Error cifrando con xsalsa20_poly1305".to_string())?;

        let mut packet = Vec::with_capacity(12 + buffer.len());
        packet.extend_from_slice(rtp_header);
        packet.extend_from_slice(&buffer);
        Ok(packet)
    } else if mode == "xsalsa20_poly1305_suffix" {
        let mut nonce_bytes = [0u8; 24];
        nonce_bytes[0..4].copy_from_slice(&nonce_counter.to_be_bytes());
        nonce_bytes[4..16].copy_from_slice(rtp_header);
        let nonce = SalsaNonce::from_slice(&nonce_bytes);

        let mut buffer = opus_payload.to_vec();
        cipher
            .encrypt_in_place(nonce, b"", &mut buffer)
            .map_err(|_| "Error cifrando con xsalsa20_poly1305_suffix".to_string())?;

        let mut packet = Vec::with_capacity(12 + buffer.len() + 24);
        packet.extend_from_slice(rtp_header);
        packet.extend_from_slice(&buffer);
        packet.extend_from_slice(&nonce_bytes);
        Ok(packet)
    } else {
        // Por defecto: xsalsa20_poly1305_lite (o xsalsa20_poly1305_lite_rtpsize)
        let mut nonce_bytes = [0u8; 24];
        let counter_be = nonce_counter.to_be_bytes();
        nonce_bytes[0..4].copy_from_slice(&counter_be);
        let nonce = SalsaNonce::from_slice(&nonce_bytes);

        let mut buffer = opus_payload.to_vec();
        cipher
            .encrypt_in_place(nonce, b"", &mut buffer)
            .map_err(|_| "Error cifrando con xsalsa20_poly1305_lite".to_string())?;

        let mut packet = Vec::with_capacity(12 + buffer.len() + 4);
        packet.extend_from_slice(rtp_header);
        packet.extend_from_slice(&buffer);
        packet.extend_from_slice(&counter_be);
        Ok(packet)
    }
}

/// Descifra un paquete RTP entrante y extrae el frame Opus limpio (removiendo cabeceras de extensión RTP si existen)
fn decrypt_rtp_opus_packet(
    mode: &str,
    secret_key: &[u8; 32],
    packet: &[u8],
) -> Option<(u16, u32, Vec<u8>)> {
    if packet.len() < 28 {
        return None;
    }

    let b0 = packet[0];
    let b1 = packet[1];

    // Verificar versión RTP 2 ((b0 >> 6) == 2) e ignorar paquetes RTCP (payload types 200..=204)
    if (b0 >> 6) != 2 || (200..=204).contains(&b1) {
        return None;
    }

    let has_extension = (b0 & 0x10) != 0;
    let csrc_count = (b0 & 0x0F) as usize;
    let seq = u16::from_be_bytes([packet[2], packet[3]]);
    let ssrc = u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]);

    let base_header_len = 12 + csrc_count * 4;
    if packet.len() <= base_header_len + 16 {
        return None;
    }

    let is_rtpsize_mode = mode.ends_with("_rtpsize");
    let unencrypted_header_len = if is_rtpsize_mode && has_extension {
        base_header_len + 4
    } else {
        base_header_len
    };

    if packet.len() <= unencrypted_header_len + 16 {
        return None;
    }

    let mut decrypted_buf: Vec<u8>;

    if mode.contains("xchacha20") {
        if packet.len() < unencrypted_header_len + 16 + 4 {
            return None;
        }
        let cipher_end = packet.len() - 4;
        let mut nonce_bytes = [0u8; 24];
        nonce_bytes[0..4].copy_from_slice(&packet[cipher_end..]);
        let nonce = ChaChaXNonce::from_slice(&nonce_bytes);

        let key = ChaChaKey::from_slice(secret_key);
        let cipher = XChaCha20Poly1305::new(key);

        let aad = &packet[..unencrypted_header_len];
        decrypted_buf = packet[unencrypted_header_len..cipher_end].to_vec();
        if cipher
            .decrypt_in_place(nonce, aad, &mut decrypted_buf)
            .is_err()
        {
            return None;
        }
    } else {
        let key = SalsaKey::from_slice(secret_key);
        let cipher = XSalsa20Poly1305::new(key);

        if mode == "xsalsa20_poly1305" {
            let mut nonce_bytes = [0u8; 24];
            nonce_bytes[0..12].copy_from_slice(&packet[0..12]);
            let nonce = SalsaNonce::from_slice(&nonce_bytes);

            decrypted_buf = packet[base_header_len..].to_vec();
            if cipher
                .decrypt_in_place(nonce, b"", &mut decrypted_buf)
                .is_err()
            {
                return None;
            }
        } else if mode == "xsalsa20_poly1305_suffix" {
            if packet.len() < base_header_len + 16 + 24 {
                return None;
            }
            let cipher_end = packet.len() - 24;
            let nonce = SalsaNonce::from_slice(&packet[cipher_end..]);

            decrypted_buf = packet[base_header_len..cipher_end].to_vec();
            if cipher
                .decrypt_in_place(nonce, b"", &mut decrypted_buf)
                .is_err()
            {
                return None;
            }
        } else {
            // xsalsa20_poly1305_lite (o _rtpsize)
            if packet.len() < unencrypted_header_len + 16 + 4 {
                return None;
            }
            let cipher_end = packet.len() - 4;
            let mut nonce_bytes = [0u8; 24];
            nonce_bytes[0..4].copy_from_slice(&packet[cipher_end..]);
            let nonce = SalsaNonce::from_slice(&nonce_bytes);

            decrypted_buf = packet[unencrypted_header_len..cipher_end].to_vec();
            if cipher
                .decrypt_in_place(nonce, b"", &mut decrypted_buf)
                .is_err()
            {
                // Respaldo: si el emisor usó offset de extensión distinto, probar base_header_len
                if unencrypted_header_len != base_header_len && packet.len() > base_header_len + 20
                {
                    let mut alt_buf = packet[base_header_len..cipher_end].to_vec();
                    if cipher.decrypt_in_place(nonce, b"", &mut alt_buf).is_ok() {
                        decrypted_buf = alt_buf;
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
        }
    }

    // Si el paquete RTP tenía el bit de extensión activo, saltar la extensión para dejar solo el payload Opus
    let mut opus_offset = 0usize;
    if has_extension {
        if is_rtpsize_mode && packet.len() >= base_header_len + 4 {
            let ext_words =
                u16::from_be_bytes([packet[base_header_len + 2], packet[base_header_len + 3]])
                    as usize;
            opus_offset = ext_words * 4;
        } else if decrypted_buf.len() >= 4 && decrypted_buf[0] == 0xBE && decrypted_buf[1] == 0xDE {
            let ext_words = u16::from_be_bytes([decrypted_buf[2], decrypted_buf[3]]) as usize;
            opus_offset = 4 + ext_words * 4;
        }
    }

    if opus_offset >= decrypted_buf.len() {
        return None;
    }

    Some((seq, ssrc, decrypted_buf[opus_offset..].to_vec()))
}

/// Envía el paquete Opcode 5 (SPEAKING) al Voice Gateway
fn send_speaking_opcode(ws_tx: &tokio::sync::mpsc::Sender<WsMessage>, ssrc: u32, speaking: bool) {
    let payload = serde_json::json!({
        "op": 5,
        "d": {
            "speaking": if speaking { 1 } else { 0 },
            "delay": 0,
            "ssrc": ssrc
        }
    });
    let _ = ws_tx.try_send(WsMessage::Text(payload.to_string().into()));
}

pub struct VoiceAudioEngine;

impl VoiceAudioEngine {
    pub fn is_muted() -> bool {
        IS_MUTED.load(Ordering::Relaxed)
    }

    pub fn is_deafened() -> bool {
        IS_DEAFENED.load(Ordering::Relaxed)
    }

    pub fn is_speaking() -> bool {
        IS_SPEAKING.load(Ordering::Relaxed)
    }

    pub fn set_mute_deaf(muted: bool, deafened: bool) {
        let effective_mute = muted || deafened;
        IS_MUTED.store(effective_mute, Ordering::Relaxed);
        IS_DEAFENED.store(deafened, Ordering::Relaxed);
        if effective_mute {
            IS_SPEAKING.store(false, Ordering::Relaxed);
        }
    }

    pub fn stop() {
        IS_SPEAKING.store(false, Ordering::Relaxed);
        if let Ok(mut guard) = pipeline_lock().lock() {
            if let Some(handle) = guard.take() {
                handle.stop_flag.store(true, Ordering::Relaxed);
                handle.rx_task.abort();
                println!("[VOICE AUDIO] Motor de audio bidireccional detenido limpiamente.");
                let _ = std::io::stdout().flush();
            }
        }
        if let Ok(mut dave) = dave_session_lock().lock() {
            *dave = None;
        }
        if let Ok(mut map) = ssrc_map_lock().lock() {
            map.clear();
        }
    }

    /// Inicia la recepción (Fase B) y transmisión (Fase C) de audio bidireccional sobre el socket UDP
    pub fn start<F>(
        udp_socket: Arc<UdpSocket>,
        secret_key_vec: Vec<u8>,
        mode: String,
        ssrc: u32,
        ws_tx: tokio::sync::mpsc::Sender<WsMessage>,
        on_speaking_change: F,
    ) where
        F: Fn(bool) + Send + Sync + 'static,
    {
        Self::stop();

        if secret_key_vec.len() < 32 {
            eprintln!(
                "[VOICE AUDIO] secret_key inválida ({} bytes, se requieren 32)",
                secret_key_vec.len()
            );
            return;
        }

        let mut secret_key = [0u8; 32];
        secret_key.copy_from_slice(&secret_key_vec[0..32]);

        let stop_flag = Arc::new(AtomicBool::new(false));
        let jitter_buffer = Arc::new(std::sync::Mutex::new(PlaybackJitterBuffer::new()));

        // =========================================================================
        // 1. HILO DE SALIDA DE AUDIO (cpal Default Output Device - Altavoces)
        // =========================================================================
        let stop_out = stop_flag.clone();
        let jb_out = jitter_buffer.clone();
        std::thread::Builder::new()
            .name("dashcord-audio-out".to_string())
            .spawn(move || {
                let host = cpal::default_host();
                let Some(device) = host.default_output_device() else {
                    eprintln!("[VOICE AUDIO] No se encontró dispositivo de salida de audio.");
                    return;
                };

                let Ok(default_cfg) = device.default_output_config() else {
                    eprintln!("[VOICE AUDIO] No se pudo obtener configuración de salida.");
                    return;
                };

                let out_channels = default_cfg.channels() as usize;
                let out_sample_rate = default_cfg.sample_rate().0;
                let sample_format = default_cfg.sample_format();
                let config: StreamConfig = default_cfg.into();

                println!(
                    "[VOICE AUDIO] Salida cpal iniciada: {} Hz, {} canales ({:?})",
                    out_sample_rate, out_channels, sample_format
                );
                let _ = std::io::stdout().flush();

                let ratio = TARGET_SAMPLE_RATE as f64 / out_sample_rate as f64;
                let mut frac_pos = 0.0f64;
                let mut current_pair = (0.0f32, 0.0f32);

                let mut next_output_frame = move || -> (f32, f32) {
                    if IS_DEAFENED.load(Ordering::Relaxed) {
                        if let Ok(mut jb) = jb_out.lock() {
                            jb.samples.clear();
                            jb.primed = false;
                        }
                        return (0.0, 0.0);
                    }

                    frac_pos += ratio;
                    if frac_pos >= 1.0 {
                        let steps = frac_pos as usize;
                        frac_pos -= steps as f64;
                        if let Ok(mut jb) = jb_out.lock() {
                            for _ in 0..steps {
                                current_pair = jb.pop_stereo_pair();
                            }
                        }
                    }
                    current_pair
                };

                let err_fn = |err| {
                    eprintln!("[VOICE AUDIO] Error en stream de salida cpal: {}", err);
                };

                let stream_res = match sample_format {
                    SampleFormat::F32 => device.build_output_stream(
                        &config,
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            for frame in data.chunks_mut(out_channels.max(1)) {
                                let (l, r) = next_output_frame();
                                if frame.len() == 1 {
                                    frame[0] = (l + r) * 0.5;
                                } else {
                                    frame[0] = l;
                                    frame[1] = r;
                                    for sample in frame.iter_mut().skip(2) {
                                        *sample = 0.0;
                                    }
                                }
                            }
                        },
                        err_fn,
                        None,
                    ),
                    SampleFormat::I16 => device.build_output_stream(
                        &config,
                        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                            for frame in data.chunks_mut(out_channels.max(1)) {
                                let (l, r) = next_output_frame();
                                let li = (l.clamp(-1.0, 1.0) * 32767.0) as i16;
                                let ri = (r.clamp(-1.0, 1.0) * 32767.0) as i16;
                                if frame.len() == 1 {
                                    frame[0] = ((li as i32 + ri as i32) / 2) as i16;
                                } else {
                                    frame[0] = li;
                                    frame[1] = ri;
                                    for sample in frame.iter_mut().skip(2) {
                                        *sample = 0;
                                    }
                                }
                            }
                        },
                        err_fn,
                        None,
                    ),
                    _ => return,
                };

                if let Ok(stream) = stream_res {
                    let _ = stream.play();
                    while !stop_out.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            })
            .ok();

        // =========================================================================
        // 2. TAREA ASÍNCRONA DE RECEPCIÓN UDP (Fase B: UDP -> Decrypt -> Opus -> PCM)
        // =========================================================================
        let udp_rx = udp_socket.clone();
        let mode_rx = mode.clone();
        let jb_rx = jitter_buffer.clone();
        let stop_rx = stop_flag.clone();

        let rx_join = tokio::spawn(async move {
            let mut recv_buf = [0u8; 2048];
            let mut peer_decoders: HashMap<u32, PeerDecoderState> = HashMap::new();

            while !stop_rx.load(Ordering::Relaxed) {
                let Ok(len) = udp_rx.recv(&mut recv_buf).await else {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    continue;
                };

                if IS_DEAFENED.load(Ordering::Relaxed) {
                    continue;
                }

                let packet = &recv_buf[..len];
                let Some((seq, peer_ssrc, mut opus_data)) =
                    decrypt_rtp_opus_packet(&mode_rx, &secret_key, packet)
                else {
                    continue;
                };

                // Si la sesión DAVE E2EE está lista, descifrar el frame Opus entrante
                if let Ok(mut dave_guard) = dave_session_lock().lock() {
                    if let Some(dave) = dave_guard.as_mut().filter(|s| s.is_ready()) {
                        let ssrc_map = ssrc_map_lock().lock().unwrap_or_else(|e| e.into_inner());
                        let target_uid = ssrc_map.get(&peer_ssrc).copied().or_else(|| {
                            dave.get_user_ids()
                                .and_then(|ids| ids.into_iter().find(|&id| id != dave.user_id()))
                        });
                        if let Some(uid) = target_uid {
                            if let Ok(dec_dave) =
                                dave.decrypt(uid, davey::MediaType::AUDIO, &opus_data)
                            {
                                opus_data = dec_dave;
                            }
                        }
                    }
                }

                // Ignorar paquetes vacíos o de silencio puro de 3 bytes si no hay datos útiles
                if opus_data.is_empty() {
                    continue;
                }

                let peer_state = match peer_decoders.entry(peer_ssrc) {
                    std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
                    std::collections::hash_map::Entry::Vacant(v) => match PeerDecoderState::new() {
                        Ok(st) => v.insert(st),
                        Err(_) => continue,
                    },
                };

                // Jitter guard: ignorar paquetes viejos o duplicados
                if !peer_state.accept_sequence(seq) {
                    continue;
                }

                if let Ok(decoded_samples_per_ch) = peer_state
                    .decoder
                    .decode_float(&opus_data, &mut peer_state.pcm_buf, false)
                {
                    let total_stereo_samples =
                        (decoded_samples_per_ch * 2).min(peer_state.pcm_buf.len());
                    if total_stereo_samples > 0 {
                        if let Ok(mut jb) = jb_rx.lock() {
                            jb.push_stereo_frame(&peer_state.pcm_buf[..total_stereo_samples]);
                        }
                    }
                }
            }
        });

        // =========================================================================
        // 3. HILOS DE CAPTURA + DENOISE (nnnoiseless) + OPUS + CIFRADO + ENVÍO UDP (Fase C)
        // =========================================================================
        let (mic_tx, mic_rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(64);
        let stop_in = stop_flag.clone();

        // 3A. Hilo de captura cpal del micrófono
        std::thread::Builder::new()
            .name("dashcord-audio-in".to_string())
            .spawn(move || {
                let host = cpal::default_host();
                let Some(device) = host.default_input_device() else {
                    eprintln!("[VOICE AUDIO] No se encontró micrófono predeterminado.");
                    return;
                };

                let Ok(default_cfg) = device.default_input_config() else {
                    eprintln!("[VOICE AUDIO] No se pudo obtener configuración del micrófono.");
                    return;
                };

                let in_channels = default_cfg.channels() as usize;
                let in_sample_rate = default_cfg.sample_rate().0;
                let sample_format = default_cfg.sample_format();
                let config: StreamConfig = default_cfg.into();

                println!(
                    "[VOICE AUDIO] Micrófono cpal iniciado: {} Hz, {} canales ({:?})",
                    in_sample_rate, in_channels, sample_format
                );
                let _ = std::io::stdout().flush();

                let ratio = in_sample_rate as f64 / TARGET_SAMPLE_RATE as f64;
                let mut src_pos = 0.0f64;

                let mut push_mono_samples = move |mono_in: &[f32]| {
                    if IS_MUTED.load(Ordering::Relaxed) || IS_DEAFENED.load(Ordering::Relaxed) {
                        return;
                    }
                    if mono_in.is_empty() {
                        return;
                    }

                    if in_sample_rate == TARGET_SAMPLE_RATE {
                        let _ = mic_tx.try_send(mono_in.to_vec());
                    } else {
                        let mut resampled =
                            Vec::with_capacity(((mono_in.len() as f64) / ratio) as usize + 2);
                        while (src_pos as usize) < mono_in.len() {
                            resampled.push(mono_in[src_pos as usize]);
                            src_pos += ratio;
                        }
                        src_pos -= mono_in.len() as f64;
                        if src_pos < 0.0 {
                            src_pos = 0.0;
                        }
                        if !resampled.is_empty() {
                            let _ = mic_tx.try_send(resampled);
                        }
                    }
                };

                let err_fn = |err| {
                    eprintln!("[VOICE AUDIO] Error en stream de micrófono cpal: {}", err);
                };

                let stream_res = match sample_format {
                    SampleFormat::F32 => device.build_input_stream(
                        &config,
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            let mut mono = Vec::with_capacity(data.len() / in_channels.max(1));
                            for frame in data.chunks(in_channels.max(1)) {
                                let sum: f32 = frame.iter().copied().sum();
                                mono.push(sum / (frame.len() as f32));
                            }
                            push_mono_samples(&mono);
                        },
                        err_fn,
                        None,
                    ),
                    SampleFormat::I16 => device.build_input_stream(
                        &config,
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            let mut mono = Vec::with_capacity(data.len() / in_channels.max(1));
                            for frame in data.chunks(in_channels.max(1)) {
                                let sum: f32 = frame.iter().map(|&s| (s as f32) / 32768.0).sum();
                                mono.push(sum / (frame.len() as f32));
                            }
                            push_mono_samples(&mono);
                        },
                        err_fn,
                        None,
                    ),
                    _ => return,
                };

                if let Ok(stream) = stream_res {
                    let _ = stream.play();
                    while !stop_in.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            })
            .ok();

        // 3B. Hilo dedicado de DSP (nnnoiseless + Opus Encoder + XSalsa20Poly1305 + UDP Send)
        let stop_dsp = stop_flag.clone();
        let udp_tx = udp_socket;
        let mode_tx = mode;
        let speaking_cb = Arc::new(on_speaking_change);

        std::thread::Builder::new()
            .name("dashcord-voice-dsp".to_string())
            .spawn(move || {
                let Ok(mut encoder) =
                    OpusEncoder::new(TARGET_SAMPLE_RATE, OpusChannels::Mono, OpusApp::Voip)
                else {
                    eprintln!("[VOICE AUDIO] No se pudo inicializar OpusEncoder.");
                    return;
                };

                let _ = encoder.set_bitrate(OpusBitrate::Bits(64_000));
                let _ = encoder.set_signal(OpusSignal::Voice);
                let _ = encoder.set_inband_fec(true);
                let _ = encoder.set_packet_loss_perc(15);

                let mut denoise = DenoiseState::new();
                let mut raw_accum: Vec<f32> = Vec::with_capacity(OPUS_FRAME_SAMPLES_MONO * 2);
                let mut denoised_accum: Vec<i16> = Vec::with_capacity(OPUS_FRAME_SAMPLES_MONO * 2);
                let mut opus_out_buf = [0u8; 512];

                let mut rtp_seq: u16 = 1;
                let mut rtp_timestamp: u32 = 0;
                let mut nonce_counter: u32 = 1;
                let mut currently_speaking = false;
                let mut last_voice_instant = Instant::now();

                while !stop_dsp.load(Ordering::Relaxed) {
                    let chunk = match mic_rx.recv_timeout(Duration::from_millis(30)) {
                        Ok(c) => c,
                        Err(_) => {
                            // Si dejó de hablar hace > 250ms, apagar indicador de voz y enviar 5 frames de silencio Opus
                            if currently_speaking
                                && last_voice_instant.elapsed() > Duration::from_millis(250)
                            {
                                currently_speaking = false;
                                IS_SPEAKING.store(false, Ordering::Relaxed);
                                speaking_cb(false);

                                for _ in 0..5 {
                                    rtp_seq = rtp_seq.wrapping_add(1);
                                    rtp_timestamp =
                                        rtp_timestamp.wrapping_add(OPUS_FRAME_SAMPLES_MONO as u32);
                                    nonce_counter = nonce_counter.wrapping_add(1);

                                    let mut header = [0u8; 12];
                                    header[0] = 0x80;
                                    header[1] = 0x78;
                                    header[2..4].copy_from_slice(&rtp_seq.to_be_bytes());
                                    header[4..8].copy_from_slice(&rtp_timestamp.to_be_bytes());
                                    header[8..12].copy_from_slice(&ssrc.to_be_bytes());

                                    if let Ok(pkt) = encrypt_rtp_opus_packet(
                                        &mode_tx,
                                        &secret_key,
                                        &header,
                                        &OPUS_SILENCE_FRAME,
                                        nonce_counter,
                                    ) {
                                        let _ = udp_tx.try_send(&pkt);
                                    }
                                }
                                send_speaking_opcode(&ws_tx, ssrc, false);
                            }
                            continue;
                        }
                    };

                    if IS_MUTED.load(Ordering::Relaxed) || IS_DEAFENED.load(Ordering::Relaxed) {
                        raw_accum.clear();
                        denoised_accum.clear();
                        if currently_speaking {
                            currently_speaking = false;
                            IS_SPEAKING.store(false, Ordering::Relaxed);
                            speaking_cb(false);
                            send_speaking_opcode(&ws_tx, ssrc, false);
                        }
                        continue;
                    }

                    raw_accum.extend_from_slice(&chunk);

                    // Procesar en bloques de 480 muestras (10ms @ 48kHz) con nnnoiseless
                    while raw_accum.len() >= DENOISE_FRAME_SAMPLES {
                        let mut in_480 = [0.0f32; DENOISE_FRAME_SAMPLES];
                        let mut out_480 = [0.0f32; DENOISE_FRAME_SAMPLES];

                        for (i, sample) in raw_accum.drain(..DENOISE_FRAME_SAMPLES).enumerate() {
                            // nnnoiseless requiere muestras f32 escaladas al rango de i16 [-32768.0, 32767.0]
                            in_480[i] = (sample * 32767.0).clamp(-32768.0, 32767.0);
                        }

                        let vad_prob = denoise.process_frame(&mut out_480, &in_480);

                        // Calcular energía RMS de la señal limpia tras nnnoiseless
                        let mut sum_sq = 0.0f64;
                        for &s in &out_480 {
                            let norm = (s as f64) / 32768.0;
                            sum_sq += norm * norm;
                        }
                        let rms = (sum_sq / (DENOISE_FRAME_SAMPLES as f64)).sqrt();

                        // Detectar actividad de voz combinando VAD de RNNoise y umbral RMS
                        let voice_detected = vad_prob > 0.35 || rms > 0.012;
                        if voice_detected {
                            last_voice_instant = Instant::now();
                            if !currently_speaking {
                                currently_speaking = true;
                                IS_SPEAKING.store(true, Ordering::Relaxed);
                                speaking_cb(true);
                                send_speaking_opcode(&ws_tx, ssrc, true);
                            }
                        } else if currently_speaking
                            && last_voice_instant.elapsed() > Duration::from_millis(280)
                        {
                            currently_speaking = false;
                            IS_SPEAKING.store(false, Ordering::Relaxed);
                            speaking_cb(false);
                            send_speaking_opcode(&ws_tx, ssrc, false);
                        }

                        for &s in &out_480 {
                            denoised_accum.push(s.clamp(-32768.0, 32767.0) as i16);
                        }
                    }

                    // Cuando tenemos 960 muestras limpias (20ms @ 48kHz) y el usuario está hablando, codificar a Opus y enviar por UDP
                    while denoised_accum.len() >= OPUS_FRAME_SAMPLES_MONO {
                        let pcm_960: Vec<i16> =
                            denoised_accum.drain(..OPUS_FRAME_SAMPLES_MONO).collect();

                        if !currently_speaking {
                            continue;
                        }

                        let Ok(encoded_len) = encoder.encode(&pcm_960, &mut opus_out_buf) else {
                            continue;
                        };

                        rtp_seq = rtp_seq.wrapping_add(1);
                        rtp_timestamp = rtp_timestamp.wrapping_add(OPUS_FRAME_SAMPLES_MONO as u32);
                        nonce_counter = nonce_counter.wrapping_add(1);

                        let mut rtp_header = [0u8; 12];
                        rtp_header[0] = 0x80;
                        rtp_header[1] = 0x78;
                        rtp_header[2..4].copy_from_slice(&rtp_seq.to_be_bytes());
                        rtp_header[4..8].copy_from_slice(&rtp_timestamp.to_be_bytes());
                        rtp_header[8..12].copy_from_slice(&ssrc.to_be_bytes());

                        // Cifrar con DAVE E2EE si la sesión MLS está lista
                        let encrypted_dave = if let Ok(mut dave_guard) = dave_session_lock().lock() {
                            if let Some(dave) = dave_guard.as_mut().filter(|s| s.is_ready()) {
                                dave.encrypt_opus(&opus_out_buf[..encoded_len]).ok()
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let final_opus_slice: &[u8] = match &encrypted_dave {
                            Some(cow) => cow.as_ref(),
                            None => &opus_out_buf[..encoded_len],
                        };

                        if let Ok(encrypted_packet) = encrypt_rtp_opus_packet(
                            &mode_tx,
                            &secret_key,
                            &rtp_header,
                            final_opus_slice,
                            nonce_counter,
                        ) {
                            let _ = udp_tx.try_send(&encrypted_packet);
                        }
                    }
                }
            })
            .ok();

        if let Ok(mut guard) = pipeline_lock().lock() {
            *guard = Some(AudioPipelineHandle {
                stop_flag,
                rx_task: rx_join.abort_handle(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opus_encoder_decoder_roundtrip() {
        let mut encoder = OpusEncoder::new(
            TARGET_SAMPLE_RATE,
            OpusChannels::Stereo,
            OpusApp::Voip,
        )
        .expect("Fallo al inicializar codificador Opus");

        let mut decoder = OpusDecoder::new(
            TARGET_SAMPLE_RATE,
            OpusChannels::Stereo,
        )
        .expect("Fallo al inicializar decodificador Opus");

        // Frame de 20ms a 48000Hz (960 muestras por canal * 2 canales = 1920 f32)
        let pcm_in = vec![0.0f32; 1920];
        let mut opus_out = vec![0u8; 1275];
        let encoded_len = encoder
            .encode_float(&pcm_in, &mut opus_out)
            .expect("Fallo al codificar audio Opus");
        assert!(encoded_len > 0);

        let mut pcm_out = vec![0.0f32; 1920];
        let decoded_samples = decoder
            .decode_float(&opus_out[..encoded_len], &mut pcm_out, false)
            .expect("Fallo al decodificar audio Opus");
        assert_eq!(decoded_samples, 960);
    }

    #[test]
    fn test_audio_xsalsa20_crypto_roundtrip() {
        let key = [0x5au8; 32];
        let nonce = [0x33u8; 24];
        let plaintext = b"DashCord Encrypted Voice RTP Packet Data";
        let mut buffer = plaintext.to_vec();

        let cipher = XSalsa20Poly1305::new(SalsaKey::from_slice(&key));
        cipher
            .encrypt_in_place(SalsaNonce::from_slice(&nonce), b"", &mut buffer)
            .expect("Fallo al cifrar paquete con xsalsa20poly1305");

        assert_ne!(&buffer[..], plaintext);

        cipher
            .decrypt_in_place(SalsaNonce::from_slice(&nonce), b"", &mut buffer)
            .expect("Fallo al descifrar paquete con xsalsa20poly1305");

        assert_eq!(&buffer[..], plaintext);
    }

    #[test]
    #[ignore = "Requiere hardware de audio real (tarjeta de sonido), omitido en CI"]
    fn test_audio_hardware_detection_ci_safe() {
        if std::env::var("CI").is_ok() {
            println!("[AUDIO TEST] Entorno CI detectado (sin dispositivo de audio). Omitiendo consulta cpal.");
            return;
        }

        let host = cpal::default_host();
        let _ = host.default_output_device();
        let _ = host.default_input_device();
    }

    #[test]
    fn test_dave_e2ee_init_and_codec() {
        use davey::DaveSession;
        use std::num::NonZeroU16;
        let session = DaveSession::new(
            NonZeroU16::new(1).unwrap(),
            123456789,
            987654321,
            None,
        )
        .expect("Fallo al crear DaveSession");

        assert_eq!(session.user_id(), 123456789);
        assert_eq!(session.channel_id(), 987654321);
        assert!(!session.is_ready());
    }
}
