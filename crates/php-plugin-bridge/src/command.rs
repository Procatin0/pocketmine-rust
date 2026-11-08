//! Native command registry (`CommandMap`).
//!
//! Ports `pocketmine\command\CommandMap` (`SimpleCommandMap`) from the PHP
//! bridge (`php/command.php`): every command is stored under
//! `fallbackPrefix:label`, the bare `label` only stays dispatchable while it
//! is free (vanilla commands win it), and `dispatch` accepts both bare and
//! `prefix:label` forms. `CommandMap::execute` also merges whitespace-split
//! selector tokens so `[...]` arguments with interior spaces survive as
//! single args, exactly like `server-core/src/command.rs::merge_selector_tokens`.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::context::{PlayerContext, PluginAction};
use crate::error::PluginError;

#[derive(Debug, Clone, PartialEq)]
pub enum CommandSender {
    Console,
    Player(PlayerContext),
}

impl CommandSender {
    pub fn name(&self) -> &str {
        match self {
            Self::Console => "CONSOLE",
            Self::Player(p) => &p.name,
        }
    }

    pub fn is_player(&self) -> bool {
        matches!(self, Self::Player(_))
    }
}

pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str {
        ""
    }
    fn usage(&self) -> &str {
        ""
    }
    fn permission(&self) -> Option<&str> {
        None
    }
    fn aliases(&self) -> &[String] {
        &[]
    }
    fn execute(
        &self,
        sender: &CommandSender,
        args: &[String],
    ) -> Result<Vec<PluginAction>, PluginError>;
}

pub struct CommandMap {
    commands: RwLock<HashMap<String, Arc<dyn Command>>>,
    aliases: RwLock<HashMap<String, String>>,
}

impl Default for CommandMap {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandMap {
    pub fn new() -> Self {
        Self {
            commands: RwLock::new(HashMap::new()),
            aliases: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a command with the `minecraft` fallback prefix.
    ///
    /// Keeps the historical one-argument shape: bare dispatch works while
    /// the label is free, and `minecraft:<name>` always works.
    pub fn register(&self, command: Arc<dyn Command>) -> bool {
        self.register_with_prefix("minecraft", command)
    }

    /// PM `SimpleCommandMap::register($fallbackPrefix, $command)` parity.
    ///
    /// Always stores `fallbackPrefix:label`. The bare `label` (and bare
    /// aliases) are stored only while free; when taken, the command is
    /// only reachable as `fallbackPrefix:label`. Returns `true` when the
    /// bare label was claimed.
    pub fn register_with_prefix(
        &self,
        fallback_prefix: &str,
        command: Arc<dyn Command>,
    ) -> bool {
        let prefix = fallback_prefix.trim().to_lowercase();
        let name = command.name().to_lowercase();
        let prefixed = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}:{name}")
        };

        let mut map = self.commands.write();
        map.insert(prefixed.clone(), Arc::clone(&command));
        // Bare label only if free (vanilla wins the bare label in PM).
        let claimed = if map.contains_key(&name) {
            false
        } else {
            map.insert(name.clone(), Arc::clone(&command));
            true
        };
        drop(map);

        let mut aliases_map = self.aliases.write();
        for alias in command.aliases() {
            let alias_key = alias.to_lowercase();
            let alias_prefixed = if prefix.is_empty() {
                alias_key.clone()
            } else {
                format!("{prefix}:{alias_key}")
            };
            // Aliases resolve to the prefixed canonical entry so collisions
            // on the bare name cannot steal them.
            aliases_map.insert(alias_key, prefixed.clone());
            aliases_map.insert(alias_prefixed, prefixed.clone());
        }
        drop(aliases_map);

        claimed
    }

    pub fn unregister_by_plugin(&self, _plugin_name: &str) {
        // Can be extended to track owning plugin
    }

    /// Looks up a command by bare name, alias or `prefix:name`.
    pub fn get_command(&self, name: &str) -> Option<Arc<dyn Command>> {
        let key = name.to_lowercase();
        {
            let map = self.commands.read();
            if let Some(command) = map.get(&key) {
                return Some(Arc::clone(command));
            }
        }
        let resolved = {
            let aliases = self.aliases.read();
            aliases.get(&key).cloned()
        }?;
        let map = self.commands.read();
        map.get(&resolved).cloned()
    }

