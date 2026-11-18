//! World events of `pocketmine\event\world\*`, ported from PM5.
//!
//! `WorldState` has no plugin manager (the dispatch lives in `SessionManager`),
//! so these events are queued by the world mutation that causes them
//! (`World::setSpawnLocation()`, `World::setDifficulty()`, `World::save()`) and
//! dispatched on the next tick. All three are non-cancellable in PocketMine, so
//! the deferral cannot change the outcome: a listener only observes the change.

use serde_json::{json, Value};

use crate::event::Event;

fn apply_string(field: &mut String, data: &Value, key: &str) {
    if let Some(value) = data.get(key).and_then(Value::as_str) {
        *field = value.to_string();
    }
}

fn apply_f64(field: &mut f64, data: &Value, key: &str) {
    if let Some(value) = data.get(key) {
        if let Some(number) = value.as_f64() {
            *field = number;
        }
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

/// `ChunkLoadEvent`: fired when a chunk is loaded or newly created by the world
/// generator. `new_chunk` is `false` when it came from storage; this port has no
/// chunk storage, so every column is generated on demand (`new_chunk = true`).
pub struct ChunkLoadEvent {
    pub world: String,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub new_chunk: bool,
}

impl ChunkLoadEvent {
    pub fn new(world: impl Into<String>, chunk_x: i32, chunk_z: i32, new_chunk: bool) -> Self {
        Self {
            world: world.into(),
            chunk_x,
            chunk_z,
            new_chunk,
        }
    }
}

impl Event for ChunkLoadEvent {
    fn name(&self) -> &str {
        "ChunkLoadEvent"
    }

    fn to_json(&self) -> Value {
        json!({
            "world": self.world,
            "chunk_x": self.chunk_x,
            "chunk_z": self.chunk_z,
            "new_chunk": self.new_chunk,
            "cancelled": false
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_string(&mut self.world, value, "world");
        apply_i32(&mut self.chunk_x, value, "chunk_x");
        apply_i32(&mut self.chunk_z, value, "chunk_z");
        if let Some(new_chunk) = value.get("new_chunk").and_then(Value::as_bool) {
            self.new_chunk = new_chunk;
        }
    }
}

/// `WorldSaveEvent`: fired by `World::save()` before the snapshot is written
/// (manual save, `/save-all` and the autosave pass).
pub struct WorldSaveEvent {
    pub world: String,
}

impl WorldSaveEvent {
    pub fn new(world: impl Into<String>) -> Self {
        Self {
            world: world.into(),
        }
    }
}

impl Event for WorldSaveEvent {
    fn name(&self) -> &str {
        "WorldSaveEvent"
    }

    fn to_json(&self) -> Value {
        json!({
            "world": self.world,
            "cancelled": false
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_string(&mut self.world, value, "world");
    }
}

/// `SpawnChangeEvent`: fired after `World::setSpawnLocation()` moved the spawn,
/// carrying the previous one.
pub struct SpawnChangeEvent {
    pub world: String,
    pub previous_x: f64,
    pub previous_y: f64,
    pub previous_z: f64,
}

impl SpawnChangeEvent {
    pub fn new(world: impl Into<String>, previous_x: f64, previous_y: f64, previous_z: f64) -> Self {
        Self {
            world: world.into(),
            previous_x,
            previous_y,
            previous_z,
        }
    }
}

impl Event for SpawnChangeEvent {
    fn name(&self) -> &str {
        "SpawnChangeEvent"
    }

    fn to_json(&self) -> Value {
        json!({
            "world": self.world,
            "previous_x": self.previous_x,
            "previous_y": self.previous_y,
            "previous_z": self.previous_z,
            "cancelled": false
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_string(&mut self.world, value, "world");
        apply_f64(&mut self.previous_x, value, "previous_x");
        apply_f64(&mut self.previous_y, value, "previous_y");
        apply_f64(&mut self.previous_z, value, "previous_z");
    }
}

/// `WorldDifficultyChangeEvent`: fired by `World::setDifficulty()` with the old
/// and the requested difficulty.
pub struct WorldDifficultyChangeEvent {
    pub world: String,
    pub old_difficulty: i32,
    pub new_difficulty: i32,
}

impl WorldDifficultyChangeEvent {
    pub fn new(world: impl Into<String>, old_difficulty: i32, new_difficulty: i32) -> Self {
        Self {
            world: world.into(),
            old_difficulty,
            new_difficulty,
        }
    }
}

impl Event for WorldDifficultyChangeEvent {
    fn name(&self) -> &str {
        "WorldDifficultyChangeEvent"
    }

    fn to_json(&self) -> Value {
        json!({
            "world": self.world,
            "old_difficulty": self.old_difficulty,
            "new_difficulty": self.new_difficulty,
            "cancelled": false
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_string(&mut self.world, value, "world");
        apply_i32(&mut self.old_difficulty, value, "old_difficulty");
        apply_i32(&mut self.new_difficulty, value, "new_difficulty");
    }
}

fn apply_recipients(field: &mut Vec<String>, data: &Value) {
    if let Some(values) = data.get("recipients").and_then(Value::as_array) {
        *field = values
            .iter()
            .filter_map(|value| value.as_str())
            .map(str::to_string)
            .collect();
    }
}

/// `WorldSoundEvent`: `World::addSound()` is about to play a sound.
///
/// A PHP host call cannot dispatch events (the runtime is already inside the
/// call), so `world.add_sound` queues the effect and `SessionManager` fires
/// this event on the next tick, before broadcasting. Cancelling drops the
/// packet; `setSound()`/`setRecipients()` change what is sent.
///
/// The sound travels as its wire description (`sound` name or `event_id`)
/// because a PHP `Sound` object cannot cross the IPC boundary.
pub struct WorldSoundEvent {
    pub world: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// `LevelSoundEvent` name (`"place"`, ...), when the sound is one.
    pub sound: Option<String>,
    /// `LevelEvent` id (`ClickSound`, ...), when the sound is one.
    pub event_id: Option<i32>,
    /// Block runtime id (level sound) or `pitch * 1000` (level event).
    pub extra_data: i32,
    /// Lower-case player names; empty means every viewer of the chunk.
    pub recipients: Vec<String>,
    pub cancelled: bool,
}

impl WorldSoundEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world: impl Into<String>,
        sound: Option<String>,
        event_id: Option<i32>,
        extra_data: i32,
        x: f64,
        y: f64,
        z: f64,
        recipients: Vec<String>,
    ) -> Self {
        Self {
            world: world.into(),
            x,
            y,
            z,
            sound,
            event_id,
            extra_data,
            recipients,
            cancelled: false,
        }
    }
}

impl Event for WorldSoundEvent {
    fn name(&self) -> &str {
        "WorldSoundEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancelled: bool) {
        self.cancelled = cancelled;
    }

    fn to_json(&self) -> Value {
        json!({
            "world": self.world,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "sound": self.sound,
            "event_id": self.event_id,
            "extra_data": self.extra_data,
            "recipients": self.recipients,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_string(&mut self.world, value, "world");
        apply_f64(&mut self.x, value, "x");
        apply_f64(&mut self.y, value, "y");
        apply_f64(&mut self.z, value, "z");
        if value.get("sound").is_some() {
            self.sound = value
                .get("sound")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        if value.get("event_id").is_some() {
            self.event_id = value
                .get("event_id")
                .and_then(Value::as_i64)
                .map(|id| id as i32);
        }
        apply_i32(&mut self.extra_data, value, "extra_data");
        apply_recipients(&mut self.recipients, value);
    }
}

/// `WorldParticleEvent`: `World::addParticle()` is about to spawn particles.
///
/// Same deferral as [`WorldSoundEvent`]. The particle travels as its
/// `LevelEventPacket` id/data pair; `event_id == 0` means the listener replaced
/// it with a particle the bridge cannot encode and the packet is dropped
/// (PM would send nothing for an empty `encode()`).
pub struct WorldParticleEvent {
    pub world: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// `LevelEventPacket` event id (already masked for standard particles).
    pub event_id: i32,
    pub event_data: i32,
    pub recipients: Vec<String>,
    pub cancelled: bool,
}

impl WorldParticleEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world: impl Into<String>,
        event_id: i32,
        event_data: i32,
        x: f64,
        y: f64,
        z: f64,
        recipients: Vec<String>,
    ) -> Self {
        Self {
            world: world.into(),
            x,
            y,
            z,
            event_id,
            event_data,
            recipients,
            cancelled: false,
        }
    }
}

impl Event for WorldParticleEvent {
    fn name(&self) -> &str {
        "WorldParticleEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancelled: bool) {
        self.cancelled = cancelled;
    }

    fn to_json(&self) -> Value {
        json!({
            "world": self.world,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "event_id": self.event_id,
            "event_data": self.event_data,
            "recipients": self.recipients,
            "cancelled": self.cancelled
        })
    }

    fn apply_json(&mut self, value: &Value) {
        apply_string(&mut self.world, value, "world");
        apply_f64(&mut self.x, value, "x");
        apply_f64(&mut self.y, value, "y");
        apply_f64(&mut self.z, value, "z");
        apply_i32(&mut self.event_id, value, "event_id");
        apply_i32(&mut self.event_data, value, "event_data");
        apply_recipients(&mut self.recipients, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_load_keeps_coordinates_and_the_generated_flag() {
        let event = ChunkLoadEvent::new("world", -3, 12, true);
        assert_eq!(event.name(), "ChunkLoadEvent");
        assert!(!event.is_cancellable());
        let mut copy = ChunkLoadEvent::new("", 0, 0, false);
        copy.apply_json(&event.to_json());
        assert_eq!((copy.chunk_x, copy.chunk_z), (-3, 12));
        assert!(copy.new_chunk);
    }

    #[test]
    fn world_save_roundtrips() {
        let event = WorldSaveEvent::new("world");
        assert_eq!(event.name(), "WorldSaveEvent");
        assert!(!event.is_cancellable());
        let mut copy = WorldSaveEvent::new("");
        copy.apply_json(&event.to_json());
        assert_eq!(copy.world, "world");
    }

    #[test]
    fn spawn_change_keeps_the_previous_spawn() {
        let event = SpawnChangeEvent::new("nether", 1.5, 70.0, -8.25);
        let json = event.to_json();
        assert_eq!(json["previous_x"], json!(1.5));
        assert_eq!(json["previous_y"], json!(70.0));
        assert_eq!(json["previous_z"], json!(-8.25));

        let mut copy = SpawnChangeEvent::new("", 0.0, 0.0, 0.0);
        copy.apply_json(&json);
        assert_eq!(copy.world, "nether");
        assert_eq!(copy.previous_z, -8.25);
    }

    #[test]
    fn difficulty_change_keeps_both_values() {
        let event = WorldDifficultyChangeEvent::new("world", 2, 3);
        assert!(!event.is_cancellable());
        let mut copy = WorldDifficultyChangeEvent::new("", 0, 0);
        copy.apply_json(&event.to_json());
        assert_eq!(copy.old_difficulty, 2);
        assert_eq!(copy.new_difficulty, 3);
    }

    #[test]
    fn world_sound_event_roundtrips_and_applies_listener_changes() {
        let event = WorldSoundEvent::new(
            "world",
            Some("place".to_string()),
            None,
            42,
            1.5,
            64.25,
            -2.5,
            vec!["steve".to_string()],
        );
        assert_eq!(event.name(), "WorldSoundEvent");
        assert!(event.is_cancellable());

        let mut copy = WorldSoundEvent::new("", None, None, 0, 0.0, 0.0, 0.0, vec![]);
        copy.apply_json(&event.to_json());
        assert_eq!(copy.world, "world");
        assert_eq!(copy.sound.as_deref(), Some("place"));
        assert_eq!(copy.event_id, None);
        assert_eq!(copy.extra_data, 42);
        assert_eq!(copy.x, 1.5);
        assert_eq!(copy.recipients, vec!["steve".to_string()]);

        // A listener switching to a level-event sound and cancelling.
        copy.apply_json(&json!({
            "sound": null,
            "event_id": 1000,
            "extra_data": 500
        }));
        assert_eq!(copy.sound, None);
        assert_eq!(copy.event_id, Some(1000));

        // La cancelación llega por `response.cancelled`, no en el snapshot.
        copy.set_cancelled(true);
        assert!(copy.is_cancelled());
    }

    #[test]
    fn world_particle_event_roundtrips() {
        let event = WorldParticleEvent::new(
            "world",
            0x4000 | 8,
            0,
            3.25,
            65.5,
            4.0,
            vec!["alex".to_string()],
        );
        assert_eq!(event.name(), "WorldParticleEvent");
        assert!(event.is_cancellable());
        let mut copy = WorldParticleEvent::new("", 0, 0, 0.0, 0.0, 0.0, vec![]);
        copy.apply_json(&event.to_json());
        assert_eq!(copy.event_id, 0x4000 | 8);
        assert_eq!(copy.x, 3.25);
        assert_eq!(copy.recipients, vec!["alex".to_string()]);

        copy.apply_json(&json!({ "event_id": 0 }));
        assert_eq!(copy.event_id, 0);
        copy.set_cancelled(true);
        assert!(copy.is_cancelled());
    }
}
