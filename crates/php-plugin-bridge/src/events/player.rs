use serde_json::json;
use crate::context::PlayerContext;
use crate::event::Event;

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

    fn to_json(&self) -> serde_json::Value {
        json!({
            "player": self.player,
            "join_message": self.join_message
        })
    }
}

/// Fired when a player disconnects from the server.
pub struct PlayerQuitEvent {
    pub player: PlayerContext,
    pub quit_message: String,
    pub reason: String,
}

impl PlayerQuitEvent {
    pub fn new(player: PlayerContext, quit_message: impl Into<String>, reason: impl Into<String>) -> Self {
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

    fn to_json(&self) -> serde_json::Value {
        json!({
            "player": self.player,
            "quit_message": self.quit_message,
            "reason": self.reason
        })
    }
}

/// Fired when a player sends a chat message. Can be cancelled.
pub struct PlayerChatEvent {
    pub player: PlayerContext,
    pub message: String,
    pub format: String,
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
            cancelled: false,
        }
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

    fn to_json(&self) -> serde_json::Value {
        json!({
            "player": self.player,
            "message": self.message,
            "format": self.format,
            "cancelled": self.cancelled
        })
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

    fn to_json(&self) -> serde_json::Value {
        json!({
            "player": self.player,
            "from": self.from,
            "to": self.to,
            "cancelled": self.cancelled
        })
    }
}
