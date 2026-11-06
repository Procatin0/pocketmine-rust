//! Shared world state for the server core and the PHP plugin host.
//!
//! The generator produces base terrain; this type layers persistent block
//! edits on top (plugins changing blocks), answers `get_block` queries and
//! produces chunk payloads that include those edits.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use bedrock_world::chunk::MIN_SUBCHUNK_INDEX;
use bedrock_world::WorldGenerator;
use parking_lot::RwLock;

/// Absolute block coordinate -> network block id.
type BlockEdits = HashMap<(i32, i32, i32), u32>;

pub const WORLD_MIN_Y: i32 = MIN_SUBCHUNK_INDEX * 16;
pub const WORLD_MAX_Y: i32 = 320;

/// Ticks in a full Minecraft day.
pub const TICKS_PER_DAY: i32 = 24_000;
/// Initial world time: morning.
pub const DEFAULT_WORLD_TIME: i32 = 1_000;

pub struct WorldState {
    pub name: String,
    generator: Arc<dyn WorldGenerator>,
    spawn: RwLock<(i32, i32, i32)>,
    time: AtomicI32,
    weather: AtomicI32,
    difficulty: AtomicI32,
    edits: RwLock<HashMap<(i32, i32), BlockEdits>>,
}

impl WorldState {
    pub fn new(name: impl Into<String>, generator: Arc<dyn WorldGenerator>) -> Self {
        let spawn = generator.spawn_position();
        Self {
            name: name.into(),
            generator,
            spawn: RwLock::new(spawn),
            time: AtomicI32::new(DEFAULT_WORLD_TIME),
            weather: AtomicI32::new(0),
            // PocketMine defaults to normal difficulty.
            difficulty: AtomicI32::new(2),
            edits: RwLock::new(HashMap::new()),
        }
    }

    pub fn generator(&self) -> &Arc<dyn WorldGenerator> {
        &self.generator
    }

    /// Seed reported by `/seed`.
    pub fn seed(&self) -> u64 {
        self.generator.seed() as u64
    }

    /// Difficulty: 0 peaceful, 1 easy, 2 normal, 3 hard.
    pub fn difficulty(&self) -> i32 {
        self.difficulty.load(Ordering::Relaxed)
    }

    /// `SetDifficultyPacket` value used when a world is created or `/difficulty`
    /// runs.
    pub fn set_difficulty(&self, difficulty: i32) -> i32 {
        let clamped = difficulty.clamp(0, 3);
        self.difficulty.store(clamped, Ordering::Relaxed);
        clamped
    }

    pub fn spawn(&self) -> (i32, i32, i32) {
        *self.spawn.read()
    }

    /// Moves the world spawn (`/setworldspawn`).
    pub fn set_spawn(&self, x: i32, y: i32, z: i32) {
        *self.spawn.write() = (x, y, z);
    }

    /// Current world time, wrapped to a full day.
    pub fn time(&self) -> i32 {
        self.time.load(Ordering::Relaxed)
    }

    /// Sets the world time (`/time set`) and returns the wrapped value.
    pub fn set_time(&self, ticks: i32) -> i32 {
        let wrapped = ticks.rem_euclid(TICKS_PER_DAY);
        self.time.store(wrapped, Ordering::Relaxed);
        wrapped
    }

    /// Adds ticks to the world time (`/time add`) and returns the new value.
    pub fn add_time(&self, ticks: i32) -> i32 {
        self.set_time(self.time().saturating_add(ticks))
    }

    /// Weather state: 0 clear, 1 rain, 2 thunder.
    pub fn weather(&self) -> i32 {
        self.weather.load(Ordering::Relaxed)
    }

    pub fn set_weather(&self, weather: i32) {
        self.weather.store(weather, Ordering::Relaxed);
    }

    pub fn player_position(&self) -> (f32, f32, f32) {
        self.generator.player_position()
    }

    /// Block id at absolute coordinates, taking plugin edits into account.
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> u32 {
        if let Some(edit) = self
            .edits
            .read()
            .get(&(x >> 4, z >> 4))
            .and_then(|chunk| chunk.get(&(x, y, z)))
        {
            return *edit;
        }
        self.generator.block_at(x, y, z)
    }

    /// Applies a block edit. Returns false when the block did not change.
    pub fn set_block(&self, x: i32, y: i32, z: i32, block_id: u32) -> bool {
        if !(WORLD_MIN_Y..WORLD_MAX_Y).contains(&y) {
            return false;
        }

        let previous = self.block_at(x, y, z);
        if previous == block_id {
            return false;
        }

        self.edits
            .write()
            .entry((x >> 4, z >> 4))
            .or_default()
            .insert((x, y, z), block_id);
        true
    }

