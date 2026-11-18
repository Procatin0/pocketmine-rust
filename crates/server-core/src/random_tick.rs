//! Random block ticks: grass spread and decay, leaf decay, crop growth,
//! saplings, sugar cane, cactus and vines.
//!
//! # Model
//!
//! The engine side of the mechanic is described at
//! <https://minecraft.wiki/w/Tick#Random_tick>: every game tick a chunk
//! selects `randomTickSpeed` block positions in each of its 16x16x16
//! sections (the Java default is 3, so one position in 4096 is sampled three
//! times, ~1/1365 game ticks per block). A sampled position is only acted on
//! when the block at it has a random-tick behaviour; air and plain stone are
//! sampled and ignored.
//!
//! [`WorldState::random_tick`] implements that sampling loop for the chunks
//! the caller marks as ticking (`SessionManager` passes the simulation area
//! around every in-game player, Bedrock's `simulation-distance` of 4 chunks).
//! Reads go through [`TickScene`], a per-pass snapshot of the columns the
//! server already streamed to a client (same data as
//! [`WorldState::loaded_block_at`], without a lock and two hash lookups per
//! block); a column that was never loaded returns `None` and is skipped,
//! exactly like a chunk nobody is near.
//!
//! # Per-block rules
//!
//! Numbers are from the Minecraft Wiki pages linked next to each handler.
//! Edition differences are resolved towards Bedrock, which is the primary
//! client here; the one Java rule kept is the leaf support distance (see
//! [`LEAF_DECAY_DISTANCE`]).
//!
//! | Block | Rule | Source |
//! |---|---|---|
//! | Grass block | dies under an opaque cover or light < 4, spreads into plain dirt within 3x5x3 when light above is >= 9 | <https://minecraft.wiki/w/Grass_Block> |
//! | Leaves | decay on a random tick when no log/wood is reachable through leaves | <https://minecraft.wiki/w/Leaves> |
//! | Wheat/carrots/potatoes/beetroot | one growth stage per random tick at `1/(floor(25/speed)+1)`, light >= 9, farmland speed 2 dry / 4 hydrated | <https://minecraft.wiki/w/Tutorial:Crop_farming> |
//! | Sapling | 1/7 per random tick to age, then 1/7 to grow the tree, light >= 9 | <https://minecraft.wiki/w/Sapling> |
//! | Sugar cane / cactus | one growth step per 16 random ticks (`age` 0..15), stops at height 3 | <https://minecraft.wiki/w/Sugar_Cane> <https://minecraft.wiki/w/Cactus> |
//! | Vines | 25% per random tick to spread one block down its wall | <https://minecraft.wiki/w/Vines> |
//!
//! # Documented divergences
//!
//! * **Light.** This server has no light-propagation engine, so
//!   [`sky_light`] re-derives sunlight from the column above a cell (the
//!   terrain surface plus any edit in that column) instead of reading a
//!   propagated skylight array. A cave under an overhang in a *neighbouring*
//!   column reads as lit. `World::getFullLight` returns block+sky light; block
//!   light (torches, lava) is not modelled at all, so a torch-lit cellar does
//!   not grow crops.
//! * **Leaf drops.** A decayed leaf block leaves the world; its sapling/stick/
//!   apple drops are not generated.
//! * **Bone meal** (`BoneMealItem::onInteractBlock`) is a different mechanic
//!   and is not part of the random-tick path.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use bedrock_world::block_id::{self, AIR};
use bedrock_world::block_states;
use bedrock_world::generator::manager::ChunkManager;
use bedrock_world::generator::populators::tree::{TreePopulator, TreeType};
use bedrock_world::generator::random::Random;

use crate::world_state::{ColumnView, WorldState, WORLD_MAX_Y, WORLD_MIN_Y};

/// `randomTickSpeed`: block positions sampled per 16x16x16 section per game
/// tick.
///
/// 3 is the Java default and the value this fixed-rate model is calibrated
/// for (one position in 4096 sampled three times, ~1/1365 ticks per block).
/// Bedrock's own default (1) is a *relative* speed in its engine, so it does
/// not translate to a sample count here.
pub const DEFAULT_RANDOM_TICK_SPEED: i32 = 3;

/// Bedrock's `simulation-distance` in chunks: the radius around a player whose
/// columns keep ticking. `chunk-ticking.tick-radius` in `pocketmine.yml` has
/// the same default (4), so a 9x9 area per player.
pub const SIMULATION_DISTANCE: i32 = 4;

/// Height of one chunk section (`Tick`: 16x16x16 = 4096 positions).
const SECTION_HEIGHT: i32 = 16;