    pub fn execute(
        &self,
        sender: &CommandSender,
        command_line: &str,
    ) -> Result<Option<Vec<PluginAction>>, PluginError> {
        let trimmed = command_line.trim().trim_start_matches('/');
        if trimmed.is_empty() {
            return Ok(None);
        }

        let mut parts = trimmed.split_whitespace();
        let cmd_name = match parts.next() {
            Some(name) => name.to_lowercase(),
            None => return Ok(None),
        };
        let raw_args: Vec<String> = parts.map(|s| s.to_string()).collect();
        let args = merge_selector_tokens(&raw_args);

        let Some(command) = self.get_command(&cmd_name) else {
            return Ok(None);
        };

        command.execute(sender, &args).map(Some)
    }

    pub fn command_count(&self) -> usize {
        self.commands.read().len()
    }
}

/// Merges whitespace-split tokens that belong to a single `@` selector
/// `[...]` (see `command.rs::merge_selector_tokens` and PHP
/// `CommandMap::mergeSelectorTokens`).
///
/// ```text
/// ["@a[name=Alex,", "limit=2]"] -> ["@a[name=Alex, limit=2]"]
/// ```
pub fn merge_selector_tokens(parts: &[String]) -> Vec<String> {
    let mut merged = Vec::new();
    let mut buffer: Option<String> = None;
    let mut depth: i32 = 0;
    for token in parts {
        let delta = token.matches('[').count() as i32 - token.matches(']').count() as i32;
        match &mut buffer {
            None => {
                depth = delta;
                if depth <= 0 {
                    merged.push(token.clone());
                } else {
                    buffer = Some(token.clone());
                }
            }
            Some(buf) => {
                buf.push(' ');
                buf.push_str(token);
                depth += delta;
                if depth <= 0 {
                    merged.push(buf.clone());
                    buffer = None;
                }
            }
        }
    }
    if let Some(buf) = buffer {
        merged.push(buf);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PingCommand;
    impl Command for PingCommand {
        fn name(&self) -> &str {
            "ping"
        }
        fn execute(
            &self,
            sender: &CommandSender,
            args: &[String],
        ) -> Result<Vec<PluginAction>, PluginError> {
            Ok(vec![PluginAction::SendMessage {
                target_player: sender.name().to_string(),
                message: format!("pong {}", args.join(" ")),
            }])
        }
    }

    struct PingOther;
    impl Command for PingOther {
        fn name(&self) -> &str {
            "ping"
        }
        fn execute(
            &self,
            _sender: &CommandSender,
            _args: &[String],
        ) -> Result<Vec<PluginAction>, PluginError> {
            Ok(vec![PluginAction::BroadcastMessage {
                message: "other ping".to_string(),
            }])
        }
    }

    #[test]
    fn fallback_prefix_keeps_bare_label_with_first_command() {
        let map = CommandMap::new();
        assert!(map.register_with_prefix("minecraft", Arc::new(PingCommand)));
        // Second registration loses the bare label, keeps the prefixed one.
        assert!(!map.register_with_prefix("other", Arc::new(PingOther)));
        assert!(map.get_command("other:ping").is_some());
        assert!(map.get_command("ping").is_some());
        // Bare `ping` still resolves to the first command.
        let sender = CommandSender::Console;
        let actions = map.execute(&sender, "/ping").unwrap().unwrap();
        assert_eq!(
            actions[0],
            PluginAction::SendMessage {
                target_player: "CONSOLE".to_string(),
                message: "pong ".to_string(),
            }
        );
        let prefixed = map.execute(&sender, "/other:ping").unwrap().unwrap();
        assert_eq!(
            prefixed[0],
            PluginAction::BroadcastMessage {
                message: "other ping".to_string(),
            }
        );
    }

    #[test]
    fn selector_tokens_merge_inside_brackets() {
        let merged = merge_selector_tokens(&["@a[name=Alex,".to_string(), "limit=2]".to_string()]);
        assert_eq!(merged, vec!["@a[name=Alex, limit=2]"]);
    }
}
