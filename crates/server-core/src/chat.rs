//! Java-bridge chat & commands, ported from PHP `JavaServer`
//! (`JavaServer::handleChat` / `handleCommand` / `deliverPluginMessages`).
//!
//! The Java bridge used to own chat locally in PHP: plain chat fired
//! `PlayerChatEvent` with recipients from `getOnlinePlayers()`, commands
//! (`help` / `list` / `me` / `spawn` / `tp` / `gamemode` / `time`) were
//! executed against the API `Player` objects one by one, plugin commands
//! fell through `CommandMap::dispatch()`, and plugin lines were drained
//! with an IPC `player.take_messages` host call.
//!
//! This module moves that router into Rust, directly over the shared
//! `PlayerRegistry` inbox — no IPC:
//!
//! * `handle_java_chat` — `handleChat`: trims, routes `/...` to
//!   `handle_java_command`, otherwise fires `PlayerChatEvent` with
//!   `ChatFormatter` support (BetterChat's `pocketmine\player\chat`
//!   contract) and broadcasts the rendered line.
//! * `handle_java_command` — `handleCommand`: core commands first
//!   (`crate::command::handle_command` over the registry snapshot, which
//!   already includes virtual Java players so `/list` counts them and
//!   `@a`/`@s`/`@p`/`@e`/`@r` resolve with the shared selector engine),
//!   then plugin commands via `CommandMap` (`fallbackPrefix` included),
//!   then `pocketmine.command.notFound`. Feedback goes to the sender's
//!   inbox; side effects are applied immediately via
//!   `SessionManager::apply_command_actions` (gamemode/teleport UI for
//!   virtual players is registry-driven, so no rejoin is needed).
//! * `deliver_plugin_messages` — `deliverPluginMessages`: drains the
//!   sender player's inbox in-process (`PlayerRegistry::take_messages`).
//!
//! Protocol/chunk code is untouched: Bedrock chat keeps flowing through
//! `PlayerSession::handle_text`, which already uses the same event and
//! command plumbing.

use php_plugin_bridge::{CommandSender, PlayerChatEvent, PlayerContext};

use crate::command::{CommandSource, OnlinePlayer};
use crate::session::SessionManager;

/// Result of one Java chat/command line.
#[derive(Debug, Clone, PartialEq)]
pub struct JavaChatOutcome {
    /// Lines queued to the sender's inbox (feedback / private replies).
    pub feedback: Vec<String>,
    /// Rendered chat line broadcast to everyone (plain chat only).
    pub broadcast: Option<String>,
    /// `PlayerChatEvent` was cancelled by a plugin.
    pub cancelled: bool,
}

/// Chat or command sent by a Java player (`JavaServer::handleChat`).
///
/// `sender` is the virtual `PlayerContext` registered by the bridge
/// (`is_virtual = true`). The registry is the source of truth for position
/// and gamemode, so the context is refreshed from it when available.
pub fn handle_java_chat(manager: &SessionManager, sender: &PlayerContext, message: &str) -> JavaChatOutcome {
    let trimmed = message.trim().to_string();
    if trimmed.is_empty() {
        return JavaChatOutcome {
            feedback: Vec::new(),
            broadcast: None,
            cancelled: false,
        };
    }
    if trimmed.starts_with('/') {
        return handle_java_command(manager, sender, &trimmed);
    }
    dispatch_chat(manager, sender, &trimmed)
}

/// Plain chat through `PlayerChatEvent` with `ChatFormatter` support.
///
/// Without a plugin listener the default `<name> message` line is used;
/// with PHP listeners the event's `format` (rendered in PHP via
/// `ChatFormatter::format()` when BetterChat sets one) is honoured.
/// A Java sender is verbatim for both audiences
/// (`java_bridge::display::chat_line` with `sender_is_bedrock == false`);
/// [`SessionManager::broadcast_text`] still rewrites any live Bedrock name
/// mentioned inside the line to its Floodgate `"." + raw` form for Java
/// inboxes only, while Bedrock sessions get the returned `broadcast` verbatim.
fn dispatch_chat(manager: &SessionManager, sender: &PlayerContext, message: &str) -> JavaChatOutcome {
    let sender = refresh_sender(manager, sender);
    let recipients = manager.registry().names();
    let mut event = PlayerChatEvent::new(sender.clone(), message.to_string());
    event.recipients = recipients;
    if let Some(plugins) = manager.plugin_manager() {
        match plugins.dispatch_event_sync(&mut event) {
            Ok(actions) => {
                manager.apply_actions(&actions, 0);
            }
            Err(error) => {
                tracing::warn!("Java PlayerChatEvent failed: {}", error);
            }
        }
    }
    if event.cancelled {
        return JavaChatOutcome {
            feedback: Vec::new(),
            broadcast: None,
            cancelled: true,
        };
    }
    let line = if event.format.trim().is_empty() {
        php_plugin_bridge::default_format(&sender.display_name, &event.message)
    } else {
        event.format.clone()
    };
    // Broadcast: Bedrock sessions get the packet, virtual Java players get
    // the inbox line (both via `broadcast_text` after this patch).
    manager.broadcast_text(&line);
    JavaChatOutcome {
        feedback: Vec::new(),
        broadcast: Some(line),
        cancelled: false,
    }
}

