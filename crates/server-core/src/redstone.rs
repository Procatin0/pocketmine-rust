//! Redstone circuits: dust, torches, repeaters, comparators, levers, buttons
//! and pressure plates.
//!
//! Neither PocketMine-MP 5 nor Axolotl ships a circuit simulator (upstream's
//! `RedstoneWire`/`RedstoneRepeater` only carry block state and a
//! `//TODO: redstone functionality`), so the behaviour below is modelled on
//! vanilla Minecraft, using the canonical Bedrock block states this server
//! already ships (`redstone_wire`, `redstone_torch`/`unlit_redstone_torch`,
//! `repeater`/`powered_repeater`, `comparator`/`powered_comparator`,
//! `*_pressure_plate`, `*_button`, `lever`, `redstone_lamp`/`lit_redstone_lamp`).
//!
//! # Timing model (game ticks, 20 TPS)
//!
//! A **redstone tick is 2 game ticks** (100 ms); everything below is expressed
//! in game ticks because that is the granularity the server ticks at.
//!
//! | Component | Delay |
//! |---|---|
//! | Redstone dust | 0: power propagates through block updates inside the tick |
//! | Redstone torch | 1 redstone tick (2 gt) to turn on and to turn off |
//! | Repeater | 1-4 redstone ticks (2/4/6/8 gt, `repeater_delay` 0-3) |
//! | Comparator | 1 redstone tick (2 gt) |
//! | Wooden button | 30 gt pressed, then releases |
//! | Stone/blackstone button | 20 gt pressed, then releases |
//! | Wooden/stone pressure plate | rechecks occupancy every 20 gt |
//! | Light/heavy weighted plate | rechecks occupancy every 10 gt |
//!
//! A repeater also *extends* short pulses: its output is only re-evaluated
//! when the scheduled tick fires, so an input pulse shorter than the delay
//! still produces an output pulse of at least the delay. Dust and torches are
//! the opposite case, documented under "subticks" below.
//!
//! # Subtick ordering
//!
//! A game tick is not atomic: a circuit is evaluated in ordered phases, and a
//! signal that comes and goes inside one phase can be swallowed before a
//! component looks at it (this is what players call a "sub-tick"). This module
//! reproduces the three phases `World::doTick()` runs, in this order:
//!
//! 1. **Scheduled component ticks** (torches, repeaters, comparators,
//!    pressure-plate rechecks, button releases), ordered by due tick and then
//!    by insertion order.
//! 2. **Entity contacts**: plates under players/entities are refreshed, which
//!    is how a plate notices somebody standing on it (nothing else would send
//!    a block update when a player walks), before the updates that carry the
//!    new signal.
//! 3. **Block updates** (neighbour notifications) in FIFO order: this is where
//!    dust recomputes its whole connected network, immediately and with no
//!    delay. Work queued by a block update (a lamp lighting, a wire changing
//!    shape) is drained in the same phase, so a dust circuit settles within the
//!    tick it was disturbed in; a torch or repeater that noticed the change
//!    only *schedules* its own reaction for a later phase of a later tick.
//!
//! # Power model
//!
//! Signal strength is 0-15. Dust loses one level per block, so a 15 signal
//! reaches 15 dust blocks. A block's power distinguishes the two vanilla
//! notions:
//!
//! - **strong power** — a repeater/comparator strongly powers the block in
//!   front of it, a lit torch strongly powers the block above it. Strong power
//!   feeds adjacent dust.
//! - **weak power** — a block adjacent to powered dust (or strongly powered)
//!   activates the mechanisms next to it, but does not feed further dust.
//!
//! Consumed by: lamps (`redstone_lamp` <-> `lit_redstone_lamp`), doors,
//! trapdoors and fence gates (`open_bit`), repeaters, comparators, torches and
//! dust itself.
//!
//! # What is intentionally not modelled
//!
//! - Torch burnout (a torch toggled more than 8 times in 60 gt stops working).
//! - Comparator *container* readings (furnace/hopper fullness, cake,
//!   cauldron): this server has no block entities, so a comparator reads only
//!   redstone inputs. Comparator strength is kept in [`RedstoneState`] because
//!   Bedrock comparators expose only an on/off `output_lit_bit`, not a level.
//! - Observers, rails, target blocks and sculk sensors.
//! - Quasi-connectivity (a piston only reacts to its own neighbours).
//! - Piston *animation*: the blocks move in one step instead of through
//!   `moving_block` entities, so there is no slide to watch.
//! - Dispenser/dropper container *UIs*: the inventory lives in
//!   [`RedstoneState`] but no window opens it yet.

use std::collections::{HashMap, HashSet, VecDeque};

use bedrock_protocol::packets::item_stack::ItemStack;
use bedrock_protocol::types::Vector3f;
use bedrock_world::block_id::AIR;
use bedrock_world::block_states;
use parking_lot::Mutex;

use crate::world_state::WorldState;

/// Highest signal strength (vanilla: 0-15).
pub const MAX_POWER: i32 = 15;

/// One redstone tick, in game ticks (100 ms at 20 TPS).
pub const REDSTONE_TICK: i32 = 2;

/// Torch on/off delay: 1 redstone tick.
pub const TORCH_DELAY: i32 = REDSTONE_TICK;
/// Comparator output delay: 1 redstone tick.
pub const COMPARATOR_DELAY: i32 = REDSTONE_TICK;
/// Repeater delay for `repeater_delay = 0..=3`: 1-4 redstone ticks.
pub fn repeater_delay(repeater_delay: i32) -> i32 {
    (repeater_delay.clamp(0, 3) + 1) * REDSTONE_TICK
}
/// `PressurePlate` recheck interval (wooden/stone).
pub const PLATE_RECHECK: i32 = 20;
/// Weighted pressure plate recheck interval.
pub const WEIGHTED_PLATE_RECHECK: i32 = 10;

/// Hard bounds so a griefing circuit cannot stall the tick: at most this many
/// wire cells are re-evaluated as one network, and this many block updates are
/// drained per tick (the rest carry over, like PocketMine's queue).
const MAX_WIRE_CELLS: usize = 512;
const MAX_RELAXATION_PASSES: usize = 16;
pub const MAX_UPDATES_PER_TICK: usize = 2048;

/// A wire network stops relaxing after `MAX_POWER` steps; one extra pass proves
/// the fixed point.
const _: () = assert!(MAX_RELAXATION_PASSES > MAX_POWER as usize);

/// Horizontal offsets in the order the Bedrock wire properties use them:
/// north (`-z`), east (`+x`), south (`+z`), west (`-x`).
const HORIZONTAL: [(i32, i32, i32); 4] = [(0, 0, -1), (1, 0, 0), (0, 0, 1), (-1, 0, 0)];
const SUFFIXES: [&str; 4] = ["north", "east", "south", "west"];
const DOWN: (i32, i32, i32) = (0, -1, 0);
/// Every neighbour, used for "what powers this cell" scans.
const FACES: [(i32, i32, i32); 6] = [
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (1, 0, 0),
    (-1, 0, 0),
];

type Pos = (i32, i32, i32);

/// What a due scheduled tick should re-evaluate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheduled {
    /// A torch, repeater or comparator that noticed an input change.
    Component,
    /// A pressure plate rechecking whether an entity is still on it.
    Plate,
    /// A piston that must extend or retract to match its power.
    Piston,
}

/// Redstone state that is not derivable from the world: the block-update queue,
/// the scheduled component ticks and the comparator signal levels.
///
/// Everything else (power, lit lamps, open gates) lives in the block states, so
/// it is broadcast to Bedrock and Java clients by the normal `set_block` path.
pub struct RedstoneState {
    queue: Mutex<UpdateQueue>,
    scheduled: Mutex<ScheduledTicks>,
    /// Bedrock comparators carry no analogue output level in their state
    /// (`output_lit_bit` is only on/off), so the computed strength is kept
    /// here, exactly like the vanilla block entity.
    comparator_power: Mutex<HashMap<Pos, i32>>,
    /// Block container inventories (dispensers, droppers, chests, furnaces,
    /// brewing stands, ...), keyed by position and shared with the chest
    /// window a player opens.
    ///
    /// This server has no block entities, so the contents live here instead of
    /// inside the chunk: breaking a container loses them.
    containers: Mutex<HashMap<Pos, Vec<ItemStack>>>,
    /// Last power level this module applied to each `open_bit` mechanism
    /// (doors, trapdoors and fence gates).
    ///
    /// Bedrock has no `powered` property, so the block state alone cannot tell
    /// a door opened by hand from one opened by redstone. Vanilla keeps that
    /// bit and only flips `open` when the signal *changes*; without this map a
    /// hand click re-closed the door on the very next block update.
    mechanism_powered: Mutex<HashMap<Pos, bool>>,
}

struct UpdateQueue {
    pending: VecDeque<Pos>,
    /// Every position currently queued, so a wire cascade cannot push the same
    /// cell thousands of times before the drain.
    queued: HashSet<Pos>,
}

struct ScheduledTicks {
    /// `(due tick, insertion order, position, kind)`.
    entries: Vec<(i32, u64, Pos, Scheduled)>,
    next_order: u64,
}

impl Default for RedstoneState {
    fn default() -> Self {
        Self::new()
    }
}

