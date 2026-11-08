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
    /// lines wait here until `player.take_messages` drains them.
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
    /// Used by `player.unregister` so a stale Java disconnect cannot delete
    /// a live Bedrock entry that took the name meanwhile.
    pub fn remove_if_virtual(&self, name: &str) -> bool {
        let key = name.to_lowercase();
        let is_virtual = self.players.read().get(&key).map(|p| p.is_virtual).unwrap_or(false);
        if is_virtual {
            self.players.write().remove(&key);
            self.inboxes.write().remove(&key);
        }
        is_virtual
    }

    /// Removes the entry only if it is owned by a Bedrock session.
    /// Used by `finalize_session` so a Bedrock disconnect cannot delete
    /// a live virtual player that took the name.
    pub fn remove_if_bedrock(&self, name: &str) -> bool {
        let key = name.to_lowercase();
        let is_bedrock = self.players.read().get(&key).map(|p| !p.is_virtual).unwrap_or(false);
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
    /// Invoked after a virtual player registers, moves or unregisters so the
    /// session manager can sync Bedrock viewers (`AddPlayer` /
    /// `MoveActorAbsolute` / `RemoveActor`). Installed by `main.rs`;
    /// `None` in unit tests (polling still covers it).
    virtual_notify: RwLock<Option<Arc<dyn Fn() + Send + Sync>>>,
}

/// `players/<name>.json` shared with Bedrock sessions, relative to the CWD
/// (where `main.rs` enables persistence with `PathBuf::from(".")`).
fn player_data_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new("players").join(format!("{}.json", name.to_lowercase()))
}

fn read_player_data(name: &str) -> Option<Value> {
    let bytes = std::fs::read(player_data_path(name)).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    if value.is_object() {
        Some(value)
    } else {
        None
    }
}

