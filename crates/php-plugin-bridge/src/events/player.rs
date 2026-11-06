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

/// Fired when a player moves. Can be cancelled to prevent movement.
pub struct PlayerMoveEvent {
    pub player: PlayerContext,
    pub from: (f64, f64, f64),
    pub to: (f64, f64, f64),
    pub cancelled: bool,
}

impl PlayerMoveEvent {
    pub fn new(player: PlayerContext, from: (f64, f64, f64), to: (f64, f64, f64)) -> Self {
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
            "from": [self.from.0, self.from.1, self.from.2],
            "to": [self.to.0, self.to.1, self.to.2],
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);

        if let Some(from) = value.get("from").and_then(position_from_json) {
            self.from = from;
        }
        if let Some(to) = value.get("to").and_then(position_from_json) {
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