/// Sections in a column: `-64..320` at [`MIN_SUBCHUNK_INDEX`] `-4..19`.
const SECTIONS: i32 = (WORLD_MAX_Y - WORLD_MIN_Y) / SECTION_HEIGHT;

/// Blocks a leaf may be from a log/wood and still be alive.
///
/// Java's reachable distance is 6 (the blockstate `distance` runs to 7 for
/// "decayed"); Bedrock documents 4. The port keeps Java's 6 because the
/// server's own [`TreePopulator`] oaks put canopy corners 5 steps from the
/// trunk, so Bedrock's 4 would dissolve trees the generator just placed.
/// <https://minecraft.wiki/w/Leaves#Decay>
const LEAF_DECAY_DISTANCE: i32 = 6;

/// Natural height cap of sugar cane and cactus (`SugarCane`, `Cactus`).
const PLANT_MAX_HEIGHT: i32 = 3;

/// `age` value at which sugar cane and cactus add a block: 16 random ticks,
/// 15 for the tick that adds it and 15 more counted up from 0.
const PLANT_GROWTH_TICKS: i32 = 15;

/// SplitMix64. The tick pass needs "unpredictable enough", not a reproducible
/// engine stream, so the world keeps one 64-bit counter and derives a fresh
/// stream per tick from it.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// `mt_rand($min, $max)`, inclusive.
    fn range(&mut self, min: i32, max: i32) -> i32 {
        let span = (max - min + 1) as u64;
        min + (self.next_u64() % span) as i32
    }

    /// Uniform in `[0, 1)`.
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// `Block::ticksRandomly()` for the blocks this module implements.
///
/// Everything else (including air, the most commonly sampled block) is
/// sampled and ignored, which is what the engine does too.
pub fn ticks_randomly(id: u32) -> bool {
    let Some(name) = block_states::state_name(id) else {
        return false;
    };
    name == "grass_block"
        || name.ends_with("_leaves")
        || matches!(name, "wheat" | "carrots" | "potatoes" | "beetroot")
        || name.ends_with("_sapling")
        || matches!(name, "reeds" | "cactus" | "vine")
}

/// Per-pass read cache for the random-tick sampler.
///
/// One player at `simulation-distance: 4` samples 5 832 positions per tick
/// (3 per 16x16x16 section over 81 columns) and every sampled leaf runs a
/// support BFS. Reading that through the live world costs two locks and two
/// hash lookups per position, which made `tick.random_blocks` 70% of the tick.
/// The scene loads each touched column once ([`ColumnView`]: `Arc` snapshots,
/// no copies) and answers from memory; any write from the pass drops it, so
/// every read still sees the blocks the pass changed.
struct TickScene<'a> {
    world: &'a WorldState,
    columns: RefCell<Columns>,
}

#[derive(Default)]
struct Columns {
    index: HashMap<(i32, i32), usize>,
    views: Vec<ColumnView>,
    /// Highest shading cell per `(x, z)`: a grass/crop tick walks the column
    /// under it, and every walk asked the terrain cache for the same answer.
    tops: HashMap<(i32, i32), i32>,
    /// Last resolved column. Reads are spatially clustered (a leaf BFS stays
    /// around the sample), so this answers most of them without hashing.
    last: Option<((i32, i32), usize)>,
}

impl Columns {
    fn view(&mut self, world: &WorldState, chunk_x: i32, chunk_z: i32) -> &ColumnView {
        if let Some(((last_x, last_z), index)) = self.last {
            if last_x == chunk_x && last_z == chunk_z {
                return &self.views[index];
            }
        }
        if let Some(&index) = self.index.get(&(chunk_x, chunk_z)) {
            self.last = Some(((chunk_x, chunk_z), index));
            return &self.views[index];
        }
        let index = self.views.len();
        self.index.insert((chunk_x, chunk_z), index);
        self.views.push(world.column_view(chunk_x, chunk_z));
        self.last = Some(((chunk_x, chunk_z), index));
        &self.views[index]
    }

    fn clear(&mut self) {
        self.index.clear();
        self.views.clear();
        self.tops.clear();
        self.last = None;
    }
}

impl<'a> TickScene<'a> {
    fn new(world: &'a WorldState) -> Self {
        Self {
            world,
            columns: RefCell::new(Columns::default()),
        }
    }

    fn is_loaded(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.columns
            .borrow_mut()
            .view(self.world, chunk_x, chunk_z)
            .is_loaded()
    }

