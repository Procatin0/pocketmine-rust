//! Shared world state for the server core and the PHP plugin host.
//!
//! The generator produces base terrain; this type layers persistent block
//! edits on top (plugins changing blocks), answers `get_block` queries and
//! produces chunk payloads that include those edits.
//!
//! # Persistence divergence (documented, not reimplemented)
//!
//! PocketMine persists worlds in LevelDB/Anvil (`level.dat` + `db/` chunks).
//! This port deliberately does **not** reimplement LevelDB: `world.json`
//! (format v2) stores the same logical snapshot — time, weather, difficulty,
//! spawn, seed and every block edit — without an NBT/chunk serializer.
//! Consequences:
//! - Existing PM `level.dat` worlds are **not** readable; the generator
//!   recreates terrain deterministically from `seed` and edits replay on top.
//! - Chunk edits are stored as a flat `[x, y, z, block_id]` list, so large
//!   edited worlds grow linearly with edits, not with world size.
//! - v1 files (no `version` key) still load; v2 adds `version`, `auto_save`
//!   and `time_running` keys.
//!
//! # Multi-world
//!
//! `SessionManager` owns a single `WorldState` ("world"). Cross-world
//! teleports in `PluginAction::Teleport` already swap the session's `world`
//! (a fresh `WorldState` with the same generator) when the target name
//! differs — see `session.rs`. Full multi-world (`WorldManager` with
//! per-world generators, `/tp` across worlds) is **not** implemented: PHP
//! `WorldManager::generateWorld`/`loadWorld(string)` therefore fall back to
//! the single overworld and are documented as best-effort in `world.php`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use bedrock_protocol::types::Vector3f;
use bedrock_protocol::ItemStack;
use bedrock_world::chunk::{Chunk, MIN_SUBCHUNK_INDEX};
use bedrock_world::WorldGenerator;
use parking_lot::{Mutex, RwLock};

/// Deterministic `lcg_value()` stand-in for `World::dropItem()`'s random
/// sideways motion (PocketMine seeds from the RNG; the exact value is not
/// observable behaviour, only the [0, 0.2) spread is).
fn pseudo_random(seed: i64) -> f32 {
    let mut x = seed as u64 ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    (x % 10_000) as f32 / 10_000.0
}

/// Absolute block coordinate -> network block id.
type BlockEdits = HashMap<(i32, i32, i32), u32>;

/// Immutable snapshot of one column for the random-tick pass.
///
/// The pass reads thousands of positions per tick. Going through
/// [`WorldState::loaded_block_at`] costs two locks and two hash lookups per
/// position, which made `tick.random_blocks` 70% of the tick. The snapshot is
/// a refcount of the shared base column (no copy) plus, for edited columns, an
/// immutable `Arc` of the edit map: [`WorldState::set_block`] publishes a new
/// map (copy-on-write) instead of mutating one in place, so a snapshot can
/// never observe a half-written column and needs no lock to read.
#[derive(Clone, Default)]
pub(crate) struct ColumnView {
    chunk: Option<Arc<Chunk>>,
    edits: Option<Arc<BlockEdits>>,
}

impl ColumnView {
    /// True when the server built (streamed) this column, i.e. it ticks.
    pub(crate) fn is_loaded(&self) -> bool {
        self.chunk.is_some()
    }

    /// Block at absolute coordinates: the edit overlay first, then the loaded
    /// column's base terrain. `None` when the column was never streamed and
    /// the cell has no edit (the same answer as
    /// [`WorldState::loaded_block_at`]).
    #[inline]
    pub(crate) fn block_at(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        if let Some(edits) = &self.edits {
            if let Some(id) = edits.get(&(x, y, z)) {
                return Some(*id);
            }
        }
        self.chunk
            .as_ref()
            .map(|chunk| chunk.get_block((x & 15) as usize, y, (z & 15) as usize))
    }

    /// Highest non-air block of the loaded base terrain.
    pub(crate) fn highest_block_at(&self, x: i32, z: i32) -> Option<i32> {
        self.chunk
            .as_ref()
            .and_then(|chunk| chunk.get_highest_block_at((x & 15) as usize, (z & 15) as usize))
    }

    /// Highest edited cell of the column, ignoring terrain.
    pub(crate) fn highest_edit_at(&self, x: i32, z: i32) -> Option<i32> {
        self.edits.as_ref().and_then(|edits| {
            edits
                .iter()
                .filter(|((ex, _, ez), _)| *ex == x && *ez == z)
                .map(|((_, ey, _), _)| *ey)
                .max()
        })
    }
}

/// One coalesced edit: the id before the call and the final id.
type Edit = (u32, u32);

/// Maximum entries kept in each block-change queue (Fase 1/2 sync).
///
/// Both consumers (Java bridge via `world.take_block_changes`, Bedrock
/// sessions via `SessionManager::poll_block_changes`) get their own capped
/// queue, so a slow consumer cannot grow memory without bound: when the cap
/// is exceeded the oldest entries are dropped. The newest edit for a given
/// coordinate is always retained longest, so replaying the survivors still
/// converges to the latest block at that coordinate.
///
/// Kept above PocketMine's 512-per-chunk `LevelChunk` resend threshold
/// (`World::actuallyDoTick`) so that threshold is actually reachable: a
/// cascade that rewrites more than 512 cells of one column must still arrive
/// as one whole-column resend instead of being silently truncated here.
pub const BLOCK_CHANGE_QUEUE_CAP: usize = 4096;

/// A single block edit, as queued by [`WorldState::set_block`].
///
/// `block_id` is the Bedrock network runtime id (FNV-1a hash, stored as
/// `u32`) — the same value accepted by `world.set_block` / returned by
/// `world.get_block` and sent in `UpdateBlockPacket`.
#[derive(Debug, Clone, Copy, Eq)]
pub struct BlockChange {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: u32,
    pub previous_block_id: Option<u32>,
}

impl PartialEq for BlockChange {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x
            && self.y == other.y
            && self.z == other.z
            && self.block_id == other.block_id
            && (self.previous_block_id.is_none()
                || other.previous_block_id.is_none()
                || self.previous_block_id == other.previous_block_id)
    }
}

impl BlockChange {
    pub fn new(x: i32, y: i32, z: i32, block_id: u32) -> Self {
        Self {
            x,
            y,
            z,
            block_id,
            previous_block_id: None,
        }
    }

    pub fn with_previous(x: i32, y: i32, z: i32, block_id: u32, previous_block_id: u32) -> Self {
        Self {
            x,
            y,
            z,
            block_id,
            previous_block_id: Some(previous_block_id),
        }
    }
}

/// Pushes `change`, dropping the oldest entries past the cap.
fn push_capped(queue: &mut VecDeque<BlockChange>, change: BlockChange) {
    queue.push_back(change);
    while queue.len() > BLOCK_CHANGE_QUEUE_CAP {
        queue.pop_front();
    }
}

pub const WORLD_MIN_Y: i32 = MIN_SUBCHUNK_INDEX * 16;
pub const WORLD_MAX_Y: i32 = 320;

/// Ticks in a full Minecraft day.
pub const TICKS_PER_DAY: i32 = 24_000;
/// Initial world time: morning.
pub const DEFAULT_WORLD_TIME: i32 = 1_000;

/// `world.json` format version written by [`WorldState::save_to`].
pub const WORLD_FORMAT_VERSION: i32 = 2;

pub struct WorldState {
    pub name: String,
    generator: Arc<dyn WorldGenerator>,
    spawn: RwLock<(i32, i32, i32)>,
    time: AtomicI32,
    weather: AtomicI32,
    difficulty: AtomicI32,
    /// `World::startTime()`/`stopTime()`: when false the tick loop must not
    /// advance the clock (`/time start`/`stop`). Defaults to `true`.
    time_running: std::sync::atomic::AtomicBool,
    /// `getAutoSave()` state, persisted in v2 (defaults to `true`).
    auto_save: std::sync::atomic::AtomicBool,
    /// Period in ticks between automatic saves (`WorldManager`).
    auto_save_interval: std::sync::atomic::AtomicI32,
    /// Last tick recorded from PHP (`World::doTick`/`WorldManager::tick`).
    tick_count: std::sync::atomic::AtomicI32,
    /// PM `World::scheduleDelayedBlockUpdate()` queue: `(due tick, position,
    /// block id, sound)`. Pressed buttons are its only producer today, so a
    /// linear scan beats a heap. Each entry restores a block state and queues
    /// the sound PM plays from `onScheduledUpdate()`.
    scheduled_block_updates: Mutex<Vec<(i32, (i32, i32, i32), u32, Option<String>)>>,
    /// PM `Liquid::onNearbyBlockChange()` -> `scheduleDelayedBlockUpdate()`:
    /// `position -> due tick`. Every liquid cell re-evaluates itself every
    /// `tickRate()` ticks (5 for water, 30 for lava); keeping the earliest due
    /// tick per cell means repeated neighbour notifications cannot pile up.
    fluid_updates: Mutex<HashMap<(i32, i32, i32), i32>>,
    /// Tick counter driving the scheduled queue. It advances even with
    /// `/time stop`, because scheduled updates are counted in ticks.
    scheduled_tick: AtomicI32,
    /// Sounds produced while the queue fired, drained once per tick by
    /// `SessionManager::tick` (PM `World::addSound` from `onScheduledUpdate`).
    pending_sounds: Mutex<Vec<(String, i32, i32, i32, i32)>>,
    /// `World::addSound()` called with a fractional position (PHP passes a
    /// `Vector3`, not block coordinates), so the packet keeps the exact spot.
    pending_sounds_at: Mutex<Vec<(String, i32, f32, f32, f32)>>,
    /// Per-column biome overrides from `World::setBiomeId()`.
    ///
    /// Bedrock serializes one biome per subchunk (24 palettes per column), so
    /// the bridge keeps one id per `(x, z)` column, the same 2D granularity
    /// the generator writes. `build_chunk` applies them, which is what makes
    /// them visible in the network payload and in `/locatebiome`-style reads.
    biome_overrides: RwLock<HashMap<(i32, i32), u8>>,
    /// Redstone: the block-update queue, the scheduled component ticks and the
    /// comparator output levels that Bedrock block states cannot carry (see
    /// [`crate::redstone`]). Driven by [`WorldState::tick`].
    pub redstone: crate::redstone::RedstoneState,
    /// Feet positions of the players in this world, refreshed once per tick by
    /// `SessionManager::tick`. Pressure plates read this: nothing else tells the
    /// world that a player stepped onto one.
    player_positions: RwLock<Vec<(f32, f32, f32)>>,
    /// Per-column edit overlays. Values are `Arc` so the random-tick pass can
    /// snapshot a column in O(1); writers use copy-on-write
    /// ([`Arc::make_mut`]), so a snapshot taken during a pass stays consistent.
    edits: RwLock<HashMap<(i32, i32), Arc<BlockEdits>>>,
    /// Fase 1 (Bedrock -> Java): changes waiting for
    /// `world.take_block_changes`. Drained by that host call (1x/tick from
    /// the Java bridge); no other consumer drains it.
    block_changes: RwLock<VecDeque<BlockChange>>,
    /// Fase 2 (Java -> Bedrock): the same edits, waiting for
    /// `SessionManager::poll_block_changes` to turn them into
    /// `UpdateBlockPacket`s. Kept separate from `block_changes` so the two
    /// consumers never steal changes from each other: Java draining its
    /// queue must not starve Bedrock, and Bedrock polling must not starve
    /// Java.
    broadcast_changes: RwLock<VecDeque<BlockChange>>,
    /// Serialized [`WorldState::chunk_payload`] results, shared by every
    /// consumer (Bedrock streaming via rayon, `world.get_chunk` host call
    /// in the PHP tick). Without this cache every request regenerated
    /// the column from scratch (~20 ms per chunk in the generator),
    /// collapsing TPS during the initial 169-column load. Invalidated
    /// per-chunk by [`WorldState::set_block`].
    chunk_cache: Mutex<ChunkPayloadCache>,
    /// Per-column edit counter for the converted Java payloads cached by
    /// `CoreHooks` (legacy/modern): bumped by [`WorldState::set_block`]
    /// so a stale converted payload is never served after an edit. The
    /// background chunk prefetch (main.rs) also reads it to skip columns
    /// whose payloads are already warm.
    chunk_versions: Mutex<HashMap<(i32, i32), u64>>,
    /// `World::$entities`: every live entity, keyed by `Entity::getId()`.
    /// Translated from `World::addEntity()`/`removeEntity()`; the tick is
    /// [`WorldState::tick_entities`].
    entities: RwLock<HashMap<i64, crate::entity::Entity>>,
    /// Next `Entity::getId()` (`nextEntityId`).
    next_entity_id: std::sync::atomic::AtomicI64,
    /// Actor events (`ActorEventPacket`: hurt/death animations) produced by
    /// the entity tick. Drained once per tick by `SessionManager`.
    pending_entity_events: Mutex<Vec<EntityEvent>>,
    /// `Entity::$effects`: active status effects per entity id. Kept next to
    /// the entity map (instead of inside `Entity`) so every entity constructor
    /// keeps its shape; removed together with the entity.
    entity_effects: Mutex<HashMap<i64, Vec<ActiveEffect>>>,
    /// Ids of the entities created since the last tick (`EntitySpawnEvent`).
    pending_entity_spawns: Mutex<Vec<i64>>,
    /// Entities closed while still alive (`EntityDespawnEvent`).
    pending_entity_despawns: Mutex<Vec<crate::entity::Entity>>,
    /// Ids of the entities whose `Living::onDeath()` ran this tick.
    pending_entity_deaths: Mutex<Vec<i64>>,
    /// Projectile segments traced this tick. `SessionManager` resolves them
    /// against players (the world does not know sessions) and applies damage.
    pending_projectile_hits: Mutex<Vec<ProjectileHit>>,
    pending_projectile_damage: Mutex<Vec<PendingProjectileDamage>>,
    /// Explosions (primed TNT) queued by the tick, with their block damage
    /// already applied; `SessionManager` applies player damage.
    pending_explosions: Mutex<Vec<Explosion>>,
    /// `LevelEventPacket`s produced by the world itself (PM
    /// `World::addSound()` with a level-event sound, e.g. `AnvilFallSound`):
    /// `(event id, event data, x, y, z)`. Drained once per tick by
    /// `SessionManager`.
    pending_level_events: Mutex<Vec<(i32, i32, i32, i32, i32)>>,
    /// `(event id, event data, x, y, z)` with fractional coordinates
    /// (`World::addParticle()`), drained once per tick.
    pending_level_events_at: Mutex<Vec<(i32, i32, f32, f32, f32)>>,
    /// PHP `World::addSound()`/`addParticle()` effects waiting for their
    /// `WorldSoundEvent`/`WorldParticleEvent` dispatch (`SessionManager` tick).
    pending_world_effects: Mutex<Vec<PendingWorldEffect>>,
    /// World events queued by a world mutation (`WorldSaveEvent`,
    /// `SpawnChangeEvent`, `WorldDifficultyChangeEvent`). Drained once per tick
    /// by `SessionManager`, which owns the plugin manager.
    pending_world_events: Mutex<Vec<PendingWorldEvent>>,
    /// Synchronous block-event hook (`BlockSpreadEvent`/`BlockFormEvent`),
    /// installed by `SessionManager`; `None` means no plugins.
    block_events: RwLock<Option<Arc<dyn BlockEventHook>>>,
    /// In-memory [`Chunk`] per column, shared by every
    /// [`WorldState::get_chunk_cached`] caller (Java bridge chunk build,
    /// background prefetch in main.rs). `get_chunk` still regenerates on
    /// every call for legacy callers; the cached variant serves the
    /// ~18 ms generator work from memory and is what the Java tick now
    /// consumes after the prefetch warms it. Invalidated per-chunk by
    /// [`WorldState::set_block`].
    chunk_object_cache: Mutex<ChunkObjectCache>,
    /// Columns the random-tick pass reads ([`WorldState::random_tick`]): every
    /// column the server has built for a client, shared as `Arc` so
    /// remembering one is a refcount bump instead of a copy.
    ///
    /// Deliberately **not** invalidated by [`WorldState::set_block`] (unlike
    /// `chunk_object_cache`): [`WorldState::loaded_block_at`] overlays the
    /// edit map on the base terrain, so a stale base column still answers
    /// correctly and keeps ticking after somebody builds on it.
    loaded_columns: Mutex<LoadedColumnCache>,
    /// `randomTickSpeed`: block positions sampled per 16x16x16 section per
    /// tick (0 disables random ticks).
    random_tick_speed: AtomicI32,
    /// Seed source for the random-tick sampler, bumped once per pass.
    random_tick_seed: std::sync::atomic::AtomicU64,
}

/// Maximum entities simulated at once: a griefing sand column or an item
/// explosion must not grow `World::$entities` without bound.
pub const MAX_ENTITIES: usize = 512;

/// One `ActorEventPacket` the tick wants broadcast to the entity's viewers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityEvent {
    /// `Entity::getId()`: the key of the entity-view map.
    pub unique_id: i64,
    /// `Entity::getId()` as the packet's actor runtime id.
    pub runtime_id: u64,
    /// `ActorEvent` constant (`EVENT_HURT`, `EVENT_DEATH_ANIMATION`, ...).
    pub event_id: u8,
    pub event_data: i32,
}

/// One active status effect on an entity, port of
/// `pocketmine\entity\effect\EffectInstance` (`EffectCollection` per entity).
///
/// `id` is the Bedrock effect id (`EffectIdMap`): 1 speed, 2 slowness,
/// 3 haste, ... 30 darkness. PocketMine keys the collection by effect type, so
/// adding the same id again replaces the previous instance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveEffect {
    pub id: u32,
    /// `EffectInstance::getAmplifier()`; level is `amplifier + 1`.
    pub amplifier: u8,
    /// `EffectInstance::getDuration()` in ticks (0 means expired).
    pub duration: i32,
    /// `EffectInstance::isVisible()`.
    pub visible: bool,
    /// `EffectInstance::isAmbient()`.
    pub ambient: bool,
    /// `EffectInstance`'s `infinite` flag: the duration never runs out.
    pub infinite: bool,
}

