//! Underwater decoration: seagrass (short and tall), kelp and sea pickles.
//!
//! Placement follows the vanilla rules (Minecraft Wiki, *Seagrass*, *Kelp*,
//! *Sea pickle*):
//!
//! - seagrass grows on the floor of rivers, frozen rivers, the non-frozen
//!   oceans and swamps, short or tall;
//! - kelp grows in every ocean except the frozen and the warm ones, roughly once
//!   per eighteen chunks, and its stalk stops below the surface;
//! - sea pickles belong to warm oceans and coral reefs.
//!
//! The block ids are the canonical Bedrock states (`sea_grass_type`,
//! `kelp_age`, `cluster_count`), taken from `data/canonical_block_states.nbt`
//! through `block_states`, never guessed.

use crate::block_id;
use crate::generator::biome;
use crate::generator::manager::ChunkManager;
use crate::generator::random::Random;

/// Shallowest water that gets plants: a one-block puddle stays bare.
const MIN_WATER_DEPTH: i32 = 2;

/// Seagrass per column: denser in the narrow rivers and swamps than in open
/// water, as in vanilla, where the feature tries more columns per chunk there.
const DENSITY_RIVER: f64 = 0.22;
const DENSITY_OCEAN: f64 = 0.10;

#[derive(Debug, Default, Clone, Copy)]
pub struct UnderwaterPopulator;

impl UnderwaterPopulator {
    pub fn populate<M: ChunkManager>(
        &self,
        world: &mut M,
        chunk_x: i32,
        chunk_z: i32,
        random: &mut Random,
        biome_id: u8,
    ) {
        if has_kelp(biome_id) && random.next_float() < 1.0 / 18.0 {
            self.kelp_patch(world, chunk_x, chunk_z, random);
        }
        if biome_id == biome::WARM_OCEAN && random.next_float() < 1.0 / 8.0 {
            self.sea_pickles(world, chunk_x, chunk_z, random);
        }

        if !has_seagrass(biome_id) {
            return;
        }
        let dense = matches!(
            biome_id,
            biome::RIVER | biome::FROZEN_RIVER | biome::SWAMPLAND | biome::MANGROVE_SWAMP
        );
        let density = if dense { DENSITY_RIVER } else { DENSITY_OCEAN };
        let base_x = chunk_x * 16;
        let base_z = chunk_z * 16;

        for x in base_x..base_x + 16 {
            for z in base_z..base_z + 16 {
                if random.next_float() >= density {
                    continue;
                }
                let Some((floor, depth)) = water_column(world, x, z) else {
                    continue;
                };
                if depth < MIN_WATER_DEPTH || !is_bed(world.get_block_at(x, floor, z)) {
                    continue;
                }
                // Tall seagrass needs room for both halves.
                if depth >= 3 && random.next_float() < 0.125 {
                    world.set_block_at(x, floor + 1, z, block_id::SEAGRASS_DOUBLE_BOT);
                    world.set_block_at(x, floor + 2, z, block_id::SEAGRASS_DOUBLE_TOP);
                } else {
                    world.set_block_at(x, floor + 1, z, block_id::SEAGRASS);
                }
            }
        }
    }

    /// One kelp vegetation: a small patch of stalks on the deepest column it
    /// can find in the chunk, each stalk stopping one block under the surface.
    fn kelp_patch<M: ChunkManager>(
        &self,
        world: &mut M,
        chunk_x: i32,
        chunk_z: i32,
        random: &mut Random,
    ) {
        let base_x = chunk_x * 16;
        let base_z = chunk_z * 16;
        let center_x = base_x + random.next_range(0, 15);
        let center_z = base_z + random.next_range(0, 15);

        for dx in -2..=2 {
            for dz in -2..=2 {
                if random.next_float() > 0.4 {
                    continue;
                }
                let (x, z) = (center_x + dx, center_z + dz);
                let Some((floor, depth)) = water_column(world, x, z) else {
                    continue;
                };
                if depth < 4 || !is_bed(world.get_block_at(x, floor, z)) {
                    continue;
                }
                let height = (depth - 1).min(3 + random.next_range(0, 12));
                for step in 0..height {
                    // Kelp age climbs with the stalk; the tip is the mature one.
                    let age = if step + 1 == height {
                        25
                    } else {
                        (step.min(24)) as u8
                    };
                    world.set_block_at(x, floor + 1 + step, z, block_id::kelp(age));
                }
            }
        }
    }

    /// Sea pickles: a cluster of one to three on the warm ocean floor.
    fn sea_pickles<M: ChunkManager>(
        &self,
        world: &mut M,
        chunk_x: i32,
        chunk_z: i32,
        random: &mut Random,
    ) {
        let base_x = chunk_x * 16;
        let base_z = chunk_z * 16;
        for _ in 0..random.next_range(1, 3) {
            let x = base_x + random.next_range(0, 15);
            let z = base_z + random.next_range(0, 15);
            let Some((floor, _depth)) = water_column(world, x, z) else {
                continue;
            };
            let bed = world.get_block_at(x, floor, z);
            if bed != block_id::SAND && bed != block_id::GRAVEL {
                continue;
            }
            let clusters = random.next_range(1, 3);
            world.set_block_at(x, floor + 1, z, block_id::sea_pickle(clusters as u8));
        }
    }
}

fn has_seagrass(biome_id: u8) -> bool {
    matches!(
        biome_id,
        biome::OCEAN
            | biome::DEEP_OCEAN
            | biome::WARM_OCEAN
            | biome::LUKEWARM_OCEAN
            | biome::DEEP_LUKEWARM_OCEAN
            | biome::COLD_OCEAN
            | biome::DEEP_COLD_OCEAN
            | biome::RIVER
            | biome::FROZEN_RIVER
            | biome::SWAMPLAND
            | biome::MANGROVE_SWAMP
    )
}