    /// Snapshot of one column. Sampling holds it for a whole chunk (the sample
    /// coordinates always fall inside `(chunk_x, chunk_z)`), so the inner loop
    /// is a plain array read instead of a cache lookup per position.
    fn column(&self, chunk_x: i32, chunk_z: i32) -> ColumnView {
        self.columns
            .borrow_mut()
            .view(self.world, chunk_x, chunk_z)
            .clone()
    }

    /// Sunlight reaching a cell, 0..15 (see the free function's documentation).
    ///
    /// The walk stays inside one column, so it takes the column snapshot once
    /// instead of resolving it per block.
    fn sky_light(&self, x: i32, y: i32, z: i32) -> u8 {
        let top = self.column_top(x, z);
        let mut columns = self.columns.borrow_mut();
        let view = columns.view(self.world, x >> 4, z >> 4);
        let mut level: i32 = 15;
        let mut cursor = y + 1;
        while cursor <= top && cursor < WORLD_MAX_Y && level > 0 {
            let id = view.block_at(x, cursor, z).unwrap_or(AIR);
            level -= attenuation(id);
            cursor += 1;
        }
        level.max(0) as u8
    }

    #[inline]
    fn block_at(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        self.columns
            .borrow_mut()
            .view(self.world, x >> 4, z >> 4)
            .block_at(x, y, z)
    }

    #[inline]
    fn block_or_air(&self, x: i32, y: i32, z: i32) -> u32 {
        self.block_at(x, y, z).unwrap_or(AIR)
    }

    /// Highest cell of the column that can shade it: the loaded base terrain or
    /// any edit above it, whichever is higher.
    fn column_top(&self, x: i32, z: i32) -> i32 {
        let mut columns = self.columns.borrow_mut();
        if let Some(&top) = columns.tops.get(&(x, z)) {
            return top;
        }
        let (terrain, edited) = {
            let view = columns.view(self.world, x >> 4, z >> 4);
            (
                view.highest_block_at(x, z),
                view.highest_edit_at(x, z),
            )
        };
        let top = terrain.unwrap_or(WORLD_MIN_Y).max(edited.unwrap_or(WORLD_MIN_Y));
        columns.tops.insert((x, z), top);
        top
    }

    /// Writes through to the world and drops the snapshots: a block the pass
    /// changed must be visible to its next read.
    fn set_block(&self, x: i32, y: i32, z: i32, id: u32) -> bool {
        let _write = crate::profiler::scope("tick.random.write");
        self.columns.borrow_mut().clear();
        self.world.set_block(x, y, z, id)
    }
}

impl WorldState {
    /// One random-tick pass over `chunks` (`World::doTick` ->
    /// `LevelChunk::randomTick`).
    ///
    /// Chunks nobody streamed are skipped: nothing would see the result and the
    /// generator would have to run on the tick path. Returns how many blocks
    /// reacted.
    pub fn random_tick(&self, chunks: &[(i32, i32)]) -> usize {
        let speed = self.random_tick_speed();
        if speed <= 0 || chunks.is_empty() {
            return 0;
        }
        let scene = TickScene::new(self);
        // Profiler breakdown: this scope's self time is sampling + snapshot
        // reads; `tick.random.rule`/`write` nest inside it. With the locks gone
        // from the sampling path, a spike inside `rule` means a rule (tree
        // growth, physics cascade) and a spike in `sample` means a stall.
        let _sample = crate::profiler::scope("tick.random.sample");
        let mut rng = Rng::new(self.next_random_tick_seed());
        let mut reacted = 0usize;
        for &(chunk_x, chunk_z) in chunks {
            if !scene.is_loaded(chunk_x, chunk_z) {
                continue;
            }
            let base_x = chunk_x << 4;
            let base_z = chunk_z << 4;
            // Every sample of this chunk reads the same column; the snapshot is
            // refreshed after a block reacts, because a rule can edit the cell
            // the next sample reads.
            let mut column = scene.column(chunk_x, chunk_z);
            for section in 0..SECTIONS {
                let base_y = WORLD_MIN_Y + section * SECTION_HEIGHT;
                for _ in 0..speed {
                    let x = base_x + rng.range(0, 15);
                    let z = base_z + rng.range(0, 15);
                    let y = base_y + rng.range(0, 15);
                    let Some(id) = column.block_at(x, y, z) else {
                        continue;
                    };
                    // Air is the most sampled block and never ticks: skip the
                    // name lookup the rules would do.
                    if id == AIR || !ticks_randomly(id) {
                        continue;
                    }
                    let changed = {
                        let _rule = crate::profiler::scope("tick.random.rule");
                        tick_block(&scene, x, y, z, id, &mut rng)
                    };
                    if changed {
                        reacted += 1;
                        column = scene.column(chunk_x, chunk_z);
                    }
                }
            }
        }
        reacted
    }
}

