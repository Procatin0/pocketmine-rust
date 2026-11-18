//! Falling blocks regression: mining sand faster than the fall (several breaks
//! per tick) must never leave the sand as an entity instead of a block.
//!
//! The burst of breaks lets one column's falling sand land in a cell another
//! entity is still crossing: that entity ends up inside a solid cell, where
//! it cannot settle. It used to be dropped there as an item, so the sand
//! "stayed as an entity". It must be lifted on top of the burying block and
//! keep falling instead.

use std::sync::Arc;

use bedrock_world::block_id::AIR;
use bedrock_world::block_states;
use bedrock_world::FlatGenerator;
use server_core::world_state::WorldState;

fn world() -> WorldState {
    WorldState::new("world", Arc::new(FlatGenerator::new()))
}

fn leftovers(world: &WorldState) -> (usize, usize) {
    let entities = world.entities_snapshot();
    let falling = entities
        .iter()
        .filter(|entity| entity.network_type_id() == "minecraft:falling_block")
        .count();
    let items = entities
        .iter()
        .filter(|entity| entity.network_type_id() == "minecraft:item")
        .count();
    (falling, items)
}

/// A falling block spawned into an occupied cell is lifted onto the burying
/// block and becomes a block there instead of an item.
#[test]
fn a_buried_falling_block_is_lifted() {
    let world = world();
    let stone = block_states::network_id("stone").unwrap();
    let sand = block_states::network_id("sand").unwrap();
    world.set_block(40, 10, 40, stone);
    world.set_block(40, 11, 40, sand);
    world
        .spawn_falling_block(40, 11, 40, sand)
        .expect("falling block entity");
    for _ in 0..20 {
        world.tick_entities();
    }
    assert_eq!(
        world.entity_count(),
        0,
        "the sand must not stay as an entity"
    );
    assert_eq!(
        block_states::state_name(world.block_at(40, 11, 40)),
        Some("sand")
    );
    assert_eq!(
        block_states::state_name(world.block_at(40, 12, 40)),
        Some("sand")
    );
}

/// A falling block landing on a torch must break into an item drop and play
/// sound/particles instead of getting stuck in a bounce/teleport loop.
#[test]
fn a_falling_block_landing_on_a_torch_breaks_into_an_item() {
    let world = world();
    let stone = block_states::network_id("stone").unwrap();
    let torch = block_states::network_id("torch").unwrap();
    let sand = block_states::network_id("sand").unwrap();

    world.set_block(10, 20, 10, stone);
    world.set_block(10, 21, 10, torch);
    world.spawn_falling_block(10, 25, 10, sand).expect("spawn sand");

    for _ in 0..50 {
        world.tick_entities();
    }

    let (falling, items) = leftovers(&world);
    assert_eq!(falling, 0, "falling block entity must not remain");
    assert_eq!(items, 1, "sand must break into an item drop");
    assert_eq!(world.block_at(10, 21, 10), torch, "torch must remain in place");
}

#[test]
fn multiple_falling_blocks_landing_on_a_torch_all_break_into_items() {
    let world = world();
    let stone = block_states::network_id("stone").unwrap();
    let torch = block_states::network_id("torch").unwrap();
    let sand = block_states::network_id("sand").unwrap();

    world.set_block(10, 20, 10, stone);
    world.set_block(10, 21, 10, torch);
    world.spawn_falling_block(10, 23, 10, sand).expect("sand 1");
    world.spawn_falling_block(10, 24, 10, sand).expect("sand 2");
    world.spawn_falling_block(10, 25, 10, sand).expect("sand 3");

    for _ in 0..60 {
        world.tick_entities();
    }

    let (falling, items) = leftovers(&world);
    assert_eq!(falling, 0, "no falling blocks must remain");
    assert!(items >= 1, "sand must break into item drops");
    assert_eq!(world.block_at(10, 21, 10), torch, "torch must remain in place");
}

/// Deterministic fast-mining fuzz: several sand breaks per tick. Every sand
/// block must end up as a block (or, if it truly cannot land, as an item on
/// free ground) and no falling entity may survive.
#[test]
fn fast_mining_leaves_no_sand_entities() {
    let world = world();
    let stone = block_states::network_id("stone").unwrap();
    let sand = block_states::network_id("sand").unwrap();
    for dx in 0..10 {
        for dz in 0..10 {
            world.set_block(20 + dx, 10, 20 + dz, stone);
            for dy in 11..=14 {
                world.set_block(20 + dx, dy, 20 + dz, sand);
            }
        }
    }
    let mut seed = 0x1234_5678_9abc_def0u64;
    for step in 0..900 {
        for _ in 0..6 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let dx = ((seed >> 33) % 10) as i32;
            let dz = ((seed >> 40) % 10) as i32;
            let dy = 11 + ((seed >> 50) % 4) as i32;
            let pos = (20 + dx, dy, 20 + dz);
            if world.block_at(pos.0, pos.1, pos.2) == sand {
                world.set_block(pos.0, pos.1, pos.2, AIR);
            }
        }
        world.tick_entities();
        for entity in world.entities_snapshot() {
            if entity.network_type_id() == "minecraft:falling_block" {
                assert!(
                    entity.ticks_lived <= 120,
                    "step {step}: falling block stuck at {:?} (ground={}, y motion {})",
                    entity.position,
                    entity.on_ground,
                    entity.motion.y
                );
            }
        }
    }
    for _ in 0..300 {
        world.tick_entities();
    }
    let (falling, items) = leftovers(&world);
    assert_eq!(falling, 0, "falling blocks left behind");
    assert_eq!(items, 0, "sand left behind as an item entity");
}
