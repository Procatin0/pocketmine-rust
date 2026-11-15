//! Water (and lava) flow, translated from PocketMine's `Liquid`.
//!
//! The reference's model is used instead of Minecraft's own fluid engine
//! because that is what this server mirrors: `Liquid::onScheduledUpdate()`
//! runs every `tickRate()` ticks (5 for water, 30 for lava) and does three
//! things — recalculate its own level from its neighbours, dry up when no
//! liquid feeds it, and spread downwards/sideways.
//!
//! Level model (`Liquid::$decay` / `$falling`, Bedrock `liquid_depth`):
//! `liquid_depth = decay | (falling ? 0x8 : 0)`
//! (`BlockStateSerializerHelper`). A **source** is `!falling && decay == 0`; a
//! falling column keeps `decay = 0` so it never runs out on the way down.
//! [`MAX_DECAY`] bounds the horizontal reach: every sideways block costs
//! `flowDecayPerBlock` (1 for water, 2 for lava), so water crosses at most 7
//! blocks and lava 3 before it dries up.
//!
//! Two water sources side by side turn the block between them into a new
//! source (`Water::getMinAdjacentSourcesToFormSource()` = 2), which is what
//! makes a 2x2 pool infinite. Lava has no source-forming behaviour.
//!
//! [`MinimumCostFlowCalculator`] decides *where* the liquid tends to go: a
//! neighbour with a hole underneath wins immediately, otherwise the shortest
//! path to a hole (at most `4 / flowDecayPerBlock` steps) wins, so a puddle
//! runs to the nearest drop instead of spreading evenly in all directions.

use std::collections::HashMap;

use bedrock_world::block_id::AIR;
use bedrock_world::block_states;

use crate::physics;
use crate::world_state::{WorldState, WORLD_MAX_Y, WORLD_MIN_Y};

/// `Liquid::MAX_DECAY`: highest flow level before the liquid dries up.
pub const MAX_DECAY: i32 = 7;
/// `Water::tickRate()`.
pub const WATER_TICK_RATE: i32 = 5;
/// `Lava::tickRate()`.
pub const LAVA_TICK_RATE: i32 = 30;

/// `Facing::HORIZONTAL`, in the reference's order (used as the tie-break order
/// when several directions are equally good).
const HORIZONTAL: [i32; 4] = [physics::NORTH, physics::SOUTH, physics::WEST, physics::EAST];

// `MinimumCostFlowCalculator` cell states.
const CAN_FLOW_DOWN: i8 = 1;
const CAN_FLOW: i8 = 0;
const BLOCKED: i8 = -1;

/// Bucket items, from `data/item_ids.txt` (vanilla BDS 1.26.60).
pub const BUCKET_ITEM: i16 = 392;
pub const WATER_BUCKET_ITEM: i16 = 394;
pub const LAVA_BUCKET_ITEM: i16 = 395;

/// One liquid cell: `Liquid`'s `decay`, `falling` and `still` flags plus which
/// liquid it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Liquid {
    /// Water (`true`) or lava (`false`).
    pub water: bool,
    /// `Liquid::$decay` (`0` for a source and for falling columns).
    pub decay: i32,
    /// `Liquid::$falling`.
    pub falling: bool,
    /// `Liquid::$still`: only a still source has a Bedrock `water`/`lava`
    /// state; every other level is a `flowing_*` block.
    pub still: bool,
}

impl Liquid {
    /// Reads the liquid in a cell, `None` for every other block.
    pub fn from_state(id: u32) -> Option<Self> {
        let name = block_states::state_name(id)?;
        let water = match name {
            "water" | "flowing_water" => true,
            "lava" | "flowing_lava" => false,
            _ => return None,
        };
        let depth = block_states::int_property(id, "liquid_depth").unwrap_or(0);
        Some(Self {
            water,
            decay: depth & 0x7,
            falling: depth & 0x8 != 0,
            still: !name.starts_with("flowing"),
        })
    }

    /// `Liquid::isSource()`.
    pub fn is_source(&self) -> bool {
        !self.falling && self.decay == 0
    }

    /// `Liquid::tickRate()`.
    pub fn tick_rate(&self) -> i32 {
        if self.water {
            WATER_TICK_RATE
        } else {
            LAVA_TICK_RATE
        }
    }

