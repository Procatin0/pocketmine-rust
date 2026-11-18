//! Entity events of `pocketmine\event\entity\*`, ported from PM5.
//!
//! The entity snapshot is the same JSON shape `plugin_host::entity_json()`
//! produces (`Entity::applyNativeSnapshot()` reads it in PHP), so the event
//! carries `toArray()` back unchanged unless a plugin edits it.

use serde_json::{json, Value};

use crate::event::Event;

/// `EntityDamageEvent::CAUSE_CONTACT`.
pub const CAUSE_CONTACT: i32 = 0;
/// `EntityDamageEvent::CAUSE_ENTITY_ATTACK`.
pub const CAUSE_ENTITY_ATTACK: i32 = 1;
/// `EntityDamageEvent::CAUSE_PROJECTILE`.
pub const CAUSE_PROJECTILE: i32 = 2;
/// `EntityDamageEvent::CAUSE_SUFFOCATION`.
pub const CAUSE_SUFFOCATION: i32 = 3;
/// `EntityDamageEvent::CAUSE_FALL`.
pub const CAUSE_FALL: i32 = 4;
/// `EntityDamageEvent::CAUSE_FIRE`.
pub const CAUSE_FIRE: i32 = 5;
/// `EntityDamageEvent::CAUSE_FIRE_TICK`.
pub const CAUSE_FIRE_TICK: i32 = 6;
/// `EntityDamageEvent::CAUSE_LAVA`.
pub const CAUSE_LAVA: i32 = 7;
/// `EntityDamageEvent::CAUSE_DROWNING`.
pub const CAUSE_DROWNING: i32 = 8;
/// `EntityDamageEvent::CAUSE_BLOCK_EXPLOSION`.
pub const CAUSE_BLOCK_EXPLOSION: i32 = 9;
/// `EntityDamageEvent::CAUSE_ENTITY_EXPLOSION`.
pub const CAUSE_ENTITY_EXPLOSION: i32 = 10;
/// `EntityDamageEvent::CAUSE_VOID`.
pub const CAUSE_VOID: i32 = 11;
/// `EntityDamageEvent::CAUSE_SUICIDE`.
pub const CAUSE_SUICIDE: i32 = 12;
/// `EntityDamageEvent::CAUSE_MAGIC`.
pub const CAUSE_MAGIC: i32 = 13;
/// `EntityDamageEvent::CAUSE_CUSTOM`.
pub const CAUSE_CUSTOM: i32 = 14;
/// `EntityDamageEvent::CAUSE_STARVATION`.
pub const CAUSE_STARVATION: i32 = 15;

fn apply_cancelled(field: &mut bool, data: &Value) {
    if let Some(value) = data.get("cancelled").and_then(Value::as_bool) {
        *field = value;
    }
}

fn apply_i32(field: &mut i32, data: &Value, key: &str) {
    if let Some(value) = data.get(key).and_then(Value::as_i64) {
        *field = value as i32;
    }
}

fn apply_f32(field: &mut f32, data: &Value, key: &str) {
    if let Some(value) = data.get(key).and_then(Value::as_f64) {
        *field = value as f32;
    }
}

fn apply_entity(field: &mut Value, data: &Value, key: &str) {
    if let Some(value) = data.get(key) {
        if value.is_object() {
            *field = value.clone();
        }
    }
}

/// `pocketmine\event\entity\EntitySpawnEvent` (not cancellable).
pub struct EntitySpawnEvent {
    pub entity: Value,
}

impl EntitySpawnEvent {
    pub fn new(entity: Value) -> Self {
        Self { entity }
    }
}

impl Event for EntitySpawnEvent {
    fn name(&self) -> &str {
        "EntitySpawnEvent"
    }

    fn to_json(&self) -> Value {
        json!({ "entity": self.entity, "cancelled": false })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_entity(&mut self.entity, value, "entity");
    }
}

