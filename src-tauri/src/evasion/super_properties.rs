use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperProperties {
    pub os: String,
    pub browser: String,
    pub release_channel: String,
    pub client_version: String,
    pub os_version: String,
    pub os_arch: String,
    pub system_locale: String,
    pub client_build_number: u32,
    pub native_build_number: Option<u32>,
    pub client_event_source: Option<String>,
    pub design_id: u32,
}

impl Default for SuperProperties {
    fn default() -> Self {
        Self {
            os: "Windows".to_string(),
            browser: "Discord Client".to_string(),
            release_channel: "stable".to_string(),
            client_version: "1.0.9168".to_string(),
            os_version: "10.0.22631".to_string(),
            os_arch: "x64".to_string(),
            system_locale: "es-ES".to_string(),
            client_build_number: 344210,
            native_build_number: None,
            client_event_source: None,
            design_id: 0,
        }
    }
}

impl SuperProperties {
    /// Genera la cabecera X-Super-Properties codificada en Base64
    pub fn to_base64_header(&self) -> Result<String, serde_json::Error> {
        let json_str = serde_json::to_string(self)?;
        Ok(general_purpose::STANDARD.encode(json_str.as_bytes()))
    }

    /// Deserializa desde una cadena JSON
    pub fn from_json_str(raw: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(raw)
    }

    /// Deserializa a partir de una cabecera X-Super-Properties codificada en Base64
    pub fn from_base64_header(b64: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let decoded = general_purpose::STANDARD.decode(b64)?;
        let parsed = serde_json::from_slice(&decoded)?;
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_json_generation() {
        let props = SuperProperties::default();
        let json_str = serde_json::to_string(&props).expect("Error serializing SuperProperties");
        let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("Generated invalid JSON");
        
        assert_eq!(parsed["os"], "Windows");
        assert_eq!(parsed["browser"], "Discord Client");
        assert_eq!(parsed["client_build_number"], 344210);
    }

    #[test]
    fn test_base64_encode_decode_structural_integrity() {
        let props = SuperProperties::default();
        let b64 = props.to_base64_header().expect("Failed to encode to base64");
        
        let decoded = SuperProperties::from_base64_header(&b64)
            .expect("Failed to decode base64 back to SuperProperties");
            
        assert_eq!(props, decoded);
    }

    #[test]
    fn test_byte_by_byte_fixture_match() {
        let fixture_raw = include_str!("../../../tests/fixtures/super_properties.json");
        let fixture_props: SuperProperties = serde_json::from_str(fixture_raw)
            .expect("Invalid fixture JSON");
            
        let default_props = SuperProperties::default();
        assert_eq!(
            fixture_props, default_props,
            "Default SuperProperties does not match fixture byte-for-byte representation"
        );

        // Validar que ambos generen idéntica representación serializada
        let fixture_canonical = serde_json::to_string(&fixture_props).unwrap();
        let default_canonical = serde_json::to_string(&default_props).unwrap();
        assert_eq!(fixture_canonical, default_canonical);
    }
}
