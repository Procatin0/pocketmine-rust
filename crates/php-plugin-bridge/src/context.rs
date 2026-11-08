use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    BroadcastMessage { message: String },
    #[serde(rename = "console_message")]
    ConsoleMessage { message: String },
    #[serde(rename = "kick_player")]
    KickPlayer {
        target_player: String,
        #[serde(default)]
        reason: String,
    },
    #[serde(rename = "teleport")]
    Teleport {
        target_player: String,
        x: f64,
        y: f64,
        z: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        world: Option<String>,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sender: Option<String>,
    },
    #[serde(rename = "send_tip")]
    SendTip {
        target_player: String,
        message: String,
    },
    #[serde(rename = "send_popup")]
    SendPopup {
        target_player: String,
        message: String,
    },
    #[serde(rename = "send_title")]
    SendTitle {
        target_player: String,
        title: String,
        #[serde(default)]
        subtitle: String,
        #[serde(default)]
        fade_in: i32,
        #[serde(default)]
        stay: i32,
        #[serde(default)]
        fade_out: i32,
    },
    #[serde(rename = "broadcast_tip")]
    BroadcastTip { message: String },
    #[serde(rename = "broadcast_popup")]
    BroadcastPopup { message: String },
    #[serde(rename = "broadcast_title")]
    BroadcastTitle {
        title: String,
        #[serde(default)]
        subtitle: String,
        #[serde(default)]
        fade_in: i32,
        #[serde(default)]
        stay: i32,
        #[serde(default)]
        fade_out: i32,
    },
    #[serde(rename = "set_gamemode")]
    SetGamemode {
        target_player: String,
        gamemode: u32,
    },
    // Fase A1 (Server+Player API): `Player::setHealth/setMaxHealth/setXp/
    // setXpLevel` via `addAction`. `SetHealth` bundles `health` + optional
    // `max_health` so both PHP setters share one variant; `SetXp` bundles
    // `xp` (progress 0..1) + optional `level` (xp_level). Tolerant of
    // partial payloads (optionals default to `None`).
    #[serde(rename = "set_health")]
    SetHealth {
        target_player: String,
        health: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_health: Option<f32>,
    },
    #[serde(rename = "set_xp")]
    SetXp {
        target_player: String,
        xp: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<i32>,
    },
    // Fase A1: `Player::hidePlayer/showPlayer` -> `SessionManager::
    // hide_player/show_player` (`canSee`). `viewer` hides/shows `target`.
    #[serde(rename = "hide_player")]
    HidePlayer { viewer: String, target: String },
    #[serde(rename = "show_player")]
    ShowPlayer { viewer: String, target: String },
    // Fase A1: `Player::sendSubTitle` (subtitle only, no title packet).
    #[serde(rename = "send_subtitle")]
    SendSubTitle {
        target_player: String,
        subtitle: String,
    },
    // Fase A1: `Player::sendActionBarMessage` (`SetTitlePacket`
    // action-bar message, NOT `TextPacket` popup).
    #[serde(rename = "send_action_bar")]
    SendActionBar {
        target_player: String,
        message: String,
    },
}

fn default_socket_addr() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], 0))
}

/// Information about a player passed to plugins.
///
/// Mirrors the JSON object documented in `docs/plugin-api-contract.md`. All
/// fields have serde defaults so older callers (and partial PHP payloads) keep
/// working.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerContext {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default = "default_socket_addr")]
    pub address: SocketAddr,
    #[serde(default)]
    pub runtime_id: u64,
    #[serde(default)]
    pub unique_id: i64,
    #[serde(default)]
    pub ping: u32,
    #[serde(default)]
    pub gamemode: u32,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    #[serde(default)]
    pub yaw: f64,
    #[serde(default)]
    pub pitch: f64,
    #[serde(default)]
    pub world: String,
    #[serde(default)]
    pub health: f32,
    #[serde(default)]
    pub max_health: f32,
    #[serde(default)]
    pub is_op: bool,
    /// True when this entry is owned by a virtual (Java bridge) player,
    /// false when it is owned by a live Bedrock session. Ownership matters
    /// for removal: quitting one side must not delete the other's entry
    /// when both scripts share a lowercase name.
    #[serde(default)]
    pub is_virtual: bool,
}

impl PlayerContext {
    pub fn new(name: impl Into<String>, address: SocketAddr) -> Self {
        let name = name.into();
        Self {
            display_name: name.clone(),
            name,
            address,
            runtime_id: 1,
            unique_id: 1,
            ping: 0,
            gamemode: 0,
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            world: String::new(),
            health: 20.0,
            max_health: 20.0,
            is_op: false,
            is_virtual: false,
        }
    }