/// Runs the behaviour of one random-ticked block. Returns `true` when the
/// block changed the world.
fn tick_block(scene: &TickScene, x: i32, y: i32, z: i32, id: u32, rng: &mut Rng) -> bool {
    let Some(name) = block_states::state_name(id) else {
        return false;
    };
    match name {
        "grass_block" => grass_block_tick(scene, x, y, z, rng),
        "wheat" | "carrots" | "potatoes" | "beetroot" => {
            crop_tick(scene, x, y, z, id, name, rng)
        }
        "reeds" => plant_tick(scene, x, y, z, id, "reeds"),
        "cactus" => plant_tick(scene, x, y, z, id, "cactus"),
        "vine" => vine_tick(scene, x, y, z, id, rng),
        _ if name.ends_with("_leaves") => leaves_tick(scene, x, y, z, id),
        _ if name.ends_with("_sapling") => sapling_tick(scene, x, y, z, id, rng),
        _ => false,
    }
}

/// `Grass::onRandomTick()`.
///
/// Death: an opaque cover (or water/lava) above the block, or light 4 or below
/// at that cell, turns the grass back into dirt.
/// Spread: with light >= 9 above, four positions from the 3x5x3 box around the
/// block are tested; plain `dirt` with an open, lit cell above becomes grass.
fn grass_block_tick(scene: &TickScene, x: i32, y: i32, z: i32, rng: &mut Rng) -> bool {
    let above = scene.block_or_air(x, y + 1, z);
    let light_above = sky_light(scene, x, y + 1, z);
    if covers_grass(above) || light_above < 4 {
        return scene.set_block(x, y, z, block_id::DIRT);
    }
    // `GrassBlock` needs an open cell and light 9 above to spread at all.
    if !open_to_sky(above) || light_above < 9 {
        return false;
    }

    let mut changed = false;
    for _ in 0..4 {
        let target_x = x + rng.range(-1, 1);
        let target_y = y + rng.range(-3, 1);
        let target_z = z + rng.range(-1, 1);
        if scene.block_at(target_x, target_y, target_z) != Some(block_id::DIRT) {
            continue;
        }
        let target_above = scene.block_or_air(target_x, target_y + 1, target_z);
        if !open_to_sky(target_above) {
            continue;
        }
        if sky_light(scene, target_x, target_y + 1, target_z) < 4 {
            continue;
        }
        if scene.set_block(target_x, target_y, target_z, block_id::GRASS_BLOCK) {
            changed = true;
        }
    }
    changed
}

/// `Leaves::onRandomTick()`: an unsupported leaf block decays.
///
/// Player-placed leaves carry `persistent_bit` and never decay; leaves placed
/// by the generator do not, so a felled tree's canopy falls apart.
fn leaves_tick(scene: &TickScene, x: i32, y: i32, z: i32, id: u32) -> bool {
    if block_states::bool_property(id, "persistent_bit").unwrap_or(false) {
        return false;
    }
    if leaves_have_log(scene, x, y, z) {
        return false;
    }
    scene.set_block(x, y, z, AIR)
}

