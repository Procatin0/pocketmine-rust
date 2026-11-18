//! Implementation of the synchronous host bridge exposed to PHP plugins.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use php_plugin_bridge::{PlayerContext, PluginHost};
use serde_json::{json, Value};

use bedrock_protocol::types::Vector3f;

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
    /// Inventory snapshot per lowercase player name, published by
    /// `SessionManager` (`json!([{ "window": 0, "slots": [...] }, ...])`).
    inventories: RwLock<HashMap<String, Value>>,
    /// Edits queued by plugins, drained by `SessionManager` each tick.
    inventory_edits: Mutex<Vec<InventoryEdit>>,
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
        self.inventories.write().remove(&key);
        self.inventory_edits
            .lock()
            .retain(|edit| match edit {
                InventoryEdit::SetSlot { player, .. } | InventoryEdit::Clear { player } => {
                    player != &key
                }
            });
    }

    /// Replaces the inventory snapshot PHP reads through `inventory.get`.
    pub fn publish_inventory(&self, name: &str, windows: Value) {
        self.inventories
            .write()
            .insert(name.to_lowercase(), windows);
    }

    /// Latest published snapshot, or an empty window list for unknown players.
    pub fn inventory(&self, name: &str) -> Value {
        self.inventories
            .read()
            .get(&name.to_lowercase())
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()))
    }

    /// Queues an edit for the next tick of the session manager.
    pub fn push_inventory_edit(&self, edit: InventoryEdit) {
        self.inventory_edits.lock().push(edit);
    }

    /// Drains the edits queued since the last tick.
    pub fn take_inventory_edits(&self) -> Vec<InventoryEdit> {
        std::mem::take(&mut *self.inventory_edits.lock())
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

/// Window ids of the player containers, mirroring [`crate::inventory`].
pub const INV_WINDOW_INVENTORY: u32 = 0;
pub const INV_WINDOW_OFFHAND: u32 = 119;
pub const INV_WINDOW_ARMOR: u32 = 120;
pub const INV_WINDOW_CURSOR: u32 = 124;

/// An inventory edit queued by a plugin (`inventory.set_slot`,
/// `inventory.clear`).
///
/// `SessionManager` applies it to the authoritative `PlayerInventory` on the
/// next tick and resends the affected windows, so PHP never touches the
/// session map while the host call is running.
#[derive(Debug, Clone)]
pub enum InventoryEdit {
    SetSlot {
        /// Lowercase player name.
        player: String,
        window: u32,
        slot: u32,
        item: bedrock_protocol::ItemStack,
    },
    Clear {
        player: String,
    },
}

/// Slot size of a player window, or `None` when the id is not a player window.
pub fn inventory_window_size(window: u32) -> Option<u32> {
    match window {
        INV_WINDOW_INVENTORY => Some(crate::inventory::INVENTORY_SIZE as u32),
        INV_WINDOW_ARMOR => Some(crate::inventory::ARMOR_SIZE as u32),
        INV_WINDOW_OFFHAND => Some(1),
        INV_WINDOW_CURSOR => Some(crate::inventory::UI_SIZE as u32),
        _ => None,
    }
}

/// Decodes the lowercase hex NBT sent by `php/item.php` (`Item::getNbtBytes()`).
fn decode_hex(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    if bytes.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        out.push((high * 16 + low) as u8);
    }
    Some(out)
}

/// `ItemStack` as JSON for `PlayerInventory`. `null` is an empty slot.
///
/// NBT travels as lowercase hex both ways: `inventory.get` sends it and the
/// PHP `Item` parses it (`ench`, `display.Name`, ...); `inventory.set_slot`
/// accepts it back so enchantments and custom names reach the authoritative
/// stack.
pub fn item_json(item: &bedrock_protocol::ItemStack) -> Value {
    if item.is_empty() || item.count == 0 {
        return Value::Null;
    }
    json!({
        "id": item.network_id,
        "count": item.count,
        "meta": item.meta,
        "nbt": item.nbt.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
    })
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
    /// `players/<name>.json` root; same directory Bedrock sessions use
    /// (`SessionManager::enable_persistence`). Defaults to `"."` (the server
    /// CWD) so the production wiring matches `main.rs`; tests override it
    /// with a temp dir to avoid polluting the repo.
    data_dir: RwLock<std::path::PathBuf>,
}

/// `players/<name>.json` shared with Bedrock sessions.
fn player_data_path_in(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    dir.join("players").join(format!("{}.json", name.to_lowercase()))
}

#[allow(dead_code)]
fn player_data_path(name: &str) -> std::path::PathBuf {
    player_data_path_in(std::path::Path::new("."), name)
}

fn read_player_data_in(dir: &std::path::Path, name: &str) -> Option<Value> {
    let bytes = std::fs::read(player_data_path_in(dir, name)).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    if value.is_object() {
        Some(value)
    } else {
        None
    }
}

#[allow(dead_code)]
fn read_player_data(name: &str) -> Option<Value> {
    read_player_data_in(std::path::Path::new("."), name)
}

