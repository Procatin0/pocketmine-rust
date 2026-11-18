use serde_json::{json, Value};

use crate::context::PlayerContext;
use crate::event::Event;

fn apply_player(field: &mut PlayerContext, data: &Value) {
    if let Some(value) = data.get("player") {
        if let Some(player) = PlayerContext::from_json(value) {
            *field = player;
        }
    }
}

fn apply_string(field: &mut String, data: &Value, key: &str) {
    if let Some(value) = data.get(key).and_then(Value::as_str) {
        *field = value.to_string();
    }
}

fn apply_i32(field: &mut i32, data: &Value, key: &str) {
    if let Some(value) = data.get(key) {
        if let Some(number) = value.as_i64() {
            *field = number as i32;
        } else if let Some(number) = value.as_u64() {
            *field = number as i32;
        }
    }
}

fn apply_u32(field: &mut u32, data: &Value, key: &str) {
    if let Some(value) = data.get(key) {
        if let Some(number) = value.as_u64() {
            *field = number as u32;
        } else if let Some(number) = value.as_i64() {
            *field = number.max(0) as u32;
        }
    }
}

fn apply_cancelled(field: &mut bool, data: &Value) {
    if let Some(value) = data.get("cancelled").and_then(Value::as_bool) {
        *field = value;
    }
}

fn apply_u8(field: &mut u8, data: &Value, key: &str) {
    if let Some(value) = data.get(key) {
        if let Some(number) = value.as_i64() {
            *field = number.max(0) as u8;
        } else if let Some(number) = value.as_u64() {
            *field = number as u8;
        }
    }
}

/// Fired when a block break damages an anvil (PocketMine `Anvil.php`
/// `onBreak` damage roll). Cancellable: cancelling keeps the anvil intact.
pub struct AnvilDamageEvent {
    pub player: PlayerContext,
    pub world: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub old_damage: u8,
    pub new_damage: u8,
    pub broken: bool,
    pub cancelled: bool,
}

impl AnvilDamageEvent {
    pub fn new(
        player: PlayerContext,
        world: impl Into<String>,
        x: i32,
        y: i32,
        z: i32,
        old_damage: u8,
        new_damage: u8,
        broken: bool,
    ) -> Self {
        Self {
            player,
            world: world.into(),
            x,
            y,
            z,
            old_damage,
            new_damage,
            broken,
            cancelled: false,
        }
    }
}

impl Event for AnvilDamageEvent {
    fn name(&self) -> &str {
        "AnvilDamageEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "world": self.world,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "old_damage": self.old_damage,
            "new_damage": self.new_damage,
            "broken": self.broken,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);
        apply_string(&mut self.world, value, "world");
        apply_i32(&mut self.x, value, "x");
        apply_i32(&mut self.y, value, "y");
        apply_i32(&mut self.z, value, "z");
        apply_u8(&mut self.old_damage, value, "old_damage");
        apply_u8(&mut self.new_damage, value, "new_damage");
        if let Some(value) = value.get("broken").and_then(Value::as_bool) {
            self.broken = value;
        }
        apply_cancelled(&mut self.cancelled, value);
    }
}

/// Fired when a player breaks a block. Cancellable.
pub struct BlockBreakEvent {
    pub player: PlayerContext,
    pub world: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: u32,
    pub cancelled: bool,
}

impl BlockBreakEvent {
    pub fn new(
        player: PlayerContext,
        world: impl Into<String>,
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
    ) -> Self {
        Self {
            player,
            world: world.into(),
            x,
            y,
            z,
            block_id,
            cancelled: false,
        }
    }
}

impl Event for BlockBreakEvent {
    fn name(&self) -> &str {
        "BlockBreakEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "world": self.world,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "block_id": self.block_id,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);
        apply_string(&mut self.world, value, "world");
        apply_i32(&mut self.x, value, "x");
        apply_i32(&mut self.y, value, "y");
        apply_i32(&mut self.z, value, "z");
        apply_u32(&mut self.block_id, value, "block_id");
        apply_cancelled(&mut self.cancelled, value);
    }
}

/// Fired when a player places a block. Cancellable.
pub struct BlockPlaceEvent {
    pub player: PlayerContext,
    pub world: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: u32,
    pub cancelled: bool,
}

impl BlockPlaceEvent {
    pub fn new(
        player: PlayerContext,
        world: impl Into<String>,
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
    ) -> Self {
        Self {
            player,
            world: world.into(),
            x,
            y,
            z,
            block_id,
            cancelled: false,
        }
    }
}

impl Event for BlockPlaceEvent {
    fn name(&self) -> &str {
        "BlockPlaceEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "world": self.world,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "block_id": self.block_id,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);
        apply_string(&mut self.world, value, "world");
        apply_i32(&mut self.x, value, "x");
        apply_i32(&mut self.y, value, "y");
        apply_i32(&mut self.z, value, "z");
        apply_u32(&mut self.block_id, value, "block_id");
        apply_cancelled(&mut self.cancelled, value);
    }
}