impl ActiveEffect {
    pub fn new(id: u32, duration: i32, amplifier: u8) -> Self {
        Self {
            id,
            amplifier,
            duration: duration.max(0),
            visible: true,
            ambient: false,
            infinite: false,
        }
    }
}

/// A projectile hit on a mob waiting for the plugin decision
/// (`EntityDamageByEntityEvent`).
///
/// Applying the damage inside the entity tick would make cancellation
/// impossible, so the world only records it and the session manager resolves
/// it at the start of the next tick, before death and loot.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingProjectileDamage {
    pub unique_id: i64,
    pub runtime_id: u64,
    pub damage: f32,
    /// `Projectile::getOwningEntityId()` of the shooter.
    pub owner_runtime_id: u64,
    /// The projectile itself, captured before `despawns_on_hit` removes it:
    /// it is the `child` of `EntityDamageByChildEntityEvent`.
    pub child: Option<crate::entity::Entity>,
}

/// One projectile movement segment, from `Projectile::move()`. The world
/// cannot see players, so the segment is handed to `SessionManager`, which
/// checks it against every joined player.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectileHit {
    pub unique_id: i64,
    pub runtime_id: u64,
    pub from: Vector3f,
    pub to: Vector3f,
    /// Damage for a direct hit (`Projectile::getResultDamage()`).
    pub damage: f32,
    /// Shooter name; `""` when the projectile has no owner.
    pub owner_runtime_id: u64,
    /// `Entity::$ticksLived`: self-collision is ignored for the first 5 ticks.
    pub ticks_lived: u32,
    /// `Projectile::despawnsOnEntityHit()`: arrows stay, the rest vanish.
    pub despawns_on_hit: bool,
}

/// A primed TNT explosion queued by the entity tick.
///
/// The affected blocks are collected but **not** destroyed: `SessionManager`
/// fires `EntityExplodeEvent` first and then applies the surviving list, so a
/// listener can cancel the blast or edit the blocks/yield. The radius drives the
/// player damage applied in the same pass.
#[derive(Debug, Clone, PartialEq)]
pub struct Explosion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub radius: f32,
    /// `PrimedTNT` snapshot (`EntityExplodeEvent::getEntity()`), captured before
    /// the entity leaves the world.
    pub entity: Option<serde_json::Value>,
    /// Blocks inside the blast (`EntityExplodeEvent::getBlockList()`), already
    /// filtered by blast resistance. `SessionManager` destroys them after the
    /// event so a listener can cancel or edit the list.
    pub blocks: Vec<(i32, i32, i32, u32)>,
}

/// Fires the block events the world tick produces (`BlockSpreadEvent` from the
/// fluid/random-tick pass and `BlockFormEvent` from liquid collisions).
///
/// The world has no plugin manager: `SessionManager` installs itself as the
/// hook. Every call site invokes it **before** applying the change and holds no
/// world lock, so a listener can call back into the world.
pub trait BlockEventHook: Send + Sync {
    /// `BlockSpreadEvent`; false when a listener cancelled the spread.
    fn spread(&self, x: i32, y: i32, z: i32, block_id: u32, source_id: u32, new_id: u32) -> bool;
    /// `BlockFormEvent`; false when a listener cancelled the change.
    fn form(&self, x: i32, y: i32, z: i32, block_id: u32, causing_id: u32, new_id: u32) -> bool;
}

/// A falling block that reached the ground this tick, used by
/// `SessionManager` for `FallingBlock::onHitGround()` (anvil damage to
/// entities).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LandedBlock {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: u32,
    /// `Entity::getFallDistance()` of the falling block.
    pub fall_distance: f32,
}

/// FIFO cache of serialized chunk payloads.
struct ChunkPayloadCache {
    entries: HashMap<(i32, i32), (u32, Vec<u8>)>,
    order: VecDeque<(i32, i32)>,
}

/// Maximum cached chunk payloads (covers a full `view-distance: 6` area
/// of 169 columns per player with headroom for two players).
const CHUNK_PAYLOAD_CACHE_CAP: usize = 1_152;

/// FIFO cache of in-memory [`Chunk`] objects (see
/// [`WorldState::get_chunk_cached`]). Paletted chunks are small, so
/// they stay far cheaper than regeneration stalls on the Java tick.
/// Sized for two full view areas at radius 10 (441 columns each):
/// at 512 the moving window's oldest (nearest-inserted) columns were
/// FIFO-evicted while still on screen, forcing ~18 ms regenerations
/// on the tick path.
struct ChunkObjectCache {
    entries: HashMap<(i32, i32), Chunk>,
    order: VecDeque<(i32, i32)>,
}

/// Maximum cached chunk objects.
const CHUNK_OBJECT_CACHE_CAP: usize = 1024;

/// FIFO cache of the columns the random-tick pass reads.
///
/// Holds `Arc<Chunk>`, so eviction and re-insertion only move refcounts.
/// Sized for two full `view-distance: 16` view areas (33x33 columns each),
/// because the ticking set is a subset of what is streamed and the newest
/// insertion is not necessarily the nearest column.
struct LoadedColumnCache {
    entries: HashMap<(i32, i32), Arc<Chunk>>,
    order: VecDeque<(i32, i32)>,
}

/// Maximum cached ticking columns (see [`LoadedColumnCache`]).
const LOADED_COLUMN_CACHE_CAP: usize = 2048;

/// A `World::addSound()`/`World::addParticle()` call queued by the PHP host.
///
/// The PHP runtime is inside the host call, so the bridge cannot dispatch
/// `WorldSoundEvent`/`WorldParticleEvent` there. `SessionManager` fires them on
/// the next tick and only then broadcasts, so a listener can cancel the packet
/// or change the sound/particle and the recipients. Both events are cancellable
/// in PM5, unlike the deferred world events below.
#[derive(Debug, Clone, PartialEq)]
pub enum PendingWorldEffect {
    Sound {
        /// `LevelSoundEvent` name, when the sound is a level sound.
        sound: Option<String>,
        /// `LevelEvent` id, when the sound is a level event.
        event_id: Option<i32>,
        extra_data: i32,
        x: f32,
        y: f32,
        z: f32,
        /// Lower-case player names; empty means every viewer of the chunk.
        recipients: Vec<String>,
    },
    Particle {
        event_id: i32,
        event_data: i32,
        x: f32,
        y: f32,
        z: f32,
        recipients: Vec<String>,
    },
}