impl RedstoneState {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(UpdateQueue {
                pending: VecDeque::new(),
                queued: HashSet::new(),
            }),
            scheduled: Mutex::new(ScheduledTicks {
                entries: Vec::new(),
                next_order: 0,
            }),
            comparator_power: Mutex::new(HashMap::new()),
            containers: Mutex::new(HashMap::new()),
            mechanism_powered: Mutex::new(HashMap::new()),
        }
    }

    /// Records the power level of an openable mechanism and answers whether it
    /// differs from the one this module applied last time.
    ///
    /// The first evaluation adopts the signal without touching `open_bit`: the
    /// block state is authoritative until the signal changes, which is what
    /// keeps a door opened by hand open even when its first neighbour update
    /// arrives after the click. A door placed into an already powered cell
    /// therefore only swings when that signal changes.
    fn mechanism_powered_changed(&self, pos: Pos, powered: bool) -> bool {
        match self.mechanism_powered.lock().insert(pos, powered) {
            Some(previous) => previous != powered,
            None => false,
        }
    }

    /// Drops the remembered power of a cell that no longer holds an openable
    /// mechanism, so a later door placed there starts fresh.
    fn forget_mechanism(&self, pos: Pos) {
        self.mechanism_powered.lock().remove(&pos);
    }

    /// Queues a block update for every redstone component around `pos` (and for
    /// `pos` itself): this is PocketMine's
    /// `World::internalNotifyNeighbourBlockUpdate()` limited to the blocks this
    /// module reacts to.
    ///
    /// The update also *relays* through non-component blocks: a solid block
    /// carries the power change (`block_power`) to the components mounted on
    /// it, which is why a lever on top of a stone turns off a torch hanging on
    /// that stone. Without the second hop the torch would never hear about it,
    /// because it is not adjacent to the lever.
    fn notify(&self, world: &WorldState, pos: Pos) {
        let mut targets = [pos; 7];
        for (index, (dx, dy, dz)) in FACES.iter().enumerate() {
            targets[index + 1] = (pos.0 + dx, pos.1 + dy, pos.2 + dz);
        }
        let mut queue = self.queue.lock();
        let push = |target: Pos, queue: &mut UpdateQueue| {
            if !reacts(world.block_at(target.0, target.1, target.2)) {
                return;
            }
            if queue.queued.insert(target) {
                queue.pending.push_back(target);
            }
        };
        for target in targets {
            push(target, &mut queue);
            if reacts(world.block_at(target.0, target.1, target.2)) {
                continue;
            }
            for (dx, dy, dz) in FACES {
                push((target.0 + dx, target.1 + dy, target.2 + dz), &mut queue);
            }
        }
    }

    fn pop(&self) -> Option<Pos> {
        let mut queue = self.queue.lock();
        let position = queue.pending.pop_front()?;
        queue.queued.remove(&position);
        Some(position)
    }

    /// Schedules a component re-evaluation. The earliest due tick wins, so a
    /// repeater's timer is not restarted by every neighbour notification.
    fn schedule(&self, pos: Pos, due: i32, kind: Scheduled) {
        let mut scheduled = self.scheduled.lock();
        if let Some(existing) = scheduled
            .entries
            .iter_mut()
            .find(|(_, _, position, existing_kind)| *position == pos && *existing_kind == kind)
        {
            existing.0 = existing.0.min(due);
            return;
        }
        let order = scheduled.next_order;
        scheduled.next_order += 1;
        scheduled.entries.push((due, order, pos, kind));
    }

    fn take_due(&self, now: i32) -> Vec<(Pos, Scheduled)> {
        let mut scheduled = self.scheduled.lock();
        let mut ready: Vec<(i32, u64, Pos, Scheduled)> = Vec::new();
        let mut index = 0;
        while index < scheduled.entries.len() {
            if scheduled.entries[index].0 <= now {
                ready.push(scheduled.entries.remove(index));
            } else {
                index += 1;
            }
        }
        // Same due tick: insertion order, like PocketMine's FIFO queue drain.
        ready.sort_unstable_by_key(|(due, order, _, _)| (*due, *order));
        ready
            .into_iter()
            .map(|(_, _, pos, kind)| (pos, kind))
            .collect()
    }

    fn set_comparator_power(&self, pos: Pos, power: i32) {
        self.comparator_power
            .lock()
            .insert(pos, power.clamp(0, MAX_POWER));
    }

    fn comparator_power(&self, pos: Pos) -> Option<i32> {
        self.comparator_power.lock().get(&pos).copied()
    }

    fn forget_comparator(&self, pos: Pos) {
        self.comparator_power.lock().remove(&pos);
    }

    /// Slots of the dispenser/dropper at `pos` (all empty when nothing was
    /// stored there).
    pub fn container(&self, pos: Pos) -> Vec<ItemStack> {
        self.container_with_len(pos, CONTAINER_SLOTS)
    }

    /// Slots of the block container at `pos`, padded/truncated to `len`.
    ///
    /// Dispensers keep the fixed [`CONTAINER_SLOTS`] layout; chests, furnaces
    /// and brewing stands store their own slot count through this same map so
    /// the contents survive closing the window.
    pub fn container_with_len(&self, pos: Pos, len: usize) -> Vec<ItemStack> {
        let mut slots = self.containers.lock().get(&pos).cloned().unwrap_or_default();
        slots.resize(len, ItemStack::empty());
        slots
    }

    /// Replaces the dispenser/dropper inventory at `pos`.
    pub fn set_container(&self, pos: Pos, slots: Vec<ItemStack>) {
        let mut containers = self.containers.lock();
        if slots.iter().all(ItemStack::is_empty) {
            containers.remove(&pos);
        } else {
            containers.insert(pos, slots);
        }
    }

    /// Positions waiting for a block update (tests).
    pub fn pending_updates(&self) -> usize {
        self.queue.lock().pending.len()
    }

    /// Scheduled component ticks not fired yet (tests).
    pub fn pending_scheduled(&self) -> usize {
        self.scheduled.lock().entries.len()
    }
}

// ---------------------------------------------------------------------------
// Entry points called by `WorldState`
// ---------------------------------------------------------------------------

/// `World::setBlock()` follow-up: after an edit is flushed, queue the redstone
/// components around it so they re-read their inputs in the next block-update
/// phase.
pub fn on_block_changed(world: &WorldState, pos: Pos) {
    remove_orphan_arms(world, pos);
    world.redstone.notify(world, pos);
}

/// Drops every piston head that lost the piston it belongs to.
///
/// The head is a separate block (`piston_arm_collision`), so breaking, moving
/// or replacing the piston body would leave it floating. A head is only kept
/// when the cell behind it (opposite `facing_direction`) still holds a piston of
/// the same material facing the same way.
fn remove_orphan_arms(world: &WorldState, pos: Pos) {
    // The edited cell itself (a head placed on its own) and its neighbours (a
    // head left behind when the piston next to it changed).
    let candidates = std::iter::once((0, 0, 0)).chain(FACES);
    for (dx, dy, dz) in candidates {
        let arm = (pos.0 + dx, pos.1 + dy, pos.2 + dz);
        let arm_id = world.block_at(arm.0, arm.1, arm.2);
        let Some(name) = block_states::state_name(arm_id) else {
            continue;
        };
        if !is_piston_arm_name(name) || piston_for_arm(world, arm, arm_id).is_some() {
            continue;
        }
        world.set_block(arm.0, arm.1, arm.2, AIR);
    }
}

/// The piston that owns the head at `pos`, when it is still there and matches
/// its material and facing.
fn piston_for_arm(world: &WorldState, pos: Pos, arm_id: u32) -> Option<Pos> {
    let facing = block_states::int_property(arm_id, "facing_direction")?;
    let direction = *FACING_OFFSETS.get(usize::try_from(facing).ok()?)?;
    let behind = (
        pos.0 - direction.0,
        pos.1 - direction.1,
        pos.2 - direction.2,
    );
    let piston_id = world.block_at(behind.0, behind.1, behind.2);
    let piston = block_states::state_name(piston_id)?;
    if !is_piston_name(piston) {
        return None;
    }
    // A sticky head only belongs to a sticky piston and vice versa.
    let sticky_arm = block_states::state_name(arm_id)? == "sticky_piston_arm_collision";
    if sticky_arm != (piston == "sticky_piston") {
        return None;
    }
    if block_states::int_property(piston_id, "facing_direction") != Some(facing) {
        return None;
    }
    Some(behind)
}

/// One game tick of redstone, run from `WorldState::tick()` after the
/// world's own scheduled block updates (button releases) and liquid ticks.
///
/// The phases are described in the module docs; they run in vanilla order.
pub fn tick(world: &WorldState, now: i32) {
    // Phase 1: scheduled component ticks (torch/repeater/comparator/plate).
    for (pos, kind) in world.redstone.take_due(now) {
        match kind {
            Scheduled::Component => apply_component(world, pos),
            Scheduled::Plate => refresh_plate(world, pos),
            Scheduled::Piston => run_piston(world, pos),
        }
    }
    // Phase 2: entity contacts. A plate has no block update to notice a player
    // walking onto it, so the world's player/entity snapshot is checked here,
    // before the block updates that carry the resulting signal.
    for pos in contact_plates(world) {
        activate_plate(world, pos);
    }
    // Phase 3: block updates (FIFO), where dust resolves synchronously.
    let mut budget = MAX_UPDATES_PER_TICK;
    while budget > 0 {
        let Some(pos) = world.redstone.pop() else {
            break;
        };
        budget -= 1;
        on_neighbour_changed(world, pos);
    }
}

fn on_neighbour_changed(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(name) = block_states::state_name(id) else {
        return;
    };
    if !is_comparator_name(name) {
        world.redstone.forget_comparator(pos);
    }
    if name == "redstone_wire" {
        // Zero-delay: the whole connected network is recomputed right now.
        update_wire_network(world, pos);
    } else if is_torch_name(name) {
        schedule_torch(world, pos);
    } else if is_diode_name(name) {
        // Repeaters and comparators re-read their inputs one redstone tick
        // later; that delay is also what extends short pulses.
        let delay = if is_repeater_name(name) {
            repeater_delay(block_states::int_property(id, "repeater_delay").unwrap_or(0))
        } else {
            COMPARATOR_DELAY
        };
        let now = world.redstone_tick();
        world
            .redstone
            .schedule(pos, now + delay, Scheduled::Component);
    } else if is_piston_name(name) {
        apply_piston(world, pos);
    } else if is_dispenser_name(name) {
        apply_dispenser(world, pos);
    } else if is_lamp_name(name) || is_openable_name(name) {
        update_mechanism(world, pos);
    }
}

fn apply_component(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(name) = block_states::state_name(id) else {
        return;
    };
    if name == "redstone_wire" {
        update_wire_network(world, pos);
    } else if is_torch_name(name) {
        apply_torch(world, pos);
    } else if is_repeater_name(name) {
        apply_repeater(world, pos);
    } else if is_comparator_name(name) {
        apply_comparator(world, pos);
    }
}

// ---------------------------------------------------------------------------
// Block classification
// ---------------------------------------------------------------------------

pub fn is_wire_name(name: &str) -> bool {
    name == "redstone_wire"
}

pub fn is_torch_name(name: &str) -> bool {
    name == "redstone_torch" || name == "unlit_redstone_torch"
}

fn torch_is_lit(id: u32) -> bool {
    block_states::state_name(id) == Some("redstone_torch")
}

pub fn is_repeater_name(name: &str) -> bool {
    name == "unpowered_repeater" || name == "powered_repeater"
}

pub fn is_comparator_name(name: &str) -> bool {
    name == "unpowered_comparator" || name == "powered_comparator"
}

fn is_diode_name(name: &str) -> bool {
    is_repeater_name(name) || is_comparator_name(name)
}

fn is_lamp_name(name: &str) -> bool {
    name == "redstone_lamp" || name == "lit_redstone_lamp"
}

