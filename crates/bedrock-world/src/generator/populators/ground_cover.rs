//! PocketMine-MP GroundCover Populator port.
//!
//! Matches `pocketmine\world\generator\populator\GroundCover` on dry land and
//! adds the vanilla surface rules: a sea floor is not just the biome's dry cover.
//! Vanilla lays a layer of gravel over stone in deeper water, keeps dirt in
//! shallow water (so disks can generate) and uses sand over sandstone in the warm
//! oceans; patches of the other materials break it up.
//!
//! Two dry-land rules are not a flat biome cover either, and live here:
//!
//! - Old growth taiga gets podzol patches mixed with coarse dirt and grass,
//!   like vanilla's `podzol` surface rule, instead of a uniform floor.
//! - Frozen oceans and frozen rivers freeze their surface, so the top water
//!   block becomes ice.

use crate::block_id;
use crate::chunk::Chunk;
use crate::generator::biome::{self, get_biome_info};
use crate::generator::random::Random;

/// Sea level of the terrain generators; water above the ground is what the
/// underwater rules key on.
const SEA_LEVEL: i32 = 62;

/// Water at most this deep keeps the biome's own soil, as in vanilla, where
/// shallow lake beds stay dirt so disks of sand, gravel or clay can generate.
const SHALLOW_WATER: i32 = 3;

/// Scale, in blocks, of the podzol patches old growth taiga gets. Vanilla mixes
/// podzol, coarse dirt and grass there at a similar patch size.
const PODZOL_PATCH_BLOCKS: f64 = 11.0;

/// Drop, in blocks, between a column and a neighbour that turns a soil surface
/// into bare stone: a face this steep cannot hold grass in vanilla either, and
/// without it mountains came out as smooth green hills with no rock at all.
/// Measured on the 768 map, 3 leaves the gradients of normal hills covered and
/// exposes stone on the faces.
const STONE_SLOPE: i32 = 3;

/// Above this height a two-block step is already rock. Our relief noise puts
/// two-block steps all over the lowlands, so the general threshold has to be
/// three; on real high ground, vanilla exposes stone much sooner.
const MOUNTAIN_STONE_HEIGHT: i32 = 90;
const MOUNTAIN_STONE_SLOPE: i32 = 2;

/// Podzol noise above this becomes podzol, above `PODZOL_COARSE` instead becomes
/// coarse dirt; the rest keeps grass.
const PODZOL: f64 = 0.42;
const PODZOL_COARSE: f64 = 0.86;

/// Scale, in blocks, of the noise that softens a biome border. The biome data
/// steps from one biome to the next with no transition at all, so the surfaces
/// met in a ruler-straight line; vanilla interleaves them over a few blocks.
/// Only the columns with a neighbour of another biome are dithered.
const BORDER_DITHER_BLOCKS: f64 = 2.0;

#[derive(Debug, Default, Clone)]
pub struct GroundCoverPopulator {
    seed: u64,
}