    /// `Liquid::getFlowDecayPerBlock()`.
    pub fn flow_decay_per_block(&self) -> i32 {
        if self.water {
            1
        } else {
            2
        }
    }

    /// `Liquid::getMinAdjacentSourcesToFormSource()`: only water forms new
    /// sources.
    pub fn min_adjacent_sources(&self) -> Option<i32> {
        self.water.then_some(2)
    }

    /// The Bedrock block state for this level
    /// (`liquid_depth = decay | falling << 3`).
    pub fn state(&self) -> u32 {
        let still = self.still && self.is_source();
        let name = match (self.water, still) {
            (true, true) => "water",
            (true, false) => "flowing_water",
            (false, true) => "lava",
            (false, false) => "flowing_lava",
        };
        let Some(base) = block_states::network_id(name) else {
            return AIR;
        };
        let depth = self.decay | if self.falling { 0x8 } else { 0 };
        block_states::with_property(base, "liquid_depth", &depth.to_string()).unwrap_or(base)
    }

    /// `getStillForm()`/`getFlowingForm()`: `LiquidBucket::onInteractBlock()`
    /// places the flowing form.
    pub fn with_still(self, still: bool) -> Self {
        Self { still, ..self }
    }
}

/// `Liquid::canFlowInto($block)`: inside the world, flowable, and not a
/// source of any liquid (the reference leaves a TODO about limiting this to
/// the same liquid; the behaviour is kept as it is).
pub fn can_flow_into(world: &WorldState, x: i32, y: i32, z: i32) -> bool {
    if !(WORLD_MIN_Y..WORLD_MAX_Y).contains(&y) {
        return false;
    }
    let target = world.block_at(x, y, z);
    if !can_be_flowed_into(target) {
        return false;
    }
    !Liquid::from_state(target).is_some_and(|liquid| liquid.is_source())
}

/// `$block->canBeFlowedInto()`: `Flowable` blocks (air, liquids, plants, ...)
/// can be replaced by a liquid.
pub fn can_be_flowed_into(id: u32) -> bool {
    block_states::state_name(id).is_some_and(physics::is_replaceable)
}

/// `$block->isSolid()` (the reference keeps the default `true` for everything
/// that is not `Flowable`/`Liquid`).
fn is_solid(id: u32) -> bool {
    block_states::state_name(id).is_some_and(|name| !physics::is_replaceable(name))
}

/// `Liquid::getEffectiveFlowDecay($block)`: a falling column counts as a
/// source, anything that is not the same liquid as `-1`.
fn effective_flow_decay(world: &WorldState, x: i32, y: i32, z: i32, water: bool) -> i32 {
    match Liquid::from_state(world.block_at(x, y, z)) {
        Some(liquid) if liquid.water == water => {
            if liquid.falling {
                0
            } else {
                liquid.decay
            }
        }
        _ => -1,
    }
}

/// `Liquid::getSmallestFlowDecay()`: lowest decay of one horizontal
/// neighbour, counting sources for the source-forming rule.
fn smallest_flow_decay(
    world: &WorldState,
    x: i32,
    y: i32,
    z: i32,
    decay: i32,
    adjacent_sources: &mut i32,
    water: bool,
) -> i32 {
    let Some(block) = Liquid::from_state(world.block_at(x, y, z)) else {
        return decay;
    };
    if block.water != water {
        return decay;
    }
    let mut block_decay = block.decay;
    if block.is_source() {
        *adjacent_sources += 1;
    } else if block.falling {
        block_decay = 0;
    }
    if decay >= 0 && block_decay >= decay {
        decay
    } else {
        block_decay
    }
}