/// `BlockSpreadEvent`: a block spread into another cell (water/lava flowing
/// into a free cell, grass spreading to dirt). Cancellable.
///
/// Divergencia documentada: el puente no tiene `Player` ficticio ni objetos
/// `Block` vivos en Rust, así que el evento lleva coordenadas e ids de estado
/// en vez de `BlockEvent(Player|Block)`; los getters PHP reconstruyen los
/// `Block` con `Block::fromStateId()`.
pub struct BlockSpreadEvent {
    pub world: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Block being replaced (`getBlockId()`).
    pub block_id: u32,
    /// Origin of the spread (`getSource()`).
    pub source_id: u32,
    /// Replacement state (`getNewState()`).
    pub new_id: u32,
    pub cancelled: bool,
}

impl BlockSpreadEvent {
    pub fn new(
        world: impl Into<String>,
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
        source_id: u32,
        new_id: u32,
    ) -> Self {
        Self {
            world: world.into(),
            x,
            y,
            z,
            block_id,
            source_id,
            new_id,
            cancelled: false,
        }
    }
}

impl Event for BlockSpreadEvent {
    fn name(&self) -> &str {
        "BlockSpreadEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> Value {
        json!({
            "world": self.world,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "block_id": self.block_id,
            "source_id": self.source_id,
            "new_id": self.new_id,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_string(&mut self.world, value, "world");
        apply_i32(&mut self.x, value, "x");
        apply_i32(&mut self.y, value, "y");
        apply_i32(&mut self.z, value, "z");
        apply_u32(&mut self.block_id, value, "block_id");
        apply_u32(&mut self.source_id, value, "source_id");
        apply_u32(&mut self.new_id, value, "new_id");
        apply_cancelled(&mut self.cancelled, value);
    }
}

/// `BlockFormEvent`: a new block formed, e.g. lava hardening into cobblestone or
/// obsidian when it touches water. Cancellable.
pub struct BlockFormEvent {
    pub world: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Block being replaced (`getBlockId()`).
    pub block_id: u32,
    /// Block that caused the change (`getCausingBlock()`).
    pub causing_id: u32,
    /// Replacement state (`getNewState()`).
    pub new_id: u32,
    pub cancelled: bool,
}

impl BlockFormEvent {
    pub fn new(
        world: impl Into<String>,
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
        causing_id: u32,
        new_id: u32,
    ) -> Self {
        Self {
            world: world.into(),
            x,
            y,
            z,
            block_id,
            causing_id,
            new_id,
            cancelled: false,
        }
    }
}

impl Event for BlockFormEvent {
    fn name(&self) -> &str {
        "BlockFormEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> Value {
        json!({
            "world": self.world,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "block_id": self.block_id,
            "causing_id": self.causing_id,
            "new_id": self.new_id,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_string(&mut self.world, value, "world");
        apply_i32(&mut self.x, value, "x");
        apply_i32(&mut self.y, value, "y");
        apply_i32(&mut self.z, value, "z");
        apply_u32(&mut self.block_id, value, "block_id");
        apply_u32(&mut self.causing_id, value, "causing_id");
        apply_u32(&mut self.new_id, value, "new_id");
        apply_cancelled(&mut self.cancelled, value);
    }
}

/// Fired when a player interacts with a block. Cancellable.
pub struct PlayerInteractEvent {
    pub player: PlayerContext,
    pub world: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// PocketMine action constant (`ACTION_LEFT_CLICK` = 0,
    /// `ACTION_RIGHT_CLICK` = 1).
    pub action: i32,
    pub cancelled: bool,
}

impl PlayerInteractEvent {
    pub const ACTION_LEFT_CLICK: i32 = 0;
    pub const ACTION_RIGHT_CLICK: i32 = 1;

    pub fn new(
        player: PlayerContext,
        world: impl Into<String>,
        x: i32,
        y: i32,
        z: i32,
        action: i32,
    ) -> Self {
        Self {
            player,
            world: world.into(),
            x,
            y,
            z,
            action,
            cancelled: false,
        }
    }
}

impl Event for PlayerInteractEvent {
    fn name(&self) -> &str {
        "PlayerInteractEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> Value {
        json!({
            "player": self.player,
            "world": self.world,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "action": self.action,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_player(&mut self.player, value);
        apply_string(&mut self.world, value, "world");
        apply_i32(&mut self.x, value, "x");
        apply_i32(&mut self.y, value, "y");
        apply_i32(&mut self.z, value, "z");

        if let Some(action) = value.get("action") {
            if let Some(number) = action.as_i64() {
                self.action = number as i32;
            } else if let Some(name) = action.as_str() {
                self.action = match name.to_ascii_lowercase().as_str() {
                    "left" | "left_click" => Self::ACTION_LEFT_CLICK,
                    "right" | "right_click" => Self::ACTION_RIGHT_CLICK,
                    other => other.parse().unwrap_or(self.action),
                };
            }
        }

        apply_cancelled(&mut self.cancelled, value);
    }
}