/// Breadth-first search through leaves for a log/wood block within
/// [`LEAF_DECAY_DISTANCE`] steps.
fn leaves_have_log(scene: &TickScene, x: i32, y: i32, z: i32) -> bool {
    /// The 6-neighbour fan-out is 6^6 in the worst case; real canopies settle
    /// in far less, and the cap keeps a pathological leaf blob bounded.
    const MAX_VISITED: usize = 512;
    /// Every visited cell is at most [`LEAF_DECAY_DISTANCE`] steps from the
    /// sample, so a fixed cube replaces the `HashSet` of coordinates: no
    /// allocation and no hashing per neighbour.
    const SPAN: i32 = 2 * LEAF_DECAY_DISTANCE + 1;

    const NEIGHBOURS: [(i32, i32, i32); 6] = [
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, -1),
        (0, 0, 1),
        (-1, 0, 0),
        (1, 0, 0),
    ];

    let cell = |px: i32, py: i32, pz: i32| -> Option<usize> {
        let dx = px - x;
        let dy = py - y;
        let dz = pz - z;
        if dx.abs() > LEAF_DECAY_DISTANCE
            || dy.abs() > LEAF_DECAY_DISTANCE
            || dz.abs() > LEAF_DECAY_DISTANCE
        {
            return None;
        }
        Some((((dx + LEAF_DECAY_DISTANCE) * SPAN + (dy + LEAF_DECAY_DISTANCE)) * SPAN
            + (dz + LEAF_DECAY_DISTANCE)) as usize)
    };

    let mut seen = [false; (SPAN * SPAN * SPAN) as usize];
    let mut visited = 1usize;
    if let Some(index) = cell(x, y, z) {
        seen[index] = true;
    }
    let mut frontier: VecDeque<(i32, i32, i32, i32)> = VecDeque::with_capacity(32);
    frontier.push_back((x, y, z, 0));

    while let Some((px, py, pz, distance)) = frontier.pop_front() {
        for (dx, dy, dz) in NEIGHBOURS {
            let (nx, ny, nz) = (px + dx, py + dy, pz + dz);
            if !(WORLD_MIN_Y..WORLD_MAX_Y).contains(&ny) {
                continue;
            }
            let Some(id) = scene.block_at(nx, ny, nz) else {
                // A column that is not loaded cannot hold the support either.
                continue;
            };
            let Some(name) = block_states::state_name(id) else {
                continue;
            };
            if is_wood(name) {
                return true;
            }
            if distance < LEAF_DECAY_DISTANCE && name.ends_with("_leaves") && visited < MAX_VISITED {
                if let Some(index) = cell(nx, ny, nz) {
                    if !seen[index] {
                        seen[index] = true;
                        visited += 1;
                        frontier.push_back((nx, ny, nz, distance + 1));
                    }
                }
            }
        }
    }
    false
}

/// `Crops::onRandomTick()` for wheat, carrots, potatoes and beetroot.
///
/// Growth chance per crop random tick is `1/(floor(25/speedLevel)+1)`; the
/// crop's own farmland contributes 4 (hydrated) or 2 (dry), every neighbour
/// 0.75/0.25, capped at 10. Wheat, carrots and potatoes have 8 stages,
/// beetroot 4.
fn crop_tick(
    scene: &TickScene,
    x: i32,
    y: i32,
    z: i32,
    id: u32,
    name: &str,
    rng: &mut Rng,
) -> bool {
    let max_growth = if name == "beetroot" { 3 } else { 7 };
    let growth = block_states::int_property(id, "growth").unwrap_or(0);
    if growth >= max_growth {
        return false;
    }
    let Some(below) = scene.block_at(x, y - 1, z) else {
        return false;
    };
    if block_states::state_name(below) != Some("farmland") {
        return false;
    }
    // `Crops` needs light 9 at the crop itself.
    if sky_light(scene, x, y, z) < 9 {
        return false;
    }

    let mut speed: f64 = if farmland_hydrated(below) { 4.0 } else { 2.0 };
    for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let Some(neighbour) = scene.block_at(x + dx, y - 1, z + dz) else {
            continue;
        };
        if block_states::state_name(neighbour) == Some("farmland") {
            speed += if farmland_hydrated(neighbour) { 0.75 } else { 0.25 };
        }
    }
    let speed = speed.min(10.0);
    let chance = 1.0 / ((25.0 / speed).floor() + 1.0);
    if rng.next_f64() >= chance {
        return false;
    }
    match block_states::with_property(id, "growth", &(growth + 1).to_string()) {
        Some(next) => scene.set_block(x, y, z, next),
        None => false,
    }
}

/// `Sapling::onRandomTick()`: age with 1/7, then grow a tree with 1/7.
///
/// The light check is `Sapling`'s own (9 above); bone meal, which skips it, is
/// a separate item path.
fn sapling_tick(scene: &TickScene, x: i32, y: i32, z: i32, id: u32, rng: &mut Rng) -> bool {
    let Some(name) = block_states::state_name(id) else {
        return false;
    };
    let Some(tree_type) = tree_type_for(name) else {
        // Acacia/dark oak/jungle/cherry/mangrove have no populator here; their
        // saplings stay saplings rather than growing a wrong-shaped tree.
        return false;
    };
    if sky_light(scene, x, y + 1, z) < 9 {
        return false;
    }
    if rng.range(0, 6) != 0 {
        return false;
    }
    let aged = block_states::bool_property(id, "age_bit").unwrap_or(false);
    if !aged {
        return block_states::with_property(id, "age_bit", "true")
            .is_some_and(|next| scene.set_block(x, y, z, next));
    }
    // Second stage: the sapling is replaced by a tree with its trunk at the
    // sapling's own cell (`Sapling::grow()` drops the sapling first).
    scene.set_block(x, y, z, AIR);
    let _grow = crate::profiler::scope("tick.random.grow");
    let mut random = Random::new(rng.next_u64() as i64);
    let mut blocks = WorldBlocks { scene };
    TreePopulator::new(tree_type, 0, 0).grow_at(&mut blocks, x, y, z, &mut random);
    true
}

