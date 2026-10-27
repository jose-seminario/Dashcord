use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPayload {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_status() -> String {
    "online".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePayload {
    pub id: String,
    pub channel_id: String,
    pub content: String,
    pub author: UserPayload,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayHello {
    pub heartbeat_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendChannel {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub channel_type: String, // "text" | "voice" | "announcement"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(default)]
    pub unread: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendGuild {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub unread_count: u32,
    #[serde(default)]
    pub channels: Vec<FrontendChannel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendReadyPayload {
    pub user: UserPayload,
    pub guilds: Vec<FrontendGuild>,
}
