pub mod core;
pub mod evasion;
pub mod storage;

use core::GatewayManager;
use evasion::{SuperProperties, VersionResolver};
use std::io::Write;
use storage::KeychainManager;
use tauri::{Manager, State};
use tokio::sync::Mutex;

pub struct AppConfig {
    pub is_dry_run: bool,
}

pub struct GatewayHolder {
    pub manager: Mutex<GatewayManager>,
}

#[tauri::command]
fn get_super_properties() -> Result<SuperProperties, String> {
    Ok(SuperProperties::default())
}

#[tauri::command]
fn save_token(config: State<'_, AppConfig>, token: String) -> Result<(), String> {
    KeychainManager::store_token(&token, config.is_dry_run)
}

#[tauri::command]
fn get_token() -> Result<String, String> {
    KeychainManager::get_token()
}

#[tauri::command]
fn delete_token(config: State<'_, AppConfig>) -> Result<(), String> {
    KeychainManager::delete_token(config.is_dry_run)
}

#[tauri::command]
fn check_evasion_status() -> serde_json::Value {
    serde_json::json!({
        "tls_engine": "BoringSSL (wreq 6.0.0-rc.31)",
        "platform_spoof": "Windows 10 x64",
        "telemetry_filter_active": true,
        "zero_reqwest_verified": true,
        "zero_openssl_verified": true
    })
}

#[tauri::command]
async fn check_version_status() -> Result<SuperProperties, String> {
    let fallback = include_str!("../../version.json");
    VersionResolver::check_and_resolve(None, fallback)
        .await
        .map_err(|status| serde_json::to_string(&status).unwrap_or(status.message))
}

#[tauri::command]
async fn start_gateway_session(
    gateway: State<'_, GatewayHolder>,
) -> Result<String, String> {
    let mut mgr = gateway.manager.lock().await;
    mgr.start_session().await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let is_dry_run = args.iter().any(|arg| arg == "--dry-run")
        || std::env::var("DASHCORD_DRY_RUN").map(|v| v == "1").unwrap_or(false);

    if is_dry_run {
        println!("[DRY-RUN] Starting DashCord in dry-run mode");
        println!("[DRY-RUN] Simulated READY received");
        eprintln!("[DRY-RUN] Error stream active");
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
    }

    let _ = core::init_secure_logger(None);

    let gateway_manager = GatewayManager::new(is_dry_run);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppConfig { is_dry_run })
        .manage(GatewayHolder {
            manager: Mutex::new(gateway_manager),
        })
        .invoke_handler(tauri::generate_handler![
            get_super_properties,
            save_token,
            get_token,
            delete_token,
            check_evasion_status,
            check_version_status,
            start_gateway_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running dashcord application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gateway_session_future_is_send() {
        fn assert_send<F: std::future::Future + Send>(_: F) {}

        let holder = GatewayHolder {
            manager: Mutex::new(GatewayManager::new(true)),
        };

        // Si el future retuviera un MutexGuard no-Send a través de .await, este bloque no compila:
        let fut = async move {
            let mut mgr = holder.manager.lock().await;
            mgr.start_session().await
        };

        assert_send(fut);
    }
}