/// `pocketmine\event\entity\EntityDespawnEvent` (not cancellable).
pub struct EntityDespawnEvent {
    pub entity: Value,
}

impl EntityDespawnEvent {
    pub fn new(entity: Value) -> Self {
        Self { entity }
    }
}

impl Event for EntityDespawnEvent {
    fn name(&self) -> &str {
        "EntityDespawnEvent"
    }

    fn to_json(&self) -> Value {
        json!({ "entity": self.entity, "cancelled": false })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_entity(&mut self.entity, value, "entity");
    }
}

/// `pocketmine\event\entity\EntityDamageEvent`. Cancellable.
pub struct EntityDamageEvent {
    pub entity: Value,
    pub cause: i32,
    pub base_damage: f32,
    pub cancelled: bool,
}

impl EntityDamageEvent {
    pub fn new(entity: Value, cause: i32, base_damage: f32) -> Self {
        Self {
            entity,
            cause,
            base_damage,
            cancelled: false,
        }
    }
}

impl Event for EntityDamageEvent {
    fn name(&self) -> &str {
        "EntityDamageEvent"
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
            "entity": self.entity,
            "cause": self.cause,
            "base_damage": self.base_damage,
            "final_damage": self.base_damage,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_entity(&mut self.entity, value, "entity");
        apply_i32(&mut self.cause, value, "cause");
        apply_f32(&mut self.base_damage, value, "base_damage");
        apply_f32(&mut self.base_damage, value, "final_damage");
        apply_cancelled(&mut self.cancelled, value);
    }
}

/// `pocketmine\event\entity\EntityDamageByEntityEvent`. Cancellable.
pub struct EntityDamageByEntityEvent {
    pub entity: Value,
    pub damager: Value,
    pub cause: i32,
    pub base_damage: f32,
    pub cancelled: bool,
}

impl EntityDamageByEntityEvent {
    pub fn new(entity: Value, damager: Value, cause: i32, base_damage: f32) -> Self {
        Self {
            entity,
            damager,
            cause,
            base_damage,
            cancelled: false,
        }
    }
}

impl Event for EntityDamageByEntityEvent {
    fn name(&self) -> &str {
        "EntityDamageByEntityEvent"
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
            "entity": self.entity,
            "damager": self.damager,
            "cause": self.cause,
            "base_damage": self.base_damage,
            "final_damage": self.base_damage,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_entity(&mut self.entity, value, "entity");
        apply_entity(&mut self.damager, value, "damager");
        apply_i32(&mut self.cause, value, "cause");
        apply_f32(&mut self.base_damage, value, "base_damage");
        apply_f32(&mut self.base_damage, value, "final_damage");
        apply_cancelled(&mut self.cancelled, value);
    }
}

/// `pocketmine\event\entity\EntityDamageByChildEntityEvent`: the projectile
/// (or other child entity) that produced the damage. Cancellable.
pub struct EntityDamageByChildEntityEvent {
    pub entity: Value,
    pub damager: Value,
    pub child: Value,
    pub cause: i32,
    pub base_damage: f32,
    pub cancelled: bool,
}

impl EntityDamageByChildEntityEvent {
    pub fn new(
        entity: Value,
        damager: Value,
        child: Value,
        cause: i32,
        base_damage: f32,
    ) -> Self {
        Self {
            entity,
            damager,
            child,
            cause,
            base_damage,
            cancelled: false,
        }
    }
}

impl Event for EntityDamageByChildEntityEvent {
    fn name(&self) -> &str {
        "EntityDamageByChildEntityEvent"
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
            "entity": self.entity,
            "damager": self.damager,
            "child": self.child,
            "cause": self.cause,
            "base_damage": self.base_damage,
            "final_damage": self.base_damage,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_entity(&mut self.entity, value, "entity");
        apply_entity(&mut self.damager, value, "damager");
        apply_entity(&mut self.child, value, "child");
        apply_i32(&mut self.cause, value, "cause");
        apply_f32(&mut self.base_damage, value, "base_damage");
        apply_f32(&mut self.base_damage, value, "final_damage");
        apply_cancelled(&mut self.cancelled, value);
    }
}