fn write_player_data(name: &str, value: &Value) -> std::io::Result<()> {
    let path = player_data_path(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
    std::fs::write(path, bytes)
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
            virtual_notify: RwLock::new(None),
        }
    }

    /// Wires the virtual-visibility sync (see `virtual_notify`).
    pub fn set_virtual_notify(&self, notify: impl Fn() + Send + Sync + 'static) {
        *self.virtual_notify.write() = Some(Arc::new(notify));
    }

    fn notify_virtual(&self) {
        if let Some(notify) = self.virtual_notify.read().as_ref() {
            notify();
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
                // `set_block` enqueues the edit for both sync directions:
                // Java polling `world.take_block_changes` (Fase 1) and
                // Bedrock `UpdateBlockPacket` broadcast (Fase 2).
                let changed = self.world.set_block(x, y, z, block_id);
                Ok(json!({ "ok": true, "changed": changed }))
            }
            // ---- Fase 1 (Bedrock -> Java): block-change queue ----
            // Format (documented for the Java bridge, poll 1x/tick):
            //   request:  {}                                    (no params)
            //   response: {"ok": true, "changes": [
            //       {"x": i32, "y": i32, "z": i32, "block_id": u32},
            //       ...]}
            // Entries are oldest-first and the queue is drained by the
            // call, so a bridge that polls every tick applies each edit
            // exactly once. `block_id` is the same Bedrock network runtime
            // id `world.get_block` returns / `world.set_block` accepts.
            // The queue is capped (`BLOCK_CHANGE_QUEUE_CAP`, 512) with the
            // oldest entries dropped, and draining it does not affect the
            // separate Bedrock broadcast queue.
            "world.take_block_changes" | "world.drain_changes" => {
                let changes: Vec<Value> = self
                    .world
                    .take_changes()
                    .into_iter()
                    .map(|change| {
                        json!({
                            "x": change.x,
                            "y": change.y,
                            "z": change.z,
                            "block_id": change.block_id,
                        })
                    })
                    .collect();
                Ok(json!({ "ok": true, "changes": changes }))
            }
            "world.get_highest_block" => {
                let (x, z) = (int("x")?, int("z")?);
                Ok(json!({ "y": self.world.highest_block_at(x, z) }))
            }
            "world.get_spawn" => {
                let (x, y, z) = self.world.spawn();
                let (px, py, pz) = self.world.player_position();
                Ok(json!({ "x": x, "y": y, "z": z, "world": self.world.name, "player": [px, py, pz] }))
            }
            // ---- Fase A3 (World/Block/Item): solo `world.*` ----
            "world.get_time" => Ok(json!({ "time": self.world.time(), "running": self.world.is_time_running() })),
            "world.get_time_running" => Ok(json!({ "running": self.world.is_time_running(), "time": self.world.time() })),
            "world.set_time" => {
                let time = int("time").unwrap_or(self.world.time());
                Ok(json!({ "time": self.world.set_time(time) }))
            }
            "world.set_time_running" => {
                let running = params.get("running").and_then(Value::as_bool).unwrap_or(true);
                if running {
                    self.world.start_time();
                } else {
                    self.world.stop_time();
                }
                Ok(json!({ "ok": true }))
            }
            "world.get_difficulty" => Ok(json!({ "difficulty": self.world.difficulty() })),
            "world.set_difficulty" => {
                let difficulty = int("difficulty").unwrap_or(self.world.difficulty());
                Ok(json!({ "difficulty": self.world.set_difficulty(difficulty) }))
            }
            "world.get_seed" => Ok(json!({ "seed": self.world.seed() })),
            "world.save" => Ok(json!({ "ok": self.world.save().is_ok() })),
            "world.generate" => {
                // Single-world (ver `world_state.rs`): el nombre se registra
                // en PHP; aquí solo confirmamos el sobrevuelo del generador.
                Ok(json!({ "ok": true, "world": self.world.name }))
            }
            "world.get_auto_save" => Ok(json!({ "auto_save": self.world.auto_save() })),
            "world.set_auto_save" => {
                let enabled = params.get("auto_save").and_then(Value::as_bool).unwrap_or(true);
                self.world.set_auto_save(enabled);
                Ok(json!({ "ok": true }))
            }
            "world.get_auto_save_interval" => {
                Ok(json!({ "interval": self.world.auto_save_interval() }))
            }
            "world.set_auto_save_interval" => {
                let interval = params.get("interval").and_then(Value::as_i64).unwrap_or(6000) as i32;
                self.world.set_auto_save_interval(interval);
                Ok(json!({ "ok": true, "interval": self.world.auto_save_interval() }))
            }
            "world.do_auto_save" => {
                let _ = self.world.save();
                Ok(json!({ "ok": true }))
            }
            "world.tick" => {
                let tick = int("current_tick").or_else(|_| int("tick")).unwrap_or(0);
                self.world.record_tick(tick);
                Ok(json!({ "ok": true, "tick": tick }))
            }
            "world.get_chunk" => {
                // `World::getChunk()` / `Chunk` decodifica esta columna en
                // `world.php`; sin esto el puente Java nunca envía chunks.
                let chunk_x = params
                    .get("chunk_x")
                    .or_else(|| params.get("x"))
                    .and_then(Value::as_i64)
                    .map(|value| value as i32)
                    .ok_or_else(|| "missing chunk_x".to_string())?;
                let chunk_z = params
                    .get("chunk_z")
                    .or_else(|| params.get("z"))
                    .and_then(Value::as_i64)
                    .map(|value| value as i32)
                    .ok_or_else(|| "missing chunk_z".to_string())?;
                let (subchunks, payload) = self.world.chunk_payload(chunk_x, chunk_z);
                let hex: String = payload.iter().map(|byte| format!("{byte:02x}")).collect();
                Ok(json!({ "subchunks": subchunks, "data": hex }))
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
            // Full snapshots (Bedrock + Java bridge share this registry),
            // used by JavaBridge to sync entities.
            "server.get_players_full" => Ok(json!(self.players.all())),
            // ------------------------------------------------ persistencia
            // `players/<name>.json` con el mismo formato que las sesiones
            // Bedrock (`PlayerSession::save_player_data`).
            "player.has_data" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                let has_data = read_player_data(name).is_some();
                Ok(json!({ "has_data": has_data, "exists": has_data }))
            }
            "player.load" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                match read_player_data(name) {
                    Some(value) => {
                        let mut out = value.clone();
                        if let Some(object) = out.as_object_mut() {
                            object.insert("has_data".to_string(), json!(true));
                            object.insert("exists".to_string(), json!(true));
                        }
                        Ok(out)
                    }
                    None => Ok(json!({ "has_data": false, "exists": false })),
                }
            }
            "player.save" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_secs_f64())
                    .unwrap_or(0.0);
                let existing = read_player_data(name);
                let first_played = params
                    .get("first_played")
                    .and_then(Value::as_f64)
                    .filter(|value| *value > 0.0)
                    .or_else(|| {
                        existing
                            .as_ref()
                            .and_then(|entry| entry.get("first_played").and_then(Value::as_f64))
                            .filter(|value| *value > 0.0)
                    })
                    .unwrap_or(now);
                let value = json!({
                    "name": name,
                    "x": params.get("x").and_then(Value::as_f64).unwrap_or(0.0),
                    "y": params.get("y").and_then(Value::as_f64).unwrap_or(64.0),
                    "z": params.get("z").and_then(Value::as_f64).unwrap_or(0.0),
                    "yaw": params.get("yaw").and_then(Value::as_f64).unwrap_or(0.0),
                    "pitch": params.get("pitch").and_then(Value::as_f64).unwrap_or(0.0),
                    "gamemode": params.get("gamemode").and_then(Value::as_i64).unwrap_or(0),
                    "health": params.get("health").and_then(Value::as_f64).unwrap_or(20.0),
                    "world": params.get("world").and_then(Value::as_str).unwrap_or("world"),
                    "first_played": first_played,
                    "last_played": now,
                });
                write_player_data(name, &value).map_err(|error| error.to_string())?;
                // Mantiene el registro en sync con lo guardado.
                if let Some(mut player) = self.players.get(name) {
                    player.apply_save_json(&value);
                    self.players.upsert(player);
                }
                Ok(json!({ "ok": true }))
            }
            "player.save_data" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                let data = params.get("data").cloned().unwrap_or(Value::Null);
                let mut current = read_player_data(name).unwrap_or_else(|| json!({"name": name}));
                if let (Some(object), Some(data_object)) = (current.as_object_mut(), data.as_object()) {
                    for (key, value) in data_object {
                        object.insert(key.clone(), value.clone());
                    }
                    object.insert("name".to_string(), json!(name));
                } else if let Some(object) = current.as_object_mut() {
                    object.insert("data".to_string(), data);
                    object.insert("name".to_string(), json!(name));
                }
                write_player_data(name, &current).map_err(|error| error.to_string())?;
                Ok(json!({ "ok": true }))
            }
            "player.update_position" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                let x = params.get("x").and_then(Value::as_f64);
                let y = params.get("y").and_then(Value::as_f64);
                let z = params.get("z").and_then(Value::as_f64);
                let yaw = params.get("yaw").and_then(Value::as_f64);
                let pitch = params.get("pitch").and_then(Value::as_f64);
                let mut found = false;
                if let Some(mut player) = self.players.get(name) {
                    // Never move a Bedrock-owned entry from a virtual
                    // heartbeat; let PHP re-register only when it owns the name.
                    if player.is_virtual {
                        player.apply_position(
                            x.unwrap_or(player.x),
                            y.unwrap_or(player.y),
                            z.unwrap_or(player.z),
                            yaw.unwrap_or(player.yaw),
                            pitch.unwrap_or(player.pitch),
                        );
                        self.players.upsert(player);
                        found = true;
                    }
                }
                if found {
                    self.notify_virtual();
                }
                Ok(json!({ "ok": true, "found": found }))
            }
            "player.register" => {
                // Virtual (Java bridge) player: registers a snapshot in the
                // shared registry so plugins see them like Bedrock players.
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                let address = params
                    .get("address")
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse::<std::net::SocketAddr>().ok())
                    .unwrap_or_else(|| std::net::SocketAddr::from(([0, 0, 0, 0], 0)));
                let float = |key: &str, default: f64| {
                    params.get(key).and_then(Value::as_f64).unwrap_or(default)
                };
                let player = PlayerContext {
                    name: name.to_string(),
                    display_name: params
                        .get("display_name")
                        .and_then(Value::as_str)
                        .unwrap_or(name)
                        .to_string(),
                    address,
                    runtime_id: 0,
                    unique_id: 0,
                    ping: 0,
                    gamemode: params
                        .get("gamemode")
                        .and_then(Value::as_i64)
                        .unwrap_or(0)
                        .max(0) as u32,
                    x: float("x", 0.0),
                    y: float("y", 64.0),
                    z: float("z", 0.0),
                    yaw: float("yaw", 0.0),
                    pitch: float("pitch", 0.0),
                    world: params
                        .get("world")
                        .and_then(Value::as_str)
                        .unwrap_or("world")
                        .to_string(),
                    health: float("health", 20.0) as f32,
                    max_health: float("max_health", 20.0) as f32,
                    is_op: params
                        .get("is_op")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    is_virtual: true,
                };
                // Do not steal a live Bedrock name: keep the Bedrock entry
                // (the PHP bridge already rejects duplicates at login, this
                // is a last-race guard).
                if let Some(existing) = self.players.get(name) {
                    if !existing.is_virtual {
                        return Ok(json!({ "ok": false, "reason": "name taken by Bedrock" }));
                    }
                }
                self.players.upsert(player);
                self.notify_virtual();
                Ok(json!({ "ok": true }))
            }
            "player.unregister" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                if self.players.remove_if_virtual(name) {
                    self.notify_virtual();
                }
                Ok(json!({ "ok": true }))
            }
            "player.take_messages" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                Ok(json!({ "messages": self.players.take_messages(name) }))
            }
            "server.save_all" => {
                // Mundo + datos de jugadores del registro (Bedrock los guarda
                // SessionManager; aquí se guardan los del registro para que
                // `/save-all` desde Java persista también).
                let _ = self.world.save_to(std::path::Path::new("world.json"));
                let all = self.players.all();
                for player in &all {
                    let mut value = player.to_save_json();
                    // `to_save_json` no tiene tiempos; conserva los guardados
                    // para no borrar `first_played`/`last_played` en cada save.
                    let existing = read_player_data(&player.name);
                    if let Some(object) = value.as_object_mut() {
                        if let Some(existing) = existing.as_ref() {
                            for key in ["first_played", "last_played"] {
                                if let Some(saved) = existing.get(key) {
                                    object.insert(key.to_string(), saved.clone());
                                }
                            }
                        } else {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|duration| duration.as_secs_f64())
                                .unwrap_or(0.0);
                            object.insert("first_played".to_string(), json!(now));
                            object.insert("last_played".to_string(), json!(now));
                        }
                    }
                    let _ = write_player_data(&player.name, &value);
                }
                Ok(json!({ "ok": true, "players": all.len() }))
            }
            // ---- Fase A1 (Server+Player API): solo consultas server/player,
            // append (sin tocar el resto para evitar conflicto Bridge) ----
            "player.get_by_uuid" => {
                // `Server::getPlayerByUUID` fallback: compara `unique_id`
                // (i64) como string. Byte-raw UUIDs (16 bytes) no coinciden
                // aquí; PHP los resuelve en su registro local.
                let uuid = params
                    .get("uuid")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let found = self
                    .players
                    .all()
                    .into_iter()
                    .find(|player| player.unique_id.to_string() == uuid);
                match found {
                    Some(player) => Ok(json!(player)),
                    None => Ok(Value::Null),
                }
            }
            "player.get_by_raw_uuid" => {
                // `Server::getPlayerByRawUUID` fallback: hex-encoded raw
                // bytes -> compara `unique_id` hex. Sin match => null.
                let raw = params
                    .get("raw_uuid")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let found = self.players.all().into_iter().find(|player| {
                    let unique = player.unique_id;
                    let bytes = unique.to_be_bytes();
                    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                    raw.eq_ignore_ascii_case(&hex) || unique.to_string() == raw
                });
                match found {
                    Some(player) => Ok(json!(player)),
                    None => Ok(Value::Null),
                }
            }
            "server.get_query_info" => {
                Ok(json!({
                    "hostname": self.motd,
                    "numplayers": self.players.names().len(),
                    "maxplayers": self.max_players,
                    "players": self.players.names(),
                }))
            }
            "server.get_ops" => Ok(json!({ "ops": self.ops.read().iter().cloned().collect::<Vec<_>>() })),
            "server.set_op" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                let op = params.get("op").and_then(Value::as_bool).unwrap_or(false);
                let key = name.to_lowercase();
                if op {
                    self.ops.write().insert(key);
                } else {
                    self.ops.write().remove(&key);
                }
                Ok(json!({ "ok": true }))
            }
            "player.update_health" => {
                // `Player::setHealth/setMaxHealth` fallback para jugadores
                // virtuales (sin sesión Bedrock): actualiza el snapshot del
                // registro. Sesiones vivas se actualizan vía `PluginAction::
                // SetHealth` en `SessionManager::apply_actions`.
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                let health = params.get("health").and_then(Value::as_f64);
                let max_health = params.get("max_health").and_then(Value::as_f64);
                let mut found = false;
                if let Some(mut player) = self.players.get(name) {
                    if let Some(max) = max_health {
                        player.max_health = max.max(1.0) as f32;
                    }
                    if let Some(hp) = health {
                        player.health = hp.clamp(0.0, player.max_health as f64) as f32;
                    }
                    self.players.upsert(player);
                    found = true;
                }
                Ok(json!({ "ok": true, "found": found }))
            }
            other => Err(format!("unsupported host method '{}'", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use php_plugin_bridge::PluginHost as _;
    use std::collections::HashSet;

    fn test_host() -> (ServerPluginHost, Arc<PlayerRegistry>) {
        let generator: Arc<dyn bedrock_world::WorldGenerator> =
            Arc::new(bedrock_world::NormalGenerator::new(42));
        let world = Arc::new(WorldState::new("world", generator));
        let players = Arc::new(PlayerRegistry::new());
        let host = ServerPluginHost::new(
            world,
            Arc::clone(&players),
            Arc::new(RwLock::new(HashSet::new())),
            "test motd",
            100,
        );
        (host, players)
    }

    #[test]
    fn inbox_push_and_take_is_ordered_and_case_insensitive() {
        let registry = PlayerRegistry::new();
        registry.push_message("Steve", "first");
        registry.push_message("STEVE", "second");
        registry.push_message("Alex", "other");
        assert_eq!(
            registry.take_messages("steve"),
            vec!["first".to_string(), "second".to_string()]
        );
        // A drained inbox stays empty.
        assert_eq!(registry.take_messages("Steve"), Vec::<String>::new());
        assert_eq!(registry.take_messages("Alex"), vec!["other".to_string()]);
        assert_eq!(registry.take_messages("Nobody"), Vec::<String>::new());
    }

    #[test]
    fn inbox_is_capped() {
        let registry = PlayerRegistry::new();
        for index in 0..(PlayerRegistry::INBOX_CAP as i32 + 1) {
            registry.push_message("Steve", format!("line {index}"));
        }
        let drained = registry.take_messages("Steve");
        assert_eq!(drained.len(), PlayerRegistry::INBOX_CAP);
        assert_eq!(drained[0], "line 1");
    }

    #[test]
    fn remove_clears_inbox() {
        let registry = PlayerRegistry::new();
        registry.push_message("Steve", "pending");
        registry.remove("Steve");
        assert_eq!(registry.take_messages("Steve"), Vec::<String>::new());
    }

    #[test]
    fn virtual_player_register_take_and_unregister() {
        let (host, players) = test_host();

        let registered = host
            .call(
                "player.register",
                &json!({
                    "name": "Steve",
                    "display_name": "Steve",
                    "address": "127.0.0.1:25565",
                    "gamemode": 1,
                    "x": 1.5, "y": 64.0, "z": -2.5,
                    "yaw": 90.0, "pitch": 0.0,
                    "world": "world",
                    "health": 20.0, "max_health": 20.0,
                    "is_op": false,
                }),
            )
            .expect("player.register");
        assert_eq!(registered, json!({ "ok": true }));

        let snapshot = players.get("steve").expect("registered player");
        assert_eq!(snapshot.name, "Steve");
        assert_eq!(snapshot.gamemode, 1);
        assert_eq!(snapshot.x, 1.5);

        // Nothing pending yet: empty list, not an error.
        let empty = host
            .call("player.take_messages", &json!({ "name": "Steve" }))
            .expect("player.take_messages");
        assert_eq!(empty, json!({ "messages": [] }));

        players.push_message("Steve", "plugin line here");
        let drained = host
            .call("player.take_messages", &json!({ "name": "Steve" }))
            .expect("player.take_messages");
        assert_eq!(drained, json!({ "messages": ["plugin line here"] }));

        let unregistered = host
            .call("player.unregister", &json!({ "name": "Steve" }))
            .expect("player.unregister");
        assert_eq!(unregistered, json!({ "ok": true }));
        assert!(players.get("Steve").is_none());
    }

    #[test]
    fn bedrock_disconnect_does_not_delete_virtual_entry() {
        let (host, players) = test_host();
        host.call(
            "player.register",
            &json!({
                "name": "Procatin0",
                "display_name": "Procatin0",
                "address": "127.0.0.1:25565",
                "gamemode": 0,
                "x": 0.5, "y": 65.0, "z": 0.5,
                "yaw": 0.0, "pitch": 0.0,
                "world": "world",
                "health": 20.0, "max_health": 20.0,
                "is_op": false,
            }),
        )
        .expect("player.register");
        let snapshot = players.get("procatin0").expect("virtual entry");
        assert!(snapshot.is_virtual);

        // finalize_session path: same name, Bedrock owner cleanup must no-op.
        assert!(!players.remove_if_bedrock("Procatin0"));
        assert!(players.get("Procatin0").is_some());

        // Bedrock entry is removed by that path, virtual by unregister path.
        let mut bedrock = snapshot.clone();
        bedrock.is_virtual = false;
        players.upsert(bedrock);
        assert!(players.remove_if_bedrock("Procatin0"));
        assert!(players.get("Procatin0").is_none());
    }

    #[test]
    fn take_block_changes_returns_and_drains_set_block_edits() {
        let (host, _players) = test_host();

        // Nothing pending yet: empty list, not an error.
        let empty = host
            .call("world.take_block_changes", &json!({}))
            .expect("world.take_block_changes");
        assert_eq!(empty, json!({ "ok": true, "changes": [] }));

        host.call("world.set_block", &json!({ "x": 1, "y": 64, "z": 2, "block_id": 99 }))
            .expect("world.set_block");
        host.call("world.set_block", &json!({ "x": 3, "y": 65, "z": 4, "block_id": 100 }))
            .expect("world.set_block");
        // A no-change set_block must not enqueue a second entry. (A real
        // second change proves the drain below is not just echoing input.)
        host.call("world.set_block", &json!({ "x": 1, "y": 64, "z": 2, "block_id": 99 }))
            .expect("world.set_block (no change)");

        let drained = host
            .call("world.take_block_changes", &json!({}))
            .expect("world.take_block_changes");
        assert_eq!(
            drained,
            json!({ "ok": true, "changes": [
                { "x": 1, "y": 64, "z": 2, "block_id": 99 },
                { "x": 3, "y": 65, "z": 4, "block_id": 100 },
            ]})
        );

        // Drained: the next tick sees nothing; the alias drains likewise.
        let empty_again = host
            .call("world.drain_changes", &json!({}))
            .expect("world.drain_changes");
        assert_eq!(empty_again, json!({ "ok": true, "changes": [] }));
    }

    #[test]
    fn unregister_does_not_delete_bedrock_entry() {
        let (host, players) = test_host();
        let mut bedrock = php_plugin_bridge::PlayerContext::new("Steve", "127.0.0.1:19132".parse().unwrap());
        bedrock.is_virtual = false;
        players.upsert(bedrock.clone());
        host.call("player.unregister", &json!({ "name": "Steve" }))
            .expect("player.unregister");
        assert!(players.get("Steve").is_some());
        assert!(players.remove_if_virtual("Steve") == false);
    }

    #[test]
    fn virtual_notify_fires_on_register_move_and_unregister() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let (host, _players) = test_host();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        host.set_virtual_notify(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        host.call(
            "player.register",
            &json!({
                "name": "Steve",
                "display_name": "Steve",
                "address": "127.0.0.1:25565",
                "gamemode": 1,
                "x": 1.5, "y": 64.0, "z": -2.5,
                "yaw": 90.0, "pitch": 0.0,
                "world": "world",
                "health": 20.0, "max_health": 20.0,
                "is_op": false,
            }),
        )
        .expect("player.register");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "register notifies");

        host.call(
            "player.update_position",
            &json!({ "name": "Steve", "x": 3.5, "y": 64.0, "z": -2.5, "yaw": 90.0, "pitch": 0.0 }),
        )
        .expect("player.update_position");
        assert_eq!(calls.load(Ordering::SeqCst), 2, "move notifies");

        host.call("player.unregister", &json!({ "name": "Steve" }))
            .expect("player.unregister");
        assert_eq!(calls.load(Ordering::SeqCst), 3, "unregister notifies");

        // Unregistering a name that is not a virtual entry must not notify.
        host.call("player.unregister", &json!({ "name": "Steve" }))
            .expect("player.unregister (unknown)");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }
}
