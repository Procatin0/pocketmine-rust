//! Counts air cells inside the water column of a generated ocean, before and
//! after ticking the world, to see whether the fluid engine is what empties the
//! sea around the water plants.
//!
//!   cargo run -p server-core --example sea_air_scan -- [seed] [x0] [z0] [size] [ticks]

use std::sync::Arc;

use bedrock_world::block_id;
use bedrock_world::generator::bds_terrain::BDSTerrain;
use server_core::world_state::WorldState;

/// Air cells between the sea floor and the water surface, plus how many of them
/// sit right above a water plant (the plants' own cell is skipped).
fn scan(world: &WorldState, x0: i32, z0: i32, size: i32) -> (usize, usize, usize) {
    let (mut holes, mut above_plant, mut plants) = (0usize, 0usize, 0usize);
    for z in z0..z0 + size {
        for x in x0..x0 + size {
            for y in 40..=62 {
                let id = world.block_at(x, y, z);
                if id != block_id::AIR {
                    continue;
                }
                // The cell only counts as a hole when there is water above it.
                let water_above = (y + 1..=63).any(|ay| {
                    let above = world.block_at(x, ay, z);
                    above == block_id::WATER || block_id::is_water_plant(above)
                });
                if !water_above {
                    continue;
                }
                holes += 1;
                if block_id::is_water_plant(world.block_at(x, y - 1, z)) {
                    above_plant += 1;
                }
            }
            for y in 40..=62 {
                if block_id::is_water_plant(world.block_at(x, y, z)) {
                    plants += 1;
                }
            }
        }
    }
    (holes, above_plant, plants)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let x0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(280);
    let z0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(279);
    let size: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(16);
    let ticks: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(200);

    let world = WorldState::new("scan", Arc::new(BDSTerrain::new(seed)));
    let (holes, above_plant, plants) = scan(&world, x0, z0, size);
    println!("antes de tickear: huecos de aire {holes} (sobre una planta {above_plant}), plantas {plants}");

    // A player breaking the water block *next to* a plant is what starts the
    // flow that used to erase it.
    let mut target = None;
    'find: for z in z0..z0 + size {
        for x in x0..x0 + size {
            for y in 40..=62 {
                if block_id::is_water_plant(world.block_at(x, y, z))
                    && world.block_at(x + 1, y, z) == block_id::WATER
                {
                    target = Some((x, y, z));
                    break 'find;
                }
            }
        }
    }
    match target {
        Some((x, y, z)) => {
            println!("rompo el agua de al lado de la planta ({x}, {y}, {z})");
            world.set_block(x + 1, y, z, block_id::AIR);
        }
        None => println!("no encontre ninguna planta junto al agua"),
    }
    for _ in 0..ticks {
        world.tick();
    }
    let (holes, above_plant, plants) = scan(&world, x0, z0, size);
    println!("tras {ticks} ticks: huecos de aire {holes} (sobre una planta {above_plant}), plantas {plants}");
}
