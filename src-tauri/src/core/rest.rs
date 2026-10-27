use crate::evasion::{SuperProperties, TlsSpoofFactory, TrackerFilter};
use serde_json::Value;
use wreq::header::{
    HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, ORIGIN, REFERER, USER_AGENT,
};

const CHROME_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.9168 Chrome/128.0.6613.186 Electron/32.2.2 Safari/537.36";

pub struct DiscordRestClient {
    client: wreq::Client,
    token: Option<String>,
    super_properties_header: String,
}

impl DiscordRestClient {
    pub fn new(token: Option<String>, super_properties: &SuperProperties) -> Result<Self, String> {
        let client = TlsSpoofFactory::build_spoofed_client().map_err(|e| e.to_string())?;
        let super_props_b64 = super_properties
            .to_base64_header()
            .map_err(|e| e.to_string())?;

        let clean_token = token.map(|t| {
            t.trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string()
        });

        Ok(Self {
            client,
            token: clean_token,
            super_properties_header: super_props_b64,
        })
    }

    fn build_headers(&self) -> Result<HeaderMap, String> {
        let mut headers = HeaderMap::new();
        if let Some(ref t) = self.token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(t).map_err(|e| e.to_string())?,
            );
        }

        headers.insert(
            HeaderName::from_static("x-super-properties"),
            HeaderValue::from_str(&self.super_properties_header).map_err(|e| e.to_string())?,
        );

        headers.insert(
            HeaderName::from_static("x-discord-locale"),
            HeaderValue::from_static("es-ES"),
        );

        headers.insert(USER_AGENT, HeaderValue::from_static(CHROME_USER_AGENT));
        headers.insert(ORIGIN, HeaderValue::from_static("https://discord.com"));
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://discord.com/channels/@me"),
        );

        Ok(headers)
    }

    /// Obtiene el historial de mensajes de un canal vía GET /channels/{channel_id}/messages?limit=50
    pub async fn get_channel_messages(
        &self,
        channel_id: &str,
        limit: u32,
    ) -> Result<Value, String> {
        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages?limit={}",
            channel_id, limit
        );

        if TrackerFilter::is_tracking_url(&url) {
            return Err("URL bloqueada por política anti-rastreo".to_string());
        }

        let headers = self.build_headers()?;

        println!(
            "[REST] GET /channels/{}/messages?limit={} (wreq BoringSSL)",
            channel_id, limit
        );

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| format!("Error de red en GET messages: {}", e))?;

        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Error deserializando respuesta JSON: {}", e))?;

        if !status.is_success() {
            return Err(format!("Error HTTP {}: {}", status, json));
        }

        Ok(json)
    }

    /// Envía un mensaje a un canal usando la capa de evasión TLS/HTTP2 con wreq y BoringSSL
    pub async fn send_message(&self, channel_id: &str, content: &str) -> Result<Value, String> {
        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages",
            channel_id
        );

        if TrackerFilter::is_tracking_url(&url) {
            return Err("URL bloqueada por política anti-rastreo".to_string());
        }

        let headers = self.build_headers()?;

        let body = serde_json::json!({
            "content": content,
            "nonce": null,
            "tts": false,
            "flags": 0
        });

        println!(
            "[REST] POST /channels/{}/messages (wreq BoringSSL)",
            channel_id
        );
        tracing::info!(
            "Enviando mensaje a canal {} con TLS Spoofed [Auth: REDACTED]",
            channel_id
        );

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Error de red en POST message: {}", e))?;

        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Error deserializando respuesta JSON: {}", e))?;

        if !status.is_success() {
            return Err(format!("Error HTTP {}: {}", status, json));
        }

        Ok(json)
    }
}