fn is_pressure_plate_name(name: &str) -> bool {
    name.ends_with("_pressure_plate")
}

fn is_button_name(name: &str) -> bool {
    name.ends_with("_button")
}

fn is_openable_name(name: &str) -> bool {
    name.ends_with("_door")
        || name.ends_with("_trapdoor")
        || name == "fence_gate"
        || name.ends_with("_fence_gate")
}

pub fn is_piston_name(name: &str) -> bool {
    name == "piston" || name == "sticky_piston"
}

pub fn is_piston_arm_name(name: &str) -> bool {
    name == "piston_arm_collision" || name == "sticky_piston_arm_collision"
}

pub fn is_dispenser_name(name: &str) -> bool {
    name == "dispenser" || name == "dropper"
}

/// A block that must be re-evaluated when a neighbour changes: the dust-facing
/// [`is_component`] set, plus the pistons and dispensers. Dust does not connect
/// to those, so they stay out of [`is_component`].
fn reacts(id: u32) -> bool {
    if is_component(id) {
        return true;
    }
    block_states::state_name(id).is_some_and(|name| is_piston_name(name) || is_dispenser_name(name))
}

/// A block the dust connects/points at, and that reacts to a neighbour change.
fn is_component(id: u32) -> bool {
    let Some(name) = block_states::state_name(id) else {
        return false;
    };
    is_wire_name(name)
        || is_torch_name(name)
        || is_diode_name(name)
        || is_lamp_name(name)
        || is_openable_name(name)
        || name == "lever"
        || name == "redstone_block"
        || is_button_name(name)
        || is_pressure_plate_name(name)
}

fn is_wire(id: u32) -> bool {
    block_states::state_name(id) == Some("redstone_wire")
}

/// The power stored in a dust cell, `0` for anything else.
fn wire_power(world: &WorldState, pos: Pos) -> i32 {
    let id = world.block_at(pos.0, pos.1, pos.2);
    if !is_wire(id) {
        return 0;
    }
    block_states::int_property(id, "redstone_signal").unwrap_or(0)
}

/// Offset of the block a diode (repeater/comparator) outputs into, i.e. the
/// direction it faces. `None` when the state has no `cardinal_direction`.
fn diode_direction(id: u32) -> Option<(i32, i32, i32)> {
    match block_states::string_property(id, "cardinal_direction")? {
        "north" => Some((0, 0, -1)),
        "east" => Some((1, 0, 0)),
        "south" => Some((0, 0, 1)),
        "west" => Some((-1, 0, 0)),
        _ => None,
    }
}

/// Position of the block a torch is mounted on.
///
/// `torch_facing_direction` is the wall torch's facing value, whose horizontal
/// spelling Bedrock serializes flipped (MCPE-152036, see
/// `physics::state_for_placement`). Rather than trusting one convention, the
/// stored side wins only when it actually holds a full block; otherwise the
/// opposite side is used. A floor torch (`top`/`unknown`) hangs on the block
/// below.
fn torch_support(world: &WorldState, pos: Pos, id: u32) -> Option<Pos> {
    let offset = torch_attachment(id)?;
    if offset == DOWN {
        return Some((pos.0, pos.1 - 1, pos.2));
    }
    let full_block = |delta: (i32, i32, i32)| {
        block_states::state_name(world.block_at(pos.0 + delta.0, pos.1 + delta.1, pos.2 + delta.2))
            .is_some_and(block_states::is_full_cube)
    };
    let flipped = (-offset.0, -offset.1, -offset.2);
    if full_block(offset) || !full_block(flipped) {
        Some((pos.0 + offset.0, pos.1 + offset.1, pos.2 + offset.2))
    } else {
        Some((pos.0 + flipped.0, pos.1 + flipped.1, pos.2 + flipped.2))
    }
}

