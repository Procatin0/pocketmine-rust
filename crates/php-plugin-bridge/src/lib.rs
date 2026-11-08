//! PHP Plugin Bridge - FFI & Process Architecture between Rust Server Core and PocketMine Plugins

pub mod command;
pub mod context;
pub mod description;
pub mod error;
pub mod event;
pub mod events;
pub mod host;
pub mod manager;
pub mod plugin;
pub mod runtime;

pub use command::{Command, CommandMap, CommandSender};
pub use context::{PlayerContext, PluginAction};
pub use description::{CommandDescription, PermissionDescription, PluginDescription};
pub use error::PluginError;
pub use event::{Event, EventDispatcher, EventPriority, RegisteredListener};
pub use events::*;
pub use host::PluginHost;
pub use manager::{CommandOutcome, PluginManager};
pub use plugin::Plugin;
pub use runtime::{
    DiscoveredPluginManifest, IpcResponse, NativePluginWrapper, PhpPlugin, PhpWorker, PluginSource,
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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
            fn execute(
                &self,
                sender: &CommandSender,
                _args: &[String],
            ) -> Result<Vec<PluginAction>, PluginError> {
                Ok(vec![PluginAction::SendMessage {
                    target_player: sender.name().to_string(),
                    message: "Pong!".to_string(),
                }])
            }
        }

        let map = CommandMap::new();
        map.register(Arc::new(PingCommand));

        let sender = CommandSender::Player(PlayerContext::new(
            "Alex",
            "127.0.0.1:19132".parse().unwrap(),
        ));
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

    #[test]
    fn test_plugin_action_json_roundtrip() {
        let cases = vec![
            (
                json!({"action":"send_message","data":{"target_player":"Steve","message":"hi"}}),
                PluginAction::SendMessage {
                    target_player: "Steve".to_string(),
                    message: "hi".to_string(),
                },
            ),
            (
                json!({"action":"broadcast_message","data":{"message":"hello"}}),
                PluginAction::BroadcastMessage {
                    message: "hello".to_string(),
                },
            ),
            (
                json!({"action":"console_message","data":{"message":"to console"}}),
                PluginAction::ConsoleMessage {
                    message: "to console".to_string(),
                },
            ),
            (
                json!({"action":"kick_player","data":{"target_player":"Steve","reason":"bye"}}),
                PluginAction::KickPlayer {
                    target_player: "Steve".to_string(),
                    reason: "bye".to_string(),
                },
            ),
            (
                json!({"action":"teleport","data":{"target_player":"@a","x":1.0,"y":64.0,"z":-2.0,"world":"world"}}),
                PluginAction::Teleport {
                    target_player: "@a".to_string(),
                    x: 1.0,
                    y: 64.0,
                    z: -2.0,
                    world: Some("world".to_string()),
                    yaw: None,
                    pitch: None,
                },
            ),
            (
                json!({"action":"set_block","data":{"world":"world","x":1,"y":64,"z":-2,"block_id":1}}),
                PluginAction::SetBlock {
                    world: "world".to_string(),
                    x: 1,
                    y: 64,
                    z: -2,
                    block_id: 1,
                },
            ),
            (
                json!({"action":"dispatch_command","data":{"command":"say hi","sender":"Steve"}}),
                PluginAction::DispatchCommand {
                    command: "say hi".to_string(),
                    sender: Some("Steve".to_string()),
                },
            ),
            (
                json!({"action":"send_tip","data":{"target_player":"Steve","message":"tip"}}),
                PluginAction::SendTip {
                    target_player: "Steve".to_string(),
                    message: "tip".to_string(),
                },
            ),
            (
                json!({"action":"send_popup","data":{"target_player":"Steve","message":"popup"}}),
                PluginAction::SendPopup {
                    target_player: "Steve".to_string(),
                    message: "popup".to_string(),
                },
            ),
            (
                json!({"action":"send_title","data":{"target_player":"Steve","title":"T","subtitle":"S","fade_in":10,"stay":40,"fade_out":10}}),
                PluginAction::SendTitle {
                    target_player: "Steve".to_string(),
                    title: "T".to_string(),
                    subtitle: "S".to_string(),
                    fade_in: 10,
                    stay: 40,
                    fade_out: 10,
                },
            ),
            (
                json!({"action":"broadcast_tip","data":{"message":"tip"}}),
                PluginAction::BroadcastTip {
                    message: "tip".to_string(),
                },
            ),
            (
                json!({"action":"broadcast_popup","data":{"message":"popup"}}),
                PluginAction::BroadcastPopup {
                    message: "popup".to_string(),
                },
            ),
            (
                json!({"action":"broadcast_title","data":{"title":"T","subtitle":"S","fade_in":10,"stay":40,"fade_out":10}}),
                PluginAction::BroadcastTitle {
                    title: "T".to_string(),
                    subtitle: "S".to_string(),
                    fade_in: 10,
                    stay: 40,
                    fade_out: 10,
                },
            ),
            (
                json!({"action":"set_gamemode","data":{"target_player":"Steve","gamemode":1}}),
                PluginAction::SetGamemode {
                    target_player: "Steve".to_string(),
                    gamemode: 1,
                },
            ),
        ];

        for (json_value, action) in cases {
            let parsed: PluginAction =
                serde_json::from_value(json_value.clone()).expect("PluginAction JSON");
            assert_eq!(parsed, action);
            assert_eq!(serde_json::to_value(&action).unwrap(), json_value);
        }
    }

    #[test]
    fn test_plugin_action_optional_fields() {
        let teleport = PluginAction::Teleport {
            target_player: "Steve".to_string(),
            x: 1.0,
            y: 2.0,
            z: 3.0,
            world: None,
            yaw: None,
            pitch: None,
        };
        assert_eq!(
            serde_json::to_value(&teleport).unwrap(),
            json!({"action":"teleport","data":{"target_player":"Steve","x":1.0,"y":2.0,"z":3.0}})
        );
        let rotated = PluginAction::Teleport {
            target_player: "Steve".to_string(),
            x: 1.0,
            y: 2.0,
            z: 3.0,
            world: None,
            yaw: Some(90.0),
            pitch: Some(-15.0),
        };
        assert_eq!(
            serde_json::to_value(&rotated).unwrap(),
            json!({"action":"teleport","data":{"target_player":"Steve","x":1.0,"y":2.0,"z":3.0,"yaw":90.0,"pitch":-15.0}})
        );

        let command = PluginAction::DispatchCommand {
            command: "say hi".to_string(),
            sender: None,
        };
        assert_eq!(
            serde_json::to_value(&command).unwrap(),
            json!({"action":"dispatch_command","data":{"command":"say hi"}})
        );
    }

    #[test]
    fn test_player_chat_apply_json() {
        let player = PlayerContext::new("Steve", "127.0.0.1:19132".parse().unwrap());
        let mut event = PlayerChatEvent::new(player, "original");

        event.apply_json(&json!({
            "message": "edited",
            "format": "[X] <Steve> edited",
            "recipients": ["Alex", "Herobrine"],
            "cancelled": true
        }));

        assert_eq!(event.message, "edited");
        assert_eq!(event.format, "[X] <Steve> edited");
        assert_eq!(event.recipients, vec!["Alex", "Herobrine"]);
        assert!(event.is_cancelled());
    }

    #[test]
    fn test_parse_phar_manifest() {
        let item = DiscoveredPluginManifest {
            kind: "phar".to_string(),
            path: "plugins/Foo.phar".to_string(),
            manifest: "name: Foo\nversion: 1.0.0\nmain: Foo\\\\Main\napi: [5.0.0]\n".to_string(),
        };

        let (description, source) =
            PluginManager::parse_discovered_manifest(&item).expect("phar manifest");
        assert_eq!(description.name, "Foo");
        assert!(source.is_phar());
        assert!(source.root().starts_with("phar://"));
        assert!(source
            .main_file("Foo\\Main")
            .ends_with("/Foo.phar/src/Foo/Main.php"));
    }

    #[test]
    fn test_ipc_response_defaults() {
        let response: IpcResponse = serde_json::from_str(
            r#"{"status":"ok","event":{"message":"x"},"cancelled":false,"actions":[]}"#,
        )
        .unwrap();

        assert_eq!(response.status, "ok");
        assert!(response.event.is_some());
        assert!(response.actions.is_empty());
        assert!(!response.cancelled);
        assert!(response.message.is_none());
        assert!(response.plugins.is_empty());
        assert_eq!(response.tick, 0);
    }
}
