//! PHP Plugin Bridge - FFI & Process Architecture between Rust Server Core and PocketMine Plugins

pub mod command;
pub mod context;
pub mod description;
pub mod error;
pub mod event;
pub mod events;
pub mod manager;
pub mod plugin;
pub mod runtime;

pub use command::{Command, CommandMap, CommandSender};
pub use context::{PlayerContext, PluginAction};
pub use description::{CommandDescription, PermissionDescription, PluginDescription};
pub use error::PluginError;
pub use event::{Event, EventDispatcher, EventPriority, RegisteredListener};
pub use events::*;
pub use manager::PluginManager;
pub use plugin::Plugin;
pub use runtime::{NativePluginWrapper, PhpPlugin, PhpWorker};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_plugin_description_parsing() {
        let yml = r#"
name: EconomyAPI
version: "2.1.0"
main: onebone\economyapi\EconomyAPI
api: ["4.0.0", "5.0.0"]
author: onebone
description: Economy system for PocketMine
commands:
  mymoney:
    description: Shows player money
    usage: /mymoney [player]
    permission: economyapi.command.mymoney
    aliases: ["money", "bal", "balance"]
permissions:
  economyapi.command.mymoney:
    description: Allows player to view money
    default: true
"#;

        let desc = PluginDescription::from_yaml_str(yml).expect("Valid plugin.yml");
        assert_eq!(desc.name, "EconomyAPI");
        assert_eq!(desc.version, "2.1.0");
        assert_eq!(desc.main, "onebone\\economyapi\\EconomyAPI");
        assert_eq!(desc.all_authors(), vec!["onebone"]);
        assert!(desc.commands.contains_key("mymoney"));

        let cmd = desc.commands.get("mymoney").unwrap();
        assert_eq!(cmd.aliases, vec!["money", "bal", "balance"]);
    }

    #[test]
    fn test_dependency_topological_sort() {
        let p_core = PluginDescription {
            name: "CoreLib".to_string(),
            version: "1.0".to_string(),
            main: "Core\\Main".to_string(),
            api: vec!["5.0.0".to_string()],
            author: None,
            authors: vec![],
            description: None,
            website: None,
            depend: vec![],
            softdepend: vec![],
            load_before: vec![],
            commands: Default::default(),
            permissions: Default::default(),
        };

        let p_eco = PluginDescription {
            name: "EconomyAPI".to_string(),
            version: "2.0".to_string(),
            main: "Eco\\Main".to_string(),
            api: vec!["5.0.0".to_string()],
            author: None,
            authors: vec![],
            description: None,
            website: None,
            depend: vec!["CoreLib".to_string()],
            softdepend: vec![],
            load_before: vec![],
            commands: Default::default(),
            permissions: Default::default(),
        };

        let p_shop = PluginDescription {
            name: "ChestShop".to_string(),
            version: "1.0".to_string(),
            main: "Shop\\Main".to_string(),
            api: vec!["5.0.0".to_string()],
            author: None,
            authors: vec![],
            description: None,
            website: None,
            depend: vec!["EconomyAPI".to_string()],
            softdepend: vec![],
            load_before: vec![],
            commands: Default::default(),
            permissions: Default::default(),
        };

        let list = vec![
            (p_shop.clone(), PathBuf::from("plugins/ChestShop")),
            (p_eco.clone(), PathBuf::from("plugins/EconomyAPI")),
            (p_core.clone(), PathBuf::from("plugins/CoreLib")),
        ];

        let sorted = PluginManager::resolve_dependencies(&list).expect("Topological sort succeeds");
        let order: Vec<String> = sorted.into_iter().map(|(d, _)| d.name).collect();
        assert_eq!(order, vec!["CoreLib", "EconomyAPI", "ChestShop"]);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let p_a = PluginDescription {
            name: "PluginA".to_string(),
            version: "1.0".to_string(),
            main: "A\\Main".to_string(),
            api: vec![],
            author: None,
            authors: vec![],
            description: None,
            website: None,
            depend: vec!["PluginB".to_string()],
            softdepend: vec![],
            load_before: vec![],
            commands: Default::default(),
            permissions: Default::default(),
        };

        let p_b = PluginDescription {
            name: "PluginB".to_string(),
            version: "1.0".to_string(),
            main: "B\\Main".to_string(),
            api: vec![],
            author: None,
            authors: vec![],
            description: None,
            website: None,
            depend: vec!["PluginA".to_string()],
            softdepend: vec![],
            load_before: vec![],
            commands: Default::default(),
            permissions: Default::default(),
        };

        let list = vec![
            (p_a, PathBuf::from("plugins/PluginA")),
            (p_b, PathBuf::from("plugins/PluginB")),
        ];

        let result = PluginManager::resolve_dependencies(&list);
        assert!(result.is_err());
        match result {
            Err(PluginError::CircularDependency(_)) => {}
            _ => panic!("Expected CircularDependency error"),
        }
    }

    #[test]
    fn test_event_dispatcher_priorities_and_cancellation() {
        let dispatcher = EventDispatcher::new();
        let tracker = Arc::new(AtomicU32::new(0));

        let t1 = Arc::clone(&tracker);
        dispatcher.register_listener(RegisteredListener {
            plugin_name: "TestPlugin".to_string(),
            event_name: "PlayerChatEvent".to_string(),
            priority: EventPriority::Lowest,
            ignore_cancelled: false,
            callback: Arc::new(move |event| {
                assert_eq!(t1.load(Ordering::SeqCst), 0);
                t1.store(1, Ordering::SeqCst);
                // Cancel event
                event.set_cancelled(true);
                Ok(vec![PluginAction::BroadcastMessage {
                    message: "Filtered".to_string(),
                }])
            }),
        });

        let t2 = Arc::clone(&tracker);
        // This listener has Normal priority and ignore_cancelled: false -> must NOT be called because event was cancelled
        dispatcher.register_listener(RegisteredListener {
            plugin_name: "TestPlugin".to_string(),
            event_name: "PlayerChatEvent".to_string(),
            priority: EventPriority::Normal,
            ignore_cancelled: false,
            callback: Arc::new(move |_| {
                t2.store(999, Ordering::SeqCst);
                Ok(Vec::new())
            }),
        });

        let t3 = Arc::clone(&tracker);
        // Monitor listener -> MUST be called even if cancelled
        dispatcher.register_listener(RegisteredListener {
            plugin_name: "TestPlugin".to_string(),
            event_name: "PlayerChatEvent".to_string(),
            priority: EventPriority::Monitor,
            ignore_cancelled: true,
            callback: Arc::new(move |event| {
                assert!(event.is_cancelled());
                t3.store(2, Ordering::SeqCst);
                Ok(Vec::new())
            }),
        });

        let player = PlayerContext::new("Steve", "127.0.0.1:19132".parse().unwrap());
        let mut chat_event = PlayerChatEvent::new(player, "Bad word");

        let actions = dispatcher.dispatch(&mut chat_event).unwrap();
        assert!(chat_event.is_cancelled());
        assert_eq!(tracker.load(Ordering::SeqCst), 2);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            PluginAction::BroadcastMessage {
                message: "Filtered".to_string()
            }
        );
    }

    #[test]
    fn test_command_execution() {
        struct PingCommand;
        impl Command for PingCommand {
            fn name(&self) -> &str {
                "ping"
            }
            fn aliases(&self) -> &[String] {
                &[]
            }
            fn execute(&self, sender: &CommandSender, _args: &[String]) -> Result<Vec<PluginAction>, PluginError> {
                Ok(vec![PluginAction::SendMessage {
                    target_player: sender.name().to_string(),
                    message: "Pong!".to_string(),
                }])
            }
        }

        let map = CommandMap::new();
        map.register(Arc::new(PingCommand));

        let sender = CommandSender::Player(PlayerContext::new("Alex", "127.0.0.1:19132".parse().unwrap()));
        let result = map.execute(&sender, "/ping").unwrap();
        assert!(result.is_some());
        let actions = result.unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            PluginAction::SendMessage {
                target_player: "Alex".to_string(),
                message: "Pong!".to_string()
            }
        );
    }
}