/// Offset spelled by `torch_facing_direction` (see [`torch_support`]).
fn torch_attachment(id: u32) -> Option<(i32, i32, i32)> {
    match block_states::string_property(id, "torch_facing_direction")? {
        "top" | "unknown" => Some(DOWN),
        "north" => Some((0, 0, -1)),
        "south" => Some((0, 0, 1)),
        "east" => Some((1, 0, 0)),
        "west" => Some((-1, 0, 0)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Power queries
// ---------------------------------------------------------------------------

/// Signal a component at `from` pushes directly into the cell `to`.
///
/// Non-directional sources (lever, button, plate, redstone block) power every
/// neighbour; a diode only its output face; a torch every neighbour except the
/// block it is mounted on (a torch never powers its own support).
fn source_output(world: &WorldState, from: Pos, to: Pos) -> i32 {
    let id = world.block_at(from.0, from.1, from.2);
    let Some(name) = block_states::state_name(id) else {
        return 0;
    };
    if is_button_name(name) {
        return if block_states::bool_property(id, "button_pressed_bit").unwrap_or(false) {
            MAX_POWER
        } else {
            0
        };
    }
    if name == "lever" {
        return if block_states::bool_property(id, "open_bit").unwrap_or(false) {
            MAX_POWER
        } else {
            0
        };
    }
    if is_pressure_plate_name(name) {
        return block_states::int_property(id, "redstone_signal").unwrap_or(0);
    }
    if name == "redstone_block" {
        return MAX_POWER;
    }
    if is_torch_name(name) {
        if !torch_is_lit(id) {
            return 0;
        }
        let delta = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
        return if torch_attachment(id) == Some(delta) {
            0
        } else {
            MAX_POWER
        };
    }
    if is_diode_name(name) {
        let delta = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
        return if diode_direction(id) == Some(delta) {
            diode_output(world, from)
        } else {
            0
        };
    }
    0
}

/// Signal a diode drives out of its output face.
fn diode_output(world: &WorldState, pos: Pos) -> i32 {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(name) = block_states::state_name(id) else {
        return 0;
    };
    if is_repeater_name(name) {
        return if name == "powered_repeater" {
            MAX_POWER
        } else {
            0
        };
    }
    if is_comparator_name(name) {
        if let Some(power) = world.redstone.comparator_power(pos) {
            return power;
        }
        // A comparator lit by nothing but its state (e.g. loaded from a world
        // saved by another server) still outputs full strength.
        return if block_states::bool_property(id, "output_lit_bit").unwrap_or(false) {
            MAX_POWER
        } else {
            0
        };
    }
    0
}

/// Strong power: the power a *block* receives and can pass on to adjacent dust.
///
/// Only two things strongly power a block in this model: a lit torch directly
/// below it, and a repeater/comparator facing into it.
fn strong_power(world: &WorldState, pos: Pos) -> i32 {
    let mut power = 0;
    let below = (pos.0, pos.1 - 1, pos.2);
    let below_id = world.block_at(below.0, below.1, below.2);
    if is_torch_name(block_states::state_name(below_id).unwrap_or("")) && torch_is_lit(below_id) {
        power = MAX_POWER;
    }
    for (dx, dy, dz) in HORIZONTAL {
        let diode = (pos.0 - dx, pos.1 - dy, pos.2 - dz);
        let id = world.block_at(diode.0, diode.1, diode.2);
        let Some(name) = block_states::state_name(id) else {
            continue;
        };
        if is_diode_name(name) && diode_direction(id) == Some((dx, dy, dz)) {
            power = power.max(diode_output(world, diode));
        }
    }
    power.min(MAX_POWER)
}

/// Power available *at* `pos`, i.e. what a mechanism/wire there would read:
/// direct sources, adjacent dust, and adjacent powered blocks (weak power).
///
/// `exclude` skips one neighbour; a torch uses it to ignore itself while asking
/// whether its own support block is powered.
fn incoming_power(world: &WorldState, pos: Pos, exclude: Option<Pos>) -> i32 {
    let mut power = 0;
    for (dx, dy, dz) in FACES {
        let neighbour = (pos.0 + dx, pos.1 + dy, pos.2 + dz);
        if Some(neighbour) == exclude {
            continue;
        }
        power = power.max(source_output(world, neighbour, pos));
        power = power.max(block_power(world, neighbour));
    }
    power.min(MAX_POWER)
}

/// Power a block hands to the mechanisms next to it: its own strong power plus
/// the signal of the dust lying against it (vanilla "weak power": dust powers
/// the block it points into, and the block activates its neighbours).
fn block_power(world: &WorldState, pos: Pos) -> i32 {
    let mut power = strong_power(world, pos);
    if is_wire(world.block_at(pos.0, pos.1, pos.2)) {
        return wire_power(world, pos);
    }
    for (dx, dy, dz) in FACES {
        let neighbour = (pos.0 + dx, pos.1 + dy, pos.2 + dz);
        if is_wire(world.block_at(neighbour.0, neighbour.1, neighbour.2)) {
            power = power.max(wire_power(world, neighbour));
        }
    }
    power.min(MAX_POWER)
}

/// Input a comparator reads on one of its sides: the power of the block sitting
/// against that side, but only from the components that feed a side (dust, a
/// diode facing the comparator, a redstone block) -- never from a torch or
/// lever that merely sits there.
fn side_input(world: &WorldState, cell: Pos, comparator: Pos) -> i32 {
    let id = world.block_at(cell.0, cell.1, cell.2);
    match block_states::state_name(id) {
        Some("redstone_wire") => wire_power(world, cell),
        Some("redstone_block") => MAX_POWER,
        Some(name) if is_diode_name(name) => {
            let delta = (
                comparator.0 - cell.0,
                comparator.1 - cell.1,
                comparator.2 - cell.2,
            );
            if diode_direction(id) == Some(delta) {
                diode_output(world, cell)
            } else {
                0
            }
        }
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Redstone dust
// ---------------------------------------------------------------------------

/// Wire cells reachable from `pos` through the staircase links (a run may climb
/// one block per side step).
fn wire_links(world: &WorldState, pos: Pos) -> Vec<Pos> {
    let mut links = Vec::with_capacity(8);
    for (dx, _, dz) in HORIZONTAL {
        let side = (pos.0 + dx, pos.1, pos.2 + dz);
        if is_wire(world.block_at(side.0, side.1, side.2)) {
            links.push(side);
            continue;
        }
        // Up a riser: the side cell is the block the higher dust sits on.
        let above = (side.0, side.1 + 1, side.2);
        if is_wire(world.block_at(above.0, above.1, above.2)) {
            links.push(above);
        }
        // Down a step: only over an open cell, or the lower dust would be
        // reached through the block it sits on.
        let passable = block_states::state_name(world.block_at(side.0, side.1, side.2))
            .map(|name| !block_states::is_full_cube(name))
            .unwrap_or(true);
        if !passable {
            continue;
        }
        let below = (side.0, side.1 - 1, side.2);
        if is_wire(world.block_at(below.0, below.1, below.2)) {
            links.push(below);
        }
    }
    links
}

/// Recomputes every dust cell connected to `start`: external sources first,
/// then a multi-source relaxation where each hop loses one level of strength.
fn update_wire_network(world: &WorldState, start: Pos) {
    if !is_wire(world.block_at(start.0, start.1, start.2)) {
        return;
    }
    let mut cells: Vec<Pos> = Vec::with_capacity(16);
    let mut seen: HashSet<Pos> = HashSet::with_capacity(16);
    let mut frontier: VecDeque<Pos> = VecDeque::with_capacity(16);
    frontier.push_back(start);
    seen.insert(start);
    while let Some(cell) = frontier.pop_front() {
        cells.push(cell);
        if cells.len() >= MAX_WIRE_CELLS {
            break;
        }
        for link in wire_links(world, cell) {
            if seen.insert(link) {
                frontier.push_back(link);
            }
        }
    }

    let mut power: HashMap<Pos, i32> = HashMap::with_capacity(cells.len());
    for cell in &cells {
        power.insert(*cell, wire_external_power(world, *cell));
    }
    for _ in 0..MAX_RELAXATION_PASSES {
        let mut changed = false;
        for cell in &cells {
            let mut best = power[cell];
            for (dx, dy, dz) in FACES {
                let neighbour = (cell.0 + dx, cell.1 + dy, cell.2 + dz);
                if let Some(other) = power.get(&neighbour) {
                    best = best.max(other - 1);
                }
            }
            if best > power[cell] {
                power.insert(*cell, best);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    for cell in cells {
        let signal = power[&cell].clamp(0, MAX_POWER);
        let connections = wire_connections(world, cell);
        let current = world.block_at(cell.0, cell.1, cell.2);
        let Some(updated) = wire_state(current, signal, connections) else {
            continue;
        };
        if updated != current {
            world.set_block(cell.0, cell.1, cell.2, updated);
        }
    }
}

/// Signal a dust cell receives from sources around it (not from other dust,
/// which the relaxation pass handles).
fn wire_external_power(world: &WorldState, pos: Pos) -> i32 {
    let mut power = 0;
    for (dx, dy, dz) in FACES {
        let neighbour = (pos.0 + dx, pos.1 + dy, pos.2 + dz);
        power = power.max(source_output(world, neighbour, pos));
        // Strongly powered blocks feed the dust lying against them.
        power = power.max(strong_power(world, neighbour));
    }
    power.min(MAX_POWER)
}

/// Bedrock wire connection values for the four horizontal faces: `side` for a
/// wire/component at the same height, `up` for the staircase link over a
/// non-solid block.
fn wire_connections(world: &WorldState, pos: Pos) -> [&'static str; 4] {
    let mut connections = ["none"; 4];
    for (index, (dx, _, dz)) in HORIZONTAL.iter().enumerate() {
        let side = (pos.0 + dx, pos.1, pos.2 + dz);
        let id = world.block_at(side.0, side.1, side.2);
        if is_component(id) {
            connections[index] = "side";
            continue;
        }
        // A staircase link: the side cell is the riser the higher dust sits on
        // (it is usually a full block, so it must not be filtered out here).
        let above = (side.0, side.1 + 1, side.2);
        if is_wire(world.block_at(above.0, above.1, above.2)) {
            connections[index] = "up";
        }
    }
    connections
}

fn wire_state(base: u32, signal: i32, connections: [&'static str; 4]) -> Option<u32> {
    let mut id = block_states::with_property(base, "redstone_signal", &signal.to_string())?;
    for (index, suffix) in SUFFIXES.iter().enumerate() {
        id = block_states::with_property(id, &format!("redstone_{suffix}"), connections[index])?;
    }
    Some(id)
}

// ---------------------------------------------------------------------------
// Torches
// ---------------------------------------------------------------------------

/// A torch looks at the block it is mounted on: when that block is powered the
/// torch turns off, one redstone tick later.
fn schedule_torch(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(support) = torch_support(world, pos, id) else {
        return;
    };
    let powered = incoming_power(world, support, Some(pos)) > 0;
    if powered == torch_is_lit(id) {
        let now = world.redstone_tick();
        world
            .redstone
            .schedule(pos, now + TORCH_DELAY, Scheduled::Component);
    }
}

fn apply_torch(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    if !is_torch_name(block_states::state_name(id).unwrap_or("")) {
        return;
    }
    let Some(support) = torch_support(world, pos, id) else {
        return;
    };
    let powered = incoming_power(world, support, Some(pos)) > 0;
    let lit = torch_is_lit(id);
    if powered == lit {
        // Off when powered, on when unpowered: invert.
        let name = if lit {
            "unlit_redstone_torch"
        } else {
            "redstone_torch"
        };
        if let Some(updated) = block_states::with_name(id, name) {
            world.set_block(pos.0, pos.1, pos.2, updated);
        }
    }
}

// ---------------------------------------------------------------------------
// Repeaters
// ---------------------------------------------------------------------------

/// A repeater locked by a powered diode facing into one of its sides keeps its
/// current output regardless of its back input.
fn repeater_locked(world: &WorldState, pos: Pos, output: (i32, i32, i32)) -> bool {
    for (dx, dy, dz) in HORIZONTAL {
        if (dx, dy, dz) == output || (dx, dy, dz) == (-output.0, -output.1, -output.2) {
            continue;
        }
        let neighbour = (pos.0 + dx, pos.1 + dy, pos.2 + dz);
        let id = world.block_at(neighbour.0, neighbour.1, neighbour.2);
        let Some(name) = block_states::state_name(id) else {
            continue;
        };
        if !is_diode_name(name) {
            continue;
        }
        let towards = (-dx, -dy, -dz);
        if diode_direction(id) == Some(towards) && diode_output(world, neighbour) > 0 {
            return true;
        }
    }
    false
}

fn apply_repeater(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(name) = block_states::state_name(id) else {
        return;
    };
    if !is_repeater_name(name) {
        return;
    }
    let Some(output) = diode_direction(id) else {
        return;
    };
    if repeater_locked(world, pos, output) {
        return;
    }
    let input = (pos.0 - output.0, pos.1 - output.1, pos.2 - output.2);
    let powered = incoming_power(world, input, None) > 0;
    let is_powered = name == "powered_repeater";
    if powered == is_powered {
        return;
    }
    let target = if powered {
        "powered_repeater"
    } else {
        "unpowered_repeater"
    };
    if let Some(updated) = block_states::with_name(id, target) {
        world.set_block(pos.0, pos.1, pos.2, updated);
    }
}

// ---------------------------------------------------------------------------
// Comparators
// ---------------------------------------------------------------------------

/// Comparator output: `back` in compare mode when it is at least the strongest
/// side input, `back - side` in subtract mode, `0` otherwise.
fn comparator_output(world: &WorldState, pos: Pos) -> i32 {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(output) = diode_direction(id) else {
        return 0;
    };
    let back = (pos.0 - output.0, pos.1 - output.1, pos.2 - output.2);
    let back_power = incoming_power(world, back, None);
    let mut side_power = 0;
    for (dx, dy, dz) in HORIZONTAL {
        if (dx, dy, dz) == output || (dx, dy, dz) == (-output.0, -output.1, -output.2) {
            continue;
        }
        side_power = side_power.max(side_input(world, (pos.0 + dx, pos.1 + dy, pos.2 + dz), pos));
    }
    let subtract = block_states::bool_property(id, "output_subtract_bit").unwrap_or(false);
    if subtract {
        (back_power - side_power).max(0)
    } else if back_power >= side_power {
        back_power
    } else {
        0
    }
}

fn apply_comparator(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(name) = block_states::state_name(id) else {
        return;
    };
    if !is_comparator_name(name) {
        return;
    }
    let power = comparator_output(world, pos);
    let previous = world.redstone.comparator_power(pos).unwrap_or(0);
    world.redstone.set_comparator_power(pos, power);
    let lit = power > 0;
    if lit != (name == "powered_comparator") {
        let target = if lit {
            "powered_comparator"
        } else {
            "unpowered_comparator"
        };
        if let Some(updated) = block_states::with_name(id, target) {
            world.set_block(pos.0, pos.1, pos.2, updated);
        }
    } else if power != previous {
        // Same lit bit, different level: the state may not change but the dust
        // and mechanisms behind it must read the new strength.
        on_block_changed(world, pos);
    }
}

// ---------------------------------------------------------------------------
// Mechanisms (lamps, doors, trapdoors, gates)
// ---------------------------------------------------------------------------

fn update_mechanism(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(name) = block_states::state_name(id) else {
        return;
    };
    let powered = incoming_power(world, pos, None) > 0;
    if is_lamp_name(name) {
        if powered == (name == "lit_redstone_lamp") {
            return;
        }
        let target = if powered {
            "lit_redstone_lamp"
        } else {
            "redstone_lamp"
        };
        if let Some(updated) = block_states::with_name(id, target) {
            world.set_block(pos.0, pos.1, pos.2, updated);
        }
        return;
    }
    // Doors, trapdoors and fence gates: `open_bit` follows the power state,
    // but only while the *signal* changes. Bedrock stores no `powered`
    // property, so the last applied signal is remembered here (vanilla's
    // `POWERED` equivalent): without it the block update caused by a hand
    // click immediately closed the door again.
    let Some(open) = block_states::bool_property(id, "open_bit") else {
        world.redstone.forget_mechanism(pos);
        return;
    };
    if !world.redstone.mechanism_powered_changed(pos, powered) || open == powered {
        return;
    }
    if let Some(updated) =
        block_states::with_property(id, "open_bit", if powered { "true" } else { "false" })
    {
        world.set_block(pos.0, pos.1, pos.2, updated);
        world.add_level_event(
            bedrock_protocol::packets::level_event::LEVEL_EVENT_SOUND_DOOR,
            0,
            pos.0,
            pos.1,
            pos.2,
        );
        // A door is two blocks sharing `open_bit`: flip the other half too,
        // silently (vanilla plays the sound once).
        if name.ends_with("_door") {
            for dy in [1, -1] {
                let other = (pos.0, pos.1 + dy, pos.2);
                let other_id = world.block_at(other.0, other.1, other.2);
                if block_states::state_name(other_id) != Some(name) {
                    continue;
                }
                if let Some(updated_half) = block_states::with_property(
                    other_id,
                    "open_bit",
                    if powered { "true" } else { "false" },
                ) {
                    world.set_block(other.0, other.1, other.2, updated_half);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pressure plates
// ---------------------------------------------------------------------------

/// Plate cells under the players and entities of this tick.
fn contact_plates(world: &WorldState) -> HashSet<Pos> {
    let mut plates = HashSet::new();
    for (x, y, z) in world.player_positions() {
        push_plate_cells(world, x, y, z, &mut plates);
    }
    for entity in world.entities_snapshot() {
        push_plate_cells(
            world,
            entity.position.x,
            entity.position.y,
            entity.position.z,
            &mut plates,
        );
    }
    plates
}

/// A plate sits in the block whose `y` is the feet level minus one (a player
/// standing on a plate has their feet at `y + 1`), but an entity can also be
/// inside the plate's own cell, so both are checked.
fn push_plate_cells(world: &WorldState, x: f32, y: f32, z: f32, out: &mut HashSet<Pos>) {
    let bx = x.floor() as i32;
    let bz = z.floor() as i32;
    let feet = y.floor() as i32;
    for by in [feet, feet - 1] {
        let name = block_states::state_name(world.block_at(bx, by, bz));
        if name.is_some_and(is_pressure_plate_name) {
            out.insert((bx, by, bz));
        }
    }
}

/// Writes a plate's signal immediately (activation is not delayed in vanilla:
/// only the *release* waits for the next recheck).
fn activate_plate(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(name) = block_states::state_name(id) else {
        return;
    };
    if !is_pressure_plate_name(name) {
        return;
    }
    let desired = plate_signal(world, pos, name);
    let current = block_states::int_property(id, "redstone_signal").unwrap_or(0);
    if desired > current {
        if let Some(updated) =
            block_states::with_property(id, "redstone_signal", &desired.to_string())
        {
            world.set_block(pos.0, pos.1, pos.2, updated);
        }
    }
    reschedule_plate(world, pos, name);
}

/// Scheduled plate tick: recompute the signal (this is where a plate releases,
/// up to one recheck interval after the entity left) and keep rechecking while
/// it stays occupied.
fn refresh_plate(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(name) = block_states::state_name(id) else {
        return;
    };
    if !is_pressure_plate_name(name) {
        return;
    }
    let desired = plate_signal(world, pos, name);
    let current = block_states::int_property(id, "redstone_signal").unwrap_or(0);
    if desired != current {
        if let Some(updated) =
            block_states::with_property(id, "redstone_signal", &desired.to_string())
        {
            world.set_block(pos.0, pos.1, pos.2, updated);
        }
    }
    if desired > 0 {
        reschedule_plate(world, pos, name);
    }
}

fn reschedule_plate(world: &WorldState, pos: Pos, name: &str) {
    let interval = if name.starts_with("light_weighted") || name.starts_with("heavy_weighted") {
        WEIGHTED_PLATE_RECHECK
    } else {
        PLATE_RECHECK
    };
    let now = world.redstone_tick();
    world
        .redstone
        .schedule(pos, now + interval, Scheduled::Plate);
}

/// Signal from the entities standing on the plate: 15 for the wooden/stone
/// plates, the entity count for the light weighted plate and
/// `ceil(count / 10)` for the heavy one.
fn plate_signal(world: &WorldState, pos: Pos, name: &str) -> i32 {
    let count = plate_occupancy(world, pos);
    if count == 0 {
        return 0;
    }
    if name.starts_with("heavy_weighted") {
        ((count + 9) / 10).min(MAX_POWER)
    } else if name.starts_with("light_weighted") {
        count.min(MAX_POWER)
    } else {
        MAX_POWER
    }
}

fn plate_occupancy(world: &WorldState, pos: Pos) -> i32 {
    let inside = |x: f32, y: f32, z: f32| {
        x.floor() as i32 == pos.0
            && z.floor() as i32 == pos.2
            && y >= pos.1 as f32
            && y < pos.1 as f32 + 1.5
    };
    let mut count = 0;
    for (x, y, z) in world.player_positions() {
        if inside(x, y, z) {
            count += 1;
        }
    }
    for entity in world.entities_snapshot() {
        if inside(entity.position.x, entity.position.y, entity.position.z) {
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Pistons
// ---------------------------------------------------------------------------

/// `PistonStructureResolver`: a piston never moves more than this many blocks.
pub const PISTON_PUSH_LIMIT: usize = 12;

/// A piston extends or retracts one game tick after it hears the change.
pub const PISTON_DELAY: i32 = 1;

/// Dispenser/dropper slots (`Dispenser` uses a 9-slot container).
pub const CONTAINER_SLOTS: usize = 9;

/// `facing_direction` in `Facing` order: down, up, north, south, west, east.
const FACING_OFFSETS: [(i32, i32, i32); 6] = [
    (0, -1, 0),
    (0, 1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (-1, 0, 0),
    (1, 0, 0),
];

/// Blocks a piston can never move: the vanilla `movable = false` list plus the
/// pistons themselves.
const IMMOVABLE: [&str; 12] = [
    "bedrock",
    "obsidian",
    "crying_obsidian",
    "respawn_anchor",
    "enchanting_table",
    "ender_chest",
    "beacon",
    "moving_block",
    "piston",
    "sticky_piston",
    "piston_arm_collision",
    "sticky_piston_arm_collision",
];

fn step(pos: Pos, direction: (i32, i32, i32)) -> Pos {
    (
        pos.0 + direction.0,
        pos.1 + direction.1,
        pos.2 + direction.2,
    )
}

fn is_air(id: u32) -> bool {
    id == AIR
        || block_states::state_name(id).is_some_and(|name| name == "air" || name.ends_with("_air"))
}

/// Direction a piston pushes (or a dispenser faces), from `facing_direction`.
fn facing_direction(id: u32) -> Option<(i32, i32, i32)> {
    let facing = block_states::int_property(id, "facing_direction")?;
    FACING_OFFSETS.get(usize::try_from(facing).ok()?).copied()
}

/// Where the arm of the piston at `pos` is, when it is extended.
fn piston_head(world: &WorldState, pos: Pos, id: u32) -> Option<Pos> {
    let head = step(pos, facing_direction(id)?);
    let head_id = world.block_at(head.0, head.1, head.2);
    let name = block_states::state_name(head_id)?;
    if !is_piston_arm_name(name) {
        return None;
    }
    // An arm belongs to the piston behind it only when it points the same way:
    // two pistons facing each other keep separate heads.
    if block_states::int_property(head_id, "facing_direction")
        != block_states::int_property(id, "facing_direction")
    {
        return None;
    }
    Some(head)
}

/// A block the piston can carry: anything but air and the immovable set.
fn is_pushable(id: u32) -> bool {
    match block_states::state_name(id) {
        Some(name) => !name.ends_with("_air") && !IMMOVABLE.contains(&name),
        // Custom/unknown block: assume movable, like PocketMine does.
        None => !is_air(id),
    }
}

/// `Piston::onNearbyBlockChange()`: compare the power with the arm in front.
fn apply_piston(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(name) = block_states::state_name(id) else {
        return;
    };
    if !is_piston_name(name) || facing_direction(id).is_none() {
        return;
    }
    let powered = incoming_power(world, pos, None) > 0;
    let extended = piston_head(world, pos, id).is_some();
    if powered == extended {
        return;
    }
    let now = world.redstone_tick();
    world
        .redstone
        .schedule(pos, now + PISTON_DELAY, Scheduled::Piston);
}

/// Scheduled piston tick: move the arm and the blocks to match the power.
fn run_piston(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(name) = block_states::state_name(id) else {
        return;
    };
    if !is_piston_name(name) {
        return;
    }
    let Some(direction) = facing_direction(id) else {
        return;
    };
    let powered = incoming_power(world, pos, None) > 0;
    let extended = piston_head(world, pos, id).is_some();
    if powered && !extended {
        extend_piston(world, pos, id, direction);
    } else if !powered && extended {
        retract_piston(world, pos, name, id, direction);
    }
}

/// Pushes up to [`PISTON_PUSH_LIMIT`] blocks one cell forward and puts the arm
/// in the freed cell.
fn extend_piston(world: &WorldState, pos: Pos, id: u32, direction: (i32, i32, i32)) {
    let mut moving: Vec<Pos> = Vec::new();
    let mut cell = step(pos, direction);
    loop {
        let cell_id = world.block_at(cell.0, cell.1, cell.2);
        if is_air(cell_id) {
            break;
        }
        if !is_pushable(cell_id) || moving.len() == PISTON_PUSH_LIMIT {
            // Blocked: the piston stays put, like vanilla.
            return;
        }
        moving.push(cell);
        cell = step(cell, direction);
    }
    // Far end first, so a cell is never overwritten before it is vacated.
    for from in moving.iter().rev() {
        let to = step(*from, direction);
        let moved = world.block_at(from.0, from.1, from.2);
        world.set_block(to.0, to.1, to.2, moved);
        world.set_block(from.0, from.1, from.2, AIR);
    }
    let head = step(pos, direction);
    if let Some(arm) = piston_arm(id) {
        world.set_block(head.0, head.1, head.2, arm);
    }
}

/// The head block matching the piston's material and facing.
fn piston_arm(id: u32) -> Option<u32> {
    let sticky = block_states::state_name(id)? == "sticky_piston";
    let name = if sticky {
        "sticky_piston_arm_collision"
    } else {
        "piston_arm_collision"
    };
    let arm = block_states::network_id(name)?;
    let facing = block_states::int_property(id, "facing_direction")?;
    block_states::with_property(arm, "facing_direction", &facing.to_string()).or(Some(arm))
}

/// Drops the arm and, for a sticky piston, pulls the single block in front back
/// with it (only into the cell the arm leaves empty).
fn retract_piston(world: &WorldState, pos: Pos, name: &str, id: u32, direction: (i32, i32, i32)) {
    let _ = id;
    let head = step(pos, direction);
    let head_id = world.block_at(head.0, head.1, head.2);
    if block_states::state_name(head_id).is_some_and(is_piston_arm_name) {
        world.set_block(head.0, head.1, head.2, AIR);
    }
    if name != "sticky_piston" {
        return;
    }
    // The pushed block sits one cell further ahead: pull it back into the cell
    // the arm just left (vanilla: one block, never a chain).
    let source = step(head, direction);
    let source_id = world.block_at(source.0, source.1, source.2);
    if is_pushable(source_id) && is_air(world.block_at(head.0, head.1, head.2)) {
        world.set_block(head.0, head.1, head.2, source_id);
        world.set_block(source.0, source.1, source.2, AIR);
    }
}

// ---------------------------------------------------------------------------
// Dispensers and droppers
// ---------------------------------------------------------------------------

/// `Dispenser::onNearbyBlockChange()` / `Dropper`: a rising edge fires once,
/// and `triggered_bit` keeps it from firing again until it is cleared.
fn apply_dispenser(world: &WorldState, pos: Pos) {
    let id = world.block_at(pos.0, pos.1, pos.2);
    let Some(name) = block_states::state_name(id) else {
        return;
    };
    if !is_dispenser_name(name) {
        return;
    }
    let powered = incoming_power(world, pos, None) > 0;
    // `triggered_bit` latches on the rising edge, so holding the signal powers
    // the dispenser once: it fires again only after the power drops.
    let triggered = block_states::bool_property(id, "triggered_bit").unwrap_or(false);
    if powered && !triggered {
        set_triggered(world, pos, id, true);
        eject(world, pos, id, name);
    } else if !powered && triggered {
        set_triggered(world, pos, id, false);
    }
}

fn set_triggered(world: &WorldState, pos: Pos, id: u32, triggered: bool) {
    let value = if triggered { "true" } else { "false" };
    if let Some(updated) = block_states::with_property(id, "triggered_bit", value) {
        world.set_block(pos.0, pos.1, pos.2, updated);
    }
}

/// Takes one item out of the container and uses it (dispenser) or drops it
/// (dropper) out of the front face.
fn eject(world: &WorldState, pos: Pos, id: u32, name: &str) {
    let Some(direction) = facing_direction(id) else {
        return;
    };
    let front = step(pos, direction);
    let origin = Vector3f::new(
        front.0 as f32 + 0.5,
        front.1 as f32 + 0.5,
        front.2 as f32 + 0.5,
    );
    let Some(item) = take_item(world, pos) else {
        return;
    };
    // `Dispenser::dispense()`: the item leaves at 6/8 of a block per tick in
    // the facing direction, with the usual slight upward nudge.
    let motion = Vector3f::new(
        direction.0 as f32 * 0.6,
        direction.1 as f32 * 0.6 + 0.1,
        direction.2 as f32 * 0.6,
    );
    if name == "dispenser" {
        if let Some(kind) = projectile_for(item.network_id) {
            world.spawn_projectile(kind, origin, motion, "Dispenser", 0, 0.0, false);
            return;
        }
        if is_item(item.network_id, "tnt") {
            world.spawn_primed_tnt(origin, 80);
            return;
        }
    }
    // A dropper, and every dispenser item without a special action, simply
    // shoots the stack out as an item entity.
    world.spawn_item(origin, item, Some(motion), "Dispenser");
}

/// Removes one item from the first non-empty slot and returns the ejected
/// stack.
fn take_item(world: &WorldState, pos: Pos) -> Option<ItemStack> {
    let mut slots = world.redstone.container(pos);
    let index = slots.iter().position(|slot| !slot.is_empty())?;
    let mut stack = slots[index].clone();
    stack.count = 1;
    slots[index].count -= 1;
    if slots[index].count == 0 {
        slots[index] = ItemStack::empty();
    }
    world.redstone.set_container(pos, slots);
    Some(stack)
}

/// `InventoryHolder::onBreak()`: a broken dispenser or dropper spills every
/// slot into the world, on top of the block's own item form.
pub fn drop_contents(world: &WorldState, pos: Pos, block_id: u32, owner: &str) {
    let Some(name) = block_states::state_name(block_id) else {
        return;
    };
    if !is_dispenser_name(name) {
        return;
    }
    let slots = world.redstone.container(pos);
    world.redstone.set_container(pos, Vec::new());
    let origin = Vector3f::new(pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, pos.2 as f32 + 0.5);
    for item in slots.into_iter().filter(|item| !item.is_empty()) {
        world.spawn_item(origin, item, None, owner);
    }
}

fn is_item(network_id: i16, name: &str) -> bool {
    crate::inventory::item_canonical_name(network_id)
        .is_some_and(|canonical| canonical.strip_prefix("minecraft:") == Some(name))
}

/// `Dispenser::$itemTypeMap`: the items that are shot as a projectile instead
/// of being dropped.
fn projectile_for(network_id: i16) -> Option<crate::entity::ProjectileKind> {
    use crate::entity::ProjectileKind;
    let canonical = crate::inventory::item_canonical_name(network_id)?;
    match canonical.strip_prefix("minecraft:").unwrap_or(canonical) {
        "arrow" => Some(ProjectileKind::Arrow),
        "snowball" => Some(ProjectileKind::Snowball),
        "egg" => Some(ProjectileKind::Egg),
        "ender_pearl" => Some(ProjectileKind::EnderPearl),
        "splash_potion" | "lingering_potion" => Some(ProjectileKind::SplashPotion),
        "experience_bottle" => Some(ProjectileKind::ExperienceBottle),
        "trident" => Some(ProjectileKind::Trident),
        "ice_bomb" => Some(ProjectileKind::IceBomb),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use bedrock_world::generator::flat::FlatGenerator;

    fn world() -> WorldState {
        // Flat world: grass from y = 0..=3, air above.
        WorldState::new("world", Arc::new(FlatGenerator::new()))
    }

    fn network(name: &str) -> u32 {
        block_states::network_id(name).unwrap_or_else(|| panic!("no default state for {name}"))
    }

    fn name_at(world: &WorldState, x: i32, y: i32, z: i32) -> &'static str {
        block_states::state_name(world.block_at(x, y, z)).expect("canonical state")
    }

    fn signal(world: &WorldState, x: i32, y: i32, z: i32) -> i32 {
        block_states::int_property(world.block_at(x, y, z), "redstone_signal").unwrap_or(-1)
    }

    /// A lever on the floor of the flat world (the canonical default is a
    /// ceiling lever, which needs support above).
    fn floor_lever() -> u32 {
        block_states::with_property(network("lever"), "lever_direction", "up_north_south").unwrap()
    }

    /// A floor button (`facing_direction = 1` = up).
    fn floor_button() -> u32 {
        block_states::with_property(network("stone_button"), "facing_direction", "1").unwrap()
    }

    fn tick(world: &WorldState, ticks: i32) {
        for _ in 0..ticks {
            world.tick();
        }
    }

    fn open_bit(world: &WorldState, x: i32, y: i32, z: i32) -> Option<bool> {
        block_states::bool_property(world.block_at(x, y, z), "open_bit")
    }

    /// A door opened by hand stays open. The block update caused by the click
    /// used to close it again on the next tick, because Bedrock keeps no
    /// `powered` property: the reference only flips `open_bit` when the signal
    /// changes, which `mechanism_powered` mirrors.
    #[test]
    fn a_hand_opened_door_stays_open_across_ticks() {
        let world = world();
        let door = network("spruce_door");
        let upper = block_states::with_property(door, "upper_block_bit", "true").unwrap();
        world.set_block(1, 4, 0, door);
        world.set_block(1, 5, 0, upper);

        // `Door::onInteract()`: both halves open.
        let open = block_states::with_property(door, "open_bit", "true").unwrap();
        world.set_block(1, 4, 0, open);
        world.set_block(
            1,
            5,
            0,
            block_states::with_property(open, "upper_block_bit", "true").unwrap(),
        );

        tick(&world, 4);
        assert_eq!(open_bit(&world, 1, 4, 0), Some(true), "the click is not undone");
        assert_eq!(open_bit(&world, 1, 5, 0), Some(true));
    }

    /// The remembered signal still lets redstone drive the door: powering it
    /// opens both halves, dropping the signal closes them.
    #[test]
    fn a_powered_door_follows_the_signal_changes() {
        let world = world();
        let door = network("spruce_door");
        let upper = block_states::with_property(door, "upper_block_bit", "true").unwrap();
        world.set_block(2, 4, 0, door);
        world.set_block(2, 5, 0, upper);
        tick(&world, 2);
        assert_eq!(open_bit(&world, 2, 4, 0), Some(false));

        world.set_block(3, 4, 0, network("redstone_block"));
        tick(&world, 2);
        assert_eq!(open_bit(&world, 2, 4, 0), Some(true));
        assert_eq!(open_bit(&world, 2, 5, 0), Some(true), "both halves swing");

        world.set_block(3, 4, 0, network("air"));
        tick(&world, 2);
        assert_eq!(open_bit(&world, 2, 4, 0), Some(false));
        assert_eq!(open_bit(&world, 2, 5, 0), Some(false));
    }

    /// A lever powers the dust next to it and the dust lights a lamp, all
    /// inside the block-update phase of the same tick.
    #[test]
    fn a_lever_lights_a_lamp_through_dust() {
        let world = world();
        let lever = floor_lever();
        let lever_on = block_states::with_property(lever, "open_bit", "true").unwrap();
        world.set_block(0, 4, 0, lever_on);
        for x in 1..=3 {
            world.set_block(x, 4, 0, network("redstone_wire"));
        }
        world.set_block(4, 4, 0, network("redstone_lamp"));

        tick(&world, 1);
        assert_eq!(signal(&world, 1, 4, 0), 15, "dust takes the full 15");
        assert_eq!(name_at(&world, 4, 4, 0), "lit_redstone_lamp");

        // Turning the lever off releases the circuit in the same tick.
        world.set_block(0, 4, 0, lever);
        tick(&world, 1);
        assert_eq!(signal(&world, 1, 4, 0), 0);
        assert_eq!(name_at(&world, 4, 4, 0), "redstone_lamp");
    }

    /// Dust decays one level per block: 15 reaches 15 cells, the 16th is dead.
    #[test]
    fn dust_loses_one_level_per_block() {
        let world = world();
        let lever_on = block_states::with_property(floor_lever(), "open_bit", "true").unwrap();
        world.set_block(0, 4, 0, lever_on);
        for x in 1..=16 {
            world.set_block(x, 4, 0, network("redstone_wire"));
        }
        tick(&world, 2);

        assert_eq!(signal(&world, 1, 4, 0), 15);
        assert_eq!(signal(&world, 10, 4, 0), 6);
        assert_eq!(signal(&world, 15, 4, 0), 1);
        assert_eq!(
            signal(&world, 16, 4, 0),
            0,
            "the 16th block is out of range"
        );
    }

    /// A repeater waits 1 redstone tick (2 gt) and restores the signal to 15.
    #[test]
    fn a_repeater_delays_and_restores_the_signal() {
        let world = world();
        let lever_on = block_states::with_property(floor_lever(), "open_bit", "true").unwrap();
        world.set_block(0, 4, 0, lever_on);
        world.set_block(1, 4, 0, network("redstone_wire"));
        // Facing east: the back input is the west face.
        let repeater = block_states::with_property(
            network("unpowered_repeater"),
            "cardinal_direction",
            "east",
        )
        .unwrap();
        world.set_block(2, 4, 0, repeater);
        world.set_block(3, 4, 0, network("redstone_wire"));
        world.set_block(4, 4, 0, network("redstone_lamp"));

        tick(&world, 1);
        assert_eq!(
            name_at(&world, 2, 4, 0),
            "unpowered_repeater",
            "not yet: 2 gt"
        );
        tick(&world, 1);
        assert_eq!(name_at(&world, 2, 4, 0), "unpowered_repeater");
        tick(&world, 1);
        assert_eq!(name_at(&world, 2, 4, 0), "powered_repeater");
        assert_eq!(
            signal(&world, 3, 4, 0),
            15,
            "a repeater restores full strength"
        );
        assert_eq!(name_at(&world, 4, 4, 0), "lit_redstone_lamp");
    }

    /// A torch inverts the power of the block it is mounted on, after 1
    /// redstone tick.
    #[test]
    fn a_torch_inverts_the_power_of_its_support() {
        let world = world();
        let lever = floor_lever();
        let lever_on = block_states::with_property(lever, "open_bit", "true").unwrap();
        // A stone block is the torch's support; a lever on top of it powers the
        // block, which is what the torch reads. The torch hangs on the west
        // face.
        world.set_block(1, 4, 0, network("stone"));
        world.set_block(1, 5, 0, lever_on);
        let torch = block_states::with_property(
            network("redstone_torch"),
            "torch_facing_direction",
            "west",
        )
        .unwrap();
        world.set_block(2, 4, 0, torch);

        tick(&world, 1);
        assert_eq!(
            name_at(&world, 2, 4, 0),
            "redstone_torch",
            "still lit during its delay"
        );
        tick(&world, 2);
        assert_eq!(
            name_at(&world, 2, 4, 0),
            "unlit_redstone_torch",
            "powered support turns it off"
        );

        world.set_block(1, 5, 0, lever);
        tick(&world, 3);
        assert_eq!(
            name_at(&world, 2, 4, 0),
            "redstone_torch",
            "unpowered support lights it again"
        );
    }

    /// Compare mode passes the back signal through when no side input is
    /// stronger; subtract mode outputs `back - side`.
    #[test]
    fn a_comparator_compares_and_subtracts() {
        let world = world();
        let lever_on = block_states::with_property(floor_lever(), "open_bit", "true").unwrap();
        // Back input: 15 from a lever through dust.
        world.set_block(0, 4, 0, lever_on);
        world.set_block(1, 4, 0, network("redstone_wire"));
        let comparator = block_states::with_property(
            network("unpowered_comparator"),
            "cardinal_direction",
            "east",
        )
        .unwrap();
        world.set_block(2, 4, 0, comparator);
        // Side input: 15 on the north face.
        world.set_block(2, 4, -2, lever_on);
        world.set_block(2, 4, -1, network("redstone_wire"));
        // Output: dust and a lamp.
        world.set_block(3, 4, 0, network("redstone_wire"));
        world.set_block(4, 4, 0, network("redstone_lamp"));

        tick(&world, 3);
        assert_eq!(name_at(&world, 2, 4, 0), "powered_comparator");
        assert_eq!(
            signal(&world, 3, 4, 0),
            15,
            "back 15 >= side 15: full output"
        );
        assert_eq!(name_at(&world, 4, 4, 0), "lit_redstone_lamp");

        // Subtract mode: 15 - 15 = 0.
        let subtract =
            block_states::with_property(comparator, "output_subtract_bit", "true").unwrap();
        world.set_block(2, 4, 0, subtract);
        tick(&world, 3);
        assert_eq!(name_at(&world, 2, 4, 0), "unpowered_comparator");
        assert_eq!(signal(&world, 3, 4, 0), 0);
        assert_eq!(name_at(&world, 4, 4, 0), "redstone_lamp");
    }

    /// A stone button stays pressed for 20 gt, then releases itself and the
    /// circuit goes dark.
    #[test]
    fn a_button_releases_itself_after_its_duration() {
        let world = world();
        world.set_block(0, 4, 0, floor_button());
        world.set_block(1, 4, 0, network("redstone_wire"));
        world.set_block(2, 4, 0, network("redstone_lamp"));

        assert!(world.interact_block(0, 4, 0).is_some());
        tick(&world, 1);
        assert_eq!(signal(&world, 1, 4, 0), 15);
        assert_eq!(name_at(&world, 2, 4, 0), "lit_redstone_lamp");

        tick(&world, 18);
        assert_eq!(
            name_at(&world, 2, 4, 0),
            "lit_redstone_lamp",
            "still pressed at 19 gt"
        );
        tick(&world, 1);
        assert_eq!(
            name_at(&world, 2, 4, 0),
            "redstone_lamp",
            "released at 20 gt"
        );
    }

    /// A plate activates on contact and only releases at its next recheck.
    #[test]
    fn a_pressure_plate_tracks_entities_and_releases_late() {
        let world = world();
        world.set_block(0, 4, 0, network("wooden_pressure_plate"));
        world.set_block(1, 4, 0, network("redstone_wire"));
        world.set_block(2, 4, 0, network("redstone_lamp"));

        world.set_player_positions(vec![(0.5, 5.06, 0.5)]);
        tick(&world, 1);
        assert_eq!(signal(&world, 0, 4, 0), 15);
        assert_eq!(name_at(&world, 2, 4, 0), "lit_redstone_lamp");

        world.set_player_positions(Vec::new());
        tick(&world, 1);
        assert_eq!(signal(&world, 0, 4, 0), 15, "release waits for the recheck");
        tick(&world, PLATE_RECHECK);
        assert_eq!(signal(&world, 0, 4, 0), 0);
        assert_eq!(name_at(&world, 2, 4, 0), "redstone_lamp");
    }

    /// Weighted plates encode the entity count instead of a plain 15.
    #[test]
    fn weighted_plates_scale_with_the_entity_count() {
        let world = world();
        world.set_block(0, 4, 4, network("light_weighted_pressure_plate"));
        world.set_block(0, 4, 8, network("heavy_weighted_pressure_plate"));
        world.set_player_positions(vec![(0.5, 5.06, 4.5), (0.7, 5.06, 4.9), (0.5, 5.06, 8.5)]);
        tick(&world, 2);
        assert_eq!(
            signal(&world, 0, 4, 4),
            2,
            "light weighted: one level per entity"
        );
        assert_eq!(signal(&world, 0, 4, 8), 1, "heavy weighted: ceil(1 / 10)");
    }

    /// Dust connects sideways to wire and components, and upwards over a
    /// non-solid block.
    #[test]
    fn dust_connections_follow_wires_and_components() {
        let world = world();
        world.set_block(0, 4, 0, network("redstone_wire"));
        tick(&world, 1);
        // Nothing around: no connections at all.
        assert_eq!(
            block_states::string_property(world.block_at(0, 4, 0), "redstone_north"),
            Some("none")
        );

        // A lever on the south face connects, and a wire one block up the east
        // riser is a staircase link.
        world.set_block(0, 4, 1, floor_lever());
        world.set_block(1, 4, 0, network("stone"));
        world.set_block(1, 5, 0, network("redstone_wire"));
        tick(&world, 2);
        let id = world.block_at(0, 4, 0);
        assert_eq!(
            block_states::string_property(id, "redstone_south"),
            Some("side")
        );
        assert_eq!(
            block_states::string_property(id, "redstone_east"),
            Some("up")
        );
    }
}

#[cfg(test)]
mod piston_tests {
    use super::*;
    use std::sync::Arc;

    use bedrock_world::generator::flat::FlatGenerator;

    fn world() -> WorldState {
        WorldState::new("world", Arc::new(FlatGenerator::new()))
    }

    fn network(name: &str) -> u32 {
        block_states::network_id(name).unwrap()
    }

    fn name_at(world: &WorldState, x: i32, y: i32, z: i32) -> &'static str {
        block_states::state_name(world.block_at(x, y, z)).unwrap_or("air")
    }

    fn tick(world: &WorldState, ticks: i32) {
        for _ in 0..ticks {
            world.tick();
        }
    }

    /// `facing_direction` in `Facing` order: 0 down, 1 up, 5 east.
    fn facing(id: u32, direction: i32) -> u32 {
        block_states::with_property(id, "facing_direction", &direction.to_string()).unwrap()
    }

    #[test]
    fn a_piston_pushes_a_block_and_retracts() {
        let world = world();
        world.set_block(0, 4, 0, facing(network("piston"), 5));
        world.set_block(1, 4, 0, network("stone"));

        // An adjacent redstone block powers it; the arm appears one tick later.
        world.set_block(0, 4, 1, network("redstone_block"));
        tick(&world, 2);
        assert_eq!(name_at(&world, 0, 4, 0), "piston");
        assert_eq!(name_at(&world, 1, 4, 0), "piston_arm_collision");
        assert_eq!(name_at(&world, 2, 4, 0), "stone");

        world.set_block(0, 4, 1, AIR);
        tick(&world, 2);
        assert_eq!(name_at(&world, 1, 4, 0), "air", "the arm retracts");
        assert_eq!(name_at(&world, 2, 4, 0), "stone", "the block stays pushed");
    }

    /// The head is its own block, so it must not outlive the piston body.
    #[test]
    fn breaking_a_piston_removes_its_head() {
        let world = world();
        world.set_block(0, 4, 0, facing(network("piston"), 5));
        world.set_block(0, 4, 1, network("redstone_block"));
        tick(&world, 2);
        assert_eq!(name_at(&world, 1, 4, 0), "piston_arm_collision");

        // Break the piston body: the arm in front goes with it.
        world.set_block(0, 4, 0, AIR);
        assert_eq!(name_at(&world, 1, 4, 0), "air");
    }

    #[test]
    fn replacing_a_piston_removes_its_head() {
        let world = world();
        world.set_block(0, 4, 0, facing(network("sticky_piston"), 5));
        world.set_block(1, 4, 0, network("stone"));
        world.set_block(0, 4, 1, network("redstone_block"));
        tick(&world, 2);
        assert_eq!(name_at(&world, 1, 4, 0), "sticky_piston_arm_collision");
        assert_eq!(name_at(&world, 2, 4, 0), "stone");

        // Any edit over the body (a block, /setblock, another piston) drops it.
        world.set_block(0, 4, 0, network("stone"));
        assert_eq!(name_at(&world, 1, 4, 0), "air");
    }

    /// A sticky head only belongs to a sticky piston, so a mismatched or
    /// hand-placed head is cleaned up.
    #[test]
    fn a_head_needs_its_matching_piston_behind_it() {
        let world = world();
        let sticky_arm = block_states::with_property(
            network("sticky_piston_arm_collision"),
            "facing_direction",
            "5",
        )
        .unwrap();
        world.set_block(1, 4, 0, sticky_arm);
        assert_eq!(name_at(&world, 1, 4, 0), "air", "no piston behind it");

        world.set_block(0, 4, 0, facing(network("piston"), 5));
        world.set_block(1, 4, 0, sticky_arm);
        assert_eq!(
            name_at(&world, 1, 4, 0),
            "air",
            "a normal piston does not own a sticky head"
        );

        world.set_block(0, 4, 0, facing(network("sticky_piston"), 5));
        world.set_block(1, 4, 0, sticky_arm);
        assert_eq!(name_at(&world, 1, 4, 0), "sticky_piston_arm_collision");
    }

    /// A piston never pushes another piston, and a blocked push must not leave
    /// a head behind either.
    #[test]
    fn a_piston_cannot_push_its_neighbour() {
        let world = world();
        world.set_block(0, 4, 0, facing(network("piston"), 5));
        world.set_block(1, 4, 0, facing(network("piston"), 5));
        world.set_block(0, 4, 1, network("redstone_block"));
        tick(&world, 2);
        assert_eq!(name_at(&world, 1, 4, 0), "piston", "the neighbour stays");
        assert_eq!(name_at(&world, 2, 4, 0), "air");

        // Destroying the neighbour (whatever the cause) takes its head with it.
        world.set_block(1, 4, 0, AIR);
        assert_eq!(name_at(&world, 2, 4, 0), "air");
    }

    #[test]
    fn a_sticky_piston_pulls_its_block_back() {
        let world = world();
        world.set_block(0, 4, 0, facing(network("sticky_piston"), 5));
        world.set_block(1, 4, 0, network("stone"));
        world.set_block(0, 4, 1, network("redstone_block"));
        tick(&world, 2);
        assert_eq!(name_at(&world, 1, 4, 0), "sticky_piston_arm_collision");
        assert_eq!(name_at(&world, 2, 4, 0), "stone");

        world.set_block(0, 4, 1, AIR);
        tick(&world, 2);
        assert_eq!(name_at(&world, 1, 4, 0), "stone", "pulled back by the arm");
        assert_eq!(name_at(&world, 2, 4, 0), "air");
    }

    #[test]
    fn a_piston_respects_the_push_limit_and_immovable_blocks() {
        // One block over `PISTON_PUSH_LIMIT`: nothing moves at all.
        let limited = world();
        limited.set_block(0, 4, 0, facing(network("piston"), 5));
        for x in 1..=(PISTON_PUSH_LIMIT as i32 + 1) {
            limited.set_block(x, 4, 0, network("stone"));
        }
        limited.set_block(0, 4, 1, network("redstone_block"));
        tick(&limited, 2);
        assert_eq!(name_at(&limited, 1, 4, 0), "stone", "13 blocks do not move");
        assert_eq!(name_at(&limited, 2, 4, 0), "stone");

        // Obsidian never moves, so the piston cannot extend into it.
        let hardened = world();
        hardened.set_block(0, 4, 0, facing(network("piston"), 5));
        hardened.set_block(1, 4, 0, network("obsidian"));
        hardened.set_block(0, 4, 1, network("redstone_block"));
        tick(&hardened, 2);
        assert_eq!(name_at(&hardened, 1, 4, 0), "obsidian");
        assert_eq!(name_at(&hardened, 0, 4, 0), "piston");
    }

    #[test]
    fn a_dispenser_shoots_one_arrow_per_pulse() {
        let world = world();
        world.set_block(0, 4, 0, facing(network("dispenser"), 1));
        let mut slots = vec![ItemStack::empty(); CONTAINER_SLOTS];
        slots[0] = ItemStack::new(
            crate::inventory::item_network_id("minecraft:arrow").unwrap(),
            3,
        );
        world.redstone.set_container((0, 4, 0), slots);

        world.set_block(0, 4, 1, network("redstone_block"));
        tick(&world, 2);
        let kinds: Vec<&'static str> = world
            .entities_snapshot()
            .iter()
            .map(|entity| entity.network_type_id())
            .collect();
        assert_eq!(kinds, vec!["minecraft:arrow"]);
        assert_eq!(world.redstone.container((0, 4, 0))[0].count, 2);

        // Held power is a single pulse: the latch stops it from firing again.
        tick(&world, 5);
        assert_eq!(world.entities_snapshot().len(), 1);
        assert_eq!(world.redstone.container((0, 4, 0))[0].count, 2);

        // Dropping the power re-arms it for the next rising edge.
        world.set_block(0, 4, 1, AIR);
        tick(&world, 2);
        world.set_block(0, 4, 1, network("redstone_block"));
        tick(&world, 2);
        assert_eq!(world.entities_snapshot().len(), 2);
        assert_eq!(world.redstone.container((0, 4, 0))[0].count, 1);
    }

    #[test]
    fn a_dropper_ejects_an_item_entity() {
        let world = world();
        world.set_block(0, 4, 0, facing(network("dropper"), 1));
        let mut slots = vec![ItemStack::empty(); CONTAINER_SLOTS];
        slots[4] = ItemStack::new(
            crate::inventory::item_network_id("minecraft:stone").unwrap(),
            1,
        );
        world.redstone.set_container((0, 4, 0), slots);

        world.set_block(0, 4, 1, network("redstone_block"));
        tick(&world, 2);
        let entities = world.entities_snapshot();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].network_type_id(), "minecraft:item");
        // The ejected stack is the block itself, thrown out of the front face
        // (this dropper faces up), so it lands as a real item a player can
        // pick up on either edition.
        let crate::entity::EntityKind::Item { item, .. } = &entities[0].kind else {
            panic!("the dropper must eject an item entity");
        };
        assert_eq!(
            item.network_id,
            crate::inventory::item_network_id("minecraft:stone").unwrap()
        );
        assert_eq!(item.count, 1);
        assert!(entities[0].motion.y > 0.0, "thrown out the front face");
        assert!(world
            .redstone
            .container((0, 4, 0))
            .iter()
            .all(ItemStack::is_empty));
    }
}

#[cfg(test)]
mod drop_tests {
    use super::*;
    use std::sync::Arc;

    use bedrock_world::generator::flat::FlatGenerator;

    fn world() -> WorldState {
        WorldState::new("world", Arc::new(FlatGenerator::new()))
    }

    fn network(name: &str) -> u32 {
        block_states::network_id(name).unwrap()
    }

    fn facing(id: u32, direction: i32) -> u32 {
        block_states::with_property(id, "facing_direction", &direction.to_string()).unwrap()
    }

    fn item(name: &str, count: u16) -> ItemStack {
        ItemStack::new(crate::inventory::item_network_id(name).unwrap(), count)
    }

    /// Every item entity in the world as `(item id, count)`, sorted.
    fn dropped(world: &WorldState) -> Vec<(i16, u16)> {
        let mut stacks: Vec<(i16, u16)> = world
            .entities_snapshot()
            .iter()
            .filter_map(|entity| match &entity.kind {
                crate::entity::EntityKind::Item { item, .. } => Some((item.network_id, item.count)),
                _ => None,
            })
            .collect();
        stacks.sort_unstable();
        stacks
    }

    #[test]
    fn pistons_and_containers_have_an_item_form() {
        for (name, expected) in [
            ("piston", "minecraft:piston"),
            ("sticky_piston", "minecraft:sticky_piston"),
            ("dispenser", "minecraft:dispenser"),
            ("dropper", "minecraft:dropper"),
        ] {
            let stack = crate::entity::block_item(network(name))
                .unwrap_or_else(|| panic!("{name} has no item form"));
            assert_eq!(
                crate::inventory::item_canonical_name(stack.network_id),
                Some(expected),
                "{name} drops the wrong item"
            );
        }
    }

    /// `World::useBreakOn()` + `InventoryHolder::onBreak()`: the block's item
    /// form drops and a dispenser spills every slot.
    #[test]
    fn breaking_a_dispenser_drops_it_and_its_contents() {
        let world = world();
        let dispenser = facing(network("dispenser"), 1);
        world.set_block(0, 4, 0, dispenser);
        let mut slots = vec![ItemStack::empty(); CONTAINER_SLOTS];
        slots[0] = item("minecraft:arrow", 5);
        slots[3] = item("minecraft:stone", 12);
        world.redstone.set_container((0, 4, 0), slots);

        let broken = world.block_at(0, 4, 0);
        assert!(world.set_block(0, 4, 0, AIR));
        world.drop_block_item(0, 4, 0, broken, "Steve");

        let mut expected = vec![
            (item("minecraft:arrow", 5).network_id, 5),
            (item("minecraft:dispenser", 1).network_id, 1),
            (item("minecraft:stone", 12).network_id, 12),
        ];
        expected.sort_unstable();
        assert_eq!(dropped(&world), expected);
        // The container is gone, so placing a new dispenser starts empty.
        assert!(world
            .redstone
            .container((0, 4, 0))
            .iter()
            .all(ItemStack::is_empty));
    }

    #[test]
    fn breaking_a_piston_and_a_dropper_drops_their_items() {
        let world = world();
        world.set_block(0, 4, 0, facing(network("piston"), 5));
        world.set_block(2, 4, 0, facing(network("dropper"), 1));
        let mut slots = vec![ItemStack::empty(); CONTAINER_SLOTS];
        slots[8] = item("minecraft:diamond", 3);
        world.redstone.set_container((2, 4, 0), slots);

        for (x, z) in [(0, 0), (2, 0)] {
            let broken = world.block_at(x, 4, z);
            assert!(world.set_block(x, 4, z, AIR));
            world.drop_block_item(x, 4, z, broken, "Steve");
        }

        let mut expected = vec![
            (item("minecraft:piston", 1).network_id, 1),
            (item("minecraft:dropper", 1).network_id, 1),
            (item("minecraft:diamond", 3).network_id, 3),
        ];
        expected.sort_unstable();
        assert_eq!(dropped(&world), expected);
    }

    /// A block with an empty container only drops itself, and other blocks are
    /// untouched by the spill hook.
    #[test]
    fn an_empty_dispenser_drops_only_itself() {
        let world = world();
        world.set_block(0, 4, 0, facing(network("dispenser"), 1));
        let broken = world.block_at(0, 4, 0);
        assert!(world.set_block(0, 4, 0, AIR));
        world.drop_block_item(0, 4, 0, broken, "Steve");
        assert_eq!(
            dropped(&world),
            vec![(item("minecraft:dispenser", 1).network_id, 1)]
        );
    }
}
