use serde_json::{json, Value};

use crate::context::{position_from_json, PlayerContext};
use crate::event::Event;

fn apply_player(field: &mut PlayerContext, data: &Value) {
    if let Some(value) = data.get("player") {
        if let Some(player) = PlayerContext::from_json(value) {
            *field = player;
        }
    }
}

fn apply_message(field: &mut String, data: &Value, key: &str) {
    if let Some(value) = data.get(key).and_then(Value::as_str) {
        *field = value.to_string();
    }
}

fn apply_cancelled(field: &mut bool, data: &Value) {
    if let Some(value) = data.get("cancelled").and_then(Value::as_bool) {
        *field = value;
    }
}

/// Fired when a player successfully joins the server.
pub struct PlayerJoinEvent {
    pub player: PlayerContext,
    pub join_message: String,
}

impl PlayerJoinEvent {
    pub fn new(player: PlayerContext, join_message: impl Into<String>) -> Self {
        Self {
            player,
            join_message: join_message.into(),
        }
    }
}

impl Event for PlayerJoinEvent {
    fn name(&self) -> &str {
        "PlayerJoinEvent"
    }

    fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "join_message": self.join_message
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);
        apply_message(&mut self.join_message, value, "join_message");
    }
}

/// Fired when a player disconnects from the server.
pub struct PlayerQuitEvent {
    pub player: PlayerContext,
    pub quit_message: String,
    pub reason: String,
}

impl PlayerQuitEvent {
    pub fn new(
        player: PlayerContext,
        quit_message: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            player,
            quit_message: quit_message.into(),
            reason: reason.into(),
        }
    }
}

impl Event for PlayerQuitEvent {
    fn name(&self) -> &str {
        "PlayerQuitEvent"
    }

    fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "quit_message": self.quit_message,
            "reason": self.reason
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);
        apply_message(&mut self.quit_message, value, "quit_message");
        apply_message(&mut self.reason, value, "reason");
    }
}

/// Fired when a player sends a chat message. Can be cancelled.
pub struct PlayerChatEvent {
    pub player: PlayerContext,
    pub message: String,
    pub format: String,
    pub recipients: Vec<String>,
    pub cancelled: bool,
}

impl PlayerChatEvent {
    pub fn new(player: PlayerContext, message: impl Into<String>) -> Self {
        let msg = message.into();
        let format = format!("<{}> {}", player.name, msg);
        Self {
            player,
            message: msg,
            format,
            recipients: Vec::new(),
            cancelled: false,
        }
    }

    pub fn with_recipients(mut self, recipients: Vec<String>) -> Self {
        self.recipients = recipients;
        self
    }
}

impl Event for PlayerChatEvent {
    fn name(&self) -> &str {
        "PlayerChatEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "message": self.message,
            "format": self.format,
            "recipients": self.recipients,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);
        apply_message(&mut self.message, value, "message");
        apply_message(&mut self.format, value, "format");

        if let Some(recipients) = value.get("recipients").and_then(Value::as_array) {
            // Rust represents recipients as names, while the PHP runtime may
            // return either strings or full player snapshots.
            self.recipients = recipients
                .iter()
                .filter_map(|recipient| {
                    recipient
                        .as_str()
                        .or_else(|| recipient.get("name").and_then(Value::as_str))
                        .map(str::to_string)
                })
                .collect();
        }

        apply_cancelled(&mut self.cancelled, value);
    }
}

/// One endpoint of a [`PlayerMoveEvent`]: the PM `Location` datum
/// (`pocketmine\entity\Location` upstream; `pocketmine\world\Location` in
/// the bridge): feet position plus yaw/pitch and world name.
///
/// Wire form is `{"x":..,"y":..,"z":..,"yaw":..,"pitch":..,"world":..}`.
/// [`MoveLocation::from_json`] also accepts the legacy `[x, y, z]` list (and
/// `{"x","y","z"}` without rotation/world), so snapshots returned by older
/// PHP runtimes keep hydrating.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveLocation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub world: String,
}

impl MoveLocation {
    pub fn new(x: f64, y: f64, z: f64, yaw: f64, pitch: f64, world: impl Into<String>) -> Self {
        Self {
            x,
            y,
            z,
            yaw,
            pitch,
            world: world.into(),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "yaw": self.yaw,
            "pitch": self.pitch,
            "world": self.world
        })
    }

    pub fn from_json(value: &Value) -> Option<Self> {
        if let Some(array) = value.as_array() {
            if array.len() >= 3 {
                return Some(Self {
                    x: array[0].as_f64()?,
                    y: array[1].as_f64()?,
                    z: array[2].as_f64()?,
                    yaw: 0.0,
                    pitch: 0.0,
                    world: String::new(),
                });
            }
            return None;
        }

        let object = value.as_object()?;
        Some(Self {
            x: object.get("x")?.as_f64()?,
            y: object.get("y")?.as_f64()?,
            z: object.get("z")?.as_f64()?,
            yaw: object.get("yaw").and_then(Value::as_f64).unwrap_or(0.0),
            pitch: object
                .get("pitch")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            world: object
                .get("world")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
    }

    /// Squared XYZ distance (PM `Player::processMostRecentMovements` uses
    /// this `> 0.01` test to detect a plugin-rewritten `to`).
    pub fn distance_squared(&self, other: &Self) -> f64 {
        (self.x - other.x).powi(2) + (self.y - other.y).powi(2) + (self.z - other.z).powi(2)
    }
}

/// Fired when a player moves. Can be cancelled to prevent movement.
/// Ports PM `PlayerMoveEvent(Player $player, Location $from, Location $to)`
/// (`Player::processMostRecentMovements`): `from` is the last location,
/// `to` the new one; a plugin may rewrite `to` via `setTo()`.
pub struct PlayerMoveEvent {
    pub player: PlayerContext,
    pub from: MoveLocation,
    pub to: MoveLocation,
    pub cancelled: bool,
}

impl PlayerMoveEvent {
    pub fn new(player: PlayerContext, from: MoveLocation, to: MoveLocation) -> Self {
        Self {
            player,
            from,
            to,
            cancelled: false,
        }
    }
}

impl Event for PlayerMoveEvent {
    fn name(&self) -> &str {
        "PlayerMoveEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "from": self.from.to_json(),
            "to": self.to.to_json(),
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);

        if let Some(from) = value.get("from").and_then(MoveLocation::from_json) {
            self.from = from;
        }
        if let Some(to) = value.get("to").and_then(MoveLocation::from_json) {
            self.to = to;
        }

        apply_cancelled(&mut self.cancelled, value);
    }
}

