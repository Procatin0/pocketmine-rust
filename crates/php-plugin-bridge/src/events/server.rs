use serde_json::json;
use crate::event::Event;

/// Fired when a command is executed from server console. Cancellable.
pub struct ServerCommandEvent {
    pub command: String,
    pub cancelled: bool,
}

impl ServerCommandEvent {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            cancelled: false,
        }
    }
}

impl Event for ServerCommandEvent {
    fn name(&self) -> &str {
        "ServerCommandEvent"
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
            "command": self.command,
            "cancelled": self.cancelled
        })
    }
}
