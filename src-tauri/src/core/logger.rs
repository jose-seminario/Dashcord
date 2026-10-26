use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Inicializa el subsistema de telemetría y logs sanitizados con rotación de 7 días
pub fn init_secure_logger(logs_dir: Option<PathBuf>) -> Result<(), String> {
    let dir = logs_dir.unwrap_or_else(|| PathBuf::from("logs"));

    // Rotación diaria con retención estricta de 7 días
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("dashcord")
        .filename_suffix("log")
        .max_log_files(7)
        .build(dir)
        .map_err(|e| e.to_string())?;

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false);

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_ansi(true);

    #[cfg(debug_assertions)]
    let default_level = "info";
    #[cfg(not(debug_assertions))]
    let default_level = "warn";

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    Ok(())
}