    /// Highest non-air block in a column, including edits.
    pub fn highest_block_at(&self, x: i32, z: i32) -> i32 {
        let base = self.generator.highest_block_at(x, z);
        let highest_edit = self
            .edits
            .read()
            .get(&(x >> 4, z >> 4))
            .and_then(|chunk| {
                chunk
                    .iter()
                    .filter(|((ex, _, ez), _)| *ex == x && *ez == z)
                    .map(|((_, ey, _), _)| *ey)
                    .max()
            });

        let candidate = match highest_edit {
            Some(edit_y) => base.max(edit_y),
            None => base,
        };

        // Edits can also remove the top block; walk down until something solid.
        let mut y = candidate;
        while y > WORLD_MIN_Y && self.block_at(x, y, z) == bedrock_world::block_id::AIR {
            y -= 1;
            // Only a handful of iterations are expected; the generator answers
            // are cached per column by the caller in practice.
            if candidate - y > 8 {
                break;
            }
        }
        y.max(WORLD_MIN_Y)
    }

    /// Generates a chunk payload including every edit that lands in it.
    pub fn chunk_payload(&self, chunk_x: i32, chunk_z: i32) -> (u32, Vec<u8>) {
        let edits = self
            .edits
            .read()
            .get(&(chunk_x, chunk_z))
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|((x, y, z), id)| (*x, *y, *z, *id))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        self.generator
            .generate_chunk_with_edits(chunk_x, chunk_z, &edits)
    }

    /// Number of edited chunks, exposed for diagnostics.
    pub fn edited_chunk_count(&self) -> usize {
        self.edits.read().len()
    }

    /// Writes the world snapshot (`world.json`): time, weather, difficulty,
    /// spawn, seed and every block edit.
    ///
    /// PocketMine stores this in `level.dat`; JSON keeps the port free of an
    /// NBT serializer while preserving the same information.
    pub fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        let (spawn_x, spawn_y, spawn_z) = self.spawn();
        let edits: Vec<[i64; 4]> = self
            .edits
            .read()
            .values()
            .flat_map(|chunk| {
                chunk.iter().map(|((x, y, z), id)| {
                    [*x as i64, *y as i64, *z as i64, *id as i64]
                })
            })
            .collect();
        let value = serde_json::json!({
            "name": self.name,
            "time": self.time(),
            "weather": self.weather(),
            "difficulty": self.difficulty(),
            "spawn": [spawn_x, spawn_y, spawn_z],
            "seed": self.seed(),
            "edits": edits,
        });
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec(&value)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        std::fs::write(path, bytes)
    }

    /// Applies a snapshot written by [`WorldState::save_to`].
    ///
    /// Returns false when the file is missing or malformed, leaving the world
    /// untouched.
    pub fn load_from(&self, path: &std::path::Path) -> bool {
        let Ok(bytes) = std::fs::read(path) else {
            return false;
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return false;
        };

        if let Some(time) = value.get("time").and_then(|value| value.as_i64()) {
            self.set_time(time as i32);
        }
        if let Some(weather) = value.get("weather").and_then(|value| value.as_i64()) {
            self.set_weather(weather as i32);
        }
        if let Some(difficulty) = value.get("difficulty").and_then(|value| value.as_i64()) {
            self.set_difficulty(difficulty as i32);
        }
        if let Some(spawn) = value.get("spawn").and_then(|value| value.as_array()) {
            let coords: Vec<i32> = spawn
                .iter()
                .filter_map(|value| value.as_i64().map(|n| n as i32))
                .collect();
            if coords.len() == 3 {
                self.set_spawn(coords[0], coords[1], coords[2]);
            }
        }
        if let Some(edits) = value.get("edits").and_then(|value| value.as_array()) {
            let mut map = self.edits.write();
            for entry in edits {
                let Some(values) = entry.as_array() else {
                    continue;
                };
                let nums: Vec<i64> = values
                    .iter()
                    .filter_map(|value| value.as_i64())
                    .collect();
                if nums.len() != 4 {
                    continue;
                }
                let (x, y, z) = (nums[0] as i32, nums[1] as i32, nums[2] as i32);
                map.entry((x >> 4, z >> 4))
                    .or_default()
                    .insert((x, y, z), nums[3] as u32);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bedrock_world::FlatGenerator;

    #[test]
    fn snapshot_roundtrips_time_spawn_difficulty_and_edits() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        world.set_time(6_000);
        world.set_difficulty(3);
        world.set_spawn(4, 70, -8);
        assert!(world.set_block(5, 68, 9, 1));

        let dir = std::env::temp_dir().join(format!("pm-rust-world-{}", std::process::id()));
        let path = dir.join("world.json");
        world.save_to(&path).expect("save the world");

        let restored = WorldState::new("world", Arc::new(FlatGenerator::new()));
        assert!(restored.load_from(&path));
        assert_eq!(restored.time(), 6_000);
        assert_eq!(restored.difficulty(), 3);
        assert_eq!(restored.spawn(), (4, 70, -8));
        assert_eq!(restored.block_at(5, 68, 9), 1);

        // A missing file leaves the world untouched.
        let missing = dir.join("missing.json");
        assert!(!restored.load_from(&missing));
        assert_eq!(restored.time(), 6_000);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
