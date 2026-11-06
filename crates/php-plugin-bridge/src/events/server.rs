use serde_json::{json, Value};

use crate::event::Event;

/// Fired when a command is executed from the server console. Cancellable.
pub struct ServerCommandEvent {
    pub sender: String,
    pub command: String,
    pub cancelled: bool,
}

impl ServerCommandEvent {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            sender: "CONSOLE".to_string(),
            command: command.into(),
            cancelled: false,
        }
    }

    pub fn with_sender(sender: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            sender: sender.into(),
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

    fn to_json(&self) -> Value {
        json!({
            "sender": self.sender,
            "command": self.command,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        if let Some(sender) = value.get("sender") {
            if let Some(name) = sender.as_str() {
                self.sender = name.to_string();
            } else if let Some(name) = sender.get("name").and_then(Value::as_str) {
                self.sender = name.to_string();
            }
        }
        if let Some(command) = value.get("command").and_then(Value::as_str) {
            self.command = command.to_string();
        }
        if let Some(cancelled) = value.get("cancelled").and_then(Value::as_bool) {
            self.cancelled = cancelled;
        }
    }
}