/// A world event queued by the mutation that causes it.
///
/// PocketMine fires these inside `World`, which has the plugin manager; the
/// [`WorldState`] does not, so the mutation queues the event and
/// `SessionManager` dispatches it on the next tick. All three are
/// non-cancellable, so deferring them cannot change the outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingWorldEvent {
    /// `SpawnChangeEvent` after `World::setSpawnLocation()`; the previous spawn.
    SpawnChanged { previous: (i32, i32, i32) },
    /// `WorldDifficultyChangeEvent` from `World::setDifficulty()`.
    DifficultyChanged { old: i32, new: i32 },
    /// `WorldSaveEvent` from `World::save()` (manual, `/save-all`, autosave).
    Saved,
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
            time_running: std::sync::atomic::AtomicBool::new(true),
            auto_save: std::sync::atomic::AtomicBool::new(true),
            auto_save_interval: std::sync::atomic::AtomicI32::new(6000),
            tick_count: std::sync::atomic::AtomicI32::new(0),
            scheduled_block_updates: Mutex::new(Vec::new()),
            fluid_updates: Mutex::new(HashMap::new()),
            scheduled_tick: AtomicI32::new(0),
            pending_sounds: Mutex::new(Vec::new()),
            pending_sounds_at: Mutex::new(Vec::new()),
            biome_overrides: RwLock::new(HashMap::new()),
            redstone: crate::redstone::RedstoneState::new(),
            player_positions: RwLock::new(Vec::new()),
            edits: RwLock::new(HashMap::new()),
            block_changes: RwLock::new(VecDeque::new()),
            broadcast_changes: RwLock::new(VecDeque::new()),
            chunk_cache: Mutex::new(ChunkPayloadCache {
                entries: HashMap::new(),
                order: VecDeque::new(),
            }),
            chunk_versions: Mutex::new(HashMap::new()),
            entities: RwLock::new(HashMap::new()),
            // Entity ids start above the session-derived player ids (small
            // ints `>= 1`) so an actor id is never ambiguous.
            next_entity_id: std::sync::atomic::AtomicI64::new(1_000_000),
            pending_entity_events: Mutex::new(Vec::new()),
            entity_effects: Mutex::new(HashMap::new()),
            pending_entity_spawns: Mutex::new(Vec::new()),
            pending_entity_despawns: Mutex::new(Vec::new()),
            pending_entity_deaths: Mutex::new(Vec::new()),
            pending_projectile_hits: Mutex::new(Vec::new()),
            pending_projectile_damage: Mutex::new(Vec::new()),
            pending_explosions: Mutex::new(Vec::new()),
            pending_level_events: Mutex::new(Vec::new()),
            pending_level_events_at: Mutex::new(Vec::new()),
            pending_world_effects: Mutex::new(Vec::new()),
            pending_world_events: Mutex::new(Vec::new()),
            block_events: RwLock::new(None),
            chunk_object_cache: Mutex::new(ChunkObjectCache {
                entries: HashMap::new(),
                order: VecDeque::new(),
            }),
            loaded_columns: Mutex::new(LoadedColumnCache {
                entries: HashMap::new(),
                order: VecDeque::new(),
            }),
            // `World::__construct()`: the Java default random tick speed.
            random_tick_speed: AtomicI32::new(crate::random_tick::DEFAULT_RANDOM_TICK_SPEED),
            random_tick_seed: std::sync::atomic::AtomicU64::new(0x5DEE_CE66_D1CE_4E5B),
        }
    }

    pub fn generator(&self) -> &Arc<dyn WorldGenerator> {
        &self.generator
    }

    /// Seed reported by `/seed`. Never hardcoded per caller: `session.rs`
    /// must use this (see `StartGame` wiring) instead of a literal.
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
        let previous = self.difficulty.swap(clamped, Ordering::Relaxed);
        // `World::setDifficulty()` fires the event on every call, even when the
        // value does not change.
        self.queue_world_event(PendingWorldEvent::DifficultyChanged {
            old: previous,
            new: clamped,
        });
        clamped
    }

    pub fn spawn(&self) -> (i32, i32, i32) {
        *self.spawn.read()
    }

    /// Moves the world spawn (`/setworldspawn`, `World::setSpawnLocation()`).
    /// Queues `SpawnChangeEvent` with the previous spawn, like PM.
    pub fn set_spawn(&self, x: i32, y: i32, z: i32) {
        let previous = std::mem::replace(&mut *self.spawn.write(), (x, y, z));
        // `World::setSpawnLocation()` fires the event on every call, even when
        // the coordinates do not change: never dedupe here.
        self.queue_world_event(PendingWorldEvent::SpawnChanged { previous });
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

    /// `World::startTime()` (`/time start`).
    pub fn start_time(&self) {
        self.time_running.store(true, Ordering::Relaxed);
    }

    /// `World::stopTime()` (`/time stop`).
    pub fn stop_time(&self) {
        self.time_running.store(false, Ordering::Relaxed);
    }

    /// Advances the world clock by one tick (`World::doTick` time part).
    ///
    /// Called once per server tick from the main tick loop
    /// (`SessionManager::tick_world_time`): when `time_running` is false
    /// (`/time stop`) the time is returned unchanged, otherwise it is
    /// incremented by one and wrapped to a full day. Without this the
    /// sun never moves after joining: Bedrock and Java clients only
    /// received `SetTime`/`update_time` at join and on `/time set`, so
    /// after hours both platforms were frozen at the join time and
    /// diverged from each other as soon as one side ran `/time`.
    pub fn tick(&self) -> i32 {
        // Scheduled block updates count ticks, not world time, so the queue
        // keeps running under `/time stop` (PM `World::doTick` order:
        // scheduled updates first, then the clock).
        let now = self.scheduled_tick.fetch_add(1, Ordering::Relaxed) + 1;
        {
            let _scope = crate::profiler::scope("world.scheduled");
            self.run_scheduled_block_updates(now);
        }
        {
            let _scope = crate::profiler::scope("world.fluid");
            self.run_fluid_updates(now);
        }
        // Redstone runs after the world's scheduled updates (a button release
        // must wake the circuit it feeds) and drains its own phases: scheduled
        // component ticks, then block updates, then entity contacts.
        {
            let _scope = crate::profiler::scope("world.redstone");
            crate::redstone::tick(self, now);
        }
        if self.is_time_running() {
            self.set_time(self.time().saturating_add(1))
        } else {
            self.time()
        }
    }

    /// `World::getCollisionBlocks()`: solid blocks whose collision box
    /// intersects `bb`, in coordinate order (the core models every solid block
    /// as a full cube; PM uses the per-block shape list).
    pub fn collision_blocks(&self, bb: &crate::entity::Aabb) -> Vec<(i32, i32, i32, u32)> {
        let mut out = Vec::new();
        for y in bb.min_y.floor() as i32..=bb.max_y.floor() as i32 {
            for z in bb.min_z.floor() as i32..=bb.max_z.floor() as i32 {
                for x in bb.min_x.floor() as i32..=bb.max_x.floor() as i32 {
                    let id = self.block_at(x, y, z);
                    if id == bedrock_world::block_id::AIR {
                        continue;
                    }
                    let Some(name) = bedrock_world::block_states::state_name(id) else {
                        continue;
                    };
                    if bedrock_world::block_states::has_collision(name) {
                        out.push((x, y, z, id));
                    }
                }
            }
        }
        out
    }

    /// `World::notifyNeighbourBlockUpdate()`: run the connection/physics
    /// cascade around `(x, y, z)` without editing the block itself.
    pub fn notify_neighbour_block_update(&self, x: i32, y: i32, z: i32) {
        let mut changes: HashMap<(i32, i32, i32), Edit> = HashMap::with_capacity(8);
        self.propagate_connections(x, y, z, &mut changes);
        self.propagate_physics(x, y, z, &mut changes);
        self.schedule_fluid_neighbours(x, y, z);
        self.flush_changes(changes);
        crate::redstone::on_block_changed(self, (x, y, z));
    }

    /// PM `World::scheduleDelayedBlockUpdate($pos, $delay)`: `delay` ticks from
    /// now the position is set back to `block_id`, and `sound` (if any) is
    /// queued for the viewers of that chunk.
    pub fn schedule_delayed_block_update(
        &self,
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
        delay: i32,
        sound: Option<&str>,
    ) {
        let due = self.scheduled_tick.load(Ordering::Relaxed) + delay.max(1);
        self.scheduled_block_updates.lock().push((
            due,
            (x, y, z),
            block_id,
            sound.map(str::to_string),
        ));
    }

    /// Applies every scheduled update that is due (PM `World::doTick` draining
    /// `scheduledBlockUpdateQueue`). Each entry goes through [`Self::set_block`],
    /// so viewers get the change and the Java queue sees it too.
    fn run_scheduled_block_updates(&self, now: i32) {
        let ready = {
            let mut queue = self.scheduled_block_updates.lock();
            let mut ready = Vec::new();
            let mut index = 0;
            while index < queue.len() {
                if queue[index].0 <= now {
                    ready.push(queue.remove(index));
                } else {
                    index += 1;
                }
            }
            ready
        };
        for (_, (x, y, z), block_id, sound) in ready {
            self.set_block(x, y, z, block_id);
            if let Some(sound) = sound {
                self.pending_sounds.lock().push((sound, -1, x, y, z));
            }
        }
    }

    /// PM `World::doTick()` -> `Liquid::onScheduledUpdate()` for every liquid
    /// cell whose tick is due.
    ///
    /// Each update goes through [`WorldState::set_block`], so viewers get the
    /// new level (or the air where the flow dried up) in the same tick, and the
    /// cells it touches are scheduled again (`onNearbyBlockChange`) — the flow
    /// spreads one block per `tickRate()`, like the reference.
    fn run_fluid_updates(&self, now: i32) {
        /// A large pool must not drain the whole tick budget; the rest stays
        /// queued and runs on the following ticks.
        const MAX_FLUID_UPDATES_PER_TICK: usize = 256;

        let ready: Vec<(i32, i32, i32)> = {
            let mut queue = self.fluid_updates.lock();
            let mut ready = Vec::new();
            queue.retain(|position, due| {
                if *due <= now {
                    ready.push(*position);
                    false
                } else {
                    true
                }
            });
            ready
        };
        for (x, y, z) in ready.into_iter().take(MAX_FLUID_UPDATES_PER_TICK) {
            crate::fluid::on_scheduled_update(self, x, y, z);
        }
    }

    /// Queues the changed cell and its six neighbours for a liquid tick.
    ///
    /// This is PocketMine's `onNearbyBlockChange()` for liquids: every block
    /// update wakes the liquid around it, which recomputes its level (a broken
    /// source drains the flow away) or spreads further.
    fn schedule_fluid_neighbours(&self, x: i32, y: i32, z: i32) {
        let now = self.scheduled_tick.load(Ordering::Relaxed);
        let mut positions: [(i32, i32, i32); 7] = [(x, y, z); 7];
        for (index, face) in crate::physics::ALL_FACES.iter().enumerate() {
            let (dx, dy, dz) = crate::physics::offset(*face);
            positions[index + 1] = (x + dx, y + dy, z + dz);
        }
        let mut queue = self.fluid_updates.lock();
        for (bx, by, bz) in positions {
            let Some(liquid) = crate::fluid::Liquid::from_state(self.block_at(bx, by, bz)) else {
                continue;
            };
            let due = now + liquid.tick_rate();
            queue
                .entry((bx, by, bz))
                .and_modify(|existing| *existing = (*existing).min(due))
                .or_insert(due);
        }
    }

    /// Number of liquid cells waiting for their scheduled tick (tests).
    pub fn fluid_update_count(&self) -> usize {
        self.fluid_updates.lock().len()
    }

    /// Sounds queued by scheduled block updates, in firing order.
    pub fn take_pending_sounds(&self) -> Vec<(String, i32, i32, i32, i32)> {
        std::mem::take(&mut *self.pending_sounds.lock())
    }

    /// `World::addSound()`: queues a `LevelSoundEventPacket` for the next tick
    /// (`extra_data` is the block runtime id for block sounds).
    pub fn add_sound(&self, sound: impl Into<String>, extra_data: i32, x: i32, y: i32, z: i32) {
        self.pending_sounds
            .lock()
            .push((sound.into(), extra_data, x, y, z));
    }

    /// `World::addSound()` at an exact position (PHP `Vector3`): the packet is
    /// emitted at `x`/`y`/`z` without the block-centre offset.
    pub fn add_sound_at(&self, sound: impl Into<String>, extra_data: i32, x: f32, y: f32, z: f32) {
        self.pending_sounds_at
            .lock()
            .push((sound.into(), extra_data, x, y, z));
    }

    /// Drains the exact-position sounds queued since the last tick.
    pub fn take_pending_sounds_at(&self) -> Vec<(String, i32, f32, f32, f32)> {
        std::mem::take(&mut *self.pending_sounds_at.lock())
    }

    /// `World::getBiomeId()`: the per-column override, otherwise the biome the
    /// generator wrote for that column (the network payload's biome palette).
    ///
    /// `y` is accepted for signature parity but ignored: the payload only
    /// carries one biome per subchunk and this port stores one per column.
    pub fn biome_at(&self, x: i32, _y: i32, z: i32) -> u8 {
        if let Some(&biome) = self.biome_overrides.read().get(&(x, z)) {
            return biome;
        }
        let chunk_x = x >> 4;
        let chunk_z = z >> 4;
        let cached = self
            .loaded_columns
            .lock()
            .entries
            .get(&(chunk_x, chunk_z))
            .cloned();
        let chunk = match cached {
            Some(chunk) => chunk,
            None => self.build_chunk(chunk_x, chunk_z),
        };
        chunk.biomes[(x & 15) as usize * 16 + (z & 15) as usize]
    }

    /// `World::setBiomeId()`: stores the per-column override and invalidates
    /// every cached representation of the chunk so the next payload carries
    /// it. Returns false when the biome did not change.
    pub fn set_biome(&self, x: i32, y: i32, z: i32, biome_id: u8) -> bool {
        if !(WORLD_MIN_Y..WORLD_MAX_Y).contains(&y) {
            return false;
        }
        let changed = {
            let mut overrides = self.biome_overrides.write();
            if overrides.get(&(x, z)) == Some(&biome_id) {
                false
            } else {
                overrides.insert((x, z), biome_id);
                true
            }
        };
        if changed {
            let chunk = (x >> 4, z >> 4);
            self.chunk_cache.lock().entries.remove(&chunk);
            self.chunk_object_cache.lock().entries.remove(&chunk);
            self.loaded_columns.lock().entries.remove(&chunk);
            *self.chunk_versions.lock().entry(chunk).or_insert(0) += 1;
        }
        changed
    }

    /// PM `World::useItemOn()` interaction half: `$blockClicked->onInteract()`.
    /// Buttons press (and schedule their own release), levers toggle, fence
    /// gates, doors and trapdoors open/close and the container blocks resolve
    /// their window. Returns the interaction so the caller can broadcast the
    /// matching sound (`power.on`, `power.off`, `SOUND_DOOR`) or open the
    /// container; `None` when the block has no `onInteract()` and the click
    /// must fall through to item placement.
    pub fn interact_block(
        &self,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<bedrock_world::block_states::Interaction> {
        let current = self.block_at(x, y, z);
        let interaction = bedrock_world::block_states::interact(current)?;
        if interaction.new_state != current {
            self.set_block(x, y, z, interaction.new_state);
            // A door is two blocks sharing `open_bit` exactly like a redstone
            // update (`Door::onInteract` sets both halves), so the other half
            // follows the clicked one.
            if interaction.door_sound {
                self.mirror_door_half(x, y, z, interaction.new_state);
            }
            if let Some(delay) = interaction.reset_after_ticks {
                // `Button::onScheduledUpdate()` plays `RedstonePowerOffSound`.
                self.schedule_delayed_block_update(
                    x,
                    y,
                    z,
                    current,
                    delay,
                    Some(bedrock_protocol::packets::level_sound_event::LEVEL_SOUND_POWER_OFF),
                );
            }
        }
        Some(interaction)
    }

    /// Copies `open_bit` from a door half to the other half of the same door
    /// (`Door::onInteract` / `DoorTrait`): silently, because vanilla plays the
    /// door sound once.
    fn mirror_door_half(&self, x: i32, y: i32, z: i32, state: u32) {
        use bedrock_world::block_states;
        let Some(name) = block_states::state_name(state) else { return };
        if !name.ends_with("_door") {
            return;
        }
        let Some(open) = block_states::bool_property(state, "open_bit") else {
            return;
        };
        let value = if open { "true" } else { "false" };
        for dy in [1, -1] {
            let (other_x, other_y, other_z) = (x, y + dy, z);
            let other = self.block_at(other_x, other_y, other_z);
            if block_states::state_name(other) != Some(name) {
                continue;
            }
            if let Some(updated) = block_states::with_property(other, "open_bit", value) {
                self.set_block(other_x, other_y, other_z, updated);
            }
        }
    }

    /// Slots stored for the block container at `pos`, padded to `len`.
    ///
    /// Container contents live in [`RedstoneState`](crate::redstone::RedstoneState)
    /// because this server has no block entities, so a dispenser fires from
    /// the very same slots a player sees in its window.
    pub fn container_slots(&self, pos: (i32, i32, i32), len: usize) -> Vec<ItemStack> {
        self.redstone.container_with_len(pos, len)
    }

    /// Stores the slots of the block container at `pos` (`Chest::onClose`,
    /// `Dispenser::onClose`, ...: the contents stay in the block).
    pub fn set_container_slots(&self, pos: (i32, i32, i32), slots: Vec<ItemStack>) {
        self.redstone.set_container(pos, slots);
    }

    /// Number of block updates waiting in the scheduled queue (tests).
    pub fn scheduled_block_update_count(&self) -> usize {
        self.scheduled_block_updates.lock().len()
    }

    pub fn is_time_running(&self) -> bool {
        self.time_running.load(Ordering::Relaxed)
    }

    pub fn set_auto_save(&self, enabled: bool) {
        self.auto_save.store(enabled, Ordering::Relaxed);
    }

    /// `WorldManager::getAutoSave()` surface for PHP.
    pub fn auto_save(&self) -> bool {
        self.auto_save.load(Ordering::Relaxed)
    }

    /// `WorldManager::getAutoSaveInterval()` (ticks, default 6000 = 300s).
    pub fn auto_save_interval(&self) -> i32 {
        self.auto_save_interval.load(Ordering::Relaxed)
    }

    /// `WorldManager::setAutoSaveInterval()`; ignora valores <= 0.
    pub fn set_auto_save_interval(&self, ticks: i32) {
        if ticks > 0 {
            self.auto_save_interval.store(ticks, Ordering::Relaxed);
        }
    }

    /// Tick counter the redstone scheduler counts against. Advances with
    /// [`WorldState::tick`], independently of `/time stop`.
    pub fn redstone_tick(&self) -> i32 {
        self.scheduled_tick.load(Ordering::Relaxed)
    }

    /// Replaces the feet positions pressure plates test against. Called once
    /// per tick by `SessionManager::tick` with every joined player.
    pub fn set_player_positions(&self, positions: Vec<(f32, f32, f32)>) {
        *self.player_positions.write() = positions;
    }

    pub fn player_positions(&self) -> Vec<(f32, f32, f32)> {
        self.player_positions.read().clone()
    }

    /// Game time in ticks (`World::getTick()`): every tick since the world
    /// loaded, unlike [`Self::time`], which is the time of day (`0..24000`).
    ///
    /// Java's `set_time` packet carries it as the "world age" and the client
    /// draws the clouds from it (`CloudRenderer`), so it must keep growing.
    pub fn game_time(&self) -> i32 {
        self.scheduled_tick.load(Ordering::Relaxed)
    }

    /// Current tick counter (set by `WorldManager::tick()` via PHP).
    pub fn tick_count(&self) -> i32 {
        self.tick_count.load(Ordering::Relaxed)
    }

    /// Records a tick from `World::doTick()`; no game-logic beyond counting.
    pub fn record_tick(&self, tick: i32) {
        self.tick_count.store(tick, Ordering::Relaxed);
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
    ///
    /// Every successful edit is enqueued for both sync consumers
    /// (Fase 1/2), regardless of origin: Bedrock break/place
    /// (`handle_block_actions` -> `PluginAction::SetBlock` ->
    /// `apply_actions`), a plugin/`world.set_block` host call, or a core
    /// command — all funnel through this method. A no-change call enqueues
    /// nothing, so the queues only ever carry real changes.
    pub fn set_block(&self, x: i32, y: i32, z: i32, block_id: u32) -> bool {
        self.set_block_with(x, y, z, block_id, true)
    }

    /// [`WorldState::set_block`] with the block-physics cascade optional.
    ///
    /// The falling-block simulation moves one block down each tick; it must
    /// not re-run the cascade for the cell it is moving (it would start a
    /// second fall from the new cell), so it passes `physics = false`.
    fn set_block_with(&self, x: i32, y: i32, z: i32, block_id: u32, physics: bool) -> bool {
        if !(WORLD_MIN_Y..WORLD_MAX_Y).contains(&y) {
            return false;
        }
        // PocketMine records the touched cells in `changedBlocks`, a set keyed
        // by position that is flushed to clients once per tick, so a block is
        // broadcast **once with its final state**. Enqueueing every write of the
        // neighbour cascade made a fence that connects on several sides flicker
        // (raw placement -> correction -> neighbours fighting over the arms).
        let mut changes: HashMap<(i32, i32, i32), Edit> = HashMap::with_capacity(8);
        if !self.write_block(x, y, z, block_id, &mut changes) {
            return false;
        }
        self.propagate_connections(x, y, z, &mut changes);
        if physics {
            self.propagate_physics(x, y, z, &mut changes);
        }
        // `Liquid::onNearbyBlockChange()`: wake the liquid around the edit so
        // it re-evaluates its level or keeps spreading.
        self.schedule_fluid_neighbours(x, y, z);
        self.flush_changes(changes);
        // `Lava::checkForHarden()` arrives with the block update itself, not
        // with the flow tick: water meeting lava hardens immediately. Runs
        // after the flush so the hardened state is what viewers end up with.
        crate::fluid::harden_around(self, x, y, z);
        // `World::internalNotifyNeighbourBlockUpdate()` for redstone: the edit
        // wakes the components around it, which re-read their inputs in the
        // next block-update phase (dust immediately, diodes after their delay).
        crate::redstone::on_block_changed(self, (x, y, z));
        true
    }

    /// Writes one edit without broadcasting it: the change is recorded in
    /// `changes` (first previous id + last state) for [`WorldState::flush_changes`].
    fn write_block(
        &self,
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
        changes: &mut HashMap<(i32, i32, i32), Edit>,
    ) -> bool {
        if !(WORLD_MIN_Y..WORLD_MAX_Y).contains(&y) {
            return false;
        }

        let previous = self.block_at(x, y, z);
        if previous == block_id {
            return false;
        }

        {
            let mut edits = self.edits.write();
            Arc::make_mut(edits.entry((x >> 4, z >> 4)).or_default())
                .insert((x, y, z), block_id);
        }
        // The cached payload for this column no longer matches the world.
        self.chunk_cache.lock().entries.remove(&(x >> 4, z >> 4));
        // Same for the cached object the Java bridge converts.
        self.chunk_object_cache
            .lock()
            .entries
            .remove(&(x >> 4, z >> 4));
        // Converted Java payloads keyed by this version are now stale.
        *self
            .chunk_versions
            .lock()
            .entry((x >> 4, z >> 4))
            .or_insert(0) += 1;

        let entry = changes.entry((x, y, z)).or_insert((previous, block_id));
        entry.1 = block_id;
        true
    }

    /// Enqueues one [`BlockChange`] per touched cell, in coordinate order.
    ///
    /// This is PocketMine's `changedBlocks` flush: intermediate states of the
    /// neighbour cascade never reach clients, only the final one.
    fn flush_changes(&self, changes: HashMap<(i32, i32, i32), Edit>) {
        if changes.is_empty() {
            return;
        }
        let mut entries: Vec<((i32, i32, i32), Edit)> = changes.into_iter().collect();
        entries.sort_unstable_by_key(|(position, _)| *position);
        let mut java = self.block_changes.write();
        let mut broadcast = self.broadcast_changes.write();
        for ((x, y, z), (previous, block_id)) in entries {
            if previous == block_id {
                continue;
            }
            let change = BlockChange::with_previous(x, y, z, block_id, previous);
            push_capped(&mut java, change);
            push_capped(&mut broadcast, change);
            tracing::debug!("world.set_block: ({},{},{}) id={:#010X} enqueued", x, y, z, block_id);
        }
    }

    /// PocketMine's `World::internalNotifyNeighbourBlockUpdate()` cascade.
    ///
    /// `setBlockAt($update = true)` queues the block itself plus its six
    /// neighbours; in the tick each queued block runs `onNearbyBlockChange()`,,
    /// and connectables (`Fence`/`Thin` via `HorizontalConnectableTrait`,
    /// `Wall`) recompute their horizontal connections and re-set themselves when
    /// the state changed, which queues them again. Replicating it here means a
    /// fence placed next to another fence updates *both*: the placed one gets
    /// its connections and the existing one connects to it, and every state
    /// change is enqueued for Bedrock (`UpdateBlockPacket`) and Java
    /// (`block_change`) instead of only the original cell.
    ///
    /// Runs synchronously right after the edit (one tick earlier than PM, same
    /// observable result) and is bounded so a long fence line cannot loop.
    fn propagate_connections(
        &self,
        x: i32,
        y: i32,
        z: i32,
        changes: &mut HashMap<(i32, i32, i32), Edit>,
    ) {
        use bedrock_world::block_states;

        const NEIGHBOURS: [(i32, i32, i32); 6] = [
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, -1),
            (0, 0, 1),
            (-1, 0, 0),
            (1, 0, 0),
        ];
        /// A fence chain settles in two passes; this only bounds pathological
        /// layouts (the states are a pure function of the neighbours, so the
        /// loop reaches a fixed point).
        const MAX_PASSES: usize = 4;

        let mut frontier: Vec<(i32, i32, i32)> = Vec::with_capacity(32);
        frontier.push((x, y, z));
        for (dx, dy, dz) in NEIGHBOURS {
            frontier.push((x + dx, y + dy, z + dz));
        }

        for _ in 0..MAX_PASSES {
            // PM dedupes the neighbour queue with `neighbourBlockUpdateQueueIndex`:
            // each block is recomputed at most once per drain.
            let mut visited: HashSet<(i32, i32, i32)> = HashSet::with_capacity(frontier.len());
            let mut changed: Vec<(i32, i32, i32)> = Vec::new();
            for (bx, by, bz) in frontier.drain(..) {
                if !visited.insert((bx, by, bz)) || !(WORLD_MIN_Y..WORLD_MAX_Y).contains(&by) {
                    continue;
                }
                let current = self.block_at(bx, by, bz);
                let Some(updated) = block_states::recalculate_connections(current, |dx, dy, dz| {
                    self.block_at(bx + dx, by + dy, bz + dz)
                }) else {
                    continue;
                };
                if !self.write_block(bx, by, bz, updated, changes) {
                    continue;
                }
                tracing::debug!(
                    "world.connection_update: ({},{},{}) {:#010X} -> {:#010X}",
                    bx,
                    by,
                    bz,
                    current,
                    updated
                );
                changed.push((bx, by, bz));
            }
            if changed.is_empty() {
                return;
            }
            // `setBlockAt($update = true)` re-queues the block and its six
            // neighbours, which settles a chain (only the final states are
            // broadcast, so passes cannot be seen as flicker).
            for (bx, by, bz) in changed {
                frontier.push((bx, by, bz));
                for (dx, dy, dz) in NEIGHBOURS {
                    frontier.push((bx + dx, by + dy, bz + dz));
                }
            }
        }
    }

    /// Block physics cascade after an edit, translated from
    /// `FallableTrait::onNearbyBlockChange()` and
    /// `StaticSupportTrait::onNearbyBlockChange()`.
    ///
    /// PocketMine queues the changed block plus its six neighbours and runs
    /// `onNearbyBlockChange()` on each: a block whose support disappeared
    /// breaks (`World::useBreakOn`), and a fallable block whose cell below can
    /// be replaced leaves the world to fall. Every reaction queues the cells
    /// around it again, so a torch on a removed block breaks, a sand column
    /// collapses and a chest on top of collapsing sand falls too.
    ///
    /// Bounded like [`WorldState::propagate_connections`]: a pathological
    /// layout cannot make one edit walk the whole world.
    fn propagate_physics(
        &self,
        x: i32,
        y: i32,
        z: i32,
        changes: &mut HashMap<(i32, i32, i32), Edit>,
    ) {
        use bedrock_world::block_id::AIR;
        use bedrock_world::block_states;

        /// One edit can collapse a column, but never an unbounded region.
        const MAX_PHYSICS_UPDATES: usize = 256;

        let mut frontier: VecDeque<(i32, i32, i32)> = VecDeque::with_capacity(16);
        frontier.push_back((x, y, z));
        for face in crate::physics::ALL_FACES {
            let (dx, dy, dz) = crate::physics::offset(face);
            frontier.push_back((x + dx, y + dy, z + dz));
        }

        let mut visited: HashSet<(i32, i32, i32)> = HashSet::with_capacity(16);
        let mut budget = MAX_PHYSICS_UPDATES;
        while let Some((bx, by, bz)) = frontier.pop_front() {
            if budget == 0 {
                break;
            }
            if !visited.insert((bx, by, bz)) || !(WORLD_MIN_Y..WORLD_MAX_Y).contains(&by) {
                continue;
            }
            budget -= 1;
            let id = self.block_at(bx, by, bz);
            if id == AIR {
                continue;
            }
            let Some(name) = block_states::state_name(id) else {
                continue;
            };

            // `FallableTrait::onNearbyBlockChange()`: the block below can be
            // replaced, so this block leaves the world and falls.
            if crate::physics::is_fallable(name) {
                let below = self.block_at(bx, by - 1, bz);
                let can_fall = block_states::state_name(below)
                    .is_some_and(crate::physics::is_replaceable);
                if can_fall && self.write_block(bx, by, bz, AIR, changes) {
                    self.start_falling(bx, by, bz, id);
                    for face in crate::physics::ALL_FACES {
                        let (dx, dy, dz) = crate::physics::offset(face);
                        frontier.push_back((bx + dx, by + dy, bz + dz));
                    }
                }
                continue;
            }

            // `StaticSupportTrait::onNearbyBlockChange()`: support removed.
            if crate::physics::needs_support(id)
                && !crate::physics::is_supported(id, |dx, dy, dz| {
                    self.block_at(bx + dx, by + dy, bz + dz)
                })
                && self.write_block(bx, by, bz, AIR, changes)
            {
                // `World::useBreakOn($this->position)`: the block is
                // destroyed, not silently swapped for air. The destroy event
                // is what makes viewers see the break particles and hear the
                // block's break sound (`BlockBreakParticle` ->
                // `LevelEventPacket(EVENT_PARTICLE_DESTROY)`); a bare
                // `UpdateBlockPacket` to air produces neither.
                self.add_level_event(
                    bedrock_protocol::packets::level_event::LEVEL_EVENT_PARTICLE_DESTROY_BLOCK,
                    id as i32,
                    bx,
                    by,
                    bz,
                );
                tracing::debug!("world.physics_break: ({},{},{}) {:#010X}", bx, by, bz, id);
                for face in crate::physics::ALL_FACES {
                    let (dx, dy, dz) = crate::physics::offset(face);
                    frontier.push_back((bx + dx, by + dy, bz + dz));
                }
            }
        }
    }

    /// `new FallingBlock(Location::fromObject($pos->add(0.5, 0, 0.5)), $block)`
    /// followed by `spawnToAll()`: the block leaves the world and an entity
    /// with gravity/drag falls in its place.
    pub fn start_falling(&self, x: i32, y: i32, z: i32, block_id: u32) {
        self.spawn_falling_block(x, y, z, block_id);
    }

    /// Next `(Entity::getId(), runtime id)` pair. Entity ids start above the
    /// small session-derived player ids so the two can never collide. PocketMine
    /// uses the same identifier for unique_id and runtime_id ($entity->getId()).
    pub fn next_entity_ids(&self) -> (i64, u64) {
        let id = self
            .next_entity_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (id, id as u64)
    }

    /// `World::addEntity()`: registers a live entity. Returns its id, or
    /// `None` when [`MAX_ENTITIES`] is reached (PocketMine's entity cap).
    pub fn spawn_entity(&self, entity: crate::entity::Entity) -> Option<i64> {
        let mut entities = self.entities.write();
        if entities.len() >= MAX_ENTITIES {
            return None;
        }
        let id = entity.unique_id;
        entities.insert(id, entity);
        drop(entities);
        // `World::addEntity()` -> `EntitySpawnEvent`, dispatched by the
        // session manager at the end of the tick (it owns the plugins).
        self.pending_entity_spawns.lock().push(id);
        Some(id)
    }

    /// `new FallingBlock(Position::fromObject($pos->add(0.5, 0, 0.5), ...), $block)`.
    pub fn spawn_falling_block(
        &self,
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
    ) -> Option<i64> {
        if self.entities.read().len() >= MAX_ENTITIES {
            return None;
        }
        let (unique_id, runtime_id) = self.next_entity_ids();
        self.spawn_entity(crate::entity::Entity::falling_block(
            unique_id,
            runtime_id,
            Vector3f::new(x as f32 + 0.5, y as f32, z as f32 + 0.5),
            block_id,
        ))
    }

    /// `World::dropItem($source, $item, $motion)`: spawns an `ItemEntity` with
    /// the default pickup delay and PocketMine's random upward motion.
    pub fn spawn_item(
        &self,
        position: Vector3f,
        item: ItemStack,
        motion: Option<Vector3f>,
        owner: &str,
    ) -> Option<i64> {
        if item.is_empty() || item.count == 0 {
            return None;
        }
        if self.entities.read().len() >= MAX_ENTITIES {
            return None;
        }
        let (unique_id, runtime_id) = self.next_entity_ids();
        // `$motion ?? new Vector3(lcg_value() * 0.2 - 0.1, 0.1, lcg_value() * 0.2 - 0.1)`.
        let motion = motion.unwrap_or_else(|| {
            Vector3f::new(
                pseudo_random(unique_id.wrapping_mul(3)) * 0.2 - 0.1,
                0.1,
                pseudo_random(unique_id.wrapping_mul(7)) * 0.2 - 0.1,
            )
        });
        self.spawn_entity(crate::entity::Entity::item(
            unique_id, runtime_id, position, item, motion, owner,
        ))
    }

    /// `new <MobClass>(Location::fromObject($pos, $this->getWorld()))`:
    /// spawns any mob from [`crate::entity_types`] (`/summon`, plugins, spawners).
    pub fn spawn_mob(&self, type_id: &str, position: Vector3f) -> Option<i64> {
        self.spawn_mob_configured(type_id, position, 0.0, 0.0, None)
    }

    /// `/summon` (`spawnPos`, `yRot`, `xRot`, `nameTag`): the same factory call
    /// plus the rotation and name tag the command resolved.
    pub fn spawn_mob_configured(
        &self,
        type_id: &str,
        position: Vector3f,
        yaw: f32,
        pitch: f32,
        name_tag: Option<&str>,
    ) -> Option<i64> {
        if self.entities.read().len() >= MAX_ENTITIES {
            return None;
        }
        let (unique_id, runtime_id) = self.next_entity_ids();
        let mut entity = crate::entity::Entity::mob(unique_id, runtime_id, type_id, position)?;
        entity.yaw = yaw.rem_euclid(360.0);
        entity.head_yaw = entity.yaw;
        entity.pitch = pitch.clamp(-90.0, 90.0);
        if let Some(name_tag) = name_tag.filter(|tag| !tag.is_empty()) {
            entity.name_tag = name_tag.to_string();
        }
        self.spawn_entity(entity)
    }

    /// `new Arrow($location, $shootingEntity, $critical)` and the other
    /// `Projectile` subclasses. `owner` is the shooter's name and
    /// `owner_runtime_id` its runtime id (0 when unknown).
    pub fn spawn_projectile(
        &self,
        kind: crate::entity::ProjectileKind,
        position: Vector3f,
        motion: Vector3f,
        owner: &str,
        owner_runtime_id: u64,
        damage: f32,
        critical: bool,
    ) -> Option<i64> {
        if self.entities.read().len() >= MAX_ENTITIES {
            return None;
        }
        let (unique_id, runtime_id) = self.next_entity_ids();
        self.spawn_entity(crate::entity::Entity::projectile(
            unique_id,
            runtime_id,
            kind,
            position,
            motion,
            owner,
            owner_runtime_id,
            damage,
            critical,
        ))
    }

    /// `new PrimedTNT($location)`: `fuse` ticks until the explosion.
    pub fn spawn_primed_tnt(&self, position: Vector3f, fuse: i32) -> Option<i64> {
        if self.entities.read().len() >= MAX_ENTITIES {
            return None;
        }
        let (unique_id, runtime_id) = self.next_entity_ids();
        self.spawn_entity(crate::entity::Entity::primed_tnt(
            unique_id, runtime_id, position, fuse,
        ))
    }

    /// `World::dropExperience($pos, $amount)`.
    pub fn spawn_experience_orb(&self, position: Vector3f, value: i32) -> Option<i64> {
        if value <= 0 || self.entities.read().len() >= MAX_ENTITIES {
            return None;
        }
        let (unique_id, runtime_id) = self.next_entity_ids();
        self.spawn_entity(crate::entity::Entity::experience_orb(
            unique_id, runtime_id, position, value,
        ))
    }

    /// `minecraft:behavior.look_at_player` pass over every entity, using the
    /// player snapshot published for pressure plates. Returns how many mobs
    /// changed their head yaw this tick.
    pub fn look_at_nearest_players(&self) -> usize {
        let players = self.player_positions();
        if players.is_empty() {
            return 0;
        }
        let mut entities = self.entities.write();
        let mut changed = 0;
        for entity in entities.values_mut() {
            if entity.mob_look_tick(&players) {
                changed += 1;
            }
        }
        changed
    }

    /// `Living::attack()` from a player hit (`InventoryTransaction`
    /// `USE_ITEM_ON_ENTITY` attack) or a projectile. Returns the entity's
    /// runtime id and whether the hit killed it, so the caller can broadcast
    /// `ActorEventPacket` and close the entity.
    pub fn damage_entity(
        &self,
        runtime_id: u64,
        damage: f32,
    ) -> Option<(i64, crate::entity::DamageOutcome)> {
        let mut entities = self.entities.write();
        let (unique_id, entity) = entities
            .iter_mut()
            .find(|(_, entity)| entity.runtime_id == runtime_id)?;
        let outcome = entity.damage(damage);
        if outcome == crate::entity::DamageOutcome::Ignored {
            return None;
        }
        let unique_id = *unique_id;
        let runtime_id = entity.runtime_id;
        let mut events = self.pending_entity_events.lock();
        events.push(EntityEvent {
            unique_id,
            runtime_id,
            event_id: if outcome == crate::entity::DamageOutcome::Killed {
                bedrock_protocol::packets::actor_event::EVENT_DEATH_ANIMATION
            } else {
                bedrock_protocol::packets::actor_event::EVENT_HURT
            },
            event_data: 0,
        });
        Some((unique_id, outcome))
    }

    /// Applies knockback to the entity with that runtime id (`Living::knockBack`
    /// from `EntityDamageByEntityEvent`).
    pub fn knock_back_entity(&self, runtime_id: u64, dx: f32, dz: f32, force: f32) {
        let mut entities = self.entities.write();
        if let Some(entity) = entities
            .values_mut()
            .find(|entity| entity.runtime_id == runtime_id)
        {
            entity.knock_back(dx, dz, force);
        }
    }

    /// Runtime id lookup (`World::getEntity($id)` for actor packets).
    pub fn entity_by_runtime(&self, runtime_id: u64) -> Option<crate::entity::Entity> {
        self.entities
            .read()
            .values()
            .find(|entity| entity.runtime_id == runtime_id)
            .cloned()
    }

    /// `World::getEntity($id)`: snapshot by `Entity::getId()`.
    ///
    /// Used by the plugin bridge (`entity.get` / `entity.list`), which speaks
    /// PocketMine ids rather than Bedrock runtime ids.
    pub fn entity_by_id(&self, unique_id: i64) -> Option<crate::entity::Entity> {
        self.entities.read().get(&unique_id).cloned()
    }

    /// `Entity::teleport($pos, $yaw, $pitch)` from a plugin.
    ///
    /// Returns `false` when the entity is no longer in the world. Rotation
    /// arguments are optional and keep the current value when `None`.
    pub fn set_entity_position(
        &self,
        unique_id: i64,
        position: Vector3f,
        yaw: Option<f32>,
        pitch: Option<f32>,
    ) -> bool {
        let mut entities = self.entities.write();
        let Some(entity) = entities.get_mut(&unique_id) else {
            return false;
        };
        entity.position = position;
        if let Some(yaw) = yaw {
            entity.yaw = yaw.rem_euclid(360.0);
            entity.head_yaw = entity.yaw;
        }
        if let Some(pitch) = pitch {
            entity.pitch = pitch.clamp(-90.0, 90.0);
        }
        true
    }

    /// `Living::setHealth($amount)`: clamps to `[0, maxHealth]` like
    /// PocketMine and returns the resulting `(health, max_health)`.
    ///
    /// `None` when the entity is gone or is not a `Living`.
    pub fn set_entity_health(&self, unique_id: i64, health: f32) -> Option<(f32, f32)> {
        let mut entities = self.entities.write();
        let entity = entities.get_mut(&unique_id)?;
        let crate::entity::EntityKind::Mob(mob) = &mut entity.kind else {
            return None;
        };
        mob.health = health.clamp(0.0, mob.max_health);
        if mob.health <= 0.0 {
            // `Living::setHealth()` triggers `onDeath()` on the next tick.
            mob.dead_ticks = 0;
        }
        Some((mob.health, mob.max_health))
    }

    /// `Entity::setMotion($motion)` from a plugin (`setMotion` returns
    /// `true` when the entity is still alive and in the world).
    pub fn set_entity_motion(&self, unique_id: i64, motion: Vector3f) -> bool {
        let mut entities = self.entities.write();
        let Some(entity) = entities.get_mut(&unique_id) else {
            return false;
        };
        entity.motion = motion;
        true
    }

    /// `Entity::setNameTag($name)` from a plugin.
    pub fn set_entity_name_tag(&self, unique_id: i64, name_tag: &str) -> bool {
        let mut entities = self.entities.write();
        let Some(entity) = entities.get_mut(&unique_id) else {
            return false;
        };
        entity.name_tag = name_tag.to_string();
        true
    }

    /// Drains the actor events queued by [`Self::damage_entity`].
    pub fn take_entity_events(&self) -> Vec<EntityEvent> {
        std::mem::take(&mut *self.pending_entity_events.lock())
    }

    /// Drains the entities created since the last tick (`EntitySpawnEvent`).
    pub fn take_entity_spawns(&self) -> Vec<i64> {
        std::mem::take(&mut *self.pending_entity_spawns.lock())
    }

    /// Drains the entities whose `Living::onDeath()` ran this tick
    /// (`EntityDeathEvent`).
    pub fn take_entity_deaths(&self) -> Vec<i64> {
        std::mem::take(&mut *self.pending_entity_deaths.lock())
    }

    /// Drains the projectile segments traced by the entity tick.
    pub fn take_projectile_hits(&self) -> Vec<ProjectileHit> {
        std::mem::take(&mut *self.pending_projectile_hits.lock())
    }

    /// Drains the projectile hits waiting for the plugin decision.
    pub fn take_projectile_damage(&self) -> Vec<PendingProjectileDamage> {
        std::mem::take(&mut *self.pending_projectile_damage.lock())
    }

    /// Drains the explosions produced by primed TNT.
    pub fn take_explosions(&self) -> Vec<Explosion> {
        std::mem::take(&mut *self.pending_explosions.lock())
    }

    /// `World::dropItem($pos, $block->asItem())` for a broken block, plus the
    /// contents of anything that holds an inventory (`InventoryHolder`).
    pub fn drop_block_item(&self, x: i32, y: i32, z: i32, block_id: u32, owner: &str) {
        if let Some(item) = crate::entity::block_item(block_id) {
            self.spawn_item(
                Vector3f::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5),
                item,
                None,
                owner,
            );
        }
        crate::redstone::drop_contents(self, (x, y, z), block_id, owner);
    }

    /// Removes an entity from the world (`Entity::close()` -> `World::removeEntity`).
    pub fn remove_entity(&self, id: i64) -> bool {
        let Some(entity) = self.entities.write().remove(&id) else {
            return false;
        };
        // `Entity::close()` drops the effect collection with the entity.
        self.entity_effects.lock().remove(&id);
        // `World::removeEntity()` on a live entity fires `EntityDespawnEvent`;
        // a dead one already fired `EntityDeathEvent`.
        if entity.is_alive() {
            self.pending_entity_despawns.lock().push(entity);
        }
        true
    }

    /// Drains the entities closed while alive (`EntityDespawnEvent`), with the
    /// snapshot they had when they left the world.
    pub fn take_entity_despawns(&self) -> Vec<crate::entity::Entity> {
        std::mem::take(&mut *self.pending_entity_despawns.lock())
    }

    pub fn entity_count(&self) -> usize {
        self.entities.read().len()
    }

    /// `Living::getEffects()`: the entity's current `EffectInstance`s.
    pub fn entity_effects(&self, unique_id: i64) -> Vec<ActiveEffect> {
        self.entity_effects
            .lock()
            .get(&unique_id)
            .cloned()
            .unwrap_or_default()
    }

    /// `Living::addEffect(EffectInstance)`: replaces an instance of the same
    /// effect type and returns `false` when the entity does not exist.
    pub fn add_entity_effect(&self, unique_id: i64, effect: ActiveEffect) -> bool {
        if !self.entities.read().contains_key(&unique_id) {
            return false;
        }
        let mut effects = self.entity_effects.lock();
        let list = effects.entry(unique_id).or_default();
        if let Some(existing) = list.iter_mut().find(|current| current.id == effect.id) {
            *existing = effect;
        } else {
            list.push(effect);
        }
        true
    }

    /// `Living::removeEffect(Effect)`: `false` when the effect was not active.
    pub fn remove_entity_effect(&self, unique_id: i64, effect_id: u32) -> bool {
        let mut effects = self.entity_effects.lock();
        let Some(list) = effects.get_mut(&unique_id) else {
            return false;
        };
        let before = list.len();
        list.retain(|effect| effect.id != effect_id);
        let removed = list.len() != before;
        if list.is_empty() {
            effects.remove(&unique_id);
        }
        removed
    }

    /// `Living::removeAllEffects()`: returns how many instances were dropped.
    pub fn clear_entity_effects(&self, unique_id: i64) -> usize {
        self.entity_effects
            .lock()
            .remove(&unique_id)
            .map(|list| list.len())
            .unwrap_or(0)
    }

    /// `EffectCollection::tick()`: one tick off every finite effect, dropping
    /// the expired ones. Infinite effects keep their duration (`infinite`).
    fn tick_entity_effects(&self) {
        let mut effects = self.entity_effects.lock();
        effects.retain(|_, list| {
            for effect in list.iter_mut() {
                if !effect.infinite {
                    effect.duration = effect.duration.saturating_sub(1);
                }
            }
            list.retain(|effect| effect.infinite || effect.duration > 0);
            !list.is_empty()
        });
    }

    /// Copy of every live entity, for the network sync pass.
    pub fn entities_snapshot(&self) -> Vec<crate::entity::Entity> {
        self.entities.read().values().cloned().collect()
    }

    /// `World::getBlockCollisionBoxes($bb)`: one box per colliding block the
    /// bounding box touches.
    pub fn collision_boxes(&self, bb: &crate::entity::Aabb) -> Vec<crate::entity::Aabb> {
        crate::entity::collision_boxes_in(self, bb)
    }

    /// `World::actuallyDoTick()` -> entity ticks (`Entity::onUpdate()` for
    /// every registered entity).
    ///
    /// Falling blocks that reach the ground are turned back into blocks here
    /// (`FallingBlock::entityBaseTick()`), which is why this runs before the
    /// block-broadcast poll. Returns the landed blocks for
    /// `FallingBlock::onHitGround()` consumers.
    pub fn tick_entities(&self) -> Vec<LandedBlock> {
        // `Entity::entityBaseTick()` -> `$this->effects->tick()` before the AI
        // pass. Effects belong to the world map (see `entity_effects`), so the
        // pass is independent of the entity lock below.
        self.tick_entity_effects();
        let mut entities = self.entities.write();
        if entities.is_empty() {
            return Vec::new();
        }
        let mut ids: Vec<i64> = entities.keys().copied().collect();
        ids.sort_unstable();
        let mut landed = Vec::new();
        let mut spawned: Vec<crate::entity::Entity> = Vec::new();
        let mut merges: Vec<(i64, i64)> = Vec::new();
        let mut segments: Vec<ProjectileHit> = Vec::new();

        for id in ids {
            let Some(entity) = entities.get_mut(&id) else {
                continue;
            };
            if entity.closed {
                continue;
            }
            // One hotspot per entity type (`entity.zombie`, `entity.item`, ...):
            // the report shows which kinds of entity are eating the tick.
            let _entity_scope = crate::profiler::scope_dyn(|| {
                let id = entity.network_type_id();
                format!("entity.{}", id.strip_prefix("minecraft:").unwrap_or(id))
            });
            // Captured before the movement pass: projectiles need the segment
            // they travelled and the speed-scaled damage of that segment.
            let before = entity.position;
            let ticks_lived = entity.ticks_lived;
            let projectile_damage = entity.projectile_damage();
            let collided = entity.on_update(self);

            match &mut entity.kind {
                crate::entity::EntityKind::Item { .. } => {
                    entity.item_tick();
                    // `ItemEntity::entityBaseTick()` merge pass, every
                    // `MERGE_CHECK_PERIOD` ticks.
                    if entity.ticks_lived as i32 % crate::entity::ITEM_MERGE_PERIOD == 0
                        && !entity.closed
                        && entity.pickup_delay() != -1
                    {
                        let bb = entity.bounding_box();
                        let nearby: Vec<i64> = entities
                            .iter()
                            .filter(|(other_id, other)| {
                                **other_id != id
                                    && matches!(other.kind, crate::entity::EntityKind::Item { .. })
                                    && !other.closed
                                    && other.bounding_box().intersects(
                                        &crate::entity::Aabb::new(
                                            bb.min_x - 0.5,
                                            bb.min_y - 0.5,
                                            bb.min_z - 0.5,
                                            bb.max_x + 0.5,
                                            bb.max_y + 0.5,
                                            bb.max_z + 0.5,
                                        ),
                                    )
                            })
                            .map(|(other_id, _)| *other_id)
                            .collect();
                        for other_id in nearby {
                            merges.push((id, other_id));
                        }
                    }
                }
                crate::entity::EntityKind::FallingBlock { block_id } => {
                    let block_id = *block_id;
                    if !entity.on_ground {
                        continue;
                    }
                    let (bx, by, bz) = entity.falling_block_cell();
                    // With several breaks per tick another column's entity can
                    // land in the cell this one just crossed, so the block
                    // ends up inside a solid cell: it can never settle there
                    // and used to be dropped as an item, leaving the sand as
                    // an entity nobody can pick up. Only lift it if the cell
                    // is actually occupied by a solid block with collision;
                    // if it landed on a non-solid / passable block like a torch
                    // (has_collision == false), it must break instead of looping.
                    let occupying = self.block_at(bx, by, bz);
                    let is_buried_in_solid = bedrock_world::block_states::state_name(occupying)
                        .is_some_and(bedrock_world::block_states::has_collision);
                    if by >= WORLD_MIN_Y
                        && by < WORLD_MAX_Y
                        && is_buried_in_solid
                        && !crate::entity::can_falling_block_settle(self, bx, by, bz)
                    {
                        let mut target = by + 1;
                        while target < WORLD_MAX_Y
                            && !crate::entity::can_falling_block_settle(self, bx, target, bz)
                        {
                            target += 1;
                        }
                        if target < WORLD_MAX_Y {
                            entity.position.y = target as f32;
                            entity.motion = Vector3f::new(0.0, 0.0, 0.0);
                            entity.on_ground = false;
                            continue;
                        }
                    }
                    // `if($this->onGround || $blockTarget !== null)`:
                    // the entity despawns and its block is restored.
                    entity.closed = true;
                    let fall_distance = entity.landed_fall_distance.max(0.0);
                    // `$blockTarget ?? $this->block` at `$pos`: when the
                    // target cell cannot be replaced (or is out of world)
                    // PocketMine drops the item and plays
                    // `BlockBreakSound` instead of overwriting it.
                    if crate::entity::can_falling_block_settle(self, bx, by, bz) {
                        self.set_block_with(bx, by, bz, block_id, true);
                        // `Anvil::getLandSound()` -> `AnvilFallSound`, a
                        // `LevelEventPacket(SOUND_ANVIL_FALL)` at the cell.
                        if bedrock_world::block_states::state_name(block_id)
                            .is_some_and(|name| name.ends_with("anvil"))
                        {
                            self.add_level_event(
                                bedrock_protocol::packets::level_event::LEVEL_EVENT_SOUND_ANVIL_FALL,
                                0,
                                bx,
                                by,
                                bz,
                            );
                        }
                        landed.push(LandedBlock {
                            x: bx,
                            y: by,
                            z: bz,
                            block_id,
                            fall_distance,
                        });
                    } else {
                        self.add_sound(
                            bedrock_protocol::packets::level_sound_event::LEVEL_SOUND_BREAK,
                            block_id as i32,
                            bx,
                            by,
                            bz,
                        );
                        self.add_level_event(
                            bedrock_protocol::packets::level_event::LEVEL_EVENT_PARTICLE_DESTROY_BLOCK,
                            block_id as i32,
                            bx,
                            by,
                            bz,
                        );
                        if let Some(item) = crate::entity::block_item(block_id) {
                            let (unique_id, runtime_id) = self.next_entity_ids();
                            spawned.push(crate::entity::Entity::item(
                                unique_id,
                                runtime_id,
                                Vector3f::new(
                                    entity.position.x,
                                    entity.position.y,
                                    entity.position.z,
                                ),
                                item,
                                Vector3f::new(0.0, 0.0, 0.0),
                                "",
                            ));
                        }
                    }
                }
                crate::entity::EntityKind::Projectile(_) => {
                    if entity.projectile_stuck() {
                        // `Arrow::entityBaseTick()`: a stuck arrow despawns
                        // after 1200 ticks (60 s).
                        if entity.ticks_lived > 1200 {
                            entity.closed = true;
                        }
                        continue;
                    }
                    // `Projectile::move()` traced a segment this tick: the
                    // manager resolves it against players.
                    if projectile_damage > 0.0 && entity.position != before {
                        segments.push(ProjectileHit {
                            unique_id: id,
                            runtime_id: entity.runtime_id,
                            from: before,
                            to: entity.position,
                            damage: projectile_damage,
                            owner_runtime_id: entity.projectile_owner_runtime_id(),
                            ticks_lived,
                            despawns_on_hit: entity.projectile_despawns_on_hit(),
                        });
                    }
                    entity.projectile_tick(collided);
                }
                crate::entity::EntityKind::PrimedTnt { fuse, .. } => {
                    // `PrimedTNT::entityBaseTick()`.
                    *fuse -= 1;
                    if *fuse <= 0 {
                        entity.closed = true;
                        let position = entity.position;
                        let snapshot = crate::plugin_host::entity_json(&entity);
                        self.explode_into(position, snapshot);
                    }
                }
                crate::entity::EntityKind::ExperienceOrb { .. } => {
                    // `ExperienceOrb::DESPAWN_TICKS`.
                    if entity.ticks_lived as i32 >= crate::entity::XP_ORB_DESPAWN_TICKS {
                        entity.closed = true;
                    }
                }
                crate::entity::EntityKind::Mob(_) => {
                    entity.mob_tick(collided);
                    if entity.is_alive() {
                        continue;
                    }
                    let loot_pending = matches!(
                        &entity.kind,
                        crate::entity::EntityKind::Mob(mob) if !mob.loot_dropped
                    );
                    if !loot_pending {
                        continue;
                    }
                    // `Living::onDeath()`: drop every `getDrops()` entry, then
                    // `World::dropExperience()` with `getXpDropAmount()`.
                    if let crate::entity::EntityKind::Mob(mob) = &mut entity.kind {
                        mob.loot_dropped = true;
                    }
                    // `Living::onDeath()` -> `EntityDeathEvent` for the plugins.
                    self.pending_entity_deaths.lock().push(id);
                    let position = entity.position;
                    for (name, min, max) in entity.mob_loot() {
                        let Some(network_id) = crate::inventory::item_network_id(name) else {
                            continue;
                        };
                        let roll = pseudo_random(id.wrapping_add((min as i64) << 8));
                        let count = min + (roll * (max - min + 1) as f32) as u32;
                        if count == 0 {
                            continue;
                        }
                        let (item_id, item_runtime) = self.next_entity_ids();
                        spawned.push(crate::entity::Entity::item(
                            item_id,
                            item_runtime,
                            position,
                            ItemStack {
                                network_id,
                                count: count.min(u32::from(u16::MAX)) as u16,
                                ..ItemStack::empty()
                            },
                            Vector3f::new(0.0, 0.1, 0.0),
                            "",
                        ));
                    }
                    let xp = entity.mob_xp();
                    if xp > 0 {
                        let (orb_id, orb_runtime) = self.next_entity_ids();
                        spawned.push(crate::entity::Entity::experience_orb(
                            orb_id,
                            orb_runtime,
                            position,
                            xp,
                        ));
                    }
                }
            }
        }

        // `Projectile::onHitEntity()`: the world resolves mob targets here
        // (the session manager resolves players, since it owns that map).
        for hit in &segments {
            let mut consumed = false;
            for entity in entities.values_mut() {
                if !entity.is_mob() || !entity.is_alive() || entity.unique_id == hit.unique_id {
                    continue;
                }
                let bb = entity.bounding_box();
                if hit.to.x < bb.min_x - 0.3
                    || hit.to.x > bb.max_x + 0.3
                    || hit.to.y < bb.min_y - 0.3
                    || hit.to.y > bb.max_y + 0.3
                    || hit.to.z < bb.min_z - 0.3
                    || hit.to.z > bb.max_z + 0.3
                {
                    continue;
                }
                // Queued, not applied: `EntityDamageByEntityEvent` is decided
                // by the session manager before the next entity tick.
                self.pending_projectile_damage
                    .lock()
                    .push(PendingProjectileDamage {
                        unique_id: entity.unique_id,
                        runtime_id: entity.runtime_id,
                        damage: hit.damage,
                        owner_runtime_id: hit.owner_runtime_id,
                        child: Some(entity.clone()),
                    });
                if hit.despawns_on_hit {
                    consumed = true;
                }
                break;
            }
            if consumed {
                entities.remove(&hit.unique_id);
            }
        }
        self.pending_projectile_hits.lock().extend(segments);

        // `ItemEntity::tryMergeInto()`: applied after the tick pass so the
        // source and consumer never need two mutable map borrows at once.
        for (source_id, consumer_id) in merges {
            let Some(mut source) = entities.remove(&source_id) else {
                continue;
            };
            if let Some(consumer) = entities.get_mut(&consumer_id) {
                crate::entity::try_merge(&mut source, consumer);
            }
            if !source.closed {
                entities.insert(source_id, source);
            }
        }

        for entity in spawned {
            if entities.len() >= MAX_ENTITIES {
                break;
            }
            entities.insert(entity.unique_id, entity);
        }
        // `Entity::close()` from the tick (item despawn, projectile vanish,
        // falling block settling) also fires `EntityDespawnEvent` when the
        // entity is still alive; a dead entity already queued its death.
        let mut despawned: Vec<crate::entity::Entity> = Vec::new();
        entities.retain(|_, entity| {
            if entity.closed && entity.is_alive() {
                despawned.push(entity.clone());
            }
            !entity.closed
        });
        drop(entities);
        if !despawned.is_empty() {
            self.pending_entity_despawns.lock().extend(despawned);
        }
        landed
    }

    /// `PrimedTNT::explode()` -> `Explosion::explodeA()`: destroys the blocks
    /// inside the blast radius (bedrock/obsidian/barriers are blast-proof) and
    /// drops a fraction of them as items. `explodeB()`'s entity damage is
    /// queued for `SessionManager`, which owns the player list.
    fn explode_into(&self, position: Vector3f, entity: serde_json::Value) {
        self.queue_explosion(
            position,
            crate::entity::TNT_EXPLOSION_RADIUS,
            Some(entity),
            None,
        );
    }

    /// Blocks an explosion at `position` with `radius` would destroy.
    ///
    /// `Explosion::explodeA()` in PHP asks for this list and can edit it
    /// before `explodeB()`. This is the core's blast model: every non-air
    /// block inside the sphere except the hard blocks the TNT blast already
    /// ignores (bedrock/barrier/obsidian/command blocks). It is an
    /// approximation of PM's ray-traced blast resistance.
    pub fn explosion_blocks(&self, position: Vector3f, radius: f32) -> Vec<(i32, i32, i32, u32)> {
        let reach = radius.ceil() as i32;
        let center = (
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32,
        );
        let mut blocks = Vec::new();
        for dy in -reach..=reach {
            for dz in -reach..=reach {
                for dx in -reach..=reach {
                    let distance = ((dx * dx + dy * dy + dz * dz) as f32).sqrt();
                    if distance > radius {
                        continue;
                    }
                    let (x, y, z) = (center.0 + dx, center.1 + dy, center.2 + dz);
                    let block_id = self.block_at(x, y, z);
                    if block_id == bedrock_world::block_id::AIR {
                        continue;
                    }
                    let Some(name) = bedrock_world::block_states::state_name(block_id) else {
                        continue;
                    };
                    if name.contains("bedrock")
                        || name.contains("barrier")
                        || name.contains("obsidian")
                        || name.contains("command_block")
                    {
                        continue;
                    }
                    blocks.push((x, y, z, block_id));
                }
            }
        }
        blocks
    }

    /// Queues an explosion for the tick. `blocks` overrides the computed list
    /// (PHP `Explosion::$affectedBlocks`, possibly edited by the plugin);
    /// `entity` is the `EntityExplodeEvent::getEntity()` snapshot, or `None`
    /// for a sourceless explosion (PM fires no entity event then).
    /// Returns how many blocks are queued.
    pub fn queue_explosion(
        &self,
        position: Vector3f,
        radius: f32,
        entity: Option<serde_json::Value>,
        blocks: Option<Vec<(i32, i32, i32, u32)>>,
    ) -> usize {
        let blocks = blocks.unwrap_or_else(|| self.explosion_blocks(position, radius));
        let count = blocks.len();
        // The blocks are destroyed by `SessionManager` after
        // `EntityExplodeEvent` (`Explosion::explodeA()` runs before `explodeB()`
        // fires it), so a listener can still cancel or edit the list.
        self.pending_explosions.lock().push(Explosion {
            x: position.x,
            y: position.y,
            z: position.z,
            radius,
            entity,
            blocks,
        });
        count
    }

    /// `Explosion::explodeB()`'s block damage, once `EntityExplodeEvent` let it
    /// through: destroys `blocks` and drops `yield_pct`% of them as items.
    /// Returns how many blocks were destroyed.
    pub fn apply_explosion_blocks(
        &self,
        blocks: &[(i32, i32, i32, u32)],
        yield_pct: f32,
    ) -> usize {
        let mut destroyed = 0;
        for &(x, y, z, block_id) in blocks {
            // A plugin may have changed the cell while the event ran.
            if self.block_at(x, y, z) != block_id
                || !self.set_block_with(x, y, z, bedrock_world::block_id::AIR, true)
            {
                continue;
            }
            destroyed += 1;
            // `Explosion::explodeA()` drops only a fraction of the destroyed
            // blocks (`yield`, 0-100 in the event).
            let seed = (x as i64).wrapping_mul(31) ^ (z as i64) ^ (y as i64) << 13;
            if pseudo_random(seed) > yield_pct / 100.0 {
                continue;
            }
            let Some(item) = crate::entity::block_item(block_id) else {
                continue;
            };
            self.spawn_item(
                Vector3f::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5),
                item,
                Some(Vector3f::new(0.0, 0.1, 0.0)),
                "",
            );
        }
        destroyed
    }

    /// Queues a `LevelEventPacket` (PM `World::addSound()` /
    /// `World::addParticle()` with a level-event implementation).
    pub fn add_level_event(&self, event_id: i32, event_data: i32, x: i32, y: i32, z: i32) {
        self.pending_level_events
            .lock()
            .push((event_id, event_data, x, y, z));
    }

    /// `World::addParticle()` at an exact position: the packet keeps the
    /// `Vector3` the plugin passed (PM broadcasts at that exact spot).
    pub fn add_level_event_at(
        &self,
        event_id: i32,
        event_data: i32,
        x: f32,
        y: f32,
        z: f32,
    ) {
        self.pending_level_events_at
            .lock()
            .push((event_id, event_data, x, y, z));
    }

    /// Drains the exact-position level events queued since the last tick.
    pub fn take_pending_level_events_at(&self) -> Vec<(i32, i32, f32, f32, f32)> {
        std::mem::take(&mut *self.pending_level_events_at.lock())
    }

    /// Queues a PHP `World::addSound()`/`addParticle()` for the next tick,
    /// where `SessionManager` dispatches the matching world event.
    pub fn queue_world_effect(&self, effect: PendingWorldEffect) {
        self.pending_world_effects.lock().push(effect);
    }

    /// Drains the PHP effects queued since the last tick, oldest first.
    pub fn take_world_effects(&self) -> Vec<PendingWorldEffect> {
        std::mem::take(&mut *self.pending_world_effects.lock())
    }

    /// Drains the level events queued since the last tick, oldest first.
    pub fn take_pending_level_events(&self) -> Vec<(i32, i32, i32, i32, i32)> {
        std::mem::take(&mut *self.pending_level_events.lock())
    }

    /// Installs (or clears) the block-event hook used by the world tick.
    pub fn set_block_event_hook(&self, hook: Option<Arc<dyn BlockEventHook>>) {
        *self.block_events.write() = hook;
    }

    /// Fires `BlockSpreadEvent` through the hook; true when the change may be
    /// applied. Called with no world lock held.
    pub(crate) fn allow_spread(
        &self,
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
        source_id: u32,
        new_id: u32,
    ) -> bool {
        let hook = self.block_events.read().clone();
        match hook {
            Some(hook) => hook.spread(x, y, z, block_id, source_id, new_id),
            None => true,
        }
    }

    /// Fires `BlockFormEvent` through the hook; true when the change may be
    /// applied. Called with no world lock held.
    pub(crate) fn allow_form(
        &self,
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
        causing_id: u32,
        new_id: u32,
    ) -> bool {
        let hook = self.block_events.read().clone();
        match hook {
            Some(hook) => hook.form(x, y, z, block_id, causing_id, new_id),
            None => true,
        }
    }

    /// Queues a world event for the next dispatch (`SessionManager`).
    pub fn queue_world_event(&self, event: PendingWorldEvent) {
        self.pending_world_events.lock().push(event);
    }

    /// Drains the world events queued since the last tick, oldest first.
    pub fn take_world_events(&self) -> Vec<PendingWorldEvent> {
        std::mem::take(&mut *self.pending_world_events.lock())
    }

    /// Drains the Java-facing queue (Fase 1), oldest first.
    ///
    /// This is the `world.take_block_changes` host call: the Java bridge
    /// polls it once per tick and applies each `{x, y, z, block_id}` to its
    /// Java world. Draining here does *not* affect the Bedrock broadcast
    /// queue (see [`WorldState::take_broadcast_changes`]).
    pub fn take_changes(&self) -> Vec<BlockChange> {
        self.block_changes.write().drain(..).collect()
    }

    /// Alias for [`WorldState::take_changes`]; the host also accepts the
    /// `world.drain_changes` method name.
    pub fn drain_changes(&self) -> Vec<BlockChange> {
        self.take_changes()
    }

    /// Drains the Bedrock-facing queue (Fase 2), oldest first.
    ///
    /// `SessionManager::poll_block_changes` turns these into
    /// `UpdateBlockPacket`s for Bedrock sessions that have the chunk in
    /// view. Independent from the Java queue, so polling Bedrock does not
    /// starve Java.
    pub fn take_broadcast_changes(&self) -> Vec<BlockChange> {
        self.broadcast_changes.write().drain(..).collect()
    }

    /// Number of changes still waiting for `world.take_block_changes`.
    pub fn pending_block_changes(&self) -> usize {
        self.block_changes.read().len()
    }

    /// Number of changes still waiting for Bedrock `UpdateBlockPacket`s.
    pub fn pending_broadcast_changes(&self) -> usize {
        self.broadcast_changes.read().len()
    }

    /// Highest non-air block in a column, including edits.
    pub fn highest_block_at(&self, x: i32, z: i32) -> i32 {
        let base = self.generator.highest_block_at(x, z);
        let highest_edit = self.edits.read().get(&(x >> 4, z >> 4)).and_then(|chunk| {
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

    /// Highest non-air block of a column's **loaded** chunk (base terrain plus
    /// the edits it was built with), or `None` when nobody streamed it.
    ///
    /// Used by the random-tick light pass, which must never fall back to the
    /// generator: generating a column on the tick path is ~18 ms.
    pub fn loaded_highest_block_at(&self, x: i32, z: i32) -> Option<i32> {
        self.column_view(x >> 4, z >> 4).highest_block_at(x, z)
    }

    /// Snapshot of one column for [`WorldState::random_tick`].
    ///
    /// Takes the two locks once and shares the base column and the edit map by
    /// `Arc`; the pass reads the snapshot without locking (see [`ColumnView`]).
    pub(crate) fn column_view(&self, chunk_x: i32, chunk_z: i32) -> ColumnView {
        let edits = self.edits.read().get(&(chunk_x, chunk_z)).cloned();
        let chunk = self
            .loaded_columns
            .lock()
            .entries
            .get(&(chunk_x, chunk_z))
            .cloned();
        ColumnView { chunk, edits }
    }

    /// Highest edited cell of a column, ignoring terrain.
    ///
    /// The random-tick light pass uses it so a roof somebody built still
    /// shades the grass under it. Scans that chunk's edit map, which stays
    /// small for hand-built areas.
    pub fn highest_edit_at(&self, x: i32, z: i32) -> Option<i32> {
        self.column_view(x >> 4, z >> 4).highest_edit_at(x, z)
    }

    /// True when a column is loaded for the random-tick pass, i.e.
    /// [`WorldState::loaded_block_at`] can answer for it.
    pub fn has_loaded_chunk(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.loaded_columns
            .lock()
            .entries
            .contains_key(&(chunk_x, chunk_z))
    }

    /// Block id for the random-tick pass: an edit first, then the loaded
    /// column's base terrain. `None` when the column was never streamed.
    ///
    /// Reading edits first is what keeps a ticking column correct after
    /// [`WorldState::set_block`] (which does not invalidate the base column);
    /// it is also what makes the pass see blocks players just placed.
    pub fn loaded_block_at(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        self.column_view(x >> 4, z >> 4).block_at(x, y, z)
    }

    /// `randomTickSpeed` (0 disables the random-tick pass).
    pub fn random_tick_speed(&self) -> i32 {
        self.random_tick_speed.load(Ordering::Relaxed)
    }

    /// Sets `randomTickSpeed`; returns the value actually stored, clamped at 0
    /// so a negative speed cannot arm the random-tick pass.
    pub fn set_random_tick_speed(&self, speed: i32) -> i32 {
        let speed = speed.max(0);
        self.random_tick_speed.store(speed, Ordering::Relaxed);
        speed
    }

    /// Next seed for the random-tick sampler.
    pub(crate) fn next_random_tick_seed(&self) -> u64 {
        self.random_tick_seed
            .fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
    }

    /// Generates a chunk payload including every edit that lands in it.
    ///
    /// Results are cached (FIFO, [`CHUNK_PAYLOAD_CACHE_CAP`] entries) so
    /// the synchronous Java `world.get_chunk` host call and Bedrock
    /// streaming do not regenerate the same column twice: generation is
    /// ~20 ms per chunk and running it in the PHP tick collapsed TPS
    /// during initial chunk load. [`WorldState::set_block`] invalidates
    /// the edited column.
    pub fn chunk_payload(&self, chunk_x: i32, chunk_z: i32) -> (u32, Vec<u8>) {
        if let Some(cached) = self
            .chunk_cache
            .lock()
            .entries
            .get(&(chunk_x, chunk_z))
            .cloned()
        {
            return cached;
        }

        // Column version at the moment the edit snapshot is taken. If a
        // `set_block` lands while the payload is being generated, the
        // generated bytes predate it and must not poison the cache: the
        // next request regenerates with the edit. (The payload is still
        // returned for this one stream — viewers that already hold the
        // chunk get the edit as an `UpdateBlockPacket`.)
        let version_at_read = self.chunk_version(chunk_x, chunk_z);
        // One generation per column: `build_chunk` applies the edits, hands the
        // column to the random-tick cache (a streamed chunk is a ticking chunk)
        // and the caller only serializes it.
        let column = self.build_chunk(chunk_x, chunk_z);
        let dominant_biome = column.biomes[7 * 16 + 7];
        let payload = column.serialize_network_payload(dominant_biome);

        let mut cache = self.chunk_cache.lock();
        // An edit that landed during generation makes this payload stale:
        // serve it for this stream but do not cache it (see
        // `version_at_read`). The check runs under the cache lock so
        // `set_block`'s (invalidate, bump-version) pair cannot slip in
        // between the check and the insert.
        if self.chunk_version(chunk_x, chunk_z) != version_at_read {
            return payload;
        }
        if !cache.entries.contains_key(&(chunk_x, chunk_z)) {
            cache.order.push_back((chunk_x, chunk_z));
            while cache.order.len() > CHUNK_PAYLOAD_CACHE_CAP {
                if let Some(oldest) = cache.order.pop_front() {
                    cache.entries.remove(&oldest);
                }
            }
        }
        cache.entries.insert((chunk_x, chunk_z), payload.clone());
        payload
    }

    /// Payload for a column that is already generated, without generating.
    ///
    /// This is the tick path's lookup: cold columns belong to the chunk
    /// pipeline (see `crate::chunk_pipeline`), so a tick never pays for terrain
    /// generation and never holds the sessions lock while one runs.
    pub fn cached_chunk_payload(&self, chunk_x: i32, chunk_z: i32) -> Option<(u32, Vec<u8>)> {
        self.chunk_cache
            .lock()
            .entries
            .get(&(chunk_x, chunk_z))
            .cloned()
    }

    /// Whether [`WorldState::cached_chunk_payload`] would answer without
    /// generating.
    pub fn has_chunk_payload(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.chunk_cache
            .lock()
            .entries
            .contains_key(&(chunk_x, chunk_z))
    }

    /// Number of edited chunks, exposed for diagnostics.
    pub fn edited_chunk_count(&self) -> usize {
        self.edits.read().len()
    }

    /// In-memory [`Chunk`](bedrock_world::chunk::Chunk) for a column,
    /// base terrain from the generator plus every persistent edit in
    /// the column. Used by the native Java bridge to frame
    /// `ModernChunk` / `ChunkBuilder` payloads; the bridge now consumes
    /// [`WorldState::get_chunk_cached`] and regenerates only on a miss.
    pub fn get_chunk(&self, chunk_x: i32, chunk_z: i32) -> bedrock_world::chunk::Chunk {
        (*self.build_chunk(chunk_x, chunk_z)).clone()
    }

    /// Edit counter for one column (0 until the first successful edit).
    /// Converted Java payloads are keyed by this value, so a block edit
    /// can never leave a stale payload behind.
    pub fn chunk_version(&self, chunk_x: i32, chunk_z: i32) -> u64 {
        self.chunk_versions
            .lock()
            .get(&(chunk_x, chunk_z))
            .copied()
            .unwrap_or(0)
    }

    /// True when [`WorldState::get_chunk_cached`] would be served from
    /// memory. Used by the background prefetch to skip columns the Java
    /// bridge already has warm.
    pub fn has_chunk_cached(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.chunk_object_cache
            .lock()
            .entries
            .contains_key(&(chunk_x, chunk_z))
    }

    /// Cached [`WorldState::get_chunk`]: the first request for a column
    /// pays the generator (~18 ms) and stores the result (FIFO,
    /// [`CHUNK_OBJECT_CACHE_CAP`] entries) so later Java players and the
    /// background prefetch never regenerate it. Invalidated by
    /// [`WorldState::set_block`].
    pub fn get_chunk_cached(&self, chunk_x: i32, chunk_z: i32) -> bedrock_world::chunk::Chunk {
        if let Some(cached) = self
            .chunk_object_cache
            .lock()
            .entries
            .get(&(chunk_x, chunk_z))
            .cloned()
        {
            return cached;
        }

        // Same stale-payload guard as [`WorldState::chunk_payload`]: an
        // edit landing during the build must not poison the cache. The
        // check runs under the cache lock so `set_block`'s (invalidate,
        // bump-version) pair cannot slip between check and insert.
        let version_at_read = self.chunk_version(chunk_x, chunk_z);
        let chunk = self.build_chunk(chunk_x, chunk_z);

        let mut cache = self.chunk_object_cache.lock();
        if self.chunk_version(chunk_x, chunk_z) != version_at_read {
            return (*chunk).clone();
        }
        if !cache.entries.contains_key(&(chunk_x, chunk_z)) {
            cache.order.push_back((chunk_x, chunk_z));
            while cache.order.len() > CHUNK_OBJECT_CACHE_CAP {
                if let Some(oldest) = cache.order.pop_front() {
                    cache.entries.remove(&oldest);
                }
            }
        }
        cache.entries.insert((chunk_x, chunk_z), (*chunk).clone());
        (*chunk).clone()
    }

    /// Uncached column build: base terrain plus persistent edits.
    ///
    /// The result is shared as an `Arc`: the list of columns the random-tick
    /// pass may touch ([`WorldState::loaded_block_at`]) holds the same
    /// allocation, so building a column for a client is also what makes it a
    /// ticking chunk, with no second generation.
    fn build_chunk(&self, chunk_x: i32, chunk_z: i32) -> Arc<Chunk> {
        let mut chunk = self.generator.generate_chunk_object(chunk_x, chunk_z);
        if let Some(edits) = self.edits.read().get(&(chunk_x, chunk_z)) {
            for ((x, y, z), id) in edits.iter() {
                chunk.set_block((*x & 15) as usize, *y, (*z & 15) as usize, *id);
            }
        }
        // `World::setBiomeId()` edits land on the base column, so they must be
        // re-applied after generation (and before the payload is serialized).
        {
            let overrides = self.biome_overrides.read();
            if !overrides.is_empty() {
                for (&(x, z), &biome_id) in overrides.iter() {
                    if x >> 4 == chunk_x && z >> 4 == chunk_z {
                        chunk.biomes[(x & 15) as usize * 16 + (z & 15) as usize] = biome_id;
                    }
                }
            }
        }
        let chunk = Arc::new(chunk);
        self.remember_loaded_column(chunk_x, chunk_z, Arc::clone(&chunk));
        chunk
    }

    /// Stores a built column in the random-tick cache (FIFO,
    /// [`LOADED_COLUMN_CACHE_CAP`] entries).
    fn remember_loaded_column(&self, chunk_x: i32, chunk_z: i32, chunk: Arc<Chunk>) {
        let mut cache = self.loaded_columns.lock();
        if !cache.entries.contains_key(&(chunk_x, chunk_z)) {
            cache.order.push_back((chunk_x, chunk_z));
            while cache.order.len() > LOADED_COLUMN_CACHE_CAP {
                if let Some(oldest) = cache.order.pop_front() {
                    cache.entries.remove(&oldest);
                }
            }
        }
        cache.entries.insert((chunk_x, chunk_z), chunk);
    }

    /// Writes the world snapshot (`world.json`, format v2): time, weather,
    /// difficulty, spawn, seed, `auto_save`/`time_running` and every block
    /// edit.
    ///
    /// PocketMine stores this in `level.dat` (NBT) + LevelDB chunks; JSON
    /// keeps the port free of an NBT serializer while preserving the same
    /// information. See the module header for the documented divergence.
    pub fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        let (spawn_x, spawn_y, spawn_z) = self.spawn();
        let edits: Vec<[i64; 4]> = self
            .edits
            .read()
            .values()
            .flat_map(|chunk| {
                chunk
                    .iter()
                    .map(|((x, y, z), id)| [*x as i64, *y as i64, *z as i64, *id as i64])
            })
            .collect();
        let value = serde_json::json!({
            "version": WORLD_FORMAT_VERSION,
            "name": self.name,
            "time": self.time(),
            "weather": self.weather(),
            "difficulty": self.difficulty(),
            "spawn": [spawn_x, spawn_y, spawn_z],
            "seed": self.seed(),
            "auto_save": self.auto_save(),
            "auto_save_interval": self.auto_save_interval(),
            "time_running": self.is_time_running(),
            "edits": edits,
        });
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec(&value)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        std::fs::write(path, bytes)
    }

    /// Public `World::save()` alias for the PHP host (defaults to `world.json`
    /// in the current directory when `path` is `None`).
    ///
    /// `World::save()` starts with `WorldSaveEvent`; the world queues it and
    /// `SessionManager` dispatches it on the next tick (the shutdown save has
    /// no tick left to deliver it).
    pub fn save(&self) -> std::io::Result<()> {
        self.queue_world_event(PendingWorldEvent::Saved);
        self.save_to(std::path::Path::new("world.json"))
    }

    /// Applies a snapshot written by [`WorldState::save_to`].
    ///
    /// Returns false when the file is missing or malformed, leaving the world
    /// untouched. v1 files (no `version`) load with `auto_save`/`time_running`
    /// defaulting to true.
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
            // Restoring the saved value is not a plugin-visible change: PM
            // fires `WorldDifficultyChangeEvent` from `setDifficulty()`, not
            // while the provider loads the world data.
            self.difficulty
                .store((difficulty as i32).clamp(0, 3), Ordering::Relaxed);
        }
        if let Some(spawn) = value.get("spawn").and_then(|value| value.as_array()) {
            let coords: Vec<i32> = spawn
                .iter()
                .filter_map(|value| value.as_i64().map(|n| n as i32))
                .collect();
            if coords.len() == 3 {
                // Same as the difficulty: loading the saved spawn does not fire
                // `SpawnChangeEvent`.
                *self.spawn.write() = (coords[0], coords[1], coords[2]);
            }
        }
        if let Some(auto_save) = value.get("auto_save").and_then(|value| value.as_bool()) {
            self.set_auto_save(auto_save);
        }
        if let Some(interval) = value
            .get("auto_save_interval")
            .and_then(|value| value.as_i64())
        {
            self.set_auto_save_interval(interval as i32);
        }
        if let Some(running) = value.get("time_running").and_then(|value| value.as_bool()) {
            self.time_running.store(running, Ordering::Relaxed);
        }
        if let Some(edits) = value.get("edits").and_then(|value| value.as_array()) {
            let mut map = self.edits.write();
            for entry in edits {
                let Some(values) = entry.as_array() else {
                    continue;
                };
                let nums: Vec<i64> = values.iter().filter_map(|value| value.as_i64()).collect();
                if nums.len() != 4 {
                    continue;
                }
                let (x, y, z) = (nums[0] as i32, nums[1] as i32, nums[2] as i32);
                Arc::make_mut(map.entry((x >> 4, z >> 4)).or_default())
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
    fn despawning_a_live_entity_queues_its_despawn_event() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let alive = world
            .spawn_mob("minecraft:zombie", Vector3f::new(0.0, 64.0, 0.0))
            .expect("spawn a zombie");
        world.take_entity_spawns();
        assert!(world.remove_entity(alive));
        let despawned = world.take_entity_despawns();
        assert_eq!(despawned.len(), 1);
        assert_eq!(despawned[0].unique_id, alive);
        assert!(world.take_entity_despawns().is_empty());

        // A killed entity already queued `EntityDeathEvent`: no despawn event.
        let dead = world
            .spawn_mob("minecraft:zombie", Vector3f::new(0.0, 64.0, 0.0))
            .expect("spawn a zombie");
        let runtime = world.entity_by_id(dead).expect("the zombie").runtime_id;
        world.damage_entity(runtime, 100.0);
        world.take_entity_deaths();
        assert!(world.remove_entity(dead));
        assert!(world.take_entity_despawns().is_empty());
    }

    /// `EffectCollection` port: add replaces by type, the tick expires finite
    /// instances, infinite ones stay, and closing the entity drops them.
    /// The block-event hook is what lets a plugin cancel a fluid spread/form
    /// from inside the world tick; without it every change is applied.
    #[test]
    fn block_event_hook_decides_whether_spreads_and_forms_apply() {
        struct Deny;
        impl BlockEventHook for Deny {
            fn spread(&self, _: i32, _: i32, _: i32, _: u32, _: u32, _: u32) -> bool {
                false
            }
            fn form(&self, _: i32, _: i32, _: i32, _: u32, _: u32, _: u32) -> bool {
                false
            }
        }

        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        assert!(world.allow_spread(0, 4, 0, 1, 2, 3));
        assert!(world.allow_form(0, 4, 0, 1, 2, 3));

        world.set_block_event_hook(Some(Arc::new(Deny)));
        assert!(!world.allow_spread(0, 4, 0, 1, 2, 3));
        assert!(!world.allow_form(0, 4, 0, 1, 2, 3));

        world.set_block_event_hook(None);
        assert!(world.allow_spread(0, 4, 0, 1, 2, 3));
    }

    /// `World::setSpawnLocation()` fires `SpawnChangeEvent` on every call, even
    /// when the spawn does not move; the queue must not dedupe.
    #[test]
    fn set_spawn_queues_the_event_even_when_the_spawn_is_unchanged() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let spawn = world.spawn();

        world.set_spawn(spawn.0, spawn.1, spawn.2);
        world.set_spawn(spawn.0, spawn.1, spawn.2);

        assert_eq!(
            world.take_world_events(),
            vec![
                PendingWorldEvent::SpawnChanged { previous: spawn },
                PendingWorldEvent::SpawnChanged { previous: spawn },
            ]
        );
        assert_eq!(world.spawn(), spawn);
    }

    #[test]
    fn entity_effects_are_added_ticked_and_cleared() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let id = world
            .spawn_mob_configured(
                "minecraft:zombie",
                Vector3f::new(0.0, 64.0, 0.0),
                0.0,
                0.0,
                None,
            )
            .expect("spawn a zombie");
        assert!(world.entity_effects(id).is_empty());

        assert!(world.add_entity_effect(id, ActiveEffect::new(1, 3, 1)));
        assert!(world.add_entity_effect(id, ActiveEffect::new(20, 600, 0)));
        assert_eq!(world.entity_effects(id).len(), 2);

        // Adding the same type replaces the previous instance.
        assert!(world.add_entity_effect(id, ActiveEffect::new(1, 5, 2)));
        let effects = world.entity_effects(id);
        assert_eq!(effects.len(), 2);
        assert_eq!(effects[0].amplifier, 2);

        // The speed instance (5 ticks) expires; wither (600) stays.
        for _ in 0..5 {
            world.tick_entities();
        }
        let effects = world.entity_effects(id);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].id, 20);

        // `infinite` never runs out, even from duration 0.
        let mut infinite = ActiveEffect::new(10, 0, 0);
        infinite.infinite = true;
        assert!(world.add_entity_effect(id, infinite));
        for _ in 0..3 {
            world.tick_entities();
        }
        assert_eq!(world.entity_effects(id).len(), 2);

        assert!(world.remove_entity_effect(id, 10));
        assert!(!world.remove_entity_effect(id, 10));
        assert_eq!(world.clear_entity_effects(id), 1);
        assert!(world.entity_effects(id).is_empty());

        // `Entity::close()` drops the collection with the entity.
        assert!(world.add_entity_effect(id, ActiveEffect::new(1, 60, 0)));
        assert!(world.remove_entity(id));
        assert!(world.entity_effects(id).is_empty());
        assert!(!world.add_entity_effect(id, ActiveEffect::new(1, 60, 0)));
    }

    #[test]
    fn snapshot_roundtrips_time_spawn_difficulty_and_edits() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        world.set_time(6_000);
        world.set_difficulty(3);
        world.set_spawn(4, 70, -8);
        world.set_auto_save(false);
        world.stop_time();
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
        assert!(!restored.auto_save());
        assert!(!restored.is_time_running());

        // A missing file leaves the world untouched.
        let missing = dir.join("missing.json");
        assert!(!restored.load_from(&missing));
        assert_eq!(restored.time(), 6_000);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn v1_files_without_version_still_load() {
        let dir = std::env::temp_dir().join(format!("pm-rust-world-v1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("world.json");
        std::fs::write(
            &path,
            serde_json::json!({
                "time": 6000,
                "weather": 0,
                "difficulty": 2,
                "spawn": [0, 64, 0],
                "seed": 12345678,
                "edits": []
            })
            .to_string(),
        )
        .expect("write v1");

        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        assert!(world.load_from(&path));
        assert_eq!(world.time(), 6_000);
        assert!(world.auto_save());
        assert!(world.is_time_running());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tick_advances_time_only_when_running_and_wraps() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        assert_eq!(world.time(), DEFAULT_WORLD_TIME);
        assert_eq!(world.tick(), DEFAULT_WORLD_TIME + 1);
        assert_eq!(world.tick(), DEFAULT_WORLD_TIME + 2);
        world.stop_time();
        assert_eq!(world.tick(), DEFAULT_WORLD_TIME + 2);
        world.start_time();
        world.set_time(TICKS_PER_DAY - 1);
        assert_eq!(world.tick(), 0);
    }

    #[test]
    fn set_block_enqueues_and_take_changes_drains_oldest_first() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let before = world.block_at(1, 64, 2);
        assert!(world.set_block(1, 64, 2, 99));
        assert!(world.set_block(3, 65, 4, 100));
        // A no-change call must not enqueue.
        assert!(!world.set_block(1, 64, 2, 99));
        assert_ne!(before, 99);

        assert_eq!(world.pending_block_changes(), 2);
        let drained = world.take_changes();
        assert_eq!(
            drained,
            vec![
                BlockChange::new(1, 64, 2, 99),
                BlockChange::new(3, 65, 4, 100),
            ]
        );
        // Fully drained: a second take (next Java tick) sees nothing.
        assert!(world.take_changes().is_empty());
        assert_eq!(world.pending_block_changes(), 0);
    }

    /// PocketMine's neighbour cascade (`internalNotifyNeighbourBlockUpdate`):
    /// placing a fence next to an existing fence must update *both* states and
    /// queue both for broadcast, otherwise the old fence keeps rendering
    /// without its new connection on every client.
    /// True when a Bedrock connectable state has the given horizontal connection.
    fn connected(id: u32, dir: &str) -> bool {
        let key = format!("minecraft:connection_{dir}");
        bedrock_world::block_states::state_properties(id).is_some_and(|props| {
            props.iter().any(|(name, value)| {
                name == &key && *value == bedrock_world::block_states::Value::Byte(1)
            })
        })
    }

    /// A fence placed between two fences touches three cells. Like PM's
    /// `changedBlocks` flush, each cell is broadcast once with its **final**
    /// state: the intermediate states of the cascade (what made the client
    /// flicker while arms fought over connections) never leave the server.
    #[test]
    fn a_fence_between_two_fences_broadcasts_each_cell_once() {
        use bedrock_world::block_states;

        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let fence = block_states::network_id("jungle_fence").expect("jungle_fence");
        assert!(world.set_block(0, 80, 0, fence));
        assert!(world.set_block(2, 80, 0, fence));
        world.take_changes();
        world.take_broadcast_changes();

        assert!(world.set_block(1, 80, 0, fence));
        let changes = world.take_changes();
        let mut positions: Vec<i32> = changes.iter().map(|change| change.x).collect();
        positions.sort_unstable();
        assert_eq!(positions, vec![0, 1, 2], "one change per cell: {changes:?}");

        let middle = changes
            .iter()
            .find(|change| change.x == 1)
            .expect("middle fence")
            .block_id;
        assert!(connected(middle, "east") && connected(middle, "west"));

        // Idempotent: the cascade reached its fixed point, a second identical
        // set is a no-op (nothing left to flicker).
        assert!(!world.set_block(1, 80, 0, middle));
        assert!(world.take_changes().is_empty());
    }

    #[test]
    fn placing_a_fence_updates_the_neighbour_fence_too() {
        use bedrock_world::block_states;

        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let fence = block_states::network_id("jungle_fence").expect("jungle_fence");
        let (x, y, z) = (4, 80, 4);

        // Lone fence: whatever the client predicted, it ends up disconnected.
        assert!(world.set_block(x, y, z, fence));
        let changes = world.take_changes();
        let placed = changes.iter().find(|change| change.x == x).expect("placed").block_id;
        assert!(!connected(placed, "east") && !connected(placed, "west"));
        world.take_broadcast_changes();

        // Fence to its east: the placed one connects west and the existing one
        // connects east, both in the same batch.
        assert!(world.set_block(x + 1, y, z, fence));
        let changes = world.take_changes();
        // Like PM, the placement is written first and then corrected by the
        // neighbour update, so the *last* change per cell is the final state.
        let last = |cell_x: i32| {
            changes
                .iter()
                .rev()
                .find(|change| change.x == cell_x)
                .expect("a change for this cell")
                .block_id
        };
        assert!(connected(last(x + 1), "west"), "placed fence connects west");
        assert!(connected(last(x), "east"), "existing fence connects east");
        assert!(
            changes.iter().all(|change| change.x == x || change.x == x + 1),
            "no other cell may change: {changes:?}"
        );

        // Bedrock viewers get the same updates (`UpdateBlockPacket`).
        let broadcast = world.take_broadcast_changes();
        assert_eq!(broadcast, changes, "both queues carry the same changes");
    }

    #[test]
    fn java_and_bedrock_queues_are_independent() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        assert!(world.set_block(0, 64, 0, 7));
        // Draining one consumer leaves the other untouched.
        assert_eq!(world.take_changes().len(), 1);
        assert_eq!(world.pending_broadcast_changes(), 1);
        assert_eq!(
            world.take_broadcast_changes(),
            vec![BlockChange::new(0, 64, 0, 7)]
        );
        assert!(world.take_broadcast_changes().is_empty());
    }

    #[test]
    fn block_change_queue_is_capped_and_drops_oldest() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        for index in 0..(BLOCK_CHANGE_QUEUE_CAP + 10) {
            // Distinct coordinates so every call is a real change.
            assert!(world.set_block(index as i32, 64, 0, 1));
        }
        assert_eq!(world.pending_block_changes(), BLOCK_CHANGE_QUEUE_CAP);
        let drained = world.take_changes();
        assert_eq!(drained.len(), BLOCK_CHANGE_QUEUE_CAP);
        // Oldest 10 were dropped; the survivor starts at x=10.
        assert_eq!(drained[0].x, 10);
        assert_eq!(
            drained[BLOCK_CHANGE_QUEUE_CAP - 1].x,
            (BLOCK_CHANGE_QUEUE_CAP + 9) as i32
        );
    }

    #[test]
    fn chunk_object_cache_is_shared_and_invalidated_by_edits() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        assert!(!world.has_chunk_cached(3, -2));
        assert_eq!(world.chunk_version(3, -2), 0);

        let first = world.get_chunk_cached(3, -2);
        assert!(world.has_chunk_cached(3, -2));
        // A second request is served from the cache (same block data, no
        // regeneration); equality over the subchunks proves the hit.
        assert_eq!(
            world.get_chunk(3, -2).subchunks.len(),
            first.subchunks.len()
        );

        // Editing a block bumps the version and invalidates both the
        // object cache and any converted Java payload keyed by it.
        assert!(world.set_block(3 * 16 + 1, 64, -2 * 16 + 1, 99));
        assert_eq!(world.chunk_version(3, -2), 1);
        assert!(!world.has_chunk_cached(3, -2));

        let rebuilt = world.get_chunk_cached(3, -2);
        assert_eq!(world.chunk_version(3, -2), 1);
        assert!(world.has_chunk_cached(3, -2));
        // The rebuild carries the edit.
        // Local coords of block (49, 64, -31) inside column (3, -2).
        assert_eq!(rebuilt.get_block(1, 64, 1), 99);
    }

    #[test]
    fn chunk_payload_is_cached_and_invalidated_by_edits() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let first = world.chunk_payload(3, -2);
        // A second request returns the same bytes (served from the cache
        // instead of regenerating the column).
        assert_eq!(world.chunk_payload(3, -2), first);
        assert_eq!(world.chunk_cache.lock().entries.len(), 1);

        // Editing a block in the column invalidates its cached payload:
        // the next request regenerates and includes the edit.
        assert!(world.set_block(3 * 16 + 1, 64, -2 * 16 + 1, 99));
        assert!(!world.chunk_cache.lock().entries.contains_key(&(3, -2)));
        let edited = world.chunk_payload(3, -2);
        assert_ne!(edited, first);
        assert_eq!(world.chunk_cache.lock().entries.len(), 1);
        // And the edited payload is itself cached again.
        assert_eq!(world.chunk_payload(3, -2), edited);
    }

    #[test]
    fn a_detached_torch_breaks_when_its_support_is_removed() {
        use bedrock_world::block_id::AIR;
        use bedrock_world::block_states;

        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let stone = block_states::network_id("stone").unwrap();
        let torch = block_states::network_id("torch").unwrap();
        assert!(world.set_block(5, 20, 5, stone));
        assert!(world.set_block(5, 21, 5, torch));
        assert_eq!(world.block_at(5, 21, 5), torch);

        // `Torch::onNearbyBlockChange()` -> `World::useBreakOn()`.
        world.set_block(5, 20, 5, AIR);
        assert_eq!(world.block_at(5, 21, 5), AIR);
    }

    #[test]
    fn a_torch_that_loses_its_support_is_destroyed_with_its_particles() {
        use bedrock_protocol::packets::level_event::LEVEL_EVENT_PARTICLE_DESTROY_BLOCK;
        use bedrock_world::block_id::AIR;
        use bedrock_world::block_states;

        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let stone = block_states::network_id("stone").unwrap();
        let torch = block_states::network_id("torch").unwrap();
        world.set_block(6, 20, 6, stone);
        world.set_block(6, 21, 6, torch);
        world.take_pending_level_events();

        // `StaticSupportTrait::onNearbyBlockChange()` -> `World::useBreakOn()`.
        world.set_block(6, 20, 6, AIR);
        assert_eq!(world.block_at(6, 21, 6), AIR);
        let events = world.take_pending_level_events();
        assert!(
            events.iter().any(|(event_id, data, x, y, z)| {
                *event_id == LEVEL_EVENT_PARTICLE_DESTROY_BLOCK
                    && *data == torch as i32
                    && (*x, *y, *z) == (6, 21, 6)
            }),
            "the destroyed torch must queue EVENT_PARTICLE_DESTROY: {events:?}"
        );
    }

    #[test]
    fn a_lantern_breaks_when_its_ceiling_is_removed() {
        use bedrock_world::block_id::AIR;
        use bedrock_world::block_states;

        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let stone = block_states::network_id("stone").unwrap();
        let lantern = block_states::network_id("lantern").unwrap();
        let hanging = block_states::with_property(lantern, "hanging", "true").unwrap();
        world.set_block(8, 22, 8, stone);
        world.set_block(8, 21, 8, hanging);
        assert_eq!(world.block_at(8, 21, 8), hanging);

        world.set_block(8, 22, 8, AIR);
        assert_eq!(world.block_at(8, 21, 8), AIR);
    }

    #[test]
    fn an_unsupported_sand_block_falls_until_it_lands() {
        use bedrock_world::block_id::AIR;
        use bedrock_world::block_states;

        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let stone = block_states::network_id("stone").unwrap();
        let sand = block_states::network_id("sand").unwrap();
        // A platform in the air (the flat surface is at y = 3).
        world.set_block(2, 20, 2, stone);
        world.set_block(2, 21, 2, sand);
        assert_eq!(world.block_at(2, 21, 2), sand);
        assert_eq!(world.entity_count(), 0);

        // `FallableTrait::onNearbyBlockChange()`: the sand leaves the world.
        world.set_block(2, 20, 2, AIR);
        assert_eq!(world.block_at(2, 21, 2), AIR);
        assert_eq!(world.entity_count(), 1);

        for _ in 0..64 {
            world.tick_entities();
        }
        assert_eq!(world.entity_count(), 0);
        // It landed on the grass at y = 3.
        assert_eq!(world.block_at(2, 4, 2), sand);
    }

    /// `look_at_player`: with a player nearby the head eventually turns to
    /// face it, holds for a while and then returns to the body yaw; with no
    /// players it never leaves the body.
    #[test]
    fn a_mob_turns_its_head_towards_a_nearby_player() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let id = world
            .spawn_mob("cow", Vector3f::new(4.5, 4.0, 4.5))
            .expect("cow is spawnable");
        let current_cow = || {
            world
                .entities_snapshot()
                .into_iter()
                .find(|entity| entity.unique_id == id)
                .expect("the cow is alive")
        };

        // No players: the head follows the body.
        for _ in 0..200 {
            world.tick_entities();
            world.look_at_nearest_players();
            assert_eq!(current_cow().head_yaw, current_cow().yaw);
        }

        // A player two blocks east: the head turns towards it.
        let mut gaze = None;
        for _ in 0..5000 {
            world.tick_entities();
            let current = current_cow();
            let target = (current.position.x + 2.0, current.position.y, current.position.z);
            world.set_player_positions(vec![target]);
            world.look_at_nearest_players();
            let after = current_cow();
            if after.head_yaw != after.yaw {
                gaze = Some((after, target));
                break;
            }
        }
        let (cow, target) = gaze.expect("the cow looks at a nearby player");
        // Player due east: Bedrock yaw of the direction vector (1, 0) is 270.
        let expected = (-(target.0 - cow.position.x))
            .atan2(target.2 - cow.position.z)
            .to_degrees()
            .rem_euclid(360.0);
        let delta = (cow.head_yaw - expected).abs();
        assert!(
            delta.min(360.0 - delta) < 2.0,
            "head yaw {} does not face the player at {expected}",
            cow.head_yaw
        );

        // The gaze expires and the head returns to the body yaw.
        world.set_player_positions(Vec::new());
        let mut returned = false;
        for _ in 0..crate::entity::MOB_LOOK_TICKS * 2 {
            world.tick_entities();
            world.look_at_nearest_players();
            let cow = current_cow();
            if cow.head_yaw == cow.yaw {
                returned = true;
                break;
            }
        }
        assert!(returned, "the head returns to the body after the gaze");
    }

    /// The wander pass must turn the body towards the movement it sets:
    /// `Entity::getDirectionVector()` is `(-sin yaw, cos yaw)` in Bedrock.
    #[test]
    fn a_walking_mob_faces_where_it_walks() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let id = world
            .spawn_mob("cow", Vector3f::new(4.5, 4.0, 4.5))
            .expect("cow is spawnable");
        let mut headings = std::collections::HashSet::new();
        for tick in 0..300 {
            world.tick_entities();
            let Some(entity) = world
                .entities_snapshot()
                .into_iter()
                .find(|entity| entity.unique_id == id)
            else {
                break;
            };
            let speed = entity.motion.x.hypot(entity.motion.z);
            if speed <= 0.001 {
                continue;
            }
            let expected = (-entity.motion.x)
                .atan2(entity.motion.z)
                .to_degrees()
                .rem_euclid(360.0);
            let delta = (entity.yaw - expected).abs();
            let delta = delta.min(360.0 - delta);
            assert!(
                delta < 2.0,
                "tick {tick}: body yaw {} does not face the walking heading {expected}",
                entity.yaw
            );
            headings.insert((entity.yaw as i32) / 10);
        }
        assert!(
            headings.len() > 1,
            "the wander pass picks a new heading occasionally (saw {headings:?})"
        );
    }

    #[test]
    fn a_spawned_mob_falls_dies_and_drops_its_loot() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let id = world
            .spawn_mob("chicken", Vector3f::new(4.5, 20.0, 4.5))
            .expect("chicken is spawnable");
        // Unknown types are rejected by the factory, like `EntityFactory`.
        assert!(world
            .spawn_mob("minecraft:not_a_mob", Vector3f::new(0.0, 0.0, 0.0))
            .is_none());

        for _ in 0..200 {
            world.tick_entities();
        }
        let zombie = world
            .entities_snapshot()
            .into_iter()
            .find(|entity| entity.unique_id == id)
            .expect("the chicken is still alive");
        assert!(
            zombie.position.y < 5.0,
            "gravity lands the mob (y={})",
            zombie.position.y
        );
        assert_eq!(zombie.health(), Some(4.0));

        let (_, outcome) = world
            .damage_entity(zombie.runtime_id, 25.0)
            .expect("a living target accepts damage");
        assert_eq!(outcome, crate::entity::DamageOutcome::Killed);
        let events = world.take_entity_events();
        assert!(events.iter().any(|event| {
            event.event_id == bedrock_protocol::packets::actor_event::EVENT_DEATH_ANIMATION
                && event.runtime_id == zombie.runtime_id
        }));

        // `Living::onDeath()` -> drops + XP; then the death animation ends.
        world.tick_entities();
        let after_death = world.entities_snapshot();
        assert!(after_death.iter().any(|entity| {
            matches!(entity.kind, crate::entity::EntityKind::Item { .. })
        }));
        assert!(after_death.iter().any(|entity| {
            matches!(entity.kind, crate::entity::EntityKind::ExperienceOrb { .. })
        }));
        for _ in 0..crate::entity::MAX_DEAD_TICKS {
            world.tick_entities();
        }
        assert!(!world
            .entities_snapshot()
            .iter()
            .any(|entity| entity.unique_id == id));
    }

    #[test]
    fn a_projectile_hit_is_queued_instead_of_applied() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let zombie = world
            .spawn_mob_configured(
                "minecraft:zombie",
                Vector3f::new(4.5, 4.0, 4.5),
                0.0,
                0.0,
                None,
            )
            .expect("zombie spawned");
        world
            .spawn_projectile(
                crate::entity::ProjectileKind::Arrow,
                Vector3f::new(4.5, 5.0, 3.8),
                Vector3f::new(0.0, 0.0, 0.5),
                "Steve",
                7,
                2.0,
                false,
            )
            .expect("arrow spawned");
        for _ in 0..6 {
            world.tick_entities();
        }

        let queued = world.take_projectile_damage();
        assert!(!queued.is_empty(), "the hit is queued for the plugin decision");
        assert_eq!(queued[0].unique_id, zombie);
        assert!(queued[0].damage > 0.0);
        // Nothing was applied yet, so a cancelled event leaves the mob intact.
        let target = world.entity_by_id(zombie).expect("zombie alive");
        assert_eq!(target.health(), Some(20.0));

        // Applying it by hand (what the session does when not cancelled).
        let hit = queued[0].clone();
        world.damage_entity(hit.runtime_id, hit.damage);
        assert!(world.entity_by_id(zombie).expect("zombie alive").health().unwrap() < 20.0);
    }

    #[test]
    fn an_arrow_sticks_where_it_hits_the_ground() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let id = world
            .spawn_projectile(
                crate::entity::ProjectileKind::Arrow,
                Vector3f::new(4.5, 12.0, 4.5),
                Vector3f::new(0.0, -1.0, 0.0),
                "Steve",
                7,
                2.0,
                true,
            )
            .expect("spawned");
        for _ in 0..40 {
            world.tick_entities();
        }
        let arrow = world
            .entities_snapshot()
            .into_iter()
            .find(|entity| entity.unique_id == id)
            .expect("a stuck arrow stays in the world");
        assert!(arrow.projectile_stuck());
        assert_eq!(arrow.motion.y, 0.0);
        assert_eq!(arrow.health(), None);
        // The movement segments were traced for the session manager.
        let hits = world.take_projectile_hits();
        assert!(hits.iter().any(|hit| hit.unique_id == id));
    }

    #[test]
    fn primed_tnt_explodes_and_destroys_blocks() {
        use bedrock_world::block_id::AIR;
        use bedrock_world::block_states;

        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let stone = block_states::network_id("stone").unwrap();
        world.set_block(6, 4, 6, stone);
        let tnt = world
            .spawn_primed_tnt(Vector3f::new(6.5, 4.0, 6.5), 1)
            .expect("spawned");
        world.tick_entities();

        // The block is listed for the event but not destroyed yet: PM fires
        // `EntityExplodeEvent` between `explodeA()` and the actual damage.
        assert!(!world
            .entities_snapshot()
            .iter()
            .any(|entity| entity.unique_id == tnt));
        let explosions = world.take_explosions();
        assert_eq!(explosions.len(), 1);
        assert_eq!(explosions[0].radius, crate::entity::TNT_EXPLOSION_RADIUS);
        assert_eq!(world.block_at(6, 4, 6), stone);
        assert!(explosions[0]
            .blocks
            .iter()
            .any(|&(x, y, z, id)| (x, y, z) == (6, 4, 6) && id == stone));

        // `SessionManager` applies the surviving list after the event.
        world.apply_explosion_blocks(&explosions[0].blocks, 100.0);
        assert_eq!(world.block_at(6, 4, 6), AIR);
    }

    #[test]
    fn queued_explosion_uses_the_requested_radius_and_block_list() {
        use bedrock_world::block_states;

        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let stone = block_states::network_id("stone").unwrap();
        world.set_block(6, 4, 6, stone);

        let blocks = world.explosion_blocks(Vector3f::new(6.5, 4.0, 6.5), 2.0);
        assert!(blocks
            .iter()
            .any(|&(x, y, z, id)| (x, y, z) == (6, 4, 6) && id == stone));

        let count = world.queue_explosion(
            Vector3f::new(6.5, 4.0, 6.5),
            2.0,
            None,
            Some(blocks.clone()),
        );
        assert_eq!(count, blocks.len());
        let explosions = world.take_explosions();
        assert_eq!(explosions.len(), 1);
        assert_eq!(explosions[0].radius, 2.0);
        assert_eq!(explosions[0].entity, None);
        assert_eq!(explosions[0].blocks, blocks);

        // Sin lista explícita el core calcula la esfera con ese radio.
        let computed = world.explosion_blocks(Vector3f::new(6.5, 4.0, 6.5), 1.0);
        world.queue_explosion(Vector3f::new(6.5, 4.0, 6.5), 1.0, None, None);
        assert_eq!(world.take_explosions()[0].blocks, computed);
    }

    #[test]
    fn summon_configures_rotation_and_name_tag() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let id = world
            .spawn_mob_configured(
                "zombie",
                Vector3f::new(1.0, 5.0, 2.0),
                90.0,
                20.0,
                Some("Boss"),
            )
            .expect("summoned");
        let entity = world
            .entities_snapshot()
            .into_iter()
            .find(|entity| entity.unique_id == id)
            .expect("spawned");
        assert_eq!(entity.yaw, 90.0);
        assert_eq!(entity.head_yaw, 90.0);
        assert_eq!(entity.pitch, 20.0);
        assert_eq!(entity.name_tag, "Boss");
        let flags = entity
            .metadata()
            .into_iter()
            .find(|entry| entry.key == 0)
            .and_then(|entry| match entry.value {
                bedrock_protocol::MetadataValue::Long(value) => Some(value),
                _ => None,
            })
            .expect("flags metadata");
        let wanted = crate::entity::FLAG_CAN_SHOW_NAMETAG | crate::entity::FLAG_ALWAYS_SHOW_NAMETAG;
        assert_eq!(flags & wanted, wanted);
    }

    #[test]
    fn damage_is_ignored_for_unknown_or_non_living_targets() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        assert!(world.damage_entity(12345, 5.0).is_none());
        let id = world
            .spawn_item(
                Vector3f::new(0.5, 4.0, 0.5),
                ItemStack::new(1, 1),
                None,
                "",
            )
            .expect("item");
        let item = world
            .entities_snapshot()
            .into_iter()
            .find(|entity| entity.unique_id == id)
            .unwrap();
        assert!(world.damage_entity(item.runtime_id, 5.0).is_none());
    }

    #[test]
    fn a_block_placed_where_a_falling_block_was_is_not_deleted() {
        use bedrock_world::block_id::AIR;
        use bedrock_world::block_states;

        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let stone = block_states::network_id("stone").unwrap();
        let sand = block_states::network_id("sand").unwrap();
        world.set_block(9, 30, 9, stone);
        world.set_block(9, 31, 9, sand);
        world.set_block(9, 30, 9, AIR);
        assert_eq!(world.entity_count(), 1);

        // Somebody replaces the falling block's cell while the entity is in
        // the air. The entity keeps falling (`Entity::move()` only clamps
        // against the blocks it would enter) and the placed block survives.
        world.set_block(9, 31, 9, stone);
        for _ in 0..80 {
            world.tick_entities();
        }

        assert_eq!(world.block_at(9, 31, 9), stone, "the placed block survives");
        assert_eq!(world.entity_count(), 0, "the fall finished elsewhere");
        assert_eq!(world.block_at(9, 4, 9), sand, "it landed on the grass");
    }

    #[test]
    fn a_scheduled_update_restores_the_block_and_queues_its_sound() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let button = bedrock_world::block_states::with_property(
            bedrock_world::block_states::network_id("stone_button").unwrap(),
            "facing_direction",
            "1",
        )
        .unwrap();
        let pressed =
            bedrock_world::block_states::with_property(button, "button_pressed_bit", "true")
                .unwrap();
        world.set_block(4, 79, 4, bedrock_world::block_states::network_id("stone").unwrap());
        world.set_block(4, 80, 4, pressed);
        // PM `World::scheduleDelayedBlockUpdate($pos, Button::getActivationTime())`.
        world.schedule_delayed_block_update(4, 80, 4, button, 20, Some("power.off"));
        for _ in 0..19 {
            world.tick();
        }
        assert_eq!(world.block_at(4, 80, 4), pressed);
        assert!(world.take_pending_sounds().is_empty());

        world.tick();
        // `Button::onScheduledUpdate()`: released state plus `RedstonePowerOffSound`.
        assert_eq!(world.block_at(4, 80, 4), button);
        assert_eq!(
            world.take_pending_sounds(),
            vec![("power.off".to_string(), -1, 4, 80, 4)]
        );
        assert_eq!(world.scheduled_block_update_count(), 0);
    }

    #[test]
    fn biome_overrides_apply_to_reads_and_invalidate_the_payload() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let original = world.biome_at(3, 64, 4);
        world.chunk_payload(0, 0);
        assert!(world.chunk_cache.lock().entries.contains_key(&(0, 0)));

        // `World::setBiomeId()`: per-column override, payload invalidated.
        assert!(world.set_biome(3, 64, 4, 2));
        assert_eq!(world.biome_at(3, 64, 4), 2);
        assert!(!world.chunk_cache.lock().entries.contains_key(&(0, 0)));
        assert_eq!(world.biome_at(4, 64, 4), original, "other columns untouched");

        // A no-change call reports false and keeps the stored value.
        assert!(!world.set_biome(3, 64, 4, 2));

        // The override is re-applied by `build_chunk`: it survives every cache
        // being dropped and reaches a freshly serialized payload.
        world.chunk_cache.lock().entries.clear();
        world.loaded_columns.lock().entries.clear();
        assert_eq!(world.biome_at(3, 64, 4), 2);
        let (_, payload) = world.chunk_payload(0, 0);
        assert!(!payload.is_empty());
    }

    #[test]
    fn php_effects_queue_with_recipients_until_the_tick_dispatches_them() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        world.queue_world_effect(PendingWorldEffect::Sound {
            sound: Some("place".to_string()),
            event_id: None,
            extra_data: 42,
            x: 1.5,
            y: 64.0,
            z: -2.5,
            recipients: vec!["steve".to_string()],
        });
        world.queue_world_effect(PendingWorldEffect::Particle {
            event_id: 0x4000 | 8,
            event_data: 0,
            x: 3.25,
            y: 65.5,
            z: 4.0,
            recipients: vec![],
        });

        let effects = world.take_world_effects();
        assert_eq!(effects.len(), 2);
        assert!(matches!(
            &effects[0],
            PendingWorldEffect::Sound { sound, recipients, .. }
                if sound.as_deref() == Some("place") && recipients == &vec!["steve".to_string()]
        ));
        assert!(matches!(
            &effects[1],
            PendingWorldEffect::Particle { event_id, .. } if *event_id == 0x4000 | 8
        ));
        assert!(world.take_world_effects().is_empty());
        // La cola de efectos no se mezcla con las del core.
        assert!(world.take_pending_sounds_at().is_empty());
        assert!(world.take_pending_level_events_at().is_empty());
    }

    #[test]
    fn php_sounds_and_particles_keep_the_exact_position() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        world.add_sound_at("place", 7, 1.5, 64.25, -2.5);
        assert_eq!(
            world.take_pending_sounds_at(),
            vec![("place".to_string(), 7, 1.5, 64.25, -2.5)]
        );
        world.add_level_event_at(66, 0, 1.25, 64.5, -2.75);
        assert_eq!(
            world.take_pending_level_events_at(),
            vec![(66, 0, 1.25, 64.5, -2.75)]
        );
        // La cola del core no se mezcla con la de PHP.
        assert!(world.take_pending_sounds().is_empty());
        assert!(world.take_pending_level_events().is_empty());
    }

    #[test]
    fn a_scheduled_update_runs_with_the_clock_stopped() {
        // `/time stop` must not freeze scheduled block updates: they count
        // ticks, not world time (PM keeps ticking the scheduled queue).
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        world.stop_time();
        world.set_block(6, 80, 6, 42);
        world.schedule_delayed_block_update(6, 80, 6, 0, 1, None);
        world.tick();
        assert!(world.take_pending_sounds().is_empty());
        assert_eq!(world.block_at(6, 80, 6), 0);
    }
}