/// `Liquid::onScheduledUpdate()` for the cell at `(x, y, z)`.
pub fn on_scheduled_update(world: &WorldState, x: i32, y: i32, z: i32) {
    let Some(mut liquid) = Liquid::from_state(world.block_at(x, y, z)) else {
        return;
    };
    let multiplier = liquid.flow_decay_per_block();

    if !liquid.is_source() {
        let mut smallest = -100;
        let mut adjacent_sources = 0;
        for face in HORIZONTAL {
            let (dx, _, dz) = physics::offset(face);
            smallest = smallest_flow_decay(
                world,
                x + dx,
                y,
                z + dz,
                smallest,
                &mut adjacent_sources,
                liquid.water,
            );
        }

        let mut new_decay = smallest + multiplier;
        let mut falling = false;
        if new_decay > MAX_DECAY || smallest < 0 {
            new_decay = -1;
        }
        // Fed from above: the column is falling and therefore always a source
        // level, so it never thins out on the way down.
        if effective_flow_decay(world, x, y + 1, z, liquid.water) >= 0 {
            falling = true;
        }
        // Two neighbouring sources over something solid make a new source.
        if liquid.min_adjacent_sources().is_some_and(|min| adjacent_sources >= min) {
            let below = world.block_at(x, y - 1, z);
            let holds = is_solid(below)
                || Liquid::from_state(below)
                    .is_some_and(|other| other.water == liquid.water && other.is_source());
            if holds {
                new_decay = 0;
                falling = false;
            }
        }

        if falling != liquid.falling || (!falling && new_decay != liquid.decay) {
            if !falling && new_decay < 0 {
                // No liquid feeds this cell any more: it drains away.
                world.set_block(x, y, z, AIR);
                return;
            }
            liquid.falling = falling;
            liquid.decay = if falling { 0 } else { new_decay };
            world.set_block(x, y, z, liquid.state());
        }
    }

    // Read the cell again: the write above may have changed the level it
    // spreads with (the reference mutates `$this` in place).
    let Some(liquid) = Liquid::from_state(world.block_at(x, y, z)) else {
        return;
    };
    let multiplier = liquid.flow_decay_per_block();

    // `$this->flowIntoBlock($bottomBlock, 0, true)`: always try straight down.
    let below = world.block_at(x, y - 1, z);
    flow_into(world, x, y - 1, z, 0, true, liquid.water);

    if liquid.is_source() || !can_be_flowed_into(below) {
        let adjacent_decay = if liquid.falling { 1 } else { liquid.decay + multiplier };
        if adjacent_decay <= MAX_DECAY {
            for face in optimal_flow_directions(world, x, y, z, liquid.water) {
                let (dx, _, dz) = physics::offset(face);
                flow_into(world, x + dx, y, z + dz, adjacent_decay, false, liquid.water);
            }
        }
    }

    // `Liquid::onScheduledUpdate()` ends with `$this->checkForHarden()`.
    if let Some(liquid) = Liquid::from_state(world.block_at(x, y, z)) {
        check_for_harden(world, x, y, z, liquid);
    }
}

/// `Liquid::flowIntoBlock()`: only fills a cell that is flowable and not
/// already a liquid, so existing flow is never overwritten.
fn flow_into(
    world: &WorldState,
    x: i32,
    y: i32,
    z: i32,
    new_flow_decay: i32,
    falling: bool,
    water: bool,
) {
    // `Lava::flowIntoBlock()`: lava flowing into water turns that water into
    // stone instead of being blocked by it (the cell keeps the lava, the water
    // is replaced in its own position).
    if !water
        && Liquid::from_state(world.block_at(x, y, z)).is_some_and(|other| other.water)
    {
        liquid_collide(world, x, y, z, "stone");
        return;
    }
    if !can_flow_into(world, x, y, z) {
        return;
    }
    if Liquid::from_state(world.block_at(x, y, z)).is_some() {
        return;
    }
    let new = Liquid {
        water,
        decay: if falling { 0 } else { new_flow_decay },
        falling,
        // Inherited from the source, like `clone $this`; `state()` only keeps
        // the still form for a source, because a still `water` block only
        // exists at `liquid_depth` 0.
        still: true,
    };
    // PM runs `World::useBreakOn()` on a replaceable non-air block first (so a
    // flower in the way drops); the drop is not modelled here, the cell is
    // simply replaced.
    world.set_block(x, y, z, new.state());
}

/// `Liquid::liquidCollide()`: `BlockEventHelper::form()` replaces the cell and
/// the world plays `FizzSound` at its centre.
fn liquid_collide(world: &WorldState, x: i32, y: i32, z: i32, result: &str) {
    let Some(state) = block_states::network_id(result) else {
        return;
    };
    world.set_block(x, y, z, state);
    world.add_sound(
        bedrock_protocol::packets::level_sound_event::LEVEL_SOUND_FIZZ,
        -1,
        x,
        y,
        z,
    );
}

