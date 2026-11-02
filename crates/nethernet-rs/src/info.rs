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
    pub nonce: String,
    pub transport_layer: i32,
    pub connection: i32,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self {
            data_version: 7,
            name: "PocketMine-Rust Server".to_string(),
            protocol: 748,
            version: "1.21.20".to_string(),
            level: "world".to_string(),
            players: 0,
            max_players: 1000,
            game_type: 1, // Survival
            editor: false,
            hardcore: false,
            online_auth: false, // Default to false for LAN/custom servers to prevent Terracotta auth failure
            self_signed_auth: true,
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
