//! Underground cave tunnels (spaghetti caverns).
//!
//! There is no cave code upstream in
//! `github.com/niduy/minecraft-world-generator` either: that project only
//! places 2D structures. This populator fills the gap the same way the
//! PocketMine populators do for trees and ores — scatter through
//! `ChunkManager` — with the guarantees the seed-0 regression suite needs:
//!
//! - carved only below Y = 44 (section 6 and lower), so Java section 7
//!   (Y 48..=63) — the certified `fluidCount 812` / `blockCount 3866` zone
//!   of chunk (0, 0) — is never entered;
//! - only `STONE` is replaced (never water, ores, dirt, grass or bedrock),
//!   so the water column of any chunk and every ore placed earlier survive;
//! - streaming discipline matches trees/ores: writes go through the 3x3
//!   `ChunkManager`, so a tunnel crossing a border can bleed one chunk into
//!   a neighbour that the manager already holds, and is silently clipped
//!   beyond it — no chunk hash cache, no second pass;
//! - the RNG is a **dedicated stream** (`seed ^ chunk`, see
//!   `NormalGenerator::populate_chunk`): consuming it never advances the
//!   shared ores/trees/tall-grass stream, so every previously generated
//!   voxel stays bit-identical.

use std::f64::consts::PI;

use crate::block_id;
use crate::generator::manager::ChunkManager;
use crate::generator::random::Random;

#[derive(Debug, Clone)]
pub struct CavePopulator {
    pub tunnel_count: i32,
    pub min_y: i32,
    pub max_y: i32,
}

impl Default for CavePopulator {
    fn default() -> Self {
        Self::new()
    }
}

impl CavePopulator {
    pub fn new() -> Self {
        Self {
            tunnel_count: 3,
            min_y: 6,
            max_y: 44,
        }
    }

    pub fn populate<M: ChunkManager>(
        &self,
        world: &mut M,
        chunk_x: i32,
        chunk_z: i32,
        random: &mut Random,
    ) {
        let base_x = chunk_x << 4;
        let base_z = chunk_z << 4;

        for _ in 0..self.tunnel_count {
            let start_x = random.next_range(base_x, base_x + 15) as f64;
            let start_z = random.next_range(base_z, base_z + 15) as f64;
            let start_y = random.next_range(10, self.max_y - 2) as f64;
            let mut heading = random.next_float() * 2.0 * PI;
            let mut pitch = 0.0f64;
            let mut radius = 1.6 + random.next_float() * 1.2;
            let steps = 48 + random.next_bounded_int(48);

            let (mut cx, mut cy, mut cz) = (start_x, start_y, start_z);
            for _ in 0..steps {
                self.carve(world, cx, cy, cz, radius);

                cx += heading.cos();
                cz += heading.sin();
                cy += pitch;
                if !(self.min_y as f64 + 2.0..self.max_y as f64 - 2.0).contains(&cy) {
                    pitch = -pitch;
                    cy = cy.clamp(self.min_y as f64 + 2.0, self.max_y as f64 - 2.0);
                }

                heading += (random.next_float() - 0.5) * 0.35;
                pitch = (pitch + (random.next_float() - 0.5) * 0.25).clamp(-0.45, 0.45);
                radius = (radius + (random.next_float() - 0.5) * 0.3).clamp(1.2, 2.9);
            }
        }
    }

    /// Carves one ellipsoid of `STONE` into air.
    ///
    /// Per-block Y is clamped to `self.min_y..=self.max_y`, so the top of a
    /// sphere can never cross into terrain or section 7 even when its
    /// centre sits at `max_y`.
    fn carve<M: ChunkManager>(&self, world: &mut M, cx: f64, cy: f64, cz: f64, radius: f64) {
        let ry = (radius * 0.7).max(0.8);
        let x0 = (cx - radius).floor() as i32;
        let x1 = (cx + radius).floor() as i32;
        let y0 = (cy - ry).floor() as i32;
        let y1 = (cy + ry).floor() as i32;
        let z0 = (cz - radius).floor() as i32;
        let z1 = (cz + radius).floor() as i32;

        for x in x0..=x1 {
            let dx = (x as f64 + 0.5 - cx) / radius;
            if dx * dx >= 1.0 {
                continue;
            }
            for y in y0..=y1 {
                if y < self.min_y || y > self.max_y {
                    continue;
                }
                let dy = (y as f64 + 0.5 - cy) / ry;
                if dy * dy >= 1.0 {
                    continue;
                }
                for z in z0..=z1 {
                    let dz = (z as f64 + 0.5 - cz) / radius;
                    if dx * dx + dy * dy + dz * dz >= 1.0 {
                        continue;
                    }
                    if world.get_block_at(x, y, z) == block_id::STONE {
                        world.set_block_at(x, y, z, block_id::AIR);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::Chunk;

    #[test]
    fn carves_air_below_max_y_and_never_touches_surface() {
        let mut chunk = Chunk::new(1);
        // Solid stone column 0..80 with grass on top.
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..80 {
                    chunk.set_block(x, y, z, block_id::STONE);
                }
                chunk.set_block(x, 80, z, block_id::GRASS_BLOCK);
            }
        }
        let populator = CavePopulator::new();
        let mut random = Random::new(1234);
        populator.populate(&mut chunk, 0, 0, &mut random);

        let mut carved = 0;
        for x in 0..16 {
            for z in 0..16 {
                assert_eq!(
                    chunk.get_block(x, 80, z),
                    block_id::GRASS_BLOCK,
                    "surface intact"
                );
                for y in 0..=80 {
                    let b = chunk.get_block(x, y, z);
                    if y <= populator.max_y && b == block_id::AIR {
                        carved += 1;
                    }
                    if y > populator.max_y {
                        assert_ne!(b, block_id::AIR, "nothing carved above cap");
                    }
                }
            }
        }
        assert!(carved > 0, "cave should actually carve on solid stone");
    }
}
