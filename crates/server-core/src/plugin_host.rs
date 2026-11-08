//! Implementation of the synchronous host bridge exposed to PHP plugins.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use php_plugin_bridge::{PlayerContext, PluginHost};
use serde_json::{json, Value};

use crate::world_state::WorldState;

/// Lightweight, lock-independent view of the players used by the PHP runtime.
///
/// It is kept in sync by [`crate::session::SessionManager`] and avoids taking
/// the session map lock while PHP is calling back into the server.
#[derive(Default)]
pub struct PlayerRegistry {
    players: RwLock<HashMap<String, PlayerContext>>,
    /// Pending chat lines per player, keyed by lowercase name.
    ///
    /// Bedrock players receive their lines as packets from the session
    /// manager; virtual players (Java bridge) have no session, so their
    /// lines wait here until drained directly with [`PlayerRegistry::take_messages`].
    /// This is the Rust side of PHP's `deliverPluginMessages()`: instead of an
    /// IPC `player.take_messages` host call, the owning code reads the inbox
    /// directly (no IPC).
    inboxes: RwLock<HashMap<String, Vec<String>>>,
}

impl PlayerRegistry {
    /// Maximum queued chat lines per player; oldest lines are dropped past
    /// this so a player that never drains cannot grow an inbox without
    /// bound.
    const INBOX_CAP: usize = 128;
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&self, player: PlayerContext) {
        self.players
            .write()
            .insert(player.name.to_lowercase(), player);
    }

    pub fn remove(&self, name: &str) {
        let key = name.to_lowercase();
        self.players.write().remove(&key);
        self.inboxes.write().remove(&key);
    }

    /// Removes the entry only if it is a virtual (Java bridge) player.
    ///
    /// Used so a stale Java disconnect cannot delete a live Bedrock entry
    /// that took the name meanwhile.
    pub fn remove_if_virtual(&self, name: &str) -> bool {
        let key = name.to_lowercase();
        let is_virtual = self
            .players
            .read()
            .get(&key)
            .map(|p| p.is_virtual)
            .unwrap_or(false);
        if is_virtual {
            self.players.write().remove(&key);
            self.inboxes.write().remove(&key);
        }
        is_virtual
    }

    /// Removes the entry only if it is owned by a Bedrock session.
    ///
    /// Used by `finalize_session` so a Bedrock disconnect cannot delete
    /// a live virtual player that took the name.
    pub fn remove_if_bedrock(&self, name: &str) -> bool {
        let key = name.to_lowercase();
        let is_bedrock = self
            .players
            .read()
            .get(&key)
            .map(|p| !p.is_virtual)
            .unwrap_or(false);
        if is_bedrock {
            self.players.write().remove(&key);
            self.inboxes.write().remove(&key);
        }
        is_bedrock
    }

    pub fn get(&self, name: &str) -> Option<PlayerContext> {
        self.players.read().get(&name.to_lowercase()).cloned()
    }

    pub fn names(&self) -> Vec<String> {
        self.players.read().keys().cloned().collect()
    }

    pub fn all(&self) -> Vec<PlayerContext> {
        self.players.read().values().cloned().collect()
    }

    /// Queues a chat line for a player (see the `inboxes` field).
    pub fn push_message(&self, name: &str, message: impl Into<String>) {
        let mut inboxes = self.inboxes.write();
        let queue = inboxes.entry(name.to_lowercase()).or_default();
        queue.push(message.into());
        if queue.len() > Self::INBOX_CAP {
            queue.remove(0);
        }
    }

    /// Drains and returns every queued chat line for the player, oldest
    /// first. Returns an empty list when nothing is pending.
    pub fn take_messages(&self, name: &str) -> Vec<String> {
        self.inboxes
            .write()
            .remove(&name.to_lowercase())
            .unwrap_or_default()
    }
}

/// Host bridge backed by the live world and player registry.
pub struct ServerPluginHost {
    world: Arc<WorldState>,
    players: Arc<PlayerRegistry>,
    ops: Arc<RwLock<std::collections::HashSet<String>>>,
    motd: String,
    max_players: i32,
}

impl ServerPluginHost {
    pub fn new(
        world: Arc<WorldState>,
        players: Arc<PlayerRegistry>,
        ops: Arc<RwLock<std::collections::HashSet<String>>>,
        motd: impl Into<String>,
        max_players: i32,
    ) -> Self {
        Self {
            world,
            players,
            ops,
            motd: motd.into(),
            max_players,
        }
    }
}

impl PluginHost for ServerPluginHost {
    fn call(&self, method: &str, params: &Value) -> Result<Value, String> {
        let int = |key: &str| -> Result<i32, String> {
            params
                .get(key)
                .and_then(Value::as_i64)
                .map(|v| v as i32)
                .ok_or_else(|| format!("missing integer parameter '{}'", key))
        };

        match method {
            "world.get_block" => {
                let (x, y, z) = (int("x")?, int("y")?, int("z")?);
                Ok(json!({ "id": self.world.block_at(x, y, z) }))
            }
            "world.set_block" => {
                let (x, y, z) = (int("x")?, int("y")?, int("z")?);
                let block_id = params
                    .get("block_id")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| "missing block_id".to_string())? as u32;
                let changed = self.world.set_block(x, y, z, block_id);
                Ok(json!({ "ok": true, "changed": changed }))
            }
            "world.get_highest_block" => {
                let (x, z) = (int("x")?, int("z")?);
                Ok(json!({ "y": self.world.highest_block_at(x, z) }))
            }
            "world.get_spawn" => {
                let (x, y, z) = self.world.spawn();
                Ok(json!({ "x": x, "y": y, "z": z, "world": self.world.name }))
            }
            "player.get" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                match self.players.get(name) {
                    Some(player) => Ok(json!(player)),
                    None => Ok(Value::Null),
                }
            }
            "server.get_players" => Ok(json!(self.players.names())),
            "server.get_max_players" => Ok(json!({ "max": self.max_players })),
            "server.get_motd" => Ok(json!({ "motd": self.motd })),
            "server.is_op" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let is_op = self.ops.read().contains(&name.to_lowercase());
                Ok(json!({ "op": is_op }))
            }
            "server.has_permission" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let is_op = self.ops.read().contains(&name.to_lowercase());
                Ok(json!({ "allowed": is_op }))
            }
            other => Err(format!("unsupported host method '{}'", other)),
        }
    }
}
