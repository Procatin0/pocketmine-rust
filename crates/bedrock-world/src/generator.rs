//! World Generation module for Bedrock Edition
//!
//! Provides flat and procedural terrain generators that emit standard Bedrock
//! network chunk payloads.

use bedrock_protocol::Writer;
use crate::block_id;
use crate::subchunk::SubChunk;

/// Common trait for Bedrock world generators.
pub trait WorldGenerator: Send + Sync {
    /// Returns the Bedrock generator type (1 = Flat, 2 = Default/Infinite).
    fn generator_type(&self) -> i32;

    /// Recommended world spawn position (x, y, z).
    fn spawn_position(&self) -> (i32, i32, i32);

    /// Recommended player eye spawn position (x, y, z).
    fn player_position(&self) -> (f32, f32, f32);

    /// Generates LevelChunk payload for (chunk_x, chunk_z).
    /// Returns `(sub_chunk_count, payload_bytes)`.
    fn generate_chunk(&self, chunk_x: i32, chunk_z: i32) -> (u32, Vec<u8>);
}

/// Flat world generator for Bedrock 1.26.60.
///
/// World profile:
/// - Y = -64..-17 (indices -4..-2): Solid Bedrock
/// - Y = -16..-4  (in index -1): Stone
/// - Y = -3..-1   (in index -1): Dirt (3 layers)
/// - Y = 0        (in index 0): Grass Block (green surface)
/// - Y = 1..15    (in index 0): Air
///
/// Subchunk count: 5 (covering Y = -64..15).
/// Above Y=15 is air procedural sky.
pub struct FlatGenerator {
    cached_payload: Vec<u8>,
}

impl Default for FlatGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl FlatGenerator {
    pub fn new() -> Self {
        let mut writer = Writer::new();

        // 1. Subchunks -4, -3, -2 (Y = -64..-17): Solid Bedrock
        let bedrock_subchunk = SubChunk::new(block_id::BEDROCK);
        for _ in 0..3 {
            bedrock_subchunk.serialize_network_to(&mut writer);
        }

        // 2. Subchunk -1 (Y = -16..-1): Stone on bottom, 3 layers of Dirt on top
        let mut m1_subchunk = SubChunk::new(block_id::STONE);
        for x in 0..16 {
            for z in 0..16 {
                for y in 13..16 {
                    m1_subchunk.set_block(x, y, z, 0, block_id::DIRT);
                }
            }
        }
        m1_subchunk.serialize_network_to(&mut writer);

        // 3. Subchunk 0 (Y = 0..15): Grass block at Y=0, Air for Y=1..15
        let mut s0_subchunk = SubChunk::new(block_id::AIR);
        for x in 0..16 {
            for z in 0..16 {
                s0_subchunk.set_block(x, 0, z, 0, block_id::GRASS_BLOCK);
            }
        }
        s0_subchunk.serialize_network_to(&mut writer);

        // 4. 24 Overworld biome palettes (plains = 1)
        for _ in 0..24 {
            writer.write_byte(1); // header: (0 << 1) | 1 (bpb=0, runtime=1)
            writer.write_var_i32(1); // plains
        }

        // 5. Border blocks count: 0
        writer.write_byte(0);

        // 6. Block entities (tiles): 0
        let cached_payload = writer.into_bytes().to_vec();
        Self { cached_payload }
    }
}

impl WorldGenerator for FlatGenerator {
    fn generator_type(&self) -> i32 {
        1 // FLAT
    }

    fn spawn_position(&self) -> (i32, i32, i32) {
        (0, 1, 0)
    }

    fn player_position(&self) -> (f32, f32, f32) {
        // Player standing on block at Y=0: feet at Y=1.0, eye height 1.62
        (0.5, 2.62, 0.5)
    }

    fn generate_chunk(&self, _chunk_x: i32, _chunk_z: i32) -> (u32, Vec<u8>) {
        (5, self.cached_payload.clone())
    }
}

/// Simple 2D Perlin / Gradient Noise for natural terrain generation.
#[derive(Clone)]
pub struct SimpleNoise {
    perm: [u8; 512],
}

impl SimpleNoise {
    pub fn new(seed: u64) -> Self {
        let mut p = [0u8; 256];
        for i in 0..256 {
            p[i] = i as u8;
        }

        // Shuffle with seed LCG
        let mut s = seed;
        for i in (1..256).rev() {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let j = (s >> 32) as usize % (i + 1);
            p.swap(i, j);
        }

        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }

        Self { perm }
    }

    fn grad(hash: u8, x: f64, y: f64) -> f64 {
        match hash & 7 {
            0 => x + y,
            1 => -x + y,
            2 => x - y,
            3 => -x - y,
            4 => x,
            5 => -x,
            6 => y,
            7 => -y,
            _ => 0.0,
        }
    }

    fn fade(t: f64) -> f64 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    fn lerp(a: f64, b: f64, t: f64) -> f64 {
        a + t * (b - a)
    }

    pub fn sample2d(&self, x: f64, y: f64) -> f64 {
        let xi = x.floor() as i64;
        let yi = y.floor() as i64;

        let xf = x - x.floor();
        let yf = y - y.floor();

        let u = Self::fade(xf);
        let v = Self::fade(yf);

        let aa = self.perm[((self.perm[(xi & 255) as usize] as i64 + (yi & 255)) & 255) as usize];
        let ab = self.perm[((self.perm[(xi & 255) as usize] as i64 + ((yi + 1) & 255)) & 255) as usize];
        let ba = self.perm[((self.perm[((xi + 1) & 255) as usize] as i64 + (yi & 255)) & 255) as usize];
        let bb = self.perm[((self.perm[((xi + 1) & 255) as usize] as i64 + ((yi + 1) & 255)) & 255) as usize];

        let x1 = Self::lerp(Self::grad(aa, xf, yf), Self::grad(ba, xf - 1.0, yf), u);
        let x2 = Self::lerp(Self::grad(ab, xf, yf - 1.0), Self::grad(bb, xf - 1.0, yf - 1.0), u);

        Self::lerp(x1, x2, v)
    }
}

/// Procedural Natural Terrain Generator for Bedrock Edition.
///
/// Features:
/// - Smooth rolling hills with multi-octave 2D gradient noise
/// - Bedrock foundation (Y = -64..-60)
/// - Solid Stone interior
/// - 3-4 block Dirt surface layer
/// - Green Grass Block top cover
/// - Sea level water at Y <= 62
/// - Oak trees generated on higher flat ground
pub struct NormalGenerator {
    seed: u64,
    noise1: SimpleNoise,
    noise2: SimpleNoise,
    noise3: SimpleNoise,
}

impl NormalGenerator {
    pub const SEA_LEVEL: i32 = 62;

    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            noise1: SimpleNoise::new(seed),
            noise2: SimpleNoise::new(seed.wrapping_add(101)),
            noise3: SimpleNoise::new(seed.wrapping_add(202)),
        }
    }

    /// Computes terrain surface height at block coordinate (x, z).
    pub fn height_at(&self, x: i32, z: i32) -> i32 {
        let fx = x as f64;
        let fz = z as f64;

        let n1 = self.noise1.sample2d(fx * 0.008, fz * 0.008) * 14.0;
        let n2 = self.noise2.sample2d(fx * 0.025, fz * 0.025) * 6.0;
        let n3 = self.noise3.sample2d(fx * 0.06, fz * 0.06) * 2.0;

        (64.0 + n1 + n2 + n3).round() as i32
    }
}

impl WorldGenerator for NormalGenerator {
    fn generator_type(&self) -> i32 {
        2 // DEFAULT / INFINITE
    }

    fn spawn_position(&self) -> (i32, i32, i32) {
        let h = self.height_at(0, 0);
        let y = (h + 1).max(Self::SEA_LEVEL + 1);
        (0, y, 0)
    }

    fn player_position(&self) -> (f32, f32, f32) {
        let (x, y, z) = self.spawn_position();
        (x as f32 + 0.5, y as f32 + 1.62, z as f32 + 0.5)
    }

