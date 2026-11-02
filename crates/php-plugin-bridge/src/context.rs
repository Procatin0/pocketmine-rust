use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// An action emitted by a plugin (PHP or native) to be executed by the server core.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", content = "data")]
pub enum PluginAction {
    #[serde(rename = "send_message")]
    SendMessage {
        target_player: String,
        message: String,
    },
    #[serde(rename = "broadcast_message")]
    BroadcastMessage {
        message: String,
    },
    #[serde(rename = "kick_player")]
    KickPlayer {
        target_player: String,
        reason: String,
    },
    #[serde(rename = "teleport")]
    Teleport {
        target_player: String,
        x: f64,
        y: f64,
        z: f64,
    },
    #[serde(rename = "set_block")]
    SetBlock {
        world: String,
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
    },
    #[serde(rename = "dispatch_command")]
    DispatchCommand {
        command: String,
    },
}

/// Information about a player passed to plugins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerContext {
    pub name: String,
    pub address: SocketAddr,
    pub runtime_id: u64,
    pub unique_id: i64,
    pub ping: u32,
    pub gamemode: u32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl PlayerContext {
    pub fn new(name: impl Into<String>, address: SocketAddr) -> Self {
        Self {
            name: name.into(),
            address,
            runtime_id: 1,
            unique_id: 1,
            ping: 0,
            gamemode: 0,
            x: 0.0,
            y: 64.0,
            z: 0.0,
        }
    }
}