    /// Serialize the player snapshot for the PHP runtime.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Hydrate a player snapshot returned by PHP.
    ///
    /// Returns `None` when `value` is not an object (or contains invalid
    /// field types). Missing fields fall back to their serde defaults.
    pub fn from_json(value: &Value) -> Option<Self> {
        let mut player: Self = serde_json::from_value(value.clone()).ok()?;
        if player.display_name.is_empty() {
            player.display_name = player.name.clone();
        }
        Some(player)
    }

    /// Updates position/rotation in place (`player.update_position`).
    pub fn apply_position(&mut self, x: f64, y: f64, z: f64, yaw: f64, pitch: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
        self.yaw = yaw;
        self.pitch = pitch;
    }

    /// Snapshot in the `players/<name>.json` format shared with Bedrock
    /// sessions (`PlayerSession::save_player_data`).
    pub fn to_save_json(&self) -> Value {
        serde_json::json!({
            "name": self.name,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "yaw": self.yaw,
            "pitch": self.pitch,
            "gamemode": self.gamemode as i64,
            "health": self.health as f64,
            "world": self.world,
        })
    }

    /// Restores position/rotation/gamemode/health from a saved JSON object.
    ///
    /// Missing fields leave the current values untouched.
    pub fn apply_save_json(&mut self, value: &Value) {
        let number = |key: &str| value.get(key).and_then(Value::as_f64);
        if let (Some(x), Some(y), Some(z)) = (number("x"), number("y"), number("z")) {
            self.x = x;
            self.y = y;
            self.z = z;
        }
        if let Some(yaw) = number("yaw") {
            self.yaw = yaw;
        }
        if let Some(pitch) = number("pitch") {
            self.pitch = pitch;
        }
        if let Some(gamemode) = value.get("gamemode").and_then(Value::as_i64) {
            self.gamemode = gamemode.max(0) as u32;
        }
        if let Some(health) = number("health") {
            self.health = health as f32;
        }
        if let Some(world) = value.get("world").and_then(Value::as_str) {
            if !world.is_empty() {
                self.world = world.to_string();
            }
        }
    }

    /// Builds a context from a saved JSON object returning `None` when the
    /// object has no usable position.
    pub fn from_save_json(name: &str, value: &Value) -> Option<Self> {
        let (x, y, z) = {
            let object = value.as_object()?;
            (
                object.get("x")?.as_f64()?,
                object.get("y")?.as_f64()?,
                object.get("z")?.as_f64()?,
            )
        };
        let mut player = Self::new(name, default_socket_addr());
        player.x = x;
        player.y = y;
        player.z = z;
        player.apply_save_json(value);
        Some(player)
    }
}

/// Parse a position from either `[x, y, z]` or `{"x":..,"y":..,"z":..}`.
pub(crate) fn position_from_json(value: &Value) -> Option<(f64, f64, f64)> {
    if let Some(array) = value.as_array() {
        if array.len() >= 3 {
            return Some((array[0].as_f64()?, array[1].as_f64()?, array[2].as_f64()?));
        }
        return None;
    }

    let object = value.as_object()?;
    Some((
        object.get("x")?.as_f64()?,
        object.get("y")?.as_f64()?,
        object.get("z")?.as_f64()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_context_json_roundtrip() {
        let player = PlayerContext {
            name: "Steve".to_string(),
            display_name: "Steve".to_string(),
            address: "127.0.0.1:19132".parse().unwrap(),
            runtime_id: 1,
            unique_id: 1,
            ping: 0,
            gamemode: 0,
            x: 1.5,
            y: 64.0,
            z: -2.5,
            yaw: 90.0,
            pitch: 10.0,
            world: "world".to_string(),
            health: 20.0,
            max_health: 20.0,
            is_op: true,
            is_virtual: false,
        };

        let value = player.to_json();
        assert_eq!(value["name"], "Steve");
        assert_eq!(value["address"], "127.0.0.1:19132");
        assert_eq!(value["world"], "world");

        let parsed = PlayerContext::from_json(&value).expect("valid player json");
        assert_eq!(parsed, player);
    }

    #[test]
    fn player_context_defaults_for_partial_json() {
        let parsed =
            PlayerContext::from_json(&serde_json::json!({"name": "Alex"})).expect("valid player");
        assert_eq!(parsed.name, "Alex");
        assert_eq!(parsed.display_name, "Alex");
        assert_eq!(parsed.y, 0.0);
        assert_eq!(parsed.max_health, 0.0);
    }

    #[test]
    fn parses_position_arrays_and_objects() {
        assert_eq!(
            position_from_json(&serde_json::json!([1.0, 64.0, -3.0])),
            Some((1.0, 64.0, -3.0))
        );
        assert_eq!(
            position_from_json(&serde_json::json!({"x": 1, "y": 64, "z": -3})),
            Some((1.0, 64.0, -3.0))
        );
    }
}