/// Sugar cane (`reeds`) and cactus: `age` counts random ticks; at
/// [`PLANT_GROWTH_TICKS`] the top block adds one above it, up to
/// [`PLANT_MAX_HEIGHT`].
///
/// Only the top of a column reacts, so a 3-tall stalk is not ticked three
/// times.
fn plant_tick(scene: &TickScene, x: i32, y: i32, z: i32, id: u32, name: &str) -> bool {
    if block_states::state_name(scene.block_or_air(x, y + 1, z)) == Some(name) {
        return false;
    }
    let mut height = 1;
    while height < PLANT_MAX_HEIGHT {
        if block_states::state_name(scene.block_or_air(x, y - height, z)) == Some(name) {
            height += 1;
        } else {
            break;
        }
    }
    if height >= PLANT_MAX_HEIGHT {
        return false;
    }
    if scene.block_or_air(x, y + 1, z) != AIR {
        return false;
    }

    let age = block_states::int_property(id, "age").unwrap_or(0);
    if age < PLANT_GROWTH_TICKS {
        return match block_states::with_property(id, "age", &(age + 1).to_string()) {
            Some(next) => scene.set_block(x, y, z, next),
            None => false,
        };
    }
    let Some(base) = block_states::network_id(name) else {
        return false;
    };
    // Reset the counter and put a fresh block on top.
    scene.set_block(x, y, z, base);
    scene.set_block(x, y + 1, z, base)
}

/// `Vine`: 25% per random tick to spread one block, keeping the wall
/// attachment of the parent vine.
///
/// The reference picks a random direction and applies
/// `Vine::canBePlacedAt()`; side spreading needs a solid wall block to attach
/// to, which this port does not model, so only the downward spread (same
/// `vine_direction_bits` as the parent) is implemented.
fn vine_tick(scene: &TickScene, x: i32, y: i32, z: i32, id: u32, rng: &mut Rng) -> bool {
    if rng.range(0, 3) != 0 {
        return false;
    }
    let bits = block_states::int_property(id, "vine_direction_bits").unwrap_or(0);
    if bits == 0 {
        return false;
    }
    if scene.block_or_air(x, y - 1, z) != AIR {
        return false;
    }
    let Some(base) = block_states::network_id("vine") else {
        return false;
    };
    match block_states::with_property(base, "vine_direction_bits", &bits.to_string()) {
        Some(vine) => scene.set_block(x, y - 1, z, vine),
        None => false,
    }
}

/// Sunlight reaching a cell, 0..15.
///
/// Walks the column above the cell down from 15, subtracting each block's
/// light attenuation, and stops at the column's top block (see
/// [`column_top`]). This is the port's stand-in for the engine's propagated
/// skylight: it is exact for a column open to the sky and for one shaded by
/// its own overhang, and it ignores light arriving sideways from a lit
/// neighbour.
fn sky_light(scene: &TickScene, x: i32, y: i32, z: i32) -> u8 {
    scene.sky_light(x, y, z)
}

/// Light a block removes from the column above it (`Block::getLightFilter`).
fn attenuation(id: u32) -> i32 {
    if id == AIR {
        return 0;
    }
    match block_states::state_name(id) {
        Some(name) => attenuation_for_name(name),
        // A custom/unknown block is treated as opaque: conservative.
        None => 15,
    }
}

fn attenuation_for_name(name: &str) -> i32 {
    // Foliage and water dim light one level per block instead of blocking it.
    if name.ends_with("_leaves")
        || matches!(
            name,
            "water"
                | "flowing_water"
                | "ice"
                | "frosted_ice"
                | "short_grass"
                | "tall_grass"
                | "fern"
                | "large_fern"
                | "vine"
                | "glow_lichen"
                | "cobweb"
        )
    {
        return 1;
    }
    if block_states::is_full_cube(name) {
        return 15;
    }
    0
}

/// A block that keeps grass from surviving (`GrassBlock`): an opaque full cube,
/// or a liquid.
fn covers_grass(id: u32) -> bool {
    attenuation(id) >= 15 || is_liquid(id)
}

/// A cell grass may spread through: nothing that shades it and no liquid.
fn open_to_sky(id: u32) -> bool {
    id == AIR || (attenuation(id) == 0 && !is_liquid(id))
}

