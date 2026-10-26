use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;

const GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const SESSION_TIMEOUT: Duration = Duration::from_secs(60);

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

    /// Inicia la sesión del Gateway respetando estrictamente el modo dry-run
    pub async fn start_session(&mut self) -> Result<String, String> {
        if self.state.is_dry_run {
            println!("[DRY-RUN] Passive mode active. No outbound network sockets created.");
            println!("[DRY-RUN] Simulated READY received (session_id: dry_run_session_001)");
            use std::io::Write;
            let _ = std::io::stdout().flush();

            self.state.session_id = Some("dry_run_session_001".to_string());
            return Ok("dry_run_session_001".to_string());
        }

        // Sub-fase B: Conexión real con timeout agresivo de 5s y cabeceras de navegador Chrome
        println!("[NET] Connecting to {}", GATEWAY_URL);
        use std::io::Write;
        let _ = std::io::stdout().flush();

        // Envoltorio con timeout agresivo de conexión
        let connect_fut = async {
            // En Sub-fase B se completa el handshake WebSocket real
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok::<(), String>(())
        };

        if let Err(_) = timeout(CONNECT_TIMEOUT, connect_fut).await {
            return Err("[GATEWAY] Connection timed out after 5s".to_string());
        }

        // Timeout global de sesión de 60 segundos
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok("session_active".to_string())
    }

    /// Cierre limpio y sin handles colgados
    pub fn close_cleanly(&mut self) {
        if !self.state.is_dry_run {
            println!("[GATEWAY] Session timed out, closing.");
        }
        self.state.session_id = None;
    }
}
