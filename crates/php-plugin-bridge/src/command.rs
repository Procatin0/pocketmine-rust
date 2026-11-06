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

    pub fn register(&self, command: Arc<dyn Command>) {
        let name = command.name().to_lowercase();
        let mut aliases_map = self.aliases.write();
        for alias in command.aliases() {
            aliases_map.insert(alias.to_lowercase(), name.clone());
        }
        let mut map = self.commands.write();
        map.insert(name, command);
    }

    pub fn unregister_by_plugin(&self, _plugin_name: &str) {
        // Can be extended to track owning plugin
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
        let args: Vec<String> = parts.map(|s| s.to_string()).collect();

        let resolved_name = {
            let aliases = self.aliases.read();
            aliases.get(&cmd_name).cloned().unwrap_or(cmd_name)
        };

        let cmd = {
            let map = self.commands.read();
            map.get(&resolved_name).cloned()
        };

        match cmd {
            Some(command) => {
                let actions = command.execute(sender, &args)?;
                Ok(Some(actions))
            }
            None => Ok(None),
        }
    }

    pub fn command_count(&self) -> usize {
        self.commands.read().len()
    }
}