fn is_liquid(id: u32) -> bool {
    matches!(
        block_states::state_name(id),
        Some("water" | "flowing_water" | "lava" | "flowing_lava")
    )
}

/// Logs and wood are the support leaves look for; bamboo and stems are not.
fn is_wood(name: &str) -> bool {
    name.ends_with("_log") || name.ends_with("_wood") || name == "log" || name == "wood"
}

/// `Farmland::getMoisture()` > 0 means hydrated.
fn farmland_hydrated(id: u32) -> bool {
    block_states::int_property(id, "moisturized_amount").unwrap_or(0) > 0
}

fn tree_type_for(sapling: &str) -> Option<TreeType> {
    match sapling {
        "oak_sapling" => Some(TreeType::Oak),
        "birch_sapling" => Some(TreeType::Birch),
        "spruce_sapling" => Some(TreeType::Spruce),
        _ => None,
    }
}

/// [`ChunkManager`] over a live [`WorldState`], so [`TreePopulator`] can write
/// a grown tree straight into the world (recording the edits for both
/// platforms, unlike a bulk chunk transaction).
struct WorldBlocks<'a, 'b> {
    scene: &'a TickScene<'b>,
}

impl ChunkManager for WorldBlocks<'_, '_> {
    fn get_block_at(&self, x: i32, y: i32, z: i32) -> u32 {
        self.scene.block_or_air(x, y, z)
    }

    fn set_block_at(&mut self, x: i32, y: i32, z: i32, block: u32) {
        self.scene.set_block(x, y, z, block);
    }