/// `Lava::checkForHarden()`: lava touching water hardens, and the result
/// depends on how thin the lava was.
///
/// - a lava source (`decay == 0`) becomes obsidian,
/// - lava with `decay <= 4` becomes cobblestone,
/// - thinner flow (or falling lava) is left alone.
///
/// Only the sides and the top are considered (`Facing::ALL` except `DOWN`),
/// so a falling column pouring onto water does not harden this way — that path
/// goes through [`flow_into`], which turns the water into stone.
fn check_for_harden(world: &WorldState, x: i32, y: i32, z: i32, liquid: Liquid) -> bool {
    if liquid.water || liquid.falling {
        return false;
    }
    for face in [physics::UP, physics::NORTH, physics::SOUTH, physics::WEST, physics::EAST] {
        let (dx, dy, dz) = physics::offset(face);
        let Some(colliding) = Liquid::from_state(world.block_at(x + dx, y + dy, z + dz)) else {
            continue;
        };
        if !colliding.water {
            continue;
        }
        if liquid.decay == 0 {
            liquid_collide(world, x, y, z, "obsidian");
            return true;
        }
        if liquid.decay <= 4 {
            liquid_collide(world, x, y, z, "cobblestone");
            return true;
        }
    }
    false
}

/// `Liquid::onNearbyBlockChange()` -> `checkForHarden()` for the edited cell
/// and its six neighbours.
///
/// PocketMine hardens on the block update itself, not on the flow tick, so two
/// liquids meeting react immediately instead of waiting up to `tickRate()`
/// ticks (30 for lava). Called after the edit is flushed, so the hardening
/// state is the last thing a viewer sees for that cell.
pub fn harden_around(world: &WorldState, x: i32, y: i32, z: i32) {
    let mut positions: [(i32, i32, i32); 7] = [(x, y, z); 7];
    for (index, face) in physics::ALL_FACES.iter().enumerate() {
        let (dx, dy, dz) = physics::offset(*face);
        positions[index + 1] = (x + dx, y + dy, z + dz);
    }
    for (bx, by, bz) in positions {
        let Some(liquid) = Liquid::from_state(world.block_at(bx, by, bz)) else {
            continue;
        };
        check_for_harden(world, bx, by, bz, liquid);
    }
}

/// `MinimumCostFlowCalculator::getOptimalFlowDirections()`: the horizontal
/// directions the liquid should spread into, cheapest first.
fn optimal_flow_directions(world: &WorldState, x: i32, y: i32, z: i32, water: bool) -> Vec<i32> {
    let flow_decay_per_block = if water { 1 } else { 2 };
    let mut visited: HashMap<(i32, i32, i32), i8> = HashMap::new();
    let mut costs = [1000_i32; 4];
    let mut max_cost = 4 / flow_decay_per_block;

    for (index, face) in HORIZONTAL.iter().enumerate() {
        let (dx, _, dz) = physics::offset(*face);
        let (nx, nz) = (x + dx, z + dz);
        if !can_flow_into(world, nx, y, nz) {
            visited.insert((nx, y, nz), BLOCKED);
        } else if can_be_flowed_into(world.block_at(nx, y - 1, nz)) {
            // A neighbour with a hole under it wins outright.
            visited.insert((nx, y, nz), CAN_FLOW_DOWN);
            costs[index] = 0;
            max_cost = 0;
        } else if max_cost > 0 {
            visited.insert((nx, y, nz), CAN_FLOW);
            let opposite = physics::opposite(*face);
            costs[index] = calculate_flow_cost(
                world,
                nx,
                y,
                nz,
                1,
                max_cost,
                opposite,
                opposite,
                &mut visited,
            );
            max_cost = max_cost.min(costs[index]);
        }
    }

    let min_cost = costs.iter().copied().min().unwrap_or(1000);
    HORIZONTAL
        .iter()
        .zip(costs)
        .filter(|(_, cost)| *cost == min_cost)
        .map(|(face, _)| *face)
        .collect()
}