/// `pocketmine\event\entity\EntityDeathEvent` (not cancellable in PM).
pub struct EntityDeathEvent {
    pub entity: Value,
    pub drops: Vec<Value>,
    pub xp: i32,
}

impl EntityDeathEvent {
    pub fn new(entity: Value, drops: Vec<Value>, xp: i32) -> Self {
        Self { entity, drops, xp }
    }
}

impl Event for EntityDeathEvent {
    fn name(&self) -> &str {
        "EntityDeathEvent"
    }

    fn to_json(&self) -> Value {
        json!({
            "entity": self.entity,
            "drops": self.drops,
            "xp": self.xp,
            "cancelled": false
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_entity(&mut self.entity, value, "entity");
        apply_i32(&mut self.xp, value, "xp");
        if let Some(drops) = value.get("drops").and_then(Value::as_array) {
            self.drops = drops.clone();
        }
    }
}

/// A block inside an explosion's blast radius (`EntityExplodeEvent::getBlockList()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffectedBlock {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: u32,
}

/// `EntityExplodeEvent`: fired by `Explosion::explodeB()` **after** the impact
/// was calculated and **before** the blocks are destroyed, so a listener can
/// cancel the explosion, change the drop yield or replace the block list.
///
/// Translated from `pocketmine\event\entity\EntityExplodeEvent`.
pub struct EntityExplodeEvent {
    /// `PrimedTNT` snapshot (`entity_json()`), like `getEntity()` in PM.
    pub entity: Value,
    pub world: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Chance of a drop per block, `0..=100` (`Explosion`: `1 / radius * 100`).
    pub yield_pct: f64,
    pub blocks: Vec<AffectedBlock>,
    pub cancelled: bool,
}

impl EntityExplodeEvent {
    pub fn new(
        entity: Value,
        world: impl Into<String>,
        x: f64,
        y: f64,
        z: f64,
        yield_pct: f64,
        blocks: Vec<AffectedBlock>,
    ) -> Self {
        Self {
            entity,
            world: world.into(),
            x,
            y,
            z,
            yield_pct,
            blocks,
            cancelled: false,
        }
    }
}

impl Event for EntityExplodeEvent {
    fn name(&self) -> &str {
        "EntityExplodeEvent"
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
        let blocks: Vec<Value> = self
            .blocks
            .iter()
            .map(|block| {
                json!({
                    "x": block.x,
                    "y": block.y,
                    "z": block.z,
                    "block_id": block.block_id
                })
            })
            .collect();
        json!({
            "entity": self.entity,
            "world": self.world,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "yield": self.yield_pct,
            "blocks": blocks,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_entity(&mut self.entity, value, "entity");
        if let Some(world) = value.get("world").and_then(Value::as_str) {
            self.world = world.to_string();
        }
        for (field, key) in [
            (&mut self.x, "x"),
            (&mut self.y, "y"),
            (&mut self.z, "z"),
            (&mut self.yield_pct, "yield"),
        ] {
            if let Some(number) = value.get(key).and_then(Value::as_f64) {
                *field = number;
            }
        }
        if let Some(blocks) = value.get("blocks").and_then(Value::as_array) {
            self.blocks = blocks
                .iter()
                .map(|block| AffectedBlock {
                    x: block.get("x").and_then(Value::as_i64).unwrap_or(0) as i32,
                    y: block.get("y").and_then(Value::as_i64).unwrap_or(0) as i32,
                    z: block.get("z").and_then(Value::as_i64).unwrap_or(0) as i32,
                    block_id: block.get("block_id").and_then(Value::as_u64).unwrap_or(0) as u32,
                })
                .collect();
        }
        if let Some(cancelled) = value.get("cancelled").and_then(Value::as_bool) {
            self.cancelled = cancelled;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explode_event_keeps_edits_to_the_block_list_and_yield() {
        let mut event = EntityExplodeEvent::new(
            json!({"unique_id": 7}),
            "world",
            1.0,
            2.0,
            3.0,
            25.0,
            vec![AffectedBlock {
                x: 1,
                y: 2,
                z: 3,
                block_id: 9,
            }],
        );
        assert_eq!(event.name(), "EntityExplodeEvent");
        assert!(event.is_cancellable());

        let mut edited = event.to_json();
        edited["yield"] = json!(50.0);
        edited["blocks"] = json!([{"x": 4, "y": 5, "z": 6, "block_id": 11}]);
        event.apply_json(&edited);
        assert_eq!(event.yield_pct, 50.0);
        assert_eq!(event.blocks.len(), 1);
        assert_eq!(event.blocks[0].block_id, 11);
        assert!(!event.cancelled);
    }

    #[test]
    fn damage_event_round_trips_cancellation_and_damage() {
        let mut event =
            EntityDamageByEntityEvent::new(json!({"unique_id": 1}), json!({"unique_id": 2}), CAUSE_ENTITY_ATTACK, 4.0);
        assert_eq!(event.name(), "EntityDamageByEntityEvent");
        assert!(event.is_cancellable());
        // A plugin that cancels and raises the damage.
        event.apply_json(&json!({ "cancelled": true, "base_damage": 7.5 }));
        assert!(event.is_cancelled());
        assert!((event.base_damage - 7.5).abs() < 0.001);
        // A partial payload (only the entity) must not reset anything.
        event.apply_json(&json!({ "entity": { "unique_id": 9 } }));
        assert!((event.base_damage - 7.5).abs() < 0.001);
        assert!(event.is_cancelled());
        assert_eq!(event.entity["unique_id"], json!(9));
    }

    #[test]
    fn child_damage_event_carries_the_projectile() {
        let mut event = EntityDamageByChildEntityEvent::new(
            json!({"unique_id": 1}),
            json!({"unique_id": 2}),
            json!({"unique_id": 3, "type": "minecraft:arrow"}),
            CAUSE_PROJECTILE,
            2.0,
        );
        assert_eq!(event.name(), "EntityDamageByChildEntityEvent");
        assert!(event.is_cancellable());
        assert_eq!(event.to_json()["child"]["type"], json!("minecraft:arrow"));
        event.apply_json(&json!({ "cancelled": true, "final_damage": 4.0 }));
        assert!(event.is_cancelled());
        assert!((event.base_damage - 4.0).abs() < 0.001);
        // The cause constants mirror `EntityDamageEvent`.
        assert_eq!(CAUSE_FALL, 4);
        assert_eq!(CAUSE_VOID, 11);
        assert_eq!(CAUSE_STARVATION, 15);
    }

    #[test]
    fn death_event_carries_drops_and_xp() {
        let mut event = EntityDeathEvent::new(
            json!({"unique_id": 3}),
            vec![json!({"name": "minecraft:rotten_flesh", "count": 1})],
            5,
        );
        assert_eq!(event.name(), "EntityDeathEvent");
        assert!(!event.is_cancellable());
        let json = event.to_json();
        assert_eq!(json["xp"], json!(5));
        assert_eq!(json["drops"][0]["name"], json!("minecraft:rotten_flesh"));
        event.apply_json(&json!({ "xp": 12 }));
        assert_eq!(event.xp, 12);
    }

    #[test]
    fn spawn_event_keeps_the_entity_snapshot() {
        let event = EntitySpawnEvent::new(json!({"type": "minecraft:zombie"}));
        assert_eq!(event.name(), "EntitySpawnEvent");
        assert_eq!(event.to_json()["entity"]["type"], json!("minecraft:zombie"));
    }
}
