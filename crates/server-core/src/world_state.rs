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

use bedrock_world::chunk::{Chunk, MIN_SUBCHUNK_INDEX};
use bedrock_world::WorldGenerator;
use parking_lot::{Mutex, RwLock};

/// Absolute block coordinate -> network block id.
type BlockEdits = HashMap<(i32, i32, i32), u32>;

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
    /// Tick counter driving the scheduled queue. It advances even with
    /// `/time stop`, because scheduled updates are counted in ticks.
    scheduled_tick: AtomicI32,
    /// Sounds produced while the queue fired, drained once per tick by
    /// `SessionManager::tick` (PM `World::addSound` from `onScheduledUpdate`).
    pending_sounds: Mutex<Vec<(String, i32, i32, i32, i32)>>,
    edits: RwLock<HashMap<(i32, i32), BlockEdits>>,
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
    /// `FallingBlock` entities of the reference, simulated as blocks that
    /// move down one cell per tick (see [`FallingBlock`]).
    pending_falls: Mutex<Vec<FallingBlock>>,
    /// `LevelEventPacket`s produced by the world itself (PM
    /// `World::addSound()` with a level-event sound, e.g. `AnvilFallSound`):
    /// `(event id, event data, x, y, z)`. Drained once per tick by
    /// `SessionManager`.
    pending_level_events: Mutex<Vec<(i32, i32, i32, i32, i32)>>,
    /// In-memory [`Chunk`] per column, shared by every
    /// [`WorldState::get_chunk_cached`] caller (Java bridge chunk build,
    /// background prefetch in main.rs). `get_chunk` still regenerates on
    /// every call for legacy callers; the cached variant serves the
    /// ~18 ms generator work from memory and is what the Java tick now
    /// consumes after the prefetch warms it. Invalidated per-chunk by
    /// [`WorldState::set_block`].
    chunk_object_cache: Mutex<ChunkObjectCache>,
}

/// `FallingBlock::getInitialGravity()`.
pub const FALL_GRAVITY: f32 = 0.04;
/// `FallingBlock::getInitialDragMultiplier()`.
pub const FALL_DRAG: f32 = 0.02;
/// Maximum falls simulated at once: a griefing sand column must not grow the
/// fall list without bound (`BLOCK_CHANGE_QUEUE_CAP` bounds the broadcasts,
/// this bounds the simulation).
pub const MAX_PENDING_FALLS: usize = 256;