    fn get_highest_block_at(&self, x: i32, z: i32) -> Option<i32> {
        self.scene
            .columns
            .borrow_mut()
            .view(self.scene.world, x >> 4, z >> 4)
            .highest_block_at(x, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use bedrock_world::generator::flat::FlatGenerator;

    fn world() -> WorldState {
        // Flat world: bedrock at 0, dirt 1..=2, grass at 3, air above.
        let world = WorldState::new("world", Arc::new(FlatGenerator::new()));
        // Load the columns the tick pass reads: the server does this when it
        // streams a chunk, the test does it explicitly.
        for chunk_x in -1..=1 {
            for chunk_z in -1..=1 {
                world.get_chunk(chunk_x, chunk_z);
            }
        }
        world
    }

    fn rng() -> Rng {
        Rng::new(0x1234_5678_9ABC_DEF0)
    }

    #[test]
    fn grass_spreads_into_lit_dirt() {
        let world = world();
        assert!(world.set_block(4, 4, 4, block_id::DIRT));
        let scene = TickScene::new(&world);
        let mut rng = rng();
        // One random tick of the neighbouring grass block is enough: all four
        // spread attempts land inside the 3x5x3 box and the dirt is lit.
        let mut spread = false;
        for _ in 0..64 {
            if grass_block_tick(&scene, 3, 3, 4, &mut rng) {
                spread = true;
                break;
            }
        }
        assert!(spread, "grass must spread into lit dirt");
        assert_eq!(world.block_at(4, 4, 4), block_id::GRASS_BLOCK);
    }

    #[test]
    fn grass_dies_under_an_opaque_cover() {
        let world = world();
        assert!(world.set_block(5, 4, 5, block_id::STONE));
        assert!(grass_block_tick(&TickScene::new(&world), 5, 3, 5, &mut rng()));
        assert_eq!(world.block_at(5, 3, 5), block_id::DIRT);
    }

    #[test]
    fn leaves_decay_when_no_log_is_near() {
        let world = world();
        let leaves = block_states::network_id("oak_leaves").unwrap();
        assert!(world.set_block(6, 5, 6, leaves));
        assert!(leaves_tick(&TickScene::new(&world), 6, 5, 6, leaves));
        assert_eq!(world.block_at(6, 5, 6), AIR);
    }

    #[test]
    fn leaves_survive_next_to_a_log() {
        let world = world();
        let leaves = block_states::network_id("oak_leaves").unwrap();
        assert!(world.set_block(6, 5, 6, leaves));
        assert!(world.set_block(6, 5, 7, block_id::OAK_LOG));
        assert!(!leaves_tick(&TickScene::new(&world), 6, 5, 6, leaves));
        assert_eq!(world.block_at(6, 5, 6), leaves);
    }

    #[test]
    fn persistent_leaves_never_decay() {
        let world = world();
        let leaves = block_states::network_id("oak_leaves").unwrap();
        let persistent = block_states::with_property(leaves, "persistent_bit", "true").unwrap();
        assert!(world.set_block(7, 6, 7, persistent));
        assert!(!leaves_tick(&TickScene::new(&world), 7, 6, 7, persistent));
    }

    #[test]
    fn wheat_grows_a_stage_on_farmland() {
        let world = world();
        let farmland = block_states::network_id("farmland").unwrap();
        let wheat = block_states::network_id("wheat").unwrap();
        assert!(world.set_block(8, 3, 8, farmland));
        assert!(world.set_block(8, 4, 8, wheat));

        let scene = TickScene::new(&world);
        let mut rng = rng();
        let mut grew = false;
        for _ in 0..64 {
            if crop_tick(&scene, 8, 4, 8, wheat, "wheat", &mut rng) {
                grew = true;
                break;
            }
        }
        assert!(grew, "wheat must advance a growth stage");
        let growth = block_states::int_property(world.block_at(8, 4, 8), "growth").unwrap();
        assert!(growth >= 1);
    }

    #[test]
    fn crops_do_not_grow_without_farmland() {
        let world = world();
        let wheat = block_states::network_id("wheat").unwrap();
        // Grass below instead of farmland.
        assert!(world.set_block(9, 4, 9, wheat));
        let scene = TickScene::new(&world);
        let mut rng = rng();
        for _ in 0..64 {
            assert!(!crop_tick(&scene, 9, 4, 9, wheat, "wheat", &mut rng));
        }
    }

    #[test]
    fn sugar_cane_grows_after_sixteen_ticks_and_stops_at_three() {
        let world = world();
        let reeds = block_states::network_id("reeds").unwrap();
        assert!(world.set_block(10, 4, 10, reeds));
        let scene = TickScene::new(&world);

        // `age` counts the ticks; the 16th adds the block above.
        let mut grew_at = 0;
        for tick in 1..=20 {
            if plant_tick(&scene, 10, 4, 10, world.block_at(10, 4, 10), "reeds") {
                grew_at = tick;
            }
        }
        assert_eq!(grew_at, 16, "sugar cane grows on the 16th random tick");
        assert_eq!(
            block_states::state_name(world.block_at(10, 5, 10)),
            Some("reeds")
        );

        // A second column stops at height 3.
        assert!(scene.set_block(11, 4, 11, reeds));
        assert!(scene.set_block(11, 5, 11, reeds));
        assert!(scene.set_block(11, 6, 11, reeds));
        assert!(!plant_tick(&scene, 11, 6, 11, reeds, "reeds"));
        assert_eq!(world.block_at(11, 7, 11), AIR);
    }

    #[test]
    fn vines_spread_downwards() {
        let world = world();
        let vine = block_states::with_property(
            block_states::network_id("vine").unwrap(),
            "vine_direction_bits",
            "2",
        )
        .unwrap();
        // A wall for the vine to hold on to: `Vine` needs a full block on a
        // side or above, so a vine grown into open air is broken by the
        // physics cascade straight away.
        assert!(world.set_block(13, 5, 12, block_id::STONE));
        assert!(world.set_block(13, 6, 12, block_id::STONE));
        assert!(world.set_block(12, 6, 12, vine));
        let scene = TickScene::new(&world);
        let mut rng = rng();
        let mut spread = false;
        for _ in 0..32 {
            if vine_tick(&scene, 12, 6, 12, vine, &mut rng) {
                spread = true;
                break;
            }
        }
        assert!(spread);
        assert_eq!(block_states::state_name(world.block_at(12, 5, 12)), Some("vine"));
    }

    #[test]
    fn the_tick_pass_only_touches_loaded_columns() {
        let world = world();
        // A column nobody streamed: `loaded_block_at` returns `None`, so the
        // pass skips it instead of generating it.
        assert!(world.loaded_block_at(1000, 4, 1000).is_none());
        assert!(!world.has_loaded_chunk(1000 >> 4, 1000 >> 4));
        assert_eq!(world.random_tick(&[(1000 >> 4, 1000 >> 4)]), 0);
    }

    #[test]
    fn the_tick_pass_samples_every_section_of_a_loaded_chunk() {
        let world = world();
        // Speed 4096 samples every position of every section once.
        world.set_random_tick_speed(4096);
        // A single dirt block in a loaded column with grass next to it.
        assert!(world.set_block(2, 4, 2, block_id::DIRT));
        let reacted = world.random_tick(&[(0, 0)]);
        // With one block in 4096 sampled three times per section the grass
        // block itself is sampled, so the dirt becomes grass.
        assert!(reacted > 0 || world.block_at(2, 4, 2) == block_id::GRASS_BLOCK);
    }
}