fn write_player_data_in(dir: &std::path::Path, name: &str, value: &Value) -> std::io::Result<()> {
    let path = player_data_path_in(dir, name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
    std::fs::write(path, bytes)
}

#[allow(dead_code)]
fn write_player_data(name: &str, value: &Value) -> std::io::Result<()> {
    write_player_data_in(std::path::Path::new("."), name, value)
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
            data_dir: RwLock::new(std::path::PathBuf::from(".")),
        }
    }

    /// Points `players/<name>.json` at the same root Bedrock uses.
    pub fn set_data_dir(&self, dir: std::path::PathBuf) {
        *self.data_dir.write() = dir;
    }

    fn data_dir(&self) -> std::path::PathBuf {
        self.data_dir.read().clone()
    }

    fn read_player_data(&self, name: &str) -> Option<Value> {
        read_player_data_in(&self.data_dir(), name)
    }

    fn write_player_data(&self, name: &str, value: &Value) -> std::io::Result<()> {
        write_player_data_in(&self.data_dir(), name, value)
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

    /// Shared `player.save` / quit save: merges supplied fields over the
    /// existing v2 file (defaults from the live registry snapshot), stamps
    /// `last_played=now`, preserves `first_played`, and writes directly to
    /// `players/<name>.json` (no staging). Returns the host result object.
    fn save_player_v2(&self, name: &str, params: &Value) -> Result<Value, String> {
        let now = crate::session::PlayerSession::now_played_secs();
        let existing = self.read_player_data(name);
        let existing_num = |key: &str| {
            existing
                .as_ref()
                .and_then(|entry| entry.get(key))
                .and_then(Value::as_f64)
        };
        let param_num = |keys: &[&str]| {
            keys.iter()
                .filter_map(|key| params.get(*key).and_then(Value::as_f64))
                .next()
        };
        let param_i64 = |keys: &[&str]| {
            keys.iter()
                .filter_map(|key| params.get(*key).and_then(Value::as_i64))
                .next()
        };
        let param_bool = |keys: &[&str]| {
            keys.iter()
                .filter_map(|key| params.get(*key).and_then(Value::as_bool))
                .next()
        };
        let param_str = |keys: &[&str]| {
            keys.iter()
                .filter_map(|key| params.get(*key).and_then(Value::as_str))
                .next()
        };
        let merged_num = |keys: &[&str], fallback: f64| {
            param_num(keys)
                .or_else(|| keys.iter().filter_map(|key| existing_num(key)).next())
                .unwrap_or(fallback)
        };
        let merged_i64 = |keys: &[&str], fallback: i64| {
            param_i64(keys)
                .or_else(|| {
                    keys.iter()
                        .filter_map(|key| {
                            existing
                                .as_ref()
                                .and_then(|entry| entry.get(*key))
                                .and_then(Value::as_i64)
                        })
                        .next()
                })
                .unwrap_or(fallback)
        };
        let merged_bool = |keys: &[&str], fallback: bool| {
            param_bool(keys)
                .or_else(|| {
                    keys.iter()
                        .filter_map(|key| {
                            existing
                                .as_ref()
                                .and_then(|entry| entry.get(*key))
                                .and_then(Value::as_bool)
                        })
                        .next()
                })
                .unwrap_or(fallback)
        };
        let merged_str = |keys: &[&str], fallback: String| -> String {
            param_str(keys)
                .map(|value| value.to_string())
                .or_else(|| {
                    keys.iter()
                        .filter_map(|key| {
                            existing
                                .as_ref()
                                .and_then(|entry| entry.get(*key))
                                .and_then(Value::as_str)
                                .map(|value| value.to_string())
                        })
                        .next()
                })
                .unwrap_or(fallback)
        };
        // Live registry fills gaps the caller omitted.
        let live = self.players.get(name);
        let live_f64 = |current: Option<f64>, slot: Option<f64>| current.or(slot);
        let first_played = param_num(&["first_played", "firstPlayed"])
            .filter(|value| *value > 0.0)
            .or_else(|| existing_num("first_played").filter(|value| *value > 0.0))
            .or_else(|| live.as_ref().map(|player| player.first_played).filter(|value| *value > 0.0))
            .unwrap_or(now);
        let spawn_point = params
            .get("spawn_point")
            .or_else(|| params.get("spawnPoint"))
            .and_then(Value::as_array)
            .map(|point| point.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
            .filter(|point| point.len() == 3)
            .or_else(|| {
                existing
                    .as_ref()
                    .and_then(|entry| entry.get("spawn_point").or_else(|| entry.get("spawnPoint")))
                    .and_then(Value::as_array)
                    .map(|point| point.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
                    .filter(|point| point.len() == 3)
            })
            .or_else(|| live.as_ref().and_then(|player| player.spawn_point.map(|point| vec![point[0], point[1], point[2]])));
        let value = json!({
            "name": name,
            "xuid": merged_str(&["xuid"], live.as_ref().map(|player| player.xuid.clone()).unwrap_or_default()),
            "display_name": merged_str(&["display_name", "displayName"], live.as_ref().map(|player| player.display_name.clone()).unwrap_or_else(|| name.to_string())),
            "x": live_f64(param_num(&["x"]), live.as_ref().map(|player| player.x)).unwrap_or(0.0),
            "y": live_f64(param_num(&["y"]), live.as_ref().map(|player| player.y)).unwrap_or(64.0),
            "z": live_f64(param_num(&["z"]), live.as_ref().map(|player| player.z)).unwrap_or(0.0),
            "yaw": live_f64(param_num(&["yaw"]), live.as_ref().map(|player| player.yaw)).unwrap_or(0.0),
            "pitch": live_f64(param_num(&["pitch"]), live.as_ref().map(|player| player.pitch)).unwrap_or(0.0),
            "head_yaw": live_f64(param_num(&["head_yaw", "headYaw"]), live.as_ref().map(|player| player.head_yaw)).unwrap_or(0.0),
            "gamemode": merged_i64(&["gamemode"], live.as_ref().map(|player| player.gamemode as i64).unwrap_or(0)),
            "health": merged_num(&["health"], live.as_ref().map(|player| player.health as f64).unwrap_or(20.0)),
            "max_health": merged_num(&["max_health", "maxHealth"], live.as_ref().map(|player| player.max_health as f64).unwrap_or(20.0)),
            "food": merged_num(&["food"], live.as_ref().map(|player| player.food as f64).unwrap_or(20.0)),
            "saturation": merged_num(&["saturation"], live.as_ref().map(|player| player.saturation as f64).unwrap_or(20.0)),
            "xp": merged_num(&["xp"], live.as_ref().map(|player| player.xp as f64).unwrap_or(0.0)),
            "xp_level": merged_i64(&["xp_level", "xpLevel"], live.as_ref().map(|player| player.xp_level as i64).unwrap_or(0)),
            "air": merged_i64(&["air"], live.as_ref().map(|player| player.air as i64).unwrap_or(300)),
            "flying": merged_bool(&["flying", "is_flying", "isFlying"], live.as_ref().map(|player| player.is_flying).unwrap_or(false)),
            "sneaking": merged_bool(&["sneaking", "is_sneaking", "isSneaking"], live.as_ref().map(|player| player.is_sneaking).unwrap_or(false)),
            "sprinting": merged_bool(&["sprinting", "is_sprinting", "isSprinting"], live.as_ref().map(|player| player.is_sprinting).unwrap_or(false)),
            "swimming": merged_bool(&["swimming", "is_swimming", "isSwimming"], live.as_ref().map(|player| player.is_swimming).unwrap_or(false)),
            "gliding": merged_bool(&["gliding", "is_gliding", "isGliding"], live.as_ref().map(|player| player.is_gliding).unwrap_or(false)),
            "world": merged_str(&["world"], live.as_ref().map(|player| player.world.clone()).unwrap_or_else(|| "world".to_string())),
            "is_op": merged_bool(&["is_op", "op"], live.as_ref().map(|player| player.is_op).unwrap_or(false)),
            "spawn_point": spawn_point,
            "first_played": first_played,
            "last_played": now,
        });
        self.write_player_data(name, &value).map_err(|error| error.to_string())?;
        if let Some(mut player) = self.players.get(name) {
            player.apply_save_json(&value);
            self.players.upsert(player);
        }
        Ok(json!({ "ok": true }))
    }
}

/// `entity.get` / `entity.list`: the JSON shape the PHP `Entity` hydrates from
/// (`Entity::applyNativeSnapshot()` in `php/player.php`).
pub(crate) fn entity_json(entity: &crate::entity::Entity) -> Value {
    let size = entity.size();
    json!({
        "unique_id": entity.unique_id,
        "runtime_id": entity.runtime_id,
        "type": entity.network_type_id(),
        "x": entity.position.x,
        "y": entity.position.y,
        "z": entity.position.z,
        "yaw": entity.yaw,
        "pitch": entity.pitch,
        "head_yaw": entity.head_yaw,
        "motion_x": entity.motion.x,
        "motion_y": entity.motion.y,
        "motion_z": entity.motion.z,
        "name_tag": entity.name_tag,
        "health": entity.health(),
        "max_health": entity.max_health(),
        "alive": entity.is_alive(),
        "on_ground": entity.on_ground,
        "height": size.height,
        "width": size.width,
        "eye_height": size.eye_height(),
        "ticks_lived": entity.ticks_lived,
    })
}

/// Optional float parameter with a PocketMine-style default.
/// `World::addSound($pos, $sound, $players)`: the PHP side sends player names
/// (lower-case); an empty list means every viewer of the block's chunk, like
/// PM's `getViewersForPosition()` default.
fn player_names(params: &Value) -> Vec<String> {
    params
        .get("players")
        .and_then(Value::as_array)
        .map(|players| {
            players
                .iter()
                .filter_map(Value::as_str)
                .map(|name| name.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

/// `AxisAlignedBB` payload used by the collision host calls.
fn aabb_param(params: &Value) -> crate::entity::Aabb {
    crate::entity::Aabb::new(
        float_param(params, "min_x", 0.0),
        float_param(params, "min_y", 0.0),
        float_param(params, "min_z", 0.0),
        float_param(params, "max_x", 0.0),
        float_param(params, "max_y", 0.0),
        float_param(params, "max_z", 0.0),
    )
}

fn float_param(params: &Value, key: &str, default: f32) -> f32 {
    params
        .get(key)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .unwrap_or(default)
}

/// `block.state` payload: the Bedrock name and properties of a block state id.
///
/// Kept as a free function so the translation can be unit tested without a
/// running server.
fn block_state_json(id: u32) -> Option<serde_json::Value> {
    let name = bedrock_world::block_states::state_name(id)?;
    let mut properties = serde_json::Map::new();
    if let Some(states) = bedrock_world::block_states::state_properties(id) {
        for (key, value) in states {
            let value = match value {
                bedrock_world::block_states::Value::String(value) => Value::String(value.clone()),
                bedrock_world::block_states::Value::Byte(value) => json!(*value),
                bedrock_world::block_states::Value::Short(value) => json!(*value),
                bedrock_world::block_states::Value::Int(value) => json!(*value),
                bedrock_world::block_states::Value::Long(value) => json!(*value),
                bedrock_world::block_states::Value::Float(value) => json!(*value),
                bedrock_world::block_states::Value::Double(value) => json!(*value),
                // Nested compounds are not block properties PM exposes.
                bedrock_world::block_states::Value::Compound(_) => continue,
            };
            properties.insert(key.clone(), value);
        }
    }
    Some(json!({
        "id": id,
        "name": format!("minecraft:{}", name),
        "properties": properties,
        // `Block::getMeta()`: `null` cuando BDS no tiene meta legacy (`-1`) o
        // el estado no está en la paleta.
        "meta": crate::legacy_meta::legacy_meta(id),
        // `Block::isSolid()`: `has_collision` es la misma tabla que usa el
        // mundo al colocar entidades.
        "solid": bedrock_world::block_states::has_collision(name),
    }))
}

/// `EffectInstance` JSON used by `entity.effects` / `entity.add_effect`
/// (`pocketmine\entity\effect\EffectInstance` in `php/effect.php`).
fn effect_json(effect: &crate::world_state::ActiveEffect) -> Value {
    json!({
        "id": effect.id,
        "amplifier": effect.amplifier,
        "duration": effect.duration,
        "visible": effect.visible,
        "ambient": effect.ambient,
        "infinite": effect.infinite,
    })
}

/// `RecipeIngredient` JSON (`pocketmine\crafting\RecipeIngredient`).
fn ingredient_json(ingredient: &crate::crafting::RecipeIngredient) -> Value {
    use crate::crafting::RecipeIngredient;
    match ingredient {
        RecipeIngredient::Exact { network_id, meta } => {
            json!({ "type": "exact", "id": network_id, "meta": meta })
        }
        RecipeIngredient::MetaWildcard { network_id } => {
            json!({ "type": "meta", "id": network_id })
        }
        RecipeIngredient::TagWildcard { tag } => json!({ "type": "tag", "tag": tag }),
    }
}

/// Parses a `RecipeIngredient` sent by PHP (`crafting.register_*`).
fn parse_ingredient(value: &Value) -> Option<crate::crafting::RecipeIngredient> {
    use crate::crafting::RecipeIngredient;
    match value.get("type").and_then(Value::as_str).unwrap_or("exact") {
        "tag" => Some(RecipeIngredient::TagWildcard {
            tag: value.get("tag")?.as_str()?.to_string(),
        }),
        "meta" | "meta_wildcard" => Some(RecipeIngredient::MetaWildcard {
            network_id: value.get("id")?.as_i64()? as i16,
        }),
        _ => Some(RecipeIngredient::Exact {
            network_id: value.get("id")?.as_i64()? as i16,
            meta: value.get("meta").and_then(Value::as_u64).unwrap_or(0) as u32,
        }),
    }
}

/// Parses an `ItemStack` sent by PHP (item JSON, `count` defaults to 1).
fn parse_item_stack(value: &Value) -> Option<bedrock_protocol::ItemStack> {
    let count = value.get("count").and_then(Value::as_u64).unwrap_or(1);
    if count == 0 {
        return None;
    }
    Some(bedrock_protocol::ItemStack {
        network_id: value.get("id").and_then(Value::as_i64)? as i16,
        count: count.min(u64::from(u16::MAX)) as u16,
        meta: value.get("meta").and_then(Value::as_u64).unwrap_or(0) as u32,
        ..bedrock_protocol::ItemStack::empty()
    })
}

/// One recipe in the `crafting.recipes` index (`pocketmine\crafting`:
/// `ShapedRecipe` / `ShapelessRecipe` / `FurnaceRecipe`).
fn crafting_recipe_json(index: u32, kind: &crate::crafting::CraftingRecipeKind) -> Value {
    use crate::crafting::CraftingRecipeKind;
    match kind {
        CraftingRecipeKind::Shaped(recipe) => {
            let height = recipe.shape.len();
            let width = recipe.shape.first().map(Vec::len).unwrap_or(0);
            let cells: Vec<Value> = recipe
                .shape
                .iter()
                .flatten()
                .map(|cell| cell.as_ref().map(ingredient_json).unwrap_or(Value::Null))
                .collect();
            json!({
                "index": index,
                "kind": "shaped",
                "id": recipe.id,
                "result": item_json(&recipe.result),
                "height": height,
                "width": width,
                "cells": cells,
                "priority": recipe.priority,
            })
        }
        CraftingRecipeKind::Shapeless(recipe) => json!({
            "index": index,
            "kind": "shapeless",
            "id": recipe.id,
            "result": item_json(&recipe.result),
            "ingredients": recipe
                .ingredients
                .iter()
                .map(ingredient_json)
                .collect::<Vec<_>>(),
            "priority": recipe.priority,
        }),
        CraftingRecipeKind::Furnace(recipe) => json!({
            "index": index,
            "kind": "furnace",
            "id": recipe.id,
            "block": recipe.block,
            "result": item_json(&recipe.output),
            "ingredients": [ingredient_json(&recipe.input)],
            "ticks": crate::crafting::CraftingManager::furnace_cook_ticks(&recipe.block),
        }),
    }
}

/// Index of the first runtime recipe in the `crafting.recipes` list
/// (vanilla shaped, shapeless and furnace recipes come first).
fn dynamic_recipe_offset() -> u32 {
    let manager = crate::crafting::manager();
    (manager.shaped_recipes().len()
        + manager.shapeless_recipes().len()
        + manager.furnace_recipes().len()) as u32
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
            // ---- Registro de bloques (PHP `Block`/`VanillaBlocks`) ----
            //
            // request:  {"name": "minecraft:stone", "states": {"axis": "y"}}
            // response: {"id": <u32>}   (mismo id de estado que devuelve
            //            `world.get_block` y acepta `world.set_block`)
            //
            // El PHP no lleva una copia de la paleta: los ids salen de
            // `bedrock_world::block_states`, la misma tabla que usa el mundo,
            // así que cualquier bloque vanilla funciona sin listas en PHP.
            "block.network_id" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing string parameter 'name'".to_string())?;
                let mut block_id = bedrock_world::block_states::network_id(name)
                    .ok_or_else(|| format!("unknown block '{}'", name))?;
                if let Some(states) = params.get("states").and_then(Value::as_object) {
                    for (key, value) in states {
                        let value = match value {
                            Value::String(value) => value.clone(),
                            Value::Bool(value) => if *value { "true" } else { "false" }.to_string(),
                            other => other.to_string(),
                        };
                        block_id = bedrock_world::block_states::with_property(block_id, key, &value)
                            .ok_or_else(|| {
                                format!("block '{}' has no state '{}={}'", name, key, value)
                            })?;
                    }
                }
                Ok(json!({ "id": block_id }))
            }
            // ---- Estado de un bloque (PHP `BlockStateSerializer`) ----
            //
            // request:  {"id": <u32>}
            // response: {"name": "minecraft:stone", "properties": {...}}
            //
            // `properties` sale de la misma tabla que `block.network_id`, así
            // que serializar y deserializar un estado es ida y vuelta exacta.
            "block.state" => {
                let id = params
                    .get("id")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| "missing unsigned parameter 'id'".to_string())? as u32;
                block_state_json(id).ok_or_else(|| format!("unknown block state id {}", id))
            }
            // ---- Break info (PHP `BlockBreakInfo`) ----
            //
            // request:  {"id": <u32>}  o  {"name": "minecraft:stone"}
            // response: {"breakable": true, "hardness": f32, "instant": bool,
            //            "bare_hand": bool, "seconds": f32}
            //
            // `breakable: false` es lo que PM llama `isBreakable() === false`
            // (aire, bedrock, agua).
            "block.break_info" => {
                let block_id = if let Some(id) = params.get("id").and_then(Value::as_u64) {
                    id as u32
                } else {
                    let name = params
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "missing id or name".to_string())?;
                    bedrock_world::block_states::network_id(name)
                        .ok_or_else(|| format!("unknown block '{}'", name))?
                };
                Ok(match crate::block_info::break_info(block_id) {
                    Some(info) => json!({
                        "breakable": true,
                        "hardness": info.hardness,
                        "instant": info.instant,
                        "bare_hand": info.bare_hand_compatible,
                        "seconds": info.seconds,
                    }),
                    None => json!({ "breakable": false }),
                })
            }
            "world.get_block" => {
                let (x, y, z) = (int("x")?, int("y")?, int("z")?);
                Ok(json!({ "id": self.world.block_at(x, y, z) }))
            }
            // ---- Fase 15: sonido, partículas y biomas (PHP `World`) ----
            // `World::addSound()`: the PHP `Sound` resolves to a level sound
            // name plus its extra data; the tick broadcasts one
            // `LevelSoundEventPacket` at the exact position the plugin passed
            // (no block-centre offset, unlike the core's block sounds).
            "world.add_sound" => {
                let extra_data = params
                    .get("extra_data")
                    .and_then(Value::as_i64)
                    .unwrap_or(0) as i32;
                let (x, y, z) = (
                    float_param(params, "x", 0.0),
                    float_param(params, "y", 0.0),
                    float_param(params, "z", 0.0),
                );
                let recipients = player_names(params);
                // PM `Sound::encode()` devuelve `LevelSoundEventPacket` o
                // `LevelEventPacket`: el PHP elige una de las dos vías.
                if let Some(event_id) = params.get("event_id").and_then(Value::as_i64) {
                    self.world.queue_world_effect(
                        crate::world_state::PendingWorldEffect::Sound {
                            sound: None,
                            event_id: Some(event_id as i32),
                            extra_data,
                            x,
                            y,
                            z,
                            recipients,
                        },
                    );
                    return Ok(json!({ "ok": true, "kind": "level_event" }));
                }
                let sound = params
                    .get("sound")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing sound or event_id".to_string())?
                    .to_string();
                self.world
                    .queue_world_effect(crate::world_state::PendingWorldEffect::Sound {
                        sound: Some(sound),
                        event_id: None,
                        extra_data,
                        x,
                        y,
                        z,
                        recipients,
                    });
                Ok(json!({ "ok": true, "kind": "level_sound" }))
            }
            // `World::addParticle()`: a `LevelEventPacket` (eventId, position,
            // eventData) queued for the tick, matching PM's packet shape.
            "world.add_particle" => {
                let event_id = params
                    .get("event_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing event_id".to_string())? as i32;
                let event_data = params
                    .get("event_data")
                    .and_then(Value::as_i64)
                    .unwrap_or(0) as i32;
                let (x, y, z) = (
                    float_param(params, "x", 0.0),
                    float_param(params, "y", 0.0),
                    float_param(params, "z", 0.0),
                );
                self.world
                    .queue_world_effect(crate::world_state::PendingWorldEffect::Particle {
                        event_id,
                        event_data,
                        x,
                        y,
                        z,
                        recipients: player_names(params),
                    });
                                Ok(json!({ "ok": true }))
            }
            // ---- Fase 19: `pocketmine\world\Explosion` ----
            // `Explosion::explodeA()` asks the core for the blast list; the
            // plugin can edit `$affectedBlocks` before `explodeB()`.
            "world.explosion_blocks" => {
                let radius = float_param(params, "radius", 0.0);
                if !(radius > 0.0) {
                    return Err("radius must be greater than 0".to_string());
                }
                let position = Vector3f::new(
                    float_param(params, "x", 0.0),
                    float_param(params, "y", 0.0),
                    float_param(params, "z", 0.0),
                );
                let blocks = self.world.explosion_blocks(position, radius);
                Ok(json!({
                    "blocks": blocks
                        .iter()
                        .map(|&(x, y, z, state_id)| json!({
                            "x": x, "y": y, "z": z, "state_id": state_id
                        }))
                        .collect::<Vec<_>>()
                }))
            }
            // `Explosion::explodeB()`: queues the blast for the tick, which
            // fires `EntityExplodeEvent` (only with an entity source) and
            // applies the surviving block list plus player damage.
            "world.explode" => {
                let radius = float_param(params, "radius", 0.0);
                if !(radius > 0.0) {
                    return Err("radius must be greater than 0".to_string());
                }
                let position = Vector3f::new(
                    float_param(params, "x", 0.0),
                    float_param(params, "y", 0.0),
                    float_param(params, "z", 0.0),
                );
                let blocks = params.get("blocks").and_then(Value::as_array).map(|list| {
                    list.iter()
                        .filter_map(|block| {
                            Some((
                                block.get("x")?.as_i64()? as i32,
                                block.get("y")?.as_i64()? as i32,
                                block.get("z")?.as_i64()? as i32,
                                block.get("state_id")?.as_u64()? as u32,
                            ))
                        })
                        .collect::<Vec<_>>()
                });
                let entity = params
                    .get("entity_id")
                    .and_then(Value::as_i64)
                    .and_then(|id| {
                        self.world
                            .entities_snapshot()
                            .into_iter()
                            .find(|entity| entity.unique_id as i64 == id)
                    })
                    .map(|entity| entity_json(&entity));
                let count = self.world.queue_explosion(position, radius, entity, blocks);
                Ok(json!({ "ok": true, "count": count }))
            }
            // `World::getCollisionBlocks()`/`getBlockCollisionBoxes()`: the
            // core's cube collision model for the blocks inside an AABB.
            "world.collision_blocks" => {
                let bb = aabb_param(params);
                let blocks = self.world.collision_blocks(&bb);
                Ok(json!({
                    "blocks": blocks
                        .iter()
                        .map(|&(x, y, z, state_id)| json!({
                            "x": x, "y": y, "z": z, "state_id": state_id
                        }))
                        .collect::<Vec<_>>()
                }))
            }
            "world.collision_boxes" => {
                let bb = aabb_param(params);
                let boxes = self.world.collision_boxes(&bb);
                Ok(json!({
                    "boxes": boxes
                        .iter()
                        .map(|b| json!({
                            "min_x": b.min_x,
                            "min_y": b.min_y,
                            "min_z": b.min_z,
                            "max_x": b.max_x,
                            "max_y": b.max_y,
                            "max_z": b.max_z
                        }))
                        .collect::<Vec<_>>()
                }))
            }
            // `World::scheduleDelayedBlockUpdate()`: re-set the current block
            // after `delay` ticks through the core's scheduled queue.
            "world.schedule_block_update" => {
                let (x, y, z) = (int("x")?, int("y")?, int("z")?);
                let delay = params
                    .get("delay")
                    .and_then(Value::as_i64)
                    .unwrap_or(1) as i32;
                let block_id = self.world.block_at(x, y, z);
                self.world
                    .schedule_delayed_block_update(x, y, z, block_id, delay, None);
                Ok(json!({ "ok": true, "block_id": block_id }))
            }
            // `World::notifyNeighbourBlockUpdate()`: neighbour cascade without
            // changing the block (connections, falling blocks, fluids).
            "world.notify_neighbour" => {
                let (x, y, z) = (int("x")?, int("y")?, int("z")?);
                self.world.notify_neighbour_block_update(x, y, z);
                Ok(json!({ "ok": true }))
            }
            // `World::getBiomeId()`: override or the generator's column biome.
            "world.get_biome" => {
                let (x, y, z) = (int("x")?, int("y")?, int("z")?);
                Ok(json!({ "biome": self.world.biome_at(x, y, z) }))
            }
            // `World::setBiomeId()`: persists a per-column override that the
            // next chunk payload carries.
            "world.set_biome" => {
                let (x, y, z) = (int("x")?, int("y")?, int("z")?);
                let biome = params
                    .get("biome")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| "missing biome".to_string())?;
                let changed = self.world.set_biome(x, y, z, biome as u8);
                Ok(json!({ "ok": true, "changed": changed }))
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
            // `World::setSpawnLocation()`: the world queues `SpawnChangeEvent`
            // (dispatched by the tick) and answers with the block-level spawn it
            // stored, which PHP mirrors locally.
            "world.set_spawn" => {
                let number = |key: &str, fallback: i32| {
                    params
                        .get(key)
                        .and_then(Value::as_f64)
                        .map(|value| value.round() as i32)
                        .unwrap_or(fallback)
                };
                let (x, y, z) = self.world.spawn();
                self.world.set_spawn(
                    number("x", x),
                    number("y", y),
                    number("z", z),
                );
                let (x, y, z) = self.world.spawn();
                Ok(json!({ "ok": true, "x": x, "y": y, "z": z }))
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
            // ---- Fase 1 (API de plugins): entidades ----
            // `EntityFactory::create()` + `Entity::spawnToAll()` en PHP. El alta
            // en el mundo es `World::addEntity()`; la difusión a los clientes
            // la hace `SessionManager::sync_entities()` por chunks
            // (`Entity::spawnTo`), igual que en PocketMine.
            "entity.spawn" => {
                let type_id = params
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing entity type".to_string())?;
                let name_tag = params
                    .get("name_tag")
                    .and_then(Value::as_str)
                    .filter(|tag| !tag.is_empty());
                match self.world.spawn_mob_configured(
                    type_id,
                    Vector3f::new(
                        float_param(params, "x", 0.0),
                        float_param(params, "y", 64.0),
                        float_param(params, "z", 0.0),
                    ),
                    float_param(params, "yaw", 0.0),
                    float_param(params, "pitch", 0.0),
                    name_tag,
                ) {
                    Some(unique_id) => match self.world.entity_by_id(unique_id) {
                        Some(entity) => Ok(json!({ "ok": true, "entity": entity_json(&entity) })),
                        None => Ok(json!({ "ok": false, "error": "entity vanished after spawn" })),
                    },
                    None => Ok(json!({
                        "ok": false,
                        "error": format!(
                            "unknown or unspawnable entity type '{type_id}' (or entity cap reached)"
                        ),
                        "type": type_id,
                    })),
                }
            }
            "entity.get" => {
                // Acepta `unique_id` (`Entity::getId()`) o el runtime id Bedrock.
                let entity = if let Some(unique_id) =
                    params.get("unique_id").and_then(Value::as_i64)
                {
                    self.world.entity_by_id(unique_id)
                } else if let Some(runtime_id) = params.get("runtime_id").and_then(Value::as_u64) {
                    self.world.entity_by_runtime(runtime_id)
                } else {
                    return Err("missing unique_id or runtime_id".to_string());
                };
                Ok(match entity {
                    Some(entity) => json!({ "ok": true, "entity": entity_json(&entity) }),
                    None => json!({ "ok": false, "entity": Value::Null }),
                })
            }
            "entity.list" => {
                let entities: Vec<Value> = self
                    .world
                    .entities_snapshot()
                    .iter()
                    .map(entity_json)
                    .collect();
                Ok(json!({ "entities": entities, "count": entities.len() }))
            }
            "entity.despawn" | "entity.remove" => {
                let unique_id = params
                    .get("unique_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing unique_id".to_string())?;
                Ok(json!({ "ok": self.world.remove_entity(unique_id) }))
            }
            // `World::dropItem()`: an `ItemEntity` with the stack the plugin
            // built (`item.toArray()` on the PHP side).
            "entity.spawn_item" => {
                let item = parse_item_stack(params.get("item").unwrap_or(&Value::Null))
                    .ok_or_else(|| "invalid item".to_string())?;
                let position = Vector3f::new(
                    float_param(params, "x", 0.0),
                    float_param(params, "y", 64.0),
                    float_param(params, "z", 0.0),
                );
                let motion = match (
                    params.get("motion_x").and_then(Value::as_f64),
                    params.get("motion_y").and_then(Value::as_f64),
                    params.get("motion_z").and_then(Value::as_f64),
                ) {
                    (Some(x), Some(y), Some(z)) => {
                        Some(Vector3f::new(x as f32, y as f32, z as f32))
                    }
                    _ => None,
                };
                let owner = params.get("owner").and_then(Value::as_str).unwrap_or("");
                match self.world.spawn_item(position, item, motion, owner) {
                    Some(id) => match self.world.entity_by_id(id) {
                        Some(entity) => Ok(json!({ "ok": true, "entity": entity_json(&entity) })),
                        None => Ok(json!({ "ok": false })),
                    },
                    None => Ok(json!({ "ok": false, "error": "spawn failed" })),
                }
            }
            // `new FallingBlock($location, $block)`.
            "entity.spawn_falling_block" => {
                let x = params.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
                let y = params.get("y").and_then(Value::as_i64).unwrap_or(64) as i32;
                let z = params.get("z").and_then(Value::as_i64).unwrap_or(0) as i32;
                let block_id = params.get("block_id").and_then(Value::as_u64).unwrap_or(0) as u32;
                match self.world.spawn_falling_block(x, y, z, block_id) {
                    Some(id) => match self.world.entity_by_id(id) {
                        Some(entity) => Ok(json!({ "ok": true, "entity": entity_json(&entity) })),
                        None => Ok(json!({ "ok": false })),
                    },
                    None => Ok(json!({ "ok": false, "error": "spawn failed" })),
                }
            }
            // `new PrimedTNT($location, $fuse)`.
            "entity.spawn_tnt" => {
                let position = Vector3f::new(
                    float_param(params, "x", 0.0),
                    float_param(params, "y", 64.0),
                    float_param(params, "z", 0.0),
                );
                let fuse = params
                    .get("fuse")
                    .and_then(Value::as_i64)
                    .unwrap_or(80)
                    .clamp(0, 32_767) as i32;
                match self.world.spawn_primed_tnt(position, fuse) {
                    Some(id) => match self.world.entity_by_id(id) {
                        Some(entity) => Ok(json!({ "ok": true, "entity": entity_json(&entity) })),
                        None => Ok(json!({ "ok": false })),
                    },
                    None => Ok(json!({ "ok": false, "error": "spawn failed" })),
                }
            }
            // `World::dropExperience($pos, $amount)`.
            "entity.spawn_xp_orb" => {
                let position = Vector3f::new(
                    float_param(params, "x", 0.0),
                    float_param(params, "y", 64.0),
                    float_param(params, "z", 0.0),
                );
                let value = params.get("value").and_then(Value::as_i64).unwrap_or(0) as i32;
                match self.world.spawn_experience_orb(position, value) {
                    Some(id) => match self.world.entity_by_id(id) {
                        Some(entity) => Ok(json!({ "ok": true, "entity": entity_json(&entity) })),
                        None => Ok(json!({ "ok": false })),
                    },
                    None => Ok(json!({ "ok": false, "error": "spawn failed" })),
                }
            }
            // `new Arrow($location, $shooter, $critical)` and the other
            // `Projectile` subclasses (`Projectile::__construct`).
            "entity.spawn_projectile" => {
                let type_id = params
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing type".to_string())?;
                let kind = crate::entity::ProjectileKind::from_network_type_id(type_id)
                    .ok_or_else(|| format!("unsupported projectile '{}'", type_id))?;
                let position = Vector3f::new(
                    float_param(params, "x", 0.0),
                    float_param(params, "y", 64.0),
                    float_param(params, "z", 0.0),
                );
                let motion = Vector3f::new(
                    float_param(params, "motion_x", 0.0),
                    float_param(params, "motion_y", 0.0),
                    float_param(params, "motion_z", 0.0),
                );
                let owner = params.get("owner").and_then(Value::as_str).unwrap_or("");
                let owner_runtime_id = params
                    .get("owner_runtime_id")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let damage = float_param(params, "damage", 0.0);
                let critical = params
                    .get("critical")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                match self.world.spawn_projectile(
                    kind,
                    position,
                    motion,
                    owner,
                    owner_runtime_id,
                    damage,
                    critical,
                ) {
                    Some(id) => match self.world.entity_by_id(id) {
                        Some(entity) => Ok(json!({ "ok": true, "entity": entity_json(&entity) })),
                        None => Ok(json!({ "ok": false })),
                    },
                    None => Ok(json!({ "ok": false, "error": "spawn failed" })),
                }
            }
            "entity.teleport" => {
                let unique_id = params
                    .get("unique_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing unique_id".to_string())?;
                let moved = self.world.set_entity_position(
                    unique_id,
                    Vector3f::new(
                        float_param(params, "x", 0.0),
                        float_param(params, "y", 64.0),
                        float_param(params, "z", 0.0),
                    ),
                    params.get("yaw").and_then(Value::as_f64).map(|v| v as f32),
                    params
                        .get("pitch")
                        .and_then(Value::as_f64)
                        .map(|v| v as f32),
                );
                Ok(json!({
                    "ok": moved,
                    "entity": self.world.entity_by_id(unique_id).map(|entity| entity_json(&entity)),
                }))
            }
            "entity.set_health" => {
                let unique_id = params
                    .get("unique_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing unique_id".to_string())?;
                let health = params
                    .get("health")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| "missing health".to_string())? as f32;
                let applied = self.world.set_entity_health(unique_id, health);
                Ok(match self.world.entity_by_id(unique_id) {
                    Some(entity) => json!({
                        "ok": applied.is_some(),
                        "entity": entity_json(&entity),
                    }),
                    None => json!({ "ok": false, "error": "entity not found" }),
                })
            }
            "entity.damage" => {
                let unique_id = params
                    .get("unique_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing unique_id".to_string())?;
                let damage = params
                    .get("damage")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| "missing damage".to_string())? as f32;
                let Some(entity) = self.world.entity_by_id(unique_id) else {
                    return Ok(json!({ "ok": false, "error": "entity not found" }));
                };
                Ok(match self.world.damage_entity(entity.runtime_id, damage) {
                    // `Living::attack()`: the actor event (hurt / death
                    // animation) is queued by `damage_entity()` itself and
                    // goes out in `SessionManager::apply_entity_events()`.
                    Some((_, outcome)) => json!({
                        "ok": true,
                        "killed": outcome == crate::entity::DamageOutcome::Killed,
                        "entity": self.world.entity_by_id(unique_id).map(|entity| entity_json(&entity)),
                    }),
                    None => json!({
                        "ok": false,
                        "error": "damage ignored (not living, already dead or invulnerable)",
                        "entity": entity_json(&entity),
                    }),
                })
            }
            "entity.set_motion" => {
                let unique_id = params
                    .get("unique_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing unique_id".to_string())?;
                let motion = Vector3f::new(
                    float_param(params, "x", 0.0),
                    float_param(params, "y", 0.0),
                    float_param(params, "z", 0.0),
                );
                let applied = self.world.set_entity_motion(unique_id, motion);
                Ok(json!({
                    "ok": applied,
                    "entity": self.world.entity_by_id(unique_id).map(|entity| entity_json(&entity)),
                }))
            }
            "entity.knock_back" => {
                // `Living::knockBack($x, $z, $force)`: the native pass needs the
                // Bedrock runtime id, so it is resolved from the PocketMine id.
                let unique_id = params
                    .get("unique_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing unique_id".to_string())?;
                let Some(entity) = self.world.entity_by_id(unique_id) else {
                    return Ok(json!({ "ok": false, "error": "entity not found" }));
                };
                self.world.knock_back_entity(
                    entity.runtime_id,
                    float_param(params, "dx", 0.0),
                    float_param(params, "dz", 0.0),
                    float_param(params, "force", 0.4),
                );
                Ok(json!({
                    "ok": true,
                    "entity": self.world.entity_by_id(unique_id).map(|entity| entity_json(&entity)),
                }))
            }
            "entity.set_name_tag" => {
                let unique_id = params
                    .get("unique_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing unique_id".to_string())?;
                let name_tag = params
                    .get("name_tag")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let ok = self.world.set_entity_name_tag(unique_id, name_tag);
                Ok(json!({
                    "ok": ok,
                    "entity": self.world.entity_by_id(unique_id).map(|entity| entity_json(&entity)),
                }))
            }
            // ---- Fase 4 (API de plugins): inventario del jugador ----
            // `PlayerInventory` en PHP es un proxy: lee el snapshot que publica
            // el tick (`inventory.get`) y escribe por `inventory.set_slot` /
            // `inventory.clear`, que se aplican al inventario autoritativo en
            // el tick siguiente y se resincronizan al cliente.
            "inventory.get" => {
                let name = params
                    .get("player")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player".to_string())?;
                Ok(json!({
                    "ok": true,
                    "player": name,
                    "windows": self.players.inventory(name),
                }))
            }
            "inventory.set_slot" => {
                let name = params
                    .get("player")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player".to_string())?;
                let window = params
                    .get("window")
                    .and_then(Value::as_i64)
                    .unwrap_or(INV_WINDOW_INVENTORY as i64) as u32;
                let slot = params
                    .get("slot")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing slot".to_string())? as u32;
                let Some(size) = inventory_window_size(window) else {
                    return Ok(json!({
                        "ok": false,
                        "error": format!("unknown inventory window {window}"),
                    }));
                };
                if slot >= size {
                    return Ok(json!({
                        "ok": false,
                        "error": format!("slot {slot} out of range for window {window} (size {size})"),
                    }));
                }
                let stack = match params.get("item") {
                    Some(value) if !value.is_null() => {
                        let count = value.get("count").and_then(Value::as_u64).unwrap_or(0);
                        if count == 0 {
                            bedrock_protocol::ItemStack::empty()
                        } else {
                            bedrock_protocol::ItemStack {
                                network_id: value
                                    .get("id")
                                    .and_then(Value::as_i64)
                                    .unwrap_or(0) as i16,
                                count: count.min(u64::from(u16::MAX)) as u16,
                                meta: value.get("meta").and_then(Value::as_u64).unwrap_or(0) as u32,
                                nbt: value
                                    .get("nbt")
                                    .and_then(Value::as_str)
                                    .and_then(decode_hex)
                                    .unwrap_or_default(),
                                ..bedrock_protocol::ItemStack::empty()
                            }
                        }
                    }
                    _ => bedrock_protocol::ItemStack::empty(),
                };
                self.players.push_inventory_edit(InventoryEdit::SetSlot {
                    player: name.to_lowercase(),
                    window,
                    slot,
                    item: stack,
                });
                // Se aplica en el tick siguiente: `setItem()` en PHP es void.
                Ok(json!({ "ok": true, "applied": false }))
            }
            "inventory.clear" => {
                let name = params
                    .get("player")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player".to_string())?;
                self.players.push_inventory_edit(InventoryEdit::Clear {
                    player: name.to_lowercase(),
                });
                Ok(json!({ "ok": true, "applied": false }))
            }
            // `ItemFactory::get($name)` / `fromString()` for namespaced ids
            // (`minecraft:rotten_flesh`): `data/item_ids.txt` keeps the classic
            // numeric id, which is what the PHP `Item` expects.
            "item.id_by_name" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing item name".to_string())?;
                let id = crate::inventory::item_network_id(name);
                Ok(json!({ "id": id, "found": id.is_some() }))
            }
            "item.name_by_id" => {
                let id = params
                    .get("id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing item id".to_string())?;
                let name = crate::inventory::item_canonical_name(id as i16);
                Ok(json!({ "name": name, "found": name.is_some() }))
            }
            // ---- Fase 7b (API de plugins): efectos de entidad ----
            // `Living::getEffects()` / `EffectCollection`. El estado vive en
            // `WorldState::entity_effects` y decae en `tick_entities()`; los
            // paquetes de efecto del cliente (`MobEffectPacket`) todavía no
            // existen en `bedrock-protocol`, así que el efecto es autoritativo
            // en el servidor pero no se dibuja la burbuja (documentado en
            // `docs/plugin-compatibility.md`).
            "entity.effects" => {
                let unique_id = params
                    .get("unique_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing unique_id".to_string())?;
                if self.world.entity_by_id(unique_id).is_none() {
                    return Ok(json!({ "ok": false, "error": "entity not found" }));
                }
                let effects: Vec<Value> = self
                    .world
                    .entity_effects(unique_id)
                    .iter()
                    .map(effect_json)
                    .collect();
                Ok(json!({ "ok": true, "effects": effects }))
            }
            "entity.add_effect" => {
                let unique_id = params
                    .get("unique_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing unique_id".to_string())?;
                let id = params
                    .get("id")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| "missing effect id".to_string())? as u32;
                let duration = params.get("duration").and_then(Value::as_i64).unwrap_or(600);
                let mut effect = crate::world_state::ActiveEffect::new(
                    id,
                    duration as i32,
                    params.get("amplifier").and_then(Value::as_u64).unwrap_or(0).min(255) as u8,
                );
                effect.visible = params.get("visible").and_then(Value::as_bool).unwrap_or(true);
                effect.ambient = params.get("ambient").and_then(Value::as_bool).unwrap_or(false);
                effect.infinite = params.get("infinite").and_then(Value::as_bool).unwrap_or(false);
                let applied = self.world.add_entity_effect(unique_id, effect);
                let effects: Vec<Value> = self
                    .world
                    .entity_effects(unique_id)
                    .iter()
                    .map(effect_json)
                    .collect();
                Ok(json!({ "ok": applied, "effects": effects }))
            }
            "entity.remove_effect" => {
                let unique_id = params
                    .get("unique_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing unique_id".to_string())?;
                let id = params
                    .get("id")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| "missing effect id".to_string())? as u32;
                let removed = self.world.remove_entity_effect(unique_id, id);
                let effects: Vec<Value> = self
                    .world
                    .entity_effects(unique_id)
                    .iter()
                    .map(effect_json)
                    .collect();
                Ok(json!({ "ok": removed, "effects": effects }))
            }
            "entity.clear_effects" => {
                let unique_id = params
                    .get("unique_id")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| "missing unique_id".to_string())?;
                let removed = self.world.clear_entity_effects(unique_id);
                Ok(json!({ "ok": true, "removed": removed, "effects": [] }))
            }
            // ---- Fase 9 (API de plugins): crafteo ----
            // `CraftingManager` en PHP es un proxy del motor nativo: el índice
            // de recetas (shaped, shapeless, horno y las registradas por
            // plugins) sale de `crafting.recipes`, y `matchRecipe()` pregunta
            // a `crafting.match` para que el resultado sea exactamente el que
            // aplicará `ItemStackRequest`.
            "crafting.recipes" => {
                use crate::crafting::CraftingRecipeKind;
                let manager = crate::crafting::manager();
                let mut recipes = Vec::new();
                let mut index = 0u32;
                for recipe in manager.shaped_recipes() {
                    recipes.push(crafting_recipe_json(
                        index,
                        &CraftingRecipeKind::Shaped(recipe.clone()),
                    ));
                    index += 1;
                }
                for recipe in manager.shapeless_recipes() {
                    recipes.push(crafting_recipe_json(
                        index,
                        &CraftingRecipeKind::Shapeless(recipe.clone()),
                    ));
                    index += 1;
                }
                for recipe in manager.furnace_recipes() {
                    recipes.push(crafting_recipe_json(
                        index,
                        &CraftingRecipeKind::Furnace(recipe.clone()),
                    ));
                    index += 1;
                }
                for kind in crate::crafting::dynamic_recipes() {
                    recipes.push(crafting_recipe_json(index, &kind));
                    index += 1;
                }
                Ok(json!({ "recipes": recipes, "count": recipes.len() }))
            }
            "crafting.match" => {
                let size = params.get("size").and_then(Value::as_u64).unwrap_or(2) as usize;
                if size != crate::crafting::SIZE_SMALL && size != crate::crafting::SIZE_BIG {
                    return Ok(json!({
                        "matched": false,
                        "error": format!("unsupported grid size {size}"),
                    }));
                }
                let mut grid = crate::crafting::CraftingGrid::new(size);
                if let Some(items) = params.get("items").and_then(Value::as_array) {
                    for (slot, item) in items.iter().take(size * size).enumerate() {
                        if let Some(stack) = parse_item_stack(item) {
                            grid.slots[slot] = stack;
                        }
                    }
                }
                Ok(
                    match crate::crafting::manager().craft_from_grid_with_dynamic(&grid) {
                        Some(outcome) => json!({
                            "matched": true,
                            "result": item_json(&outcome.result),
                            "consume": outcome.consume,
                        }),
                        None => json!({ "matched": false }),
                    },
                )
            }
            "crafting.furnace" => {
                let block = params
                    .get("block")
                    .and_then(Value::as_str)
                    .unwrap_or("furnace");
                let Some(item) = params.get("item").and_then(parse_item_stack) else {
                    return Ok(json!({ "matched": false }));
                };
                Ok(
                    match crate::crafting::manager().find_furnace_for(block, &item) {
                        Some(recipe) => json!({
                            "matched": true,
                            "result": item_json(&recipe.output),
                            "ticks": crate::crafting::CraftingManager::furnace_cook_ticks(block),
                        }),
                        None => json!({ "matched": false }),
                    },
                )
            }
            "crafting.brewing" => {
                let input = params.get("input").and_then(parse_item_stack);
                let ingredient = params.get("ingredient").and_then(parse_item_stack);
                Ok(match (input, ingredient) {
                    (Some(input), Some(ingredient)) => {
                        match crate::crafting::manager().brewing_result(&input, &ingredient) {
                            Some(result) => json!({ "matched": true, "result": item_json(&result) }),
                            None => json!({ "matched": false }),
                        }
                    }
                    _ => json!({ "matched": false }),
                })
            }
            "crafting.register_shaped" => {
                use crate::crafting::{RecipeIngredient, ShapedRecipe, CraftingRecipeKind};
                let rows: Vec<String> = params
                    .get("shape")
                    .and_then(Value::as_array)
                    .map(|rows| {
                        rows.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                if rows.is_empty() || rows.len() > 3 {
                    return Ok(json!({ "ok": false, "error": "shaped recipes need 1..3 rows" }));
                }
                let width = rows[0].chars().count();
                if width == 0 || width > 3 || rows.iter().any(|row| row.chars().count() != width) {
                    return Ok(json!({
                        "ok": false,
                        "error": "shaped recipe rows must share a 1..3 column width",
                    }));
                }
                let symbols = params.get("ingredients").and_then(Value::as_object);
                let mut shape: Vec<Vec<Option<RecipeIngredient>>> = Vec::new();
                for row in &rows {
                    let mut cells = Vec::new();
                    for symbol in row.chars() {
                        if symbol == ' ' {
                            cells.push(None);
                            continue;
                        }
                        let Some(ingredient) = symbols
                            .and_then(|map| map.get(&symbol.to_string()))
                            .and_then(parse_ingredient)
                        else {
                            return Ok(json!({
                                "ok": false,
                                "error": format!("missing ingredient for '{symbol}'"),
                            }));
                        };
                        cells.push(Some(ingredient));
                    }
                    shape.push(cells);
                }
                let Some(result) = params
                    .get("results")
                    .and_then(Value::as_array)
                    .and_then(|results| results.first())
                    .and_then(parse_item_stack)
                else {
                    return Ok(json!({ "ok": false, "error": "missing result item" }));
                };
                let id = params
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        format!(
                            "plugin:shaped#{}",
                            crate::crafting::dynamic_recipes().len() + 1
                        )
                    });
                let kind = CraftingRecipeKind::Shaped(ShapedRecipe {
                    id,
                    shape,
                    result: result.clone(),
                    priority: 0,
                });
                crate::crafting::register_dynamic_recipe(kind);
                let index = dynamic_recipe_offset()
                    + crate::crafting::dynamic_recipes().len() as u32
                    - 1;
                Ok(json!({ "ok": true, "index": index }))
            }
            "crafting.register_shapeless" => {
                use crate::crafting::{CraftingRecipeKind, ShapelessRecipe};
                let ingredients: Vec<crate::crafting::RecipeIngredient> = params
                    .get("ingredients")
                    .and_then(Value::as_array)
                    .map(|list| list.iter().filter_map(parse_ingredient).collect())
                    .unwrap_or_default();
                if ingredients.is_empty() || ingredients.len() > 9 {
                    return Ok(json!({ "ok": false, "error": "shapeless recipes need 1..9 ingredients" }));
                }
                let Some(result) = params
                    .get("results")
                    .and_then(Value::as_array)
                    .and_then(|results| results.first())
                    .and_then(parse_item_stack)
                else {
                    return Ok(json!({ "ok": false, "error": "missing result item" }));
                };
                let id = params
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        format!(
                            "plugin:shapeless#{}",
                            crate::crafting::dynamic_recipes().len() + 1
                        )
                    });
                let kind = CraftingRecipeKind::Shapeless(ShapelessRecipe {
                    id,
                    ingredients,
                    result,
                    priority: 0,
                });
                crate::crafting::register_dynamic_recipe(kind);
                let index = dynamic_recipe_offset()
                    + crate::crafting::dynamic_recipes().len() as u32
                    - 1;
                Ok(json!({ "ok": true, "index": index }))
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
            // Bedrock (`PlayerSession::save_player_data`): JSON v2 (27
            // campos) escrito directamente, sin staging virtual. `y` canónico
            // (sin +64 legacy).
            "player.has_data" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                let has_data = self.read_player_data(name).is_some();
                Ok(json!({ "has_data": has_data, "exists": has_data }))
            }
            "player.has_played_before" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                let has_played_before = match self.read_player_data(name) {
                    Some(saved) => {
                        let first = saved.get("first_played").and_then(Value::as_f64).unwrap_or(0.0);
                        let last = saved.get("last_played").and_then(Value::as_f64).unwrap_or(0.0);
                        // Had a file before this session, or timers prove it.
                        first > 0.0 && last > first + 0.001
                    }
                    None => false,
                };
                Ok(json!({ "has_played_before": has_played_before, "has_data": has_played_before, "exists": has_played_before }))
            }
            "player.load" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                match self.read_player_data(name) {
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
                Ok(self.save_player_v2(name, params)?)
            }
            "player.save_data" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                let data = params.get("data").cloned().unwrap_or(Value::Null);
                let mut current = self.read_player_data(name).unwrap_or_else(|| {
                    PlayerContext::new(name, std::net::SocketAddr::from(([0, 0, 0, 0], 0))).to_save_json()
                });
                if let (Some(object), Some(data_object)) = (current.as_object_mut(), data.as_object()) {
                    for (key, value) in data_object {
                        object.insert(key.clone(), value.clone());
                    }
                    object.insert("name".to_string(), json!(name));
                } else if let Some(object) = current.as_object_mut() {
                    object.insert("data".to_string(), data);
                    object.insert("name".to_string(), json!(name));
                }
                // Keep a full v2 object even for a brand-new name.
                if let Some(object) = current.as_object() {
                    if object.len() < 26 {
                        let mut snapshot =
                            PlayerContext::from_save_json(name, &current)
                                .unwrap_or_else(|| PlayerContext::new(name, std::net::SocketAddr::from(([0, 0, 0, 0], 0))));
                        snapshot.apply_save_json(&current);
                        current = snapshot.to_save_json();
                        if let Some(object) = current.as_object_mut() {
                            object.insert("name".to_string(), json!(name));
                        }
                    }
                }
                self.write_player_data(name, &current).map_err(|error| error.to_string())?;
                Ok(json!({ "ok": true }))
            }
            "player.update_position" => {
                // Heartbeat only: updates the registry (canonical Bedrock Y,
                // yaw Bedrock). It never writes disk — quit calls player.save.
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
                // JavaServer::joinApi loads `player.load` BEFORE this call;
                // any persisted field the caller omitted (xuid / timers /
                // vitals) is inherited from disk here so load->register is
                // lossless even if PHP forgot a field. `y` is copied as-is
                // (canonical, no +64).
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing player name".to_string())?;
                let address = params
                    .get("address")
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse::<std::net::SocketAddr>().ok())
                    .unwrap_or_else(|| std::net::SocketAddr::from(([0, 0, 0, 0], 0)));
                let saved = self.read_player_data(name);
                let saved_num = |key: &str| saved.as_ref().and_then(|entry| entry.get(key)).and_then(Value::as_f64);
                let float = |key: &str, default: f64| {
                    params
                        .get(key)
                        .and_then(Value::as_f64)
                        .or_else(|| saved_num(key))
                        .unwrap_or(default)
                };
                let mut player = PlayerContext {
                    name: name.to_string(),
                    display_name: params
                        .get("display_name")
                        .or_else(|| params.get("displayName"))
                        .and_then(Value::as_str)
                        .or_else(|| {
                            saved
                                .as_ref()
                                .and_then(|entry| entry.get("display_name").or_else(|| entry.get("displayName")))
                                .and_then(Value::as_str)
                        })
                        .unwrap_or(name)
                        .to_string(),
                    address,
                    runtime_id: 0,
                    unique_id: 0,
                    ping: 0,
                    gamemode: params
                        .get("gamemode")
                        .and_then(Value::as_i64)
                        .or_else(|| saved.as_ref().and_then(|entry| entry.get("gamemode")).and_then(Value::as_i64))
                        .unwrap_or(0)
                        .max(0) as u32,
                    x: float("x", 0.0),
                    y: float("y", 64.0),
                    z: float("z", 0.0),
                    yaw: float("yaw", 0.0),
                    pitch: float("pitch", 0.0),
                    head_yaw: float("yaw", 0.0),
                    world: params
                        .get("world")
                        .and_then(Value::as_str)
                        .or_else(|| {
                            saved
                                .as_ref()
                                .and_then(|entry| entry.get("world"))
                                .and_then(Value::as_str)
                        })
                        .unwrap_or("world")
                        .to_string(),
                    health: float("health", 20.0) as f32,
                    max_health: params
                        .get("max_health")
                        .or_else(|| params.get("maxHealth"))
                        .and_then(Value::as_f64)
                        .or_else(|| saved_num("max_health").or_else(|| saved_num("maxHealth")))
                        .unwrap_or(20.0) as f32,
                    is_op: params
                        .get("is_op")
                        .or_else(|| params.get("op"))
                        .and_then(Value::as_bool)
                        .or_else(|| {
                            saved
                                .as_ref()
                                .and_then(|entry| entry.get("is_op").or_else(|| entry.get("op")))
                                .and_then(Value::as_bool)
                        })
                        .unwrap_or(false),
                    is_virtual: true,
                    xuid: params
                        .get("xuid")
                        .and_then(Value::as_str)
                        .or_else(|| saved.as_ref().and_then(|entry| entry.get("xuid")).and_then(Value::as_str))
                        .unwrap_or("")
                        .to_string(),
                    first_played: params
                        .get("first_played")
                        .or_else(|| params.get("firstPlayed"))
                        .and_then(Value::as_f64)
                        .or_else(|| saved_num("first_played"))
                        .unwrap_or(0.0),
                    last_played: params
                        .get("last_played")
                        .or_else(|| params.get("lastPlayed"))
                        .and_then(Value::as_f64)
                        .or_else(|| saved_num("last_played"))
                        .unwrap_or(0.0),
                    food: params.get("food").and_then(Value::as_f64).or_else(|| saved_num("food")).unwrap_or(20.0) as f32,
                    saturation: params.get("saturation").and_then(Value::as_f64).or_else(|| saved_num("saturation")).unwrap_or(20.0) as f32,
                    xp: params.get("xp").and_then(Value::as_f64).or_else(|| saved_num("xp")).unwrap_or(0.0) as f32,
                    xp_level: params
                        .get("xp_level")
                        .or_else(|| params.get("xpLevel"))
                        .and_then(Value::as_i64)
                        .or_else(|| saved.as_ref().and_then(|entry| entry.get("xp_level").or_else(|| entry.get("xpLevel"))).and_then(Value::as_i64))
                        .unwrap_or(0) as i32,
                    air: params.get("air").and_then(Value::as_i64).or_else(|| saved.as_ref().and_then(|entry| entry.get("air")).and_then(Value::as_i64)).unwrap_or(300) as i16,
                    is_flying: params
                        .get("flying")
                        .or_else(|| params.get("is_flying"))
                        .and_then(Value::as_bool)
                        .or_else(|| {
                            saved
                                .as_ref()
                                .and_then(|entry| entry.get("flying").or_else(|| entry.get("is_flying")))
                                .and_then(Value::as_bool)
                        })
                        .unwrap_or(false),
                    spawn_point: None,
                    is_sneaking: params.get("sneaking").or_else(|| params.get("is_sneaking")).and_then(Value::as_bool).unwrap_or(false),
                    is_sprinting: params.get("sprinting").or_else(|| params.get("is_sprinting")).and_then(Value::as_bool).unwrap_or(false),
                    is_swimming: params.get("swimming").or_else(|| params.get("is_swimming")).and_then(Value::as_bool).unwrap_or(false),
                    is_gliding: params.get("gliding").or_else(|| params.get("is_gliding")).and_then(Value::as_bool).unwrap_or(false),
                };
                if let Some(saved) = saved.as_ref() {
                    let live = player.clone();
                    player.apply_save_json(saved);
                    // Any field the caller supplied wins over the quit save
                    // (Java may have a newer position/health than disk).
                    if params.get("x").is_some() || params.get("y").is_some() || params.get("z").is_some() {
                        player.x = live.x;
                        player.y = live.y;
                        player.z = live.z;
                    } else {
                        // No live position supplied: spawn at the saved one.
                        player.x = live.x;
                        player.y = live.y;
                        player.z = live.z;
                    }
                    if params.get("yaw").is_some() { player.yaw = live.yaw; }
                    if params.get("pitch").is_some() { player.pitch = live.pitch; }
                    if params.get("gamemode").is_some() { player.gamemode = live.gamemode; }
                    if params.get("health").is_some() { player.health = live.health; }
                    if params.get("max_health").is_some() || params.get("maxHealth").is_some() { player.max_health = live.max_health; }
                    if params.get("food").is_some() { player.food = live.food; }
                    if params.get("saturation").is_some() { player.saturation = live.saturation; }
                    if params.get("xp").is_some() { player.xp = live.xp; }
                    if params.get("xp_level").is_some() || params.get("xpLevel").is_some() { player.xp_level = live.xp_level; }
                    if params.get("air").is_some() { player.air = live.air; }
                    if params.get("xuid").is_some() { player.xuid = live.xuid; }
                    if params.get("first_played").is_some() || params.get("firstPlayed").is_some() { player.first_played = live.first_played; }
                    if params.get("last_played").is_some() || params.get("lastPlayed").is_some() { player.last_played = live.last_played; }
                    if params.get("flying").is_some() || params.get("is_flying").is_some() { player.is_flying = live.is_flying; }
                    if params.get("display_name").is_some() || params.get("displayName").is_some() { player.display_name = live.display_name; }
                    if params.get("world").is_some() { player.world = live.world; }
                    // Persisted-only fields (spawn_point, flags) stay from disk.
                }
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
                // Mundo + datos del registro (Bedrock los guarda SessionManager;
                // aquí se guardan los del registro para que `/save-all` desde
                // Java persista también, virtuals incluidos, directo a disco).
                let _ = self.world.save_to(std::path::Path::new("world.json"));
                let all = self.players.all();
                let now = crate::session::PlayerSession::now_played_secs();
                for player in &all {
                    let existing = self.read_player_data(&player.name);
                    let mut value = player.to_save_json();
                    if let Some(object) = value.as_object_mut() {
                        if let Some(existing) = existing.as_ref() {
                            // Preserve on-disk timers/xuid when live is empty.
                            if player.first_played <= 0.0 {
                                if let Some(saved) = existing.get("first_played") {
                                    if saved.as_f64().unwrap_or(0.0) > 0.0 {
                                        object.insert("first_played".to_string(), saved.clone());
                                    }
                                }
                            }
                            if player.xuid.is_empty() {
                                if let Some(saved) = existing.get("xuid") {
                                    if saved.as_str().unwrap_or("").len() > 0 {
                                        object.insert("xuid".to_string(), saved.clone());
                                    }
                                }
                            }
                        } else if player.first_played <= 0.0 {
                            object.insert("first_played".to_string(), json!(now));
                        }
                        object.insert("last_played".to_string(), json!(now));
                    }
                    let _ = self.write_player_data(&player.name, &value);
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
    fn world_set_spawn_moves_the_spawn_and_queues_spawn_change() {
        let generator: Arc<dyn bedrock_world::WorldGenerator> =
            Arc::new(bedrock_world::NormalGenerator::new(42));
        let world = Arc::new(WorldState::new("world", generator));
        let host = ServerPluginHost::new(
            Arc::clone(&world),
            Arc::new(PlayerRegistry::new()),
            Arc::new(RwLock::new(HashSet::new())),
            "test motd",
            100,
        );
        let previous = world.spawn();
        let spawn = host
            .call(
                "world.set_spawn",
                &json!({ "x": 10.5, "y": 70.0, "z": -3.2 }),
            )
            .expect("world.set_spawn");
        assert_eq!(spawn["x"], json!(11));
        assert_eq!(spawn["y"], json!(70));
        assert_eq!(spawn["z"], json!(-3));
        assert_eq!(world.spawn(), (11, 70, -3));
        assert_eq!(
            world.take_world_events(),
            vec![crate::world_state::PendingWorldEvent::SpawnChanged {
                previous
            }]
        );
    }

    #[test]
    fn world_collision_and_update_host_calls() {
        let generator: Arc<dyn bedrock_world::WorldGenerator> =
            Arc::new(bedrock_world::NormalGenerator::new(42));
        let world = Arc::new(WorldState::new("world", generator));
        let stone = bedrock_world::block_states::network_id("stone").unwrap();
        world.set_block(2, 5, 2, stone);
        let host = ServerPluginHost::new(
            Arc::clone(&world),
            Arc::new(PlayerRegistry::new()),
            Arc::new(RwLock::new(HashSet::new())),
            "test motd",
            100,
        );
        let bb = json!({
            "min_x": 1.5, "min_y": 4.5, "min_z": 1.5,
            "max_x": 2.5, "max_y": 5.5, "max_z": 2.5
        });

        let blocks = host
            .call("world.collision_blocks", &bb)
            .expect("world.collision_blocks");
        assert!(blocks["blocks"].as_array().expect("blocks").iter().any(
            |block| block["x"] == 2
                && block["y"] == 5
                && block["z"] == 2
                && block["state_id"] == stone
        ));

        let boxes = host
            .call("world.collision_boxes", &bb)
            .expect("world.collision_boxes");
        let boxes = boxes["boxes"].as_array().expect("boxes");
        assert!(boxes
            .iter()
            .any(|b| b["min_x"] == 2.0 && b["max_y"] == 6.0));

        // `scheduleDelayedBlockUpdate()` keeps the block id at queue time and
        // restores it after the delay (here the block is removed meanwhile).
        host.call(
            "world.schedule_block_update",
            &json!({ "x": 2, "y": 5, "z": 2, "delay": 1 }),
        )
        .expect("world.schedule_block_update");
        world.set_block(2, 5, 2, bedrock_world::block_id::AIR);
        world.tick();
        assert_eq!(world.block_at(2, 5, 2), stone);

        host.call(
            "world.notify_neighbour",
            &json!({ "x": 2, "y": 5, "z": 2 }),
        )
        .expect("world.notify_neighbour");
    }

    #[test]
    fn world_explosion_host_calls_queue_the_blast() {
        let generator: Arc<dyn bedrock_world::WorldGenerator> =
            Arc::new(bedrock_world::NormalGenerator::new(42));
        let world = Arc::new(WorldState::new("world", generator));
        let stone = bedrock_world::block_states::network_id("stone").unwrap();
        world.set_block(3, 5, 3, stone);
        let host = ServerPluginHost::new(
            Arc::clone(&world),
            Arc::new(PlayerRegistry::new()),
            Arc::new(RwLock::new(HashSet::new())),
            "test motd",
            100,
        );

        // `Explosion::explodeA()` previews the blocks without touching them.
        let preview = host
            .call(
                "world.explosion_blocks",
                &json!({ "x": 3.0, "y": 5.0, "z": 3.0, "radius": 2.0 }),
            )
            .expect("world.explosion_blocks");
        let blocks = preview["blocks"].as_array().expect("blocks").clone();
        assert!(blocks.iter().any(|block| block["x"] == 3
            && block["y"] == 5
            && block["z"] == 3
            && block["state_id"] == stone));
        assert_eq!(world.block_at(3, 5, 3), stone);

        // `Explosion::explodeB()` queues that list for the tick.
        let queued = host
            .call(
                "world.explode",
                &json!({
                    "x": 3.0,
                    "y": 5.0,
                    "z": 3.0,
                    "radius": 2.0,
                    "blocks": blocks
                }),
            )
            .expect("world.explode");
        assert_eq!(queued["count"], blocks.len());
        let explosions = world.take_explosions();
        assert_eq!(explosions.len(), 1);
        assert_eq!(explosions[0].radius, 2.0);
        assert_eq!(explosions[0].entity, None);
        assert_eq!(explosions[0].blocks.len(), blocks.len());

        // PM rejects non-positive radii.
        assert!(host
            .call("world.explode", &json!({ "x": 0, "y": 0, "z": 0, "radius": 0 }))
            .is_err());
    }

    #[test]
    fn world_sound_particle_and_biome_host_calls_queue_values() {
        let generator: Arc<dyn bedrock_world::WorldGenerator> =
            Arc::new(bedrock_world::NormalGenerator::new(42));
        let world = Arc::new(WorldState::new("world", generator));
        let host = ServerPluginHost::new(
            Arc::clone(&world),
            Arc::new(PlayerRegistry::new()),
            Arc::new(RwLock::new(HashSet::new())),
            "test motd",
            100,
        );

        // `World::addSound(new BlockPlaceSound($block))` with explicit players:
        // the effect waits for `WorldSoundEvent` on the tick.
        let sound = host
            .call(
                "world.add_sound",
                &json!({
                    "sound": "place",
                    "extra_data": 42,
                    "x": 1.5,
                    "y": 64.0,
                    "z": -2.5,
                    "players": ["Steve"]
                }),
            )
            .expect("world.add_sound");
        assert_eq!(sound["kind"], json!("level_sound"));
        let effects = world.take_world_effects();
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            &effects[0],
            crate::world_state::PendingWorldEffect::Sound {
                sound,
                extra_data,
                recipients,
                ..
            } if sound.as_deref() == Some("place")
                && *extra_data == 42
                && recipients == &vec!["steve".to_string()]
        ));

        // `World::addSound(new ClickSound(0.5))`: LevelEvent, pitch*1000.
        let click = host
            .call(
                "world.add_sound",
                &json!({ "event_id": 1000, "extra_data": 500, "x": 0.0, "y": 1.0, "z": 2.0 }),
            )
            .expect("world.add_sound (event_id)");
        assert_eq!(click["kind"], json!("level_event"));
        assert!(matches!(
            &world.take_world_effects()[..],
            [crate::world_state::PendingWorldEffect::Sound { event_id: Some(1000), .. }]
        ));

        // `World::addParticle(new FlameParticle())` (0x4000 | 8).
        host.call(
            "world.add_particle",
            &json!({ "event_id": 16392, "event_data": 0, "x": 3.25, "y": 65.5, "z": 4.0 }),
        )
        .expect("world.add_particle");
        assert!(matches!(
            &world.take_world_effects()[..],
            [crate::world_state::PendingWorldEffect::Particle { event_id: 16392, x, .. }]
                if *x == 3.25
        ));
        // Las colas del core no se mezclan con los efectos de PHP.
        assert!(world.take_pending_sounds_at().is_empty());
        assert!(world.take_pending_level_events_at().is_empty());

        // `World::getBiomeId()` / `setBiomeId()`.
        let before = host
            .call("world.get_biome", &json!({ "x": 0, "y": 64, "z": 0 }))
            .expect("world.get_biome");
        let changed = host
            .call(
                "world.set_biome",
                &json!({ "x": 0, "y": 64, "z": 0, "biome": 2 }),
            )
            .expect("world.set_biome");
        assert_eq!(changed["changed"], json!(true));
        let after = host
            .call("world.get_biome", &json!({ "x": 0, "y": 64, "z": 0 }))
            .expect("world.get_biome");
        assert_eq!(after["biome"], json!(2));
        assert_ne!(before["biome"], json!(2));
    }

    #[test]
    fn entity_effect_host_calls_roundtrip() {
        let (host, _) = test_host();
        let spawned = host
            .call(
                "entity.spawn",
                &json!({ "type": "minecraft:zombie", "x": 0, "y": 64, "z": 0 }),
            )
            .expect("entity.spawn");
        let id = spawned["entity"]["unique_id"].as_i64().expect("unique id");

        assert_eq!(
            host.call("entity.effects", &json!({ "unique_id": id }))
                .expect("entity.effects")["effects"],
            json!([])
        );

        let added = host
            .call(
                "entity.add_effect",
                &json!({ "unique_id": id, "id": 1, "duration": 200, "amplifier": 1 }),
            )
            .expect("entity.add_effect");
        assert_eq!(added["ok"], json!(true));
        assert_eq!(added["effects"][0]["id"], json!(1));
        assert_eq!(added["effects"][0]["amplifier"], json!(1));
        assert_eq!(added["effects"][0]["duration"], json!(200));

        let removed = host
            .call("entity.remove_effect", &json!({ "unique_id": id, "id": 1 }))
            .expect("entity.remove_effect");
        assert_eq!(removed["ok"], json!(true));
        assert_eq!(removed["effects"], json!([]));

        host.call(
            "entity.add_effect",
            &json!({ "unique_id": id, "id": 20, "duration": 100 }),
        )
        .expect("entity.add_effect");
        let cleared = host
            .call("entity.clear_effects", &json!({ "unique_id": id }))
            .expect("entity.clear_effects");
        assert_eq!(cleared["removed"], json!(1));
        assert_eq!(
            host.call("entity.effects", &json!({ "unique_id": 999 }))
                .expect("unknown entity")["ok"],
            json!(false)
        );
    }

    #[test]
    fn object_and_projectile_spawn_calls_round_trip() {
        let (host, _players) = test_host();
        let planks = crate::inventory::item_network_id("minecraft:oak_planks").expect("planks");

        let item = host
            .call(
                "entity.spawn_item",
                &json!({
                    "x": 0.5, "y": 64.0, "z": 0.5,
                    "item": { "id": planks, "meta": 0, "count": 3 },
                }),
            )
            .expect("entity.spawn_item");
        assert_eq!(item["ok"], json!(true));
        assert_eq!(item["entity"]["type"], json!("minecraft:item"));

        let falling = host
            .call(
                "entity.spawn_falling_block",
                &json!({ "x": 1, "y": 70, "z": 2, "block_id": 1 }),
            )
            .expect("entity.spawn_falling_block");
        assert_eq!(falling["ok"], json!(true));
        assert_eq!(falling["entity"]["type"], json!("minecraft:falling_block"));

        let tnt = host
            .call("entity.spawn_tnt", &json!({ "x": 0.0, "y": 64.0, "z": 0.0 }))
            .expect("entity.spawn_tnt");
        assert_eq!(tnt["ok"], json!(true));
        assert_eq!(tnt["entity"]["type"], json!("minecraft:tnt"));

        let orb = host
            .call(
                "entity.spawn_xp_orb",
                &json!({ "x": 0.0, "y": 64.0, "z": 0.0, "value": 5 }),
            )
            .expect("entity.spawn_xp_orb");
        assert_eq!(orb["ok"], json!(true));
        assert_eq!(orb["entity"]["type"], json!("minecraft:xp_orb"));
        // `World::dropExperience()` ignores a non-positive amount.
        assert_eq!(
            host.call("entity.spawn_xp_orb", &json!({ "value": 0 }))
                .expect("empty orb")["ok"],
            json!(false)
        );

        let arrow = host
            .call(
                "entity.spawn_projectile",
                &json!({
                    "type": "minecraft:arrow",
                    "x": 0.0, "y": 65.0, "z": 0.0,
                    "motion_x": 0.0, "motion_y": 0.0, "motion_z": 3.0,
                    "owner": "Steve", "owner_runtime_id": 9,
                    "damage": 2.0, "critical": true,
                }),
            )
            .expect("entity.spawn_projectile");
        assert_eq!(arrow["ok"], json!(true));
        assert_eq!(arrow["entity"]["type"], json!("minecraft:arrow"));
        assert!(host
            .call("entity.spawn_projectile", &json!({ "type": "minecraft:cow" }))
            .is_err());
    }

    #[test]
    fn crafting_host_calls_expose_and_match_recipes() {
        let (host, _) = test_host();
        let oak_log = crate::inventory::item_network_id("minecraft:oak_log").expect("oak_log");
        let planks = crate::inventory::item_network_id("minecraft:oak_planks").expect("planks");

        let recipes = host.call("crafting.recipes", &json!({})).expect("crafting.recipes");
        assert!(recipes["count"].as_u64().unwrap() > 100, "recipe index looks empty");
        assert_eq!(recipes["recipes"][0]["kind"], json!("shaped"));

        let matched = host
            .call(
                "crafting.match",
                &json!({ "size": 2, "items": [{ "id": oak_log, "meta": 0, "count": 1 }] }),
            )
            .expect("crafting.match");
        assert_eq!(matched["matched"], json!(true));
        assert_eq!(matched["result"]["id"], json!(planks));
        assert_eq!(matched["result"]["count"], json!(4));

        let empty = host
            .call("crafting.match", &json!({ "size": 2, "items": [] }))
            .expect("empty match");
        assert_eq!(empty["matched"], json!(false));

        let unsupported = host
            .call("crafting.match", &json!({ "size": 5, "items": [] }))
            .expect("bad size");
        assert_eq!(unsupported["matched"], json!(false));

        // A plugin recipe overrides the vanilla 2x2 planks->table recipe.
        crate::crafting::clear_dynamic_recipes();
        let registered = host
            .call(
                "crafting.register_shaped",
                &json!({
                    "shape": ["GG", "GG"],
                    "ingredients": { "G": { "type": "exact", "id": planks, "meta": 0 } },
                    "results": [{ "id": 1, "meta": 0, "count": 2 }],
                }),
            )
            .expect("crafting.register_shaped");
        assert_eq!(registered["ok"], json!(true));

        let crafted = host
            .call(
                "crafting.match",
                &json!({
                    "size": 2,
                    "items": [
                        { "id": planks, "meta": 0, "count": 1 },
                        { "id": planks, "meta": 0, "count": 1 },
                        { "id": planks, "meta": 0, "count": 1 },
                        { "id": planks, "meta": 0, "count": 1 },
                    ],
                }),
            )
            .expect("dynamic match");
        assert_eq!(crafted["matched"], json!(true));
        assert_eq!(crafted["result"]["count"], json!(2));
        crate::crafting::clear_dynamic_recipes();
    }

    #[test]
    fn inventory_reads_published_snapshots_and_queues_edits() {
        let (host, players) = test_host();
        // Nothing published yet: an empty window list, not an error.
        let empty = host
            .call("inventory.get", &json!({ "player": "Steve" }))
            .expect("inventory.get");
        assert_eq!(empty["ok"], json!(true));
        assert_eq!(empty["windows"], json!([]));

        // What the tick publishes is what PHP reads.
        players.publish_inventory(
            "Steve",
            json!([{
                "window": INV_WINDOW_INVENTORY,
                "slots": [item_json(&bedrock_protocol::ItemStack::new(1, 5))],
            }]),
        );
        let snapshot = host
            .call("inventory.get", &json!({ "player": "steve" }))
            .expect("inventory.get");
        assert_eq!(snapshot["windows"][0]["window"], json!(INV_WINDOW_INVENTORY));
        assert_eq!(snapshot["windows"][0]["slots"][0]["id"], json!(1));
        assert_eq!(snapshot["windows"][0]["slots"][0]["count"], json!(5));

        // Window and slot are validated before anything is queued.
        let out_of_range = host
            .call(
                "inventory.set_slot",
                &json!({ "player": "Steve", "window": 0, "slot": 36, "item": { "id": 1, "count": 1 } }),
            )
            .expect("inventory.set_slot");
        assert_eq!(out_of_range["ok"], json!(false));
        let bad_window = host
            .call(
                "inventory.set_slot",
                &json!({ "player": "Steve", "window": 7, "slot": 0, "item": null }),
            )
            .expect("inventory.set_slot");
        assert_eq!(bad_window["ok"], json!(false));
        assert!(players.take_inventory_edits().is_empty());

        let queued = host
            .call(
                "inventory.set_slot",
                &json!({ "player": "Steve", "window": 0, "slot": 3, "item": { "id": 392, "meta": 0, "count": 2 } }),
            )
            .expect("inventory.set_slot");
        assert_eq!(queued["ok"], json!(true));
        let edits = players.take_inventory_edits();
        match edits.as_slice() {
            [InventoryEdit::SetSlot { player, window, slot, item }] => {
                assert_eq!(player, "steve");
                assert_eq!((*window, *slot), (0, 3));
                assert_eq!((item.network_id, item.count), (392, 2));
            }
            other => panic!("unexpected edits: {other:?}"),
        }

        host.call("inventory.clear", &json!({ "player": "Steve" }))
            .expect("inventory.clear");
        let edits = players.take_inventory_edits();
        assert!(matches!(edits.as_slice(), [InventoryEdit::Clear { player }] if player == "steve"));
    }

    #[test]
    fn entity_spawn_get_teleport_damage_and_despawn() {
        let (host, _players) = test_host();
        let spawned = host
            .call(
                "entity.spawn",
                &json!({
                    "type": "zombie",
                    "x": 1.5, "y": 64.0, "z": -2.5,
                    "yaw": 90.0, "pitch": 0.0,
                    "name_tag": "Guard",
                }),
            )
            .expect("entity.spawn");
        assert_eq!(spawned["ok"], json!(true));
        let entity = &spawned["entity"];
        assert_eq!(entity["type"], json!("minecraft:zombie"));
        assert_eq!(entity["name_tag"], json!("Guard"));
        assert_eq!(entity["health"], json!(20.0));
        let unique_id = entity["unique_id"].as_i64().expect("unique id");
        let runtime_id = entity["runtime_id"].as_u64().expect("runtime id");

        // `entity.get` speaks both PocketMine and Bedrock ids.
        let by_id = host
            .call("entity.get", &json!({ "unique_id": unique_id }))
            .expect("entity.get");
        assert_eq!(by_id["entity"]["unique_id"], json!(unique_id));
        let by_runtime = host
            .call("entity.get", &json!({ "runtime_id": runtime_id }))
            .expect("entity.get runtime");
        assert_eq!(by_runtime["entity"]["unique_id"], json!(unique_id));

        assert_eq!(
            host.call("entity.list", &json!({})).expect("entity.list")["count"],
            json!(1)
        );

        // The mutators act on the live world entity, not on a copy.
        let moved = host
            .call(
                "entity.teleport",
                &json!({ "unique_id": unique_id, "x": 10.5, "y": 70.0, "z": 3.5, "yaw": 180.0 }),
            )
            .expect("entity.teleport");
        assert_eq!(moved["ok"], json!(true));
        assert_eq!(moved["entity"]["x"], json!(10.5));
        assert_eq!(moved["entity"]["yaw"], json!(180.0));

        let healed = host
            .call(
                "entity.set_health",
                &json!({ "unique_id": unique_id, "health": 7.0 }),
            )
            .expect("entity.set_health");
        assert_eq!(healed["ok"], json!(true));
        assert_eq!(healed["entity"]["health"], json!(7.0));

        let renamed = host
            .call(
                "entity.set_name_tag",
                &json!({ "unique_id": unique_id, "name_tag": "Boss" }),
            )
            .expect("entity.set_name_tag");
        assert_eq!(renamed["entity"]["name_tag"], json!("Boss"));

        // `Living::attack()`: the first hit lands, the second is swallowed by
        // the invulnerability window, like in PocketMine.
        let hit = host
            .call(
                "entity.damage",
                &json!({ "unique_id": unique_id, "damage": 5.0 }),
            )
            .expect("entity.damage");
        assert_eq!(hit["ok"], json!(true));
        assert_eq!(hit["killed"], json!(false));
        assert_eq!(host.call("entity.damage", &json!({ "unique_id": unique_id, "damage": 5.0 })).expect("second hit")["ok"], json!(false));

        let despawned = host
            .call("entity.despawn", &json!({ "unique_id": unique_id }))
            .expect("entity.despawn");
        assert_eq!(despawned["ok"], json!(true));
        assert_eq!(
            host.call("entity.list", &json!({})).expect("entity.list")["count"],
            json!(0)
        );
        // Despawning twice reports false instead of erroring.
        assert_eq!(
            host.call("entity.despawn", &json!({ "unique_id": unique_id }))
                .expect("entity.despawn twice")["ok"],
            json!(false)
        );
    }

    #[test]
    fn entity_motion_knock_back_and_size_round_trip() {
        let (host, _players) = test_host();
        let spawned = host
            .call(
                "entity.spawn",
                &json!({ "type": "minecraft:zombie", "x": 0.5, "y": 64.0, "z": 0.5 }),
            )
            .expect("entity.spawn");
        let entity = &spawned["entity"];
        // `Entity::getInitialSizeInfo()` of `Zombie`. f32 rounds, so compare
        // with a tolerance instead of exact JSON numbers.
        let close = |value: &Value, expected: f64| {
            value
                .as_f64()
                .is_some_and(|actual| (actual - expected).abs() < 0.001)
        };
        assert!(close(&entity["height"], 1.9));
        assert!(close(&entity["width"], 0.6));
        assert!(close(&entity["eye_height"], 1.05));
        let unique_id = entity["unique_id"].as_i64().expect("unique id");

        let moved = host
            .call(
                "entity.set_motion",
                &json!({ "unique_id": unique_id, "x": 0.1, "y": 0.4, "z": -0.2 }),
            )
            .expect("entity.set_motion");
        assert_eq!(moved["ok"], json!(true));
        let motion = |value: &Value, expected: f64| {
            value
                .as_f64()
                .is_some_and(|actual| (actual - expected).abs() < 0.001)
        };
        assert!(motion(&moved["entity"]["motion_y"], 0.4));
        assert!(motion(&moved["entity"]["motion_z"], -0.2));

        // `Living::knockBack()` halves the current motion and adds the push.
        let knocked = host
            .call(
                "entity.knock_back",
                &json!({ "unique_id": unique_id, "dx": 1.0, "dz": 0.0, "force": 0.4 }),
            )
            .expect("entity.knock_back");
        assert_eq!(knocked["ok"], json!(true));
        assert!(knocked["entity"]["motion_x"].as_f64().is_some_and(|v| v > 0.0));
        // Dead / unknown ids are reported, not panicked on.
        assert_eq!(
            host.call("entity.knock_back", &json!({ "unique_id": 999 }))
                .expect("knock_back missing")["ok"],
            json!(false)
        );
        assert_eq!(
            host.call("entity.set_motion", &json!({ "unique_id": 999, "x": 1.0 }))
                .expect("set_motion missing")["ok"],
            json!(false)
        );
    }

    #[test]
    fn entity_spawn_rejects_unknown_and_non_mob_types() {
        let (host, _players) = test_host();
        let rejected = host
            .call(
                "entity.spawn",
                &json!({ "type": "minecraft:not_a_mob", "x": 0.0, "y": 64.0, "z": 0.0 }),
            )
            .expect("entity.spawn");
        assert_eq!(rejected["ok"], json!(false));
        assert!(rejected["error"]
            .as_str()
            .is_some_and(|error| error.contains("not_a_mob")));
        // `minecraft:item` is a real type but not a mob: no silent spawn.
        assert_eq!(
            host.call(
                "entity.spawn",
                &json!({ "type": "minecraft:item", "x": 0.0, "y": 64.0, "z": 0.0 })
            )
            .expect("entity.spawn item")["ok"],
            json!(false)
        );
        assert_eq!(
            host.call("entity.list", &json!({})).expect("entity.list")["count"],
            json!(0)
        );
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

#[cfg(test)]
mod block_state_tests {
    use super::block_state_json;

    /// `block.state` is what the PHP `BlockStateSerializer` falls back to, so it
    /// must report the palette name and every property.
    #[test]
    fn block_state_json_reports_name_and_properties() {
        let stone = bedrock_world::block_states::network_id("minecraft:stone").expect("stone state");
        let state = block_state_json(stone).expect("stone json");
        assert_eq!(state["name"], "minecraft:stone");

        // Stone has no properties in the current palette (variants are their
        // own blocks), so use the structure block for the property round-trip.
        let structure = bedrock_world::block_states::network_id("minecraft:structure_block")
            .expect("structure block state");
        let data = bedrock_world::block_states::with_property(structure, "structure_block_type", "data")
            .expect("structure block data state");
        let state = block_state_json(data).expect("structure json");
        assert_eq!(state["name"], "minecraft:structure_block");
        assert_eq!(state["properties"]["structure_block_type"], "data");

        assert!(block_state_json(0xdead_beef).is_none());
    }
}