    fn generate_chunk(&self, chunk_x: i32, chunk_z: i32) -> (u32, Vec<u8>) {
        // Calculate heightmap for this 16x16 chunk
        let mut heights = [[0i32; 16]; 16];
        let mut max_height = 0i32;

        let origin_x = chunk_x * 16;
        let origin_z = chunk_z * 16;

        for (dx, row) in heights.iter_mut().enumerate() {
            for (dz, h) in row.iter_mut().enumerate() {
                let height = self.height_at(origin_x + dx as i32, origin_z + dz as i32);
                *h = height;
                if height > max_height {
                    max_height = height;
                }
            }
        }

        // Subchunks needed: from index -4 (Y=-64) up to the subchunk covering max_height + tree canopy (~6 blocks)
        // Subchunk index for world Y is `(Y + 64) >> 4 - 4`
        let top_y = (max_height + 8).max(Self::SEA_LEVEL + 2);
        let max_subchunk_idx = ((top_y >> 4).clamp(0, 15)) as i32;
        let subchunk_count = (max_subchunk_idx + 4 + 1) as u32; // -4..max_subchunk_idx

        let mut subchunks = Vec::with_capacity(subchunk_count as usize);

        for sc_idx in -4..=max_subchunk_idx {
            let sc_base_y = sc_idx * 16;

            // Fast paths:
            if sc_base_y + 15 < -60 {
                // Completely below -60: solid bedrock
                subchunks.push(SubChunk::new(block_id::BEDROCK));
                continue;
            }

            if sc_base_y + 15 < 40 {
                // Completely below surface: solid stone
                subchunks.push(SubChunk::new(block_id::STONE));
                continue;
            }

            let mut subchunk = SubChunk::new(block_id::AIR);

            for (x, row) in heights.iter().enumerate() {
                for (z, &surface_h) in row.iter().enumerate() {
                    for y_in_sc in 0..16 {
                        let world_y = sc_base_y + y_in_sc;

                        let block = if world_y < -60 {
                            block_id::BEDROCK
                        } else if world_y < surface_h - 4 {
                            block_id::STONE
                        } else if world_y < surface_h {
                            block_id::DIRT
                        } else if world_y == surface_h {
                            if surface_h >= Self::SEA_LEVEL {
                                block_id::GRASS_BLOCK
                            } else {
                                block_id::SAND
                            }
                        } else if world_y <= Self::SEA_LEVEL {
                            block_id::WATER
                        } else {
                            block_id::AIR
                        };

                        if block != block_id::AIR {
                            subchunk.set_block(x, y_in_sc as usize, z, 0, block);
                        }
                    }
                }
            }

            // Tree placement on grass above sea level
            if sc_base_y <= max_height && sc_base_y + 15 >= 50 {
                let tree_hash = ((chunk_x as u64).wrapping_mul(73856093) ^ (chunk_z as u64).wrapping_mul(83492791)) ^ self.seed;
                let has_tree = (tree_hash % 5) == 0;
                if has_tree {
                    let tx = ((tree_hash >> 8) % 8 + 4) as usize;
                    let tz = ((tree_hash >> 16) % 8 + 4) as usize;
                    let th = heights[tx][tz];

                    if th >= Self::SEA_LEVEL + 1 {
                        // Trunk of 4 blocks
                        for dy in 1..=4 {
                            let trunk_y = th + dy;
                            if trunk_y >= sc_base_y && trunk_y < sc_base_y + 16 {
                                subchunk.set_block(tx, (trunk_y - sc_base_y) as usize, tz, 0, block_id::OAK_LOG);
                            }
                        }

                        // Leaves canopy (3x3 at height + 3..+4, 1 on top)
                        for dy in 3..=5 {
                            let leaf_y = th + dy;
                            if leaf_y >= sc_base_y && leaf_y < sc_base_y + 16 {
                                let radius = if dy == 5 { 0 } else { 1 };
                                for lx in (tx as i32 - radius)..=(tx as i32 + radius) {
                                    for lz in (tz as i32 - radius)..=(tz as i32 + radius) {
                                        if lx >= 0 && lx < 16 && lz >= 0 && lz < 16 {
                                            if lx as usize != tx || lz as usize != tz || dy >= 5 {
                                                subchunk.set_block(
                                                    lx as usize,
                                                    (leaf_y - sc_base_y) as usize,
                                                    lz as usize,
                                                    0,
                                                    block_id::OAK_LEAVES,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            subchunks.push(subchunk);
        }

        let mut writer = Writer::new();
        for sc in &subchunks {
            sc.serialize_network_to(&mut writer);
        }

        // 24 Overworld biome palettes (plains = 1)
        for _ in 0..24 {
            writer.write_byte(1);
            writer.write_var_i32(1);
        }

        // Border blocks: 0
        writer.write_byte(0);

        (subchunks.len() as u32, writer.into_bytes().to_vec())
    }
}
