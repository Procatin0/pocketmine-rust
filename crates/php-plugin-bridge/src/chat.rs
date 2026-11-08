//! Chat formatting (`pocketmine\player\chat` parity).
//!
//! PocketMine's `PlayerChatEvent` carries a `ChatFormatter` instead of a raw
//! string: the formatter receives the sender's username and the (already
//! filtered) message and renders the broadcast line. `BetterChat` ships its
//! own `ChatFormatter(format, placeholders)` that substitutes `{player}`,
//! `{message}`, `{display_name}`, `{prefix}`, `{suffix}`, `{world}` and
//! `{group}`.
//!
//! The PHP bridge mirrors this with `pocketmine\player\chat\ChatFormatter`
//! (`php/chat.php`); this module is the Rust-native equivalent used when no
//! PHP worker is present (tests, native listeners) and to document the
//! contract. The event JSON carries the rendered `format` string, so the
//! same line works regardless of which side formatted it.

use std::collections::HashMap;

/// Formats the broadcast line of a chat message.
///
/// `username` is the sender's display name, `message` the filtered message.
pub trait ChatFormatter: Send + Sync {
    fn format(&self, username: &str, message: &str) -> String;
}

/// `pocketmine\player\chat\ChatFormatter` implementation: substitute
/// `{key}` placeholders in `format` with `placeholders` plus the implicit
/// `{player}` / `{message}` entries (BetterChat's `ChatFormatter`).
#[derive(Debug, Clone)]
pub struct PlaceholderFormatter {
    pub format: String,
    pub placeholders: HashMap<String, String>,
}

impl PlaceholderFormatter {
    pub fn new(format: impl Into<String>, placeholders: HashMap<String, String>) -> Self {
        Self {
            format: format.into(),
            placeholders,
        }
    }
}

impl ChatFormatter for PlaceholderFormatter {
    fn format(&self, username: &str, message: &str) -> String {
        let mut result = self.format.clone();
        for (key, value) in &self.placeholders {
            result = result.replace(&format!("{{{key}}}"), value);
        }
        result = result.replace("{player}", username);
        result = result.replace("{message}", message);
        result
    }
}

/// Default line: `<name> message`, matching `PlayerChatEvent::new`.
pub fn default_format(username: &str, message: &str) -> String {
    format!("<{username}> {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_formatter_substitutes_player_and_message() {
        let formatter = PlaceholderFormatter::new(
            "{display_name} > {message}",
            HashMap::from([("display_name".to_string(), "Steve".to_string())]),
        );
        assert_eq!(formatter.format("Steve", "hello"), "Steve > hello");
    }

    #[test]
    fn default_format_matches_player_chat_event() {
        assert_eq!(default_format("Alex", "hi"), "<Alex> hi");
    }
}