/// Cheap border-stable hash in `[0, 1)`.
fn hash01(x: i32, z: i32, seed: u64) -> f64 {
    let mut h = (x as u64)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (z as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f)
        ^ seed.wrapping_mul(0x1656_67b1_9e37_79f9);
    h ^= h >> 29;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 32;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// Smooth value noise over the block grid, so patches line up across chunks.
fn value_noise(x: f64, z: f64, seed: u64, scale: f64) -> f64 {
    let (fx, fz) = (x / scale, z / scale);
    let (x0, z0) = (fx.floor() as i32, fz.floor() as i32);
    let (tx, tz) = (fx - x0 as f64, fz - z0 as f64);
    let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
    let (sx, sz) = (smooth(tx), smooth(tz));
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let bottom = lerp(
        hash01(x0, z0, seed),
        hash01(x0 + 1, z0, seed),
        sx,
    );
    let top = lerp(
        hash01(x0, z0 + 1, seed),
        hash01(x0 + 1, z0 + 1, seed),
        sx,
    );
    lerp(bottom, top, sz)
}

impl GroundCoverPopulator {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Sea-floor composition for one column: sand over sandstone in the warm
    /// oceans, dirt with clay in swamps, gravel over stone in deeper water, dirt
    /// with clay and sand in the shallows.
    fn fill_sea_floor(
        &self,
        chunk: &mut Chunk,
        chunk_x: i32,
        chunk_z: i32,
        x: usize,
        z: usize,
        floor: i32,
        depth: i32,
        biome_id: u8,
    ) {
        let world_x = (chunk_x * 16 + x as i32) as f64;
        let world_z = (chunk_z * 16 + z as i32) as f64;
        let patches = value_noise(world_x, world_z, self.seed, 13.0);
        let banks = value_noise(world_x, world_z, self.seed ^ 0x5eed, 27.0);

        let mut put = |y: i32, id: u32| {
            if (0..128).contains(&y) {
                chunk.set_block(x, y, z, id);
            }
        };

        match biome_id {
            biome::WARM_OCEAN | biome::LUKEWARM_OCEAN | biome::DEEP_LUKEWARM_OCEAN => {
                for i in 0..4 {
                    put(floor - i, block_id::SAND);
                }
                for i in 4..6 {
                    put(floor - i, block_id::SANDSTONE);
                }
                if patches > 0.86 {
                    put(floor, block_id::GRAVEL);
                }
            }
            biome::SWAMPLAND | biome::MANGROVE_SWAMP => {
                put(floor, block_id::DIRT);
                put(floor - 1, block_id::DIRT);
                // Swamps are the one place vanilla layers clay on the bed.
                if patches > 0.45 {
                    put(floor, block_id::CLAY);
                    if patches > 0.62 {
                        put(floor - 1, block_id::CLAY);
                    }
                }
                if patches < 0.1 {
                    put(floor, block_id::SAND);
                }
            }
            _ if depth <= SHALLOW_WATER => {
                for i in 0..3 {
                    put(floor - i, block_id::DIRT);
                }
                if patches > 0.78 {
                    put(floor, block_id::GRAVEL);
                }
                if banks < 0.1 {
                    put(floor, block_id::SAND);
                }
                // Clay sits in the shallow, quiet water, as in vanilla.
                if patches < 0.07 {
                    put(floor, block_id::CLAY);
                    put(floor - 1, block_id::CLAY);
                }
            }
            _ => {
                // Deeper water: a gravel bed over stone, with sand banks and
                // the odd dirt patch near the shallows.
                put(floor, block_id::GRAVEL);
                if patches > 0.84 {
                    put(floor - 1, block_id::GRAVEL);
                }
                if banks < 0.12 || depth <= SHALLOW_WATER + 3 && patches > 0.6 {
                    for i in 0..2 {
                        put(floor - i, block_id::SAND);
                    }
                }
                if patches < 0.05 {
                    put(floor, block_id::DIRT);
                }
                if patches > 0.9 {
                    put(floor, block_id::CLAY);
                    put(floor - 1, block_id::CLAY);
                }
            }
        }
    }

    /// Interleaves the cover of the columns sitting on a biome border: half of
    /// them take their neighbour's cover, so the seam stops being a straight
    /// line. The biome itself is untouched, only the surface block it gets.
    fn dither_biome_borders(&self, biomes: &mut [u8; 256], chunk_x: i32, chunk_z: i32) {
        let source = *biomes;
        for x in 0..16usize {
            for z in 0..16usize {
                let own = source[x * 16 + z];
                let mut other = None;
                'neighbours: for dx in -1i32..=1 {
                    for dz in -1i32..=1 {
                        let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                        if !(0..16).contains(&nx) || !(0..16).contains(&nz) {
                            continue;
                        }
                        let neighbour = source[nx as usize * 16 + nz as usize];
                        if neighbour != own {
                            other = Some(neighbour);
                            break 'neighbours;
                        }
                    }
                }
                let Some(other) = other else { continue };
                let world_x = (chunk_x * 16 + x as i32) as f64;
                let world_z = (chunk_z * 16 + z as i32) as f64;
                let noise = value_noise(world_x, world_z, self.seed ^ 0x7b1d, BORDER_DITHER_BLOCKS);
                if noise > 0.5 {
                    biomes[x * 16 + z] = other;
                }
            }
        }
    }

    /// Whether a biome's water surface freezes over: vanilla freezes the top
    /// water block of the frozen oceans and the frozen river.
    fn freezes_over(biome_id: u8) -> bool {
        matches!(
            biome_id,
            biome::FROZEN_OCEAN | biome::DEEP_FROZEN_OCEAN | biome::FROZEN_RIVER
        )
    }

    /// Vanilla's old growth taiga floor: podzol with coarse dirt and grass mixed
    /// in. The biome's cover says podzol on top, which is what selects this rule.
    fn podzol_top(&self, world_x: f64, world_z: f64) -> u32 {
        let noise = value_noise(world_x, world_z, self.seed ^ 0x9e37, PODZOL_PATCH_BLOCKS);
        if noise > PODZOL_COARSE {
            block_id::COARSE_DIRT
        } else if noise > PODZOL {
            block_id::PODZOL
        } else {
            block_id::GRASS_BLOCK
        }
    }

    pub fn populate(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32, _random: &mut Random) {
        // Surface heights before anything is rewritten: the slope of a column is
        // the difference against its neighbours, and the cover below changes the
        // top block of every column it touches.
        let mut heights = [[0i32; 16]; 16];
        for (x, column) in heights.iter_mut().enumerate() {
            for (z, height) in column.iter_mut().enumerate() {
                *height = chunk.get_highest_block_at(x, z).unwrap_or(SEA_LEVEL);
            }
        }

        // Border columns take either side's cover, so a biome seam stops being a
        // straight line.
        let mut biomes = chunk.biomes;
        self.dither_biome_borders(&mut biomes, chunk_x, chunk_z);

        for x in 0..16 {
            for z in 0..16 {
                let biome_id = biomes[x * 16 + z];

                let start_y = match chunk.get_highest_block_at(x, z) {
                    Some(y) => y,
                    None => continue,
                };

                let mut solid_y = start_y;
                while solid_y > 0 && block_id::is_transparent(chunk.get_block(x, solid_y, z)) {
                    solid_y -= 1;
                }

                let water_depth = SEA_LEVEL - solid_y;
                if water_depth > 0 {
                    self.fill_sea_floor(
                        chunk, chunk_x, chunk_z, x, z, solid_y, water_depth, biome_id,
                    );
                    if Self::freezes_over(biome_id) {
                        chunk.set_block(x, SEA_LEVEL, z, block_id::ICE);
                    }
                    continue;
                }

                let info = get_biome_info(biome_id);
                let mut cover = info.ground_cover;
                if cover.is_empty() {
                    continue;
                }

                // A steep face shows rock, whatever biome it belongs to.
                let slope = {
                    let mut steepest = 0i32;
                    for (dx, dz) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                        let nx = (x as i32 + dx).clamp(0, 15) as usize;
                        let nz = (z as i32 + dz).clamp(0, 15) as usize;
                        steepest = steepest.max((heights[x][z] - heights[nx][nz]).abs());
                    }
                    steepest
                };
                let rocky = slope >= STONE_SLOPE
                    || (solid_y >= MOUNTAIN_STONE_HEIGHT && slope >= MOUNTAIN_STONE_SLOPE);
                if rocky {
                    cover = crate::generator::biome::stone_cover();
                }

                // Only the old growth taiga covers itself in podzol; the cover's
                // top block is the data that says which rule applies.
                let top = if cover[0] == block_id::PODZOL {
                    Some(self.podzol_top(
                        (chunk_x * 16 + x as i32) as f64,
                        (chunk_z * 16 + z as i32) as f64,
                    ))
                } else {
                    None
                };

                let diff_y = if !block_id::is_solid(cover[0]) { 1 } else { 0 };

                let cover_start = (solid_y + diff_y).min(127);
                let cover_end = cover_start - cover.len() as i32;

                for y in ((cover_end + 1).max(0)..=cover_start).rev() {
                    let b = match top {
                        Some(t) if y == cover_start => t,
                        _ => cover[(cover_start - y) as usize],
                    };
                    let current = chunk.get_block(x, y, z);

                    if current == block_id::AIR && block_id::is_solid(b) {
                        break;
                    }

                    if block_id::can_be_flowed_into(b) && current == block_id::WATER {
                        continue;
                    }

                    chunk.set_block(x, y, z, b);
                }
            }
        }
    }
}
