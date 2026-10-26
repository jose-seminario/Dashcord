/// Filtro para interceptar y bloquear dominios y rutas de telemetría de Discord
pub struct TrackerFilter;

impl TrackerFilter {
    const BLOCKED_SUBSTRINGS: &'static [&'static str] = &[
        "/api/science",
        "/api/v9/science",
        "/api/v10/science",
        "/metrics",
        "/tracing",
        "sentry.io",
        "datadoghq.com",
        "google-analytics.com",
    ];

    /// Retorna `true` si la URL solicitada es un endpoint de rastreo que debe ser bloqueado
    pub fn is_tracking_url(url: &str) -> bool {
        Self::BLOCKED_SUBSTRINGS.iter().any(|pattern| url.contains(pattern))
    }
}