/// Fired when a player runs a command through chat (`/command ...`).
pub struct PlayerCommandPreprocessEvent {
    pub player: PlayerContext,
    pub message: String,
    pub cancelled: bool,
}

impl PlayerCommandPreprocessEvent {
    pub fn new(player: PlayerContext, message: impl Into<String>) -> Self {
        Self {
            player,
            message: message.into(),
            cancelled: false,
        }
    }
}

impl Event for PlayerCommandPreprocessEvent {
    fn name(&self) -> &str {
        "PlayerCommandPreprocessEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "message": self.message,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);
        apply_message(&mut self.message, value, "message");
        apply_cancelled(&mut self.cancelled, value);
    }
}

/// Fired when a player dies.
pub struct PlayerDeathEvent {
    pub player: PlayerContext,
    pub death_message: String,
}

impl PlayerDeathEvent {
    pub fn new(player: PlayerContext, death_message: impl Into<String>) -> Self {
        Self {
            player,
            death_message: death_message.into(),
        }
    }
}

impl Event for PlayerDeathEvent {
    fn name(&self) -> &str {
        "PlayerDeathEvent"
    }

    fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "death_message": self.death_message
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);
        apply_message(&mut self.death_message, value, "death_message");
    }
}

/// Fired when a player respawns. Cancelling prevents the respawn.
pub struct PlayerRespawnEvent {
    pub player: PlayerContext,
    pub position: (f64, f64, f64),
    pub cancelled: bool,
}

impl PlayerRespawnEvent {
    pub fn new(player: PlayerContext, position: (f64, f64, f64)) -> Self {
        Self {
            player,
            position,
            cancelled: false,
        }
    }
}

impl Event for PlayerRespawnEvent {
    fn name(&self) -> &str {
        "PlayerRespawnEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "position": [self.position.0, self.position.1, self.position.2],
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);

        if let Some(position) = value.get("position").and_then(position_from_json) {
            self.position = position;
        }

        apply_cancelled(&mut self.cancelled, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> PlayerContext {
        PlayerContext::new("Steve", "127.0.0.1:19132".parse().unwrap())
    }

    #[test]
    fn move_event_serialises_from_to_as_location_objects() {
        let event = PlayerMoveEvent::new(
            player(),
            MoveLocation::new(0.5, 64.0, 0.5, 0.0, 0.0, "world"),
            MoveLocation::new(1.5, 64.0, 0.5, 90.0, 10.0, "world"),
        );

        let value = event.to_json();
        // PM `Location` shape: object with x/y/z/yaw/pitch/world (not a list).
        assert_eq!(
            value["from"],
            json!({"x": 0.5, "y": 64.0, "z": 0.5, "yaw": 0.0, "pitch": 0.0, "world": "world"})
        );
        assert_eq!(value["to"]["yaw"], 90.0);
        assert_eq!(value["to"]["world"], "world");

        let mut restored = PlayerMoveEvent::new(
            player(),
            MoveLocation::new(0.0, 0.0, 0.0, 0.0, 0.0, ""),
            MoveLocation::new(0.0, 0.0, 0.0, 0.0, 0.0, ""),
        );
        restored.apply_json(&value);
        assert_eq!(restored.from, event.from);
        assert_eq!(restored.to, event.to);
    }

    #[test]
    fn move_event_apply_accepts_legacy_array_form() {
        let mut event = PlayerMoveEvent::new(
            player(),
            MoveLocation::new(0.0, 0.0, 0.0, 0.0, 0.0, ""),
            MoveLocation::new(0.0, 0.0, 0.0, 0.0, 0.0, ""),
        );
        // Snapshots from the pre-Location PHP runtime: `[x, y, z]` lists.
        event.apply_json(&json!({
            "from": [1.0, 64.0, -3.0],
            "to": {"x": 2.0, "y": 64.0, "z": -3.0},
            "cancelled": true
        }));
        assert_eq!(event.from, MoveLocation::new(1.0, 64.0, -3.0, 0.0, 0.0, ""));
        assert_eq!(event.to, MoveLocation::new(2.0, 64.0, -3.0, 0.0, 0.0, ""));
        assert!(event.cancelled);
    }
}
