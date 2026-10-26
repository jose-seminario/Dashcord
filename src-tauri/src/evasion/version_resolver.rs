use crate::evasion::{SuperProperties, TlsSpoofFactory};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const DISCORD_APP_URL: &str = "https://discord.com/app";
const CACHE_FILE_NAME: &str = "version_cache.json";
const MAX_BUILD_AGE_SECONDS: i64 = 30 * 24 * 3600; // 30 días
const FRESH_CACHE_TTL_SECONDS: i64 = 24 * 3600;    // 24 horas

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCache {
    pub client_build_number: u32,
    pub client_version: String,
    pub release_channel: String,
    pub fetched_at_timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionStatus {
    pub is_blocked_fail_closed: bool,
    pub client_build_number: u32,
    pub client_version: String,
    pub last_checked_timestamp: i64,
    pub message: String,
}

pub struct VersionResolver;

impl VersionResolver {
    /// Determina la ruta al archivo de caché en el directorio de la aplicación
    fn get_cache_path(app_data_dir: Option<PathBuf>) -> PathBuf {
        app_data_dir
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CACHE_FILE_NAME)
    }

    /// Lee la caché de versión local
    pub fn read_cache(app_data_dir: Option<PathBuf>) -> Option<VersionCache> {
        let path = Self::get_cache_path(app_data_dir);
        if let Ok(content) = fs::read_to_string(path) {
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    /// Guarda la información en caché con timestamp actual
    pub fn save_cache(cache: &VersionCache, app_data_dir: Option<PathBuf>) -> Result<(), String> {
        let path = Self::get_cache_path(app_data_dir);
        let serialized = serde_json::to_string_pretty(cache).map_err(|e| e.to_string())?;
        fs::write(path, serialized).map_err(|e| e.to_string())
    }

    /// Comprueba el estado de la versión aplicando la regla fail-closed (>30 días)
    pub async fn check_and_resolve(
        app_data_dir: Option<PathBuf>,
        fallback_json_raw: &str,
    ) -> Result<SuperProperties, VersionStatus> {
        let now = Utc::now().timestamp();
        let cache_opt = Self::read_cache(app_data_dir.clone());

        // 1. Si la caché tiene menos de 24h, usarla directamente
        if let Some(ref cache) = cache_opt {
            if now - cache.fetched_at_timestamp < FRESH_CACHE_TTL_SECONDS {
                let mut props = SuperProperties::default();
                props.client_build_number = cache.client_build_number;
                props.client_version = cache.client_version.clone();
                props.release_channel = cache.release_channel.clone();
                return Ok(props);
            }
        }

        // 2. Intentar actualizar remotamente vía wreq (BoringSSL TLS Spoofed)
        match Self::fetch_latest_remote().await {
            Ok(fetched) => {
                let _ = Self::save_cache(&fetched, app_data_dir);
                let mut props = SuperProperties::default();
                props.client_build_number = fetched.client_build_number;
                props.client_version = fetched.client_version;
                props.release_channel = fetched.release_channel;
                Ok(props)
            }
            Err(_err) => {
                // 3. Si la petición remota falla, verificar la antigüedad de la caché o fallback
                let effective_timestamp = cache_opt
                    .as_ref()
                    .map(|c| c.fetched_at_timestamp)
                    .unwrap_or(0);

                let is_older_than_30_days = (now - effective_timestamp) > MAX_BUILD_AGE_SECONDS;

                if is_older_than_30_days {
                    // FAIL-CLOSED: La versión tiene más de 30 días y no se pudo contactar a Discord
                    let build = cache_opt
                        .as_ref()
                        .map(|c| c.client_build_number)
                        .unwrap_or(344210);
                    let ver = cache_opt
                        .as_ref()
                        .map(|c| c.client_version.clone())
                        .unwrap_or_else(|| "1.0.9168".to_string());

                    Err(VersionStatus {
                        is_blocked_fail_closed: true,
                        client_build_number: build,
                        client_version: ver,
                        last_checked_timestamp: effective_timestamp,
                        message: "BLOQUEO FAIL-CLOSED: El build number tiene más de 30 días de antigüedad sin actualización remota. Conexión al Gateway cancelada por seguridad.".to_string(),
                    })
                } else {
                    // Fallback local seguro a version.json si la caché aún no supera 30 días
                    let fallback_props = SuperProperties::from_json_str(fallback_json_raw)
                        .unwrap_or_else(|_| SuperProperties::default());
                    Ok(fallback_props)
                }
            }
        }
    }

    /// Realiza la consulta remota a discord.com/app usando wreq con BoringSSL
    async fn fetch_latest_remote() -> Result<VersionCache, String> {
        let client = TlsSpoofFactory::build_spoofed_client().map_err(|e| e.to_string())?;

        let res = client
            .get(DISCORD_APP_URL)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let html = res.text().await.map_err(|e| e.to_string())?;

        // Extraer build number o usar el hash más reciente del script
        let build_number = Self::extract_build_number(&html).unwrap_or(344210);

        Ok(VersionCache {
            client_build_number: build_number,
            client_version: "1.0.9168".to_string(),
            release_channel: "stable".to_string(),
            fetched_at_timestamp: Utc::now().timestamp(),
        })
    }

    fn extract_build_number(content: &str) -> Option<u32> {
        if let Some(idx) = content.find("build_number:\"") {
            let start = idx + "build_number:\"".len();
            let slice = &content[start..];
            if let Some(end) = slice.find('"') {
                return slice[..end].parse::<u32>().ok();
            }
        }
        None
    }
}
