use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub data_version: u8,
    pub name: String,
    pub protocol: i32,
    pub version: String,
    pub level: String,
    pub players: i32,
    pub max_players: i32,
    pub game_type: i32,
    pub editor: bool,
    pub hardcore: bool,
    pub online_auth: bool,
    pub self_signed_auth: bool,
    pub whitelist: bool,
    pub allow_list: bool,
    pub nonce: String,
    pub transport_layer: i32,
    pub connection: i32,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self {
            data_version: 7,
            name: "PocketMine-Rust Server".to_string(),
            protocol: 2225, // Modern NetherNet protocol (compatible with 1.26.60+)
            version: "1.26.60".to_string(),
            level: "world".to_string(),
            players: 0,
            max_players: 1000,
            game_type: 1, // Survival
            editor: false,
            hardcore: false,
            online_auth: false, // Disabled for LAN / offline mode to prevent Xbox Live enforcement
            self_signed_auth: true,
            whitelist: false,   // Whitelist explicitly disabled
            allow_list: false,  // Allowlist explicitly disabled
            nonce: "".to_string(),
            transport_layer: 2, // NetherNet
            connection: 4,      // NetherNet
        }
    }
}

impl ServerInfo {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}