/// Commands sent by a Java player (`JavaServer::handleCommand`).
///
/// Covers the bridge-local set (`help` / `list` / `me` / `spawn` / `tp` /
/// `gamemode` / `time` — plus every other core command, since routing is
/// the shared `handle_command`) and plugin commands via `CommandMap`
/// with `fallbackPrefix` dispatch.
pub fn handle_java_command(manager: &SessionManager, sender: &PlayerContext, line: &str) -> JavaChatOutcome {
    let sender = refresh_sender(manager, sender);
    let source = java_source(manager, &sender);
    let players = manager.online_players_snapshot_public();

    if let Some(output) = crate::command::handle_command(line, &source, &players) {
        manager.apply_command_actions(&output.actions);
        for feedback in &output.feedback {
            manager.registry().push_message(&sender.name, feedback.clone());
        }
        return JavaChatOutcome {
            feedback: output.feedback,
            broadcast: None,
            cancelled: false,
        };
    }

    // Plugin commands through `CommandMap` (native map first, then PHP).
    if let Some(plugins) = manager.plugin_manager() {
        let plugin_sender = CommandSender::Player(sender.clone());
        match plugins.execute_command_sync(&plugin_sender, line) {
            Ok(Some(outcome)) => {
                manager.apply_actions(&outcome.actions, 0);
                return JavaChatOutcome {
                    feedback: Vec::new(),
                    broadcast: None,
                    cancelled: false,
                };
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!("Java plugin command '{}' failed: {}", line, error);
            }
        }
    }

    let unknown = line
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or("");
    let not_found = crate::command::not_found_message(unknown);
    manager.registry().push_message(&sender.name, not_found.clone());
    JavaChatOutcome {
        feedback: vec![not_found],
        broadcast: None,
        cancelled: false,
    }
}

/// Drains one player's inbox (`JavaServer::deliverPluginMessages`).
///
/// Direct in-process read — the PHP bridge used an IPC
/// `player.take_messages` host call; Rust callers use this instead.
pub fn deliver_plugin_messages(manager: &SessionManager, name: &str) -> Vec<String> {
    manager.registry().take_messages(name)
}

/// Refreshes the sender snapshot from the registry (position/gamemode/op
/// may have moved since registration).
fn refresh_sender(manager: &SessionManager, sender: &PlayerContext) -> PlayerContext {
    manager
        .registry()
        .get(&sender.name)
        .unwrap_or_else(|| sender.clone())
}

/// Builds a [`CommandSource`] for a virtual Java player.
fn java_source(manager: &SessionManager, sender: &PlayerContext) -> CommandSource {
    let world = manager.world();
    CommandSource {
        name: sender.name.clone(),
        position: (sender.x, sender.y, sender.z),
        is_op: sender.is_op,
        allow_cheats: manager.allow_cheats(),
        is_console: false,
        max_players: manager.max_players(),
        world_time: world.time(),
        world_seed: world.seed(),
        world_spawn: {
            let (x, y, z) = world.spawn();
            (x as f64, y as f64, z as f64)
        },
        access: Some(std::sync::Arc::clone(manager.access())),
        plugins: manager
            .plugin_manager()
            .map(|plugins| plugins.load_order())
            .unwrap_or_default(),
        tps: manager.tps(),
        permissions: Some(std::sync::Arc::clone(manager.permissions())),
    }
}

/// Test / unit helper: snapshot of the players the Java router resolves
/// targets against (sessions + virtual registry entries).
pub fn java_online_players(manager: &SessionManager) -> Vec<OnlinePlayer> {
    manager.online_players_snapshot_public()
}