/// A falling block, translated from `src/entity/object/FallingBlock.php`.
///
/// The reference spawns an entity that falls with gravity/drag and turns back
/// into a block when it lands. This server has no block-entity simulation, so
/// the block itself moves one cell down per tick (the same animation a falling
/// entity shows at terminal velocity, ~0.98 blocks/tick) and is restored with
/// the normal neighbour cascade on landing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FallingBlock {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: u32,
    /// `Entity::$motion.y` in blocks/tick (negative when falling).
    pub motion: f32,
    /// `Entity::$fallDistance`, kept for `onHitGround()` consumers.
    pub fall_distance: f32,
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
const CHUNK_PAYLOAD_CACHE_CAP: usize = 512;

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
            scheduled_tick: AtomicI32::new(0),
            pending_sounds: Mutex::new(Vec::new()),
            edits: RwLock::new(HashMap::new()),
            block_changes: RwLock::new(VecDeque::new()),
            broadcast_changes: RwLock::new(VecDeque::new()),
            chunk_cache: Mutex::new(ChunkPayloadCache {
                entries: HashMap::new(),
                order: VecDeque::new(),
            }),
            chunk_versions: Mutex::new(HashMap::new()),
            pending_falls: Mutex::new(Vec::new()),
            pending_level_events: Mutex::new(Vec::new()),
            chunk_object_cache: Mutex::new(ChunkObjectCache {
                entries: HashMap::new(),
                order: VecDeque::new(),
            }),
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
        self.run_scheduled_block_updates(now);
        if self.is_time_running() {
            self.set_time(self.time().saturating_add(1))
        } else {
            self.time()
        }
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

    /// PM `World::useItemOn()` interaction half: `$blockClicked->onInteract()`.
    /// Buttons press (and schedule their own release), levers toggle and fence
    /// gates open/close. Returns the interaction so the caller can broadcast
    /// the matching sound (`power.on`, `power.off`, `SOUND_DOOR`); `None` when
    /// the block has no `onInteract()` and the click must fall through to item
    /// placement.
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
        self.flush_changes(changes);
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

        self.edits
            .write()
            .entry((x >> 4, z >> 4))
            .or_default()
            .insert((x, y, z), block_id);
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
                tracing::debug!("world.physics_break: ({},{},{}) {:#010X}", bx, by, bz, id);
                for face in crate::physics::ALL_FACES {
                    let (dx, dy, dz) = crate::physics::offset(face);
                    frontier.push_back((bx + dx, by + dy, bz + dz));
                }
            }
        }
    }

    /// Starts the falling-block simulation for a block that just left the
    /// world (`new FallingBlock(...)->spawnToAll()` in the reference).
    pub fn start_falling(&self, x: i32, y: i32, z: i32, block_id: u32) {
        let mut pending = self.pending_falls.lock();
        if pending.len() >= MAX_PENDING_FALLS {
            // Over the budget the block is dropped instead of simulated, which
            // is what the reference effectively does at its entity cap.
            return;
        }
        pending.push(FallingBlock {
            x,
            y,
            z,
            block_id,
            motion: 0.0,
            fall_distance: 0.0,
        });
    }

    /// Number of falling blocks still simulated.
    pub fn pending_fall_count(&self) -> usize {
        self.pending_falls.lock().len()
    }

    /// Queues a `LevelEventPacket` (PM `World::addSound()` /
    /// `World::addParticle()` with a level-event implementation).
    pub fn add_level_event(&self, event_id: i32, event_data: i32, x: i32, y: i32, z: i32) {
        self.pending_level_events
            .lock()
            .push((event_id, event_data, x, y, z));
    }

    /// Drains the level events queued since the last tick, oldest first.
    pub fn take_pending_level_events(&self) -> Vec<(i32, i32, i32, i32, i32)> {
        std::mem::take(&mut *self.pending_level_events.lock())
    }

    /// Advances every falling block one tick, translating
    /// `FallingBlock::entityBaseTick()` + `Entity::tryChangeMovement()`.
    ///
    /// Returns the blocks that landed this tick (restored into the world),
    /// for `FallingBlock::onHitGround()` consumers.
    pub fn tick_falling_blocks(&self) -> Vec<LandedBlock> {
        use bedrock_world::block_id::AIR;
        use bedrock_world::block_states;

        let falls = {
            let mut pending = self.pending_falls.lock();
            std::mem::take(&mut *pending)
        };
        if falls.is_empty() {
            return Vec::new();
        }

        let mut still_falling: Vec<FallingBlock> = Vec::with_capacity(falls.len());
        let mut landed: Vec<LandedBlock> = Vec::new();
        for mut fall in falls {
            if !(WORLD_MIN_Y..WORLD_MAX_Y).contains(&fall.y) {
                continue;
            }
            // `mY -= gravity; mY *= (1 - drag)`: converges to ~0.98 blocks/tick.
            fall.motion = (fall.motion + FALL_GRAVITY) * (1.0 - FALL_DRAG);
            fall.fall_distance += fall.motion;

            // The cell is empty (first tick) or still holds this simulated
            // falling block; anything else in it means another edit replaced
            // the falling block while it was in the air. `entityBaseTick()`
            // cannot place the block there (`!$block->canBeReplaced()`), so the
            // reference drops the item and plays `BlockBreakSound` instead of
            // overwriting: this port has no item entities, so the block is
            // lost, the cell is preserved and the sound still plays.
            let current = self.block_at(fall.x, fall.y, fall.z);
            let current_name = block_states::state_name(current);
            let occupied_by_other = current != fall.block_id
                && !current_name.is_some_and(crate::physics::is_replaceable);
            if occupied_by_other {
                self.add_sound(
                    bedrock_protocol::packets::level_sound_event::LEVEL_SOUND_BREAK,
                    fall.block_id as i32,
                    fall.x,
                    fall.y,
                    fall.z,
                );
                continue;
            }

            let below = self.block_at(fall.x, fall.y - 1, fall.z);
            let below_replaceable =
                block_states::state_name(below).is_some_and(crate::physics::is_replaceable);
            if below_replaceable && fall.y - 1 > WORLD_MIN_Y {
                // Moving down one cell: leave air behind and occupy the next
                // cell. Both edits are broadcast as `UpdateBlockPacket`s, so
                // viewers see the block step down every tick.
                self.set_block_with(fall.x, fall.y, fall.z, AIR, false);
                self.set_block_with(fall.x, fall.y - 1, fall.z, fall.block_id, false);
                fall.y -= 1;
                still_falling.push(fall);
            } else {
                // `entityBaseTick()`: on the ground the block is restored and
                // the neighbour cascade runs for its final cell.
                self.set_block_with(fall.x, fall.y, fall.z, fall.block_id, true);
                // `Anvil::getLandSound()` -> `AnvilFallSound` ->
                // `LevelEventPacket(SOUND_ANVIL_FALL)` at the landing cell.
                if block_states::state_name(fall.block_id).is_some_and(|n| n.ends_with("anvil")) {
                    self.add_level_event(
                        bedrock_protocol::packets::level_event::LEVEL_EVENT_SOUND_ANVIL_FALL,
                        0,
                        fall.x,
                        fall.y,
                        fall.z,
                    );
                }
                landed.push(LandedBlock {
                    x: fall.x,
                    y: fall.y,
                    z: fall.z,
                    block_id: fall.block_id,
                    fall_distance: fall.fall_distance,
                });
            }
        }
        self.pending_falls.lock().extend(still_falling);
        landed
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

        let payload = self
            .generator
            .generate_chunk_with_edits(chunk_x, chunk_z, &edits);

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
        self.build_chunk(chunk_x, chunk_z)
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
            return chunk;
        }
        if !cache.entries.contains_key(&(chunk_x, chunk_z)) {
            cache.order.push_back((chunk_x, chunk_z));
            while cache.order.len() > CHUNK_OBJECT_CACHE_CAP {
                if let Some(oldest) = cache.order.pop_front() {
                    cache.entries.remove(&oldest);
                }
            }
        }
        cache.entries.insert((chunk_x, chunk_z), chunk.clone());
        chunk
    }

    /// Uncached column build: base terrain plus persistent edits.
    fn build_chunk(&self, chunk_x: i32, chunk_z: i32) -> bedrock_world::chunk::Chunk {
        let mut chunk = self.generator.generate_chunk_object(chunk_x, chunk_z);
        if let Some(edits) = self.edits.read().get(&(chunk_x, chunk_z)) {
            for ((x, y, z), id) in edits.iter() {
                chunk.set_block((*x & 15) as usize, *y, (*z & 15) as usize, *id);
            }
        }
        chunk
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
    pub fn save(&self) -> std::io::Result<()> {
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
        assert_eq!(world.pending_fall_count(), 0);

        // `FallableTrait::onNearbyBlockChange()`: the sand leaves the world.
        world.set_block(2, 20, 2, AIR);
        assert_eq!(world.block_at(2, 21, 2), AIR);
        assert_eq!(world.pending_fall_count(), 1);

        for _ in 0..64 {
            world.tick_falling_blocks();
        }
        assert_eq!(world.pending_fall_count(), 0);
        // It landed on the grass at y = 3.
        assert_eq!(world.block_at(2, 4, 2), sand);
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
        assert_eq!(world.pending_fall_count(), 1);

        // Somebody replaces the falling block's cell while it is in the air.
        world.set_block(9, 31, 9, stone);
        world.tick_falling_blocks();

        assert_eq!(world.block_at(9, 31, 9), stone, "the placed block survives");
        assert_eq!(world.pending_fall_count(), 0, "the stale fall is dropped");
    }

    #[test]
    fn a_scheduled_update_restores_the_block_and_queues_its_sound() {
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        let button = bedrock_world::block_states::network_id("stone_button").unwrap();
        let pressed =
            bedrock_world::block_states::with_property(button, "button_pressed_bit", "true")
                .unwrap();
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