/// `MinimumCostFlowCalculator::calculateFlowCost()`, the bounded search for
/// the shortest path to a cell that can drain downwards.
#[allow(clippy::too_many_arguments)]
fn calculate_flow_cost(
    world: &WorldState,
    block_x: i32,
    block_y: i32,
    block_z: i32,
    accumulated_cost: i32,
    max_cost: i32,
    origin_opposite: i32,
    last_opposite: i32,
    visited: &mut HashMap<(i32, i32, i32), i8>,
) -> i32 {
    let mut cost = 1000;
    for face in HORIZONTAL {
        if face == origin_opposite || face == last_opposite {
            continue;
        }
        let (dx, _, dz) = physics::offset(face);
        let (x, z) = (block_x + dx, block_z + dz);

        let status = *visited.entry((x, block_y, z)).or_insert_with(|| {
            if !can_flow_into(world, x, block_y, z) {
                BLOCKED
            } else if can_be_flowed_into(world.block_at(x, block_y - 1, z)) {
                CAN_FLOW_DOWN
            } else {
                CAN_FLOW
            }
        });

        if status == BLOCKED {
            continue;
        }
        if status == CAN_FLOW_DOWN {
            return accumulated_cost;
        }
        if accumulated_cost >= max_cost {
            continue;
        }

        let real_cost = calculate_flow_cost(
            world,
            x,
            block_y,
            z,
            accumulated_cost + 1,
            max_cost,
            origin_opposite,
            physics::opposite(face),
            visited,
        );
        if real_cost < cost {
            cost = real_cost;
        }
    }
    cost
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use bedrock_world::generator::flat::FlatGenerator;

    use crate::world_state::WorldState;

    fn world() -> WorldState {
        // Flat world: grass from y = 0..=3, air above.
        WorldState::new("world", Arc::new(FlatGenerator::new()))
    }

    fn water(decay: i32, falling: bool) -> u32 {
        Liquid {
            water: true,
            decay,
            falling,
            still: true,
        }
        .state()
    }

    fn lava(decay: i32, falling: bool) -> u32 {
        Liquid {
            water: false,
            decay,
            falling,
            still: true,
        }
        .state()
    }

    fn sounds(world: &WorldState) -> Vec<String> {
        world
            .take_pending_sounds()
            .into_iter()
            .map(|(sound, ..)| sound)
            .collect()
    }

    /// Ticks the world `n` times so scheduled liquid updates fire.
    fn tick(world: &WorldState, n: i32) {
        for _ in 0..n {
            world.tick();
        }
    }

    #[test]
    fn liquid_levels_round_trip_through_the_bedrock_state() {
        let source = Liquid::from_state(water(0, false)).expect("source water");
        assert!(source.is_source());

        // `liquid_depth = decay | falling << 3`.
        let flowing = Liquid::from_state(water(3, false)).expect("flowing water");
        assert_eq!((flowing.decay, flowing.falling), (3, false));
        assert!(!flowing.is_source());
        let falling = Liquid::from_state(water(0, true)).expect("falling water");
        assert!(falling.falling);
        assert_eq!(
            flowing.state(),
            block_states::network_id("flowing_water").map(|id| block_states::with_property(
                id,
                "liquid_depth",
                "3"
            ))
            .flatten()
            .unwrap()
        );
    }

    #[test]
    fn a_placed_source_spreads_seven_blocks_and_stops() {
        let world = world();
        // A source on the flat grass at y = 4 (the block below is solid).
        world.set_block(0, 4, 0, water(0, false));

        // 8 ticks = 1.6 spread steps: give the flow enough time to settle.
        for _ in 0..70 {
            tick(&world, 1);
        }

        // The level drops by one per block, so the 7th block is the last one
        // still holding water.
        for step in 1..=7 {
            let id = world.block_at(step, 4, 0);
            let liquid = Liquid::from_state(id).unwrap_or_else(|| {
                panic!("expected water at x = {step}, got {id:#010X}");
            });
            assert_eq!(liquid.decay, step, "decay at x = {step}");
        }
        assert_eq!(
            Liquid::from_state(world.block_at(8, 4, 0)),
            None,
            "water must not reach the 8th block"
        );
    }

    #[test]
    fn water_falls_straight_down_before_spreading() {
        let world = world();
        // Hole in the ground: nothing but air below y = 0 stays empty, so drill
        // through the flat layers by removing them.
        for y in 0..4 {
            world.set_block(0, y, 0, AIR);
        }
        world.set_block(0, 4, 0, water(0, false));

        tick(&world, 5);
        // The first tick moves the source's level down one cell (falling).
        let below = Liquid::from_state(world.block_at(0, 3, 0)).expect("water fell");
        assert!(below.falling, "a falling column keeps decay 0");
        assert_eq!(below.decay, 0);
    }

    #[test]
    fn two_adjacent_sources_form_an_infinite_source() {
        let world = world();
        // Two sources with a gap between them, on solid ground.
        world.set_block(0, 4, 0, water(0, false));
        world.set_block(2, 4, 0, water(0, false));

        // Both spread into the middle cell.
        tick(&world, 40);
        let middle = Liquid::from_state(world.block_at(1, 4, 0)).expect("water reached the gap");
        assert!(
            middle.is_source(),
            "two neighbouring sources must turn the gap into a source"
        );
    }

    #[test]
    fn a_lone_flowing_cell_dries_up_when_its_source_is_removed() {
        let world = world();
        world.set_block(0, 4, 0, water(0, false));
        tick(&world, 40);
        assert!(Liquid::from_state(world.block_at(1, 4, 0)).is_some());

        world.set_block(0, 4, 0, AIR);
        tick(&world, 120);
        assert_eq!(
            Liquid::from_state(world.block_at(1, 4, 0)),
            None,
            "flow without a source must drain away"
        );
    }

    #[test]
    fn lava_touching_water_hardens_by_its_own_level() {
        let world = world();
        let obsidian = block_states::network_id("obsidian").expect("obsidian");
        let cobblestone = block_states::network_id("cobblestone").expect("cobblestone");

        // `Lava::checkForHarden()`: a source becomes obsidian, and the world
        // plays `FizzSound` at the block centre.
        world.set_block(0, 4, 0, lava(0, false));
        world.set_block(1, 4, 0, water(0, false));
        assert_eq!(world.block_at(0, 4, 0), obsidian);
        assert!(
            sounds(&world).iter().any(|sound| sound == "fizz"),
            "the collision must play `fizz`"
        );

        // `decay <= 4` becomes cobblestone.
        world.set_block(10, 4, 0, lava(3, false));
        world.set_block(11, 4, 0, water(0, false));
        assert_eq!(world.block_at(10, 4, 0), cobblestone);

        // Thinner flow (and falling lava) is not hardened.
        world.set_block(20, 4, 0, lava(5, false));
        world.set_block(21, 4, 0, water(0, false));
        assert_eq!(
            Liquid::from_state(world.block_at(20, 4, 0)).map(|liquid| liquid.decay),
            Some(5)
        );
    }

    #[test]
    fn lava_flowing_into_water_turns_the_water_into_stone() {
        let world = world();
        let stone = block_states::network_id("stone").expect("stone");
        // `checkForHarden()` skips DOWN, so water directly under lava only
        // reacts when the flow tick tries to flow into it
        // (`Lava::flowIntoBlock()`).
        world.set_block(0, 4, 0, water(0, false));
        world.set_block(0, 5, 0, lava(0, false));
        assert!(Liquid::from_state(world.block_at(0, 4, 0)).is_some());

        tick(&world, 40);
        assert_eq!(world.block_at(0, 4, 0), stone);
        assert!(
            sounds(&world).iter().any(|sound| sound == "fizz"),
            "the collision must play `fizz`"
        );
    }

    #[test]
    fn lava_never_forms_a_source_from_two_neighbours() {
        let world = world();
        world.set_block(0, 4, 0, lava(0, false));
        world.set_block(2, 4, 0, lava(0, false));

        tick(&world, 120);
        let gap = Liquid::from_state(world.block_at(1, 4, 0)).expect("lava reached the gap");
        assert!(
            !gap.is_source(),
            "only `Water` forms a source (`getMinAdjacentSourcesToFormSource()` is null for lava)"
        );
        assert_eq!(gap.decay, 2, "lava loses two levels per block");
    }

    #[test]
    fn water_flows_towards_the_nearest_hole() {
        let world = world();
        // A hole two blocks west of the source: the flow must prefer the west
        // arm instead of spreading evenly in all four directions.
        world.set_block(-2, 3, 0, AIR);
        world.set_block(0, 4, 0, water(0, false));
        tick(&world, 40);

        assert!(
            Liquid::from_state(world.block_at(-1, 4, 0)).is_some(),
            "water reaches the cell above the hole"
        );
        assert!(
            Liquid::from_state(world.block_at(-2, 3, 0)).is_some(),
            "and pours into it"
        );
    }
}