fn has_kelp(biome_id: u8) -> bool {
    // Every ocean except the frozen and the warm ones.
    matches!(
        biome_id,
        biome::OCEAN
            | biome::DEEP_OCEAN
            | biome::LUKEWARM_OCEAN
            | biome::DEEP_LUKEWARM_OCEAN
            | biome::COLD_OCEAN
            | biome::DEEP_COLD_OCEAN
    )
}

/// Floor and water depth of a column, when the column is water.
fn water_column<M: ChunkManager>(world: &M, x: i32, z: i32) -> Option<(i32, i32)> {
    let top = world.get_highest_block_at(x, z)?;
    let surface = world.get_block_at(x, top, z);
    if surface != block_id::WATER && surface != block_id::ICE {
        return None;
    }
    let mut y = top;
    while y > 0 {
        let block = world.get_block_at(x, y, z);
        // Plants are not the floor: kelp above a seagrass patch would otherwise
        // read as the bed and block every later placement on that column.
        if block != block_id::WATER
            && block != block_id::ICE
            && !block_id::is_water_plant(block)
        {
            return Some((y, top - y));
        }
        y -= 1;
    }
    None
}

/// Beds vanilla plants on: the sea floor materials, never bare stone.
fn is_bed(id: u32) -> bool {
    matches!(
        id,
        block_id::SAND
            | block_id::GRAVEL
            | block_id::DIRT
            | block_id::CLAY
            | block_id::SANDSTONE
            | block_id::GRASS_BLOCK
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::Chunk;

    /// A 16x16 chunk with the floor at `floor` and water up to sea level.
    fn sea_chunk(biome_id: u8, floor: i32) -> Chunk {
        let mut chunk = Chunk::new(biome_id);
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..=floor {
                    chunk.set_block(x, y, z, block_id::STONE);
                }
                chunk.set_block(x, floor, z, block_id::SAND);
                for y in floor + 1..=62 {
                    chunk.set_block(x, y, z, block_id::WATER);
                }
            }
        }
        chunk
    }

    fn count(chunk: &Chunk, predicate: impl Fn(u32) -> bool) -> usize {
        let mut total = 0;
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..128 {
                    if predicate(chunk.get_block(x, y, z)) {
                        total += 1;
                    }
                }
            }
        }
        total
    }

    #[test]
    fn seagrass_lands_on_the_floor_and_tall_form_stands_on_it() {
        let mut chunk = sea_chunk(biome::OCEAN, 58);
        let populator = UnderwaterPopulator;
        let mut random = Random::new(11);
        // One pass per column window: a second pass over the same columns would
        // overwrite the lower half of a tall plant with a short one.
        populator.populate(&mut chunk, 0, 0, &mut random, biome::OCEAN);
        let bottom = count(&chunk, |id| id == block_id::SEAGRASS_DOUBLE_BOT);
        let top = count(&chunk, |id| id == block_id::SEAGRASS_DOUBLE_TOP);
        assert_eq!(bottom, top, "tall seagrass has to come in pairs");

        for _ in 0..8 {
            let mut column = sea_chunk(biome::OCEAN, 58);
            populator.populate(&mut column, 0, 0, &mut random, biome::OCEAN);
            for x in 0..16 {
                for z in 0..16 {
                    for y in 0..128 {
                        if block_id::is_seagrass(column.get_block(x, y, z)) {
                            chunk.set_block(x, y, z, column.get_block(x, y, z));
                        }
                    }
                }
            }
        }
        assert!(
            count(&chunk, |id| id == block_id::SEAGRASS) > 0,
            "no seagrass placed"
        );
        // Every plant sits in the water above the sand, never inside the floor.
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..=58 {
                    assert!(!block_id::is_seagrass(chunk.get_block(x, y, z)));
                }
            }
        }
    }

    #[test]
    fn kelp_rises_towards_the_surface_with_a_mature_tip() {
        let mut chunk = sea_chunk(biome::COLD_OCEAN, 40);
        let populator = UnderwaterPopulator;
        let mut random = Random::new(5);
        let mut saw_tip = false;
        // The chunk roll is 1/18, so a fair number of draws is needed before the
        // patch shows up; the point here is the shape of the stalk, not the rate.
        for _ in 0..400 {
            populator.populate(&mut chunk, 0, 0, &mut random, biome::COLD_OCEAN);
            saw_tip |= count(&chunk, |id| id == block_id::kelp(25)) > 0;
        }
        assert!(count(&chunk, block_id::is_kelp) > 0, "no kelp placed");
        assert!(saw_tip, "kelp needs a mature tip block");
        // Kelp stops below the surface, as in vanilla.
        for x in 0..16 {
            for z in 0..16 {
                assert!(!block_id::is_kelp(chunk.get_block(x, 62, z)));
            }
        }
    }

    #[test]
    fn sea_pickles_only_in_warm_water() {
        let mut warm = sea_chunk(biome::WARM_OCEAN, 58);
        let mut cold = sea_chunk(biome::COLD_OCEAN, 58);
        let populator = UnderwaterPopulator;
        let mut random = Random::new(13);
        for _ in 0..400 {
            populator.populate(&mut warm, 0, 0, &mut random, biome::WARM_OCEAN);
            populator.populate(&mut cold, 0, 0, &mut random, biome::COLD_OCEAN);
        }
        let is_pickle = |id: u32| {
            matches!(
                id,
                block_id::SEA_PICKLE_1 | block_id::SEA_PICKLE_2 | block_id::SEA_PICKLE_3
            )
        };
        assert!(count(&warm, is_pickle) > 0, "warm oceans carry sea pickles");
        assert_eq!(count(&cold, is_pickle), 0, "cold oceans do not");
    }
}
