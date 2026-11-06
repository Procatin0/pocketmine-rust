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
}

impl PlayerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&self, player: PlayerContext) {
        self.players
            .write()
            .insert(player.name.to_lowercase(), player);
    }

    pub fn remove(&self, name: &str) {
        self.players.write().remove(&name.to_lowercase());
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
