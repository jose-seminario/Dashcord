use wreq::Client;

/// Fábrica para construir el cliente HTTP wreq con suplantación de TLS basada en BoringSSL nativo
pub struct TlsSpoofFactory;

impl TlsSpoofFactory {
    /// Genera un cliente wreq configurado con BoringSSL (btls)
    pub fn build_spoofed_client() -> Result<Client, wreq::Error> {
        Client::builder()
            .build()
    }
}
