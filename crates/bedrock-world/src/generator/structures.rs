//! Structure placement ported from
//! `github.com/niduy/minecraft-world-generator` (TypeScript, Cubiome/MCP-Reborn
//! compatible Java-edition placement).
//!
//! The upstream project has **no voxel output**: it renders a 2D map of
//! biomes and 12 classic Java structure kinds. Its decoration is positional
//! only — `(salt, region_size, chunk_range)` per kind plus a Java `Random`
//! LCG (`0x5DEECE66D`, `0xB`, 48-bit) that decides, per region, which chunk
//! holds the feature. There are no trees, caves or ore *blocks* upstream.
//!
//! This module ports exactly that positional layer, so `NormalGenerator` can
//! answer "which structures fall in this chunk?" without touching terrain.
//! It mutates nothing, consumes no `PocketMine Random` (the Rust
//! `generator::random::Random` is XorShift128, the upstream `nextInt` is the
//! Java LCG), and therefore cannot perturb ores/trees/tall-grass or the
//! seed-0 section-7 fluid count (812).
//!
//! Block-level decoration (trees, ores, caves, ground cover) stays in the
//! PocketMine-style populators — see `generator::populators` and
//! `generator::caves`.

/// Java `Random` LCG multiplier used by upstream `next`/`setSeed`.
const LCG_A: u64 = 0x5DEECE66D;
/// Java `Random` LCG addend.
const LCG_B: u64 = 0xB;
/// 48-bit state mask `(1 << 48) - 1`.
const LCG_MASK: u64 = (1u64 << 48) - 1;
/// Region hash constants from upstream `getFeatureChunkInRegion`.
const REGION_X_FACTOR: u64 = 341_873_128_712;
const REGION_Z_FACTOR: u64 = 132_897_987_541;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructureKind {
    DesertPyramid,
    JunglePyramid,
    SwampHut,
    Igloo,
    Village,
    OceanRuins,
    Shipwreck,
    RuinedPortal,
    OceanMonument,
    Mansion,
    PillagerOutpost,
    AncientCity,
}

#[derive(Debug, Clone, Copy)]
pub struct StructureConfig {
    pub kind: StructureKind,
    pub salt: u64,
    pub region_size: i32,
    pub chunk_range: i32,
    /// Triangular offset distribution (upstream `isTriangular`): offset is
    /// `(a + b) >> 1` of two uniform draws. Used by OceanMonument and Mansion.
    pub triangular: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructurePosition {
    pub kind: StructureKind,
    pub chunk_x: i32,
    pub chunk_z: i32,
    /// Block-space origin: upstream returns `(reg * region_size + offset) << 4`,
    /// which for the owning chunk is exactly the chunk origin.
    pub block_x: i32,
    pub block_z: i32,
}

/// The 12 upstream `STRUCTURE` configs, in upstream order and values.
pub fn structure_configs() -> [StructureConfig; 12] {
    use StructureKind::*;
    [
        StructureConfig {
            kind: DesertPyramid,
            salt: 14_357_617,
            region_size: 32,
            chunk_range: 24,
            triangular: false,
        },
        StructureConfig {
            kind: JunglePyramid,
            salt: 14_357_619,
            region_size: 32,
            chunk_range: 24,
            triangular: false,
        },
        StructureConfig {
            kind: SwampHut,
            salt: 14_357_620,
            region_size: 32,
            chunk_range: 24,
            triangular: false,
        },
        StructureConfig {
            kind: Igloo,
            salt: 14_357_618,
            region_size: 32,
            chunk_range: 24,
            triangular: false,
        },
        StructureConfig {
            kind: Village,
            salt: 10_387_312,
            region_size: 34,
            chunk_range: 26,
            triangular: false,
        },
        StructureConfig {
            kind: OceanRuins,
            salt: 14_357_621,
            region_size: 20,
            chunk_range: 12,
            triangular: false,
        },
        StructureConfig {
            kind: Shipwreck,
            salt: 165_745_295,
            region_size: 24,
            chunk_range: 20,
            triangular: false,
        },
        StructureConfig {
            kind: RuinedPortal,
            salt: 34_222_645,
            region_size: 40,
            chunk_range: 25,
            triangular: false,
        },
        StructureConfig {
            kind: OceanMonument,
            salt: 10_387_313,
            region_size: 32,
            chunk_range: 27,
            triangular: true,
        },
        StructureConfig {
            kind: Mansion,
            salt: 10_387_319,
            region_size: 80,
            chunk_range: 60,
            triangular: true,
        },
        StructureConfig {
            kind: PillagerOutpost,
            salt: 165_745_296,
            region_size: 32,
            chunk_range: 24,
            triangular: false,
        },
        StructureConfig {
            kind: AncientCity,
            salt: 20_083_232,
            region_size: 24,
            chunk_range: 16,
            triangular: false,
        },
    ]
}

pub fn structure_config(kind: StructureKind) -> Option<StructureConfig> {
    structure_configs().into_iter().find(|c| c.kind == kind)
}

/// One LCG step: `(seed * A + B) & mask`, returning `(new_state, int)`.
/// Mirrors upstream `next(seed, bits)`.
fn next_state_and_int(seed: u64, bits: u32) -> (u64, u64) {
    let state = seed.wrapping_mul(LCG_A).wrapping_add(LCG_B) & LCG_MASK;
    let value = state >> (48 - bits);
    (state, value)
}

/// `setAttemptSeed` from upstream, used by the PillagerOutpost 1/5 roll.
/// The chunk coordinates arrive already shifted by the caller (`pos >> 4`)
/// and are shifted again here — kept verbatim for parity.
fn set_attempt_seed(seed: u64, chunk_x: i32, chunk_z: i32) -> u64 {
    let mixed = seed ^ (((chunk_x as u64) >> 4) ^ (((chunk_z as u64) >> 4) << 4));
    let scrambled = (mixed ^ LCG_A) & LCG_MASK;
    next_state_and_int(scrambled, 31).0
}

/// Upstream `nextInt(seed, n)` (value only; the functional `[newSeed, int]`
/// tuple in the TS keeps the state, but each call site uses a fresh seed).
fn next_int(seed: u64, n: u32) -> u32 {
    if n <= 1 {
        return 0;
    }
    let m = n - 1;
    if (m & n) == 0 {
        let (_, int) = next_state_and_int(seed, 31);
        return ((n as u64 * int) >> 31) as u32;
    }
    let mut state = seed;
    loop {
        let (next, int) = next_state_and_int(state, 31);
        state = next;
        let bits = int as u32;
        let val = bits % n;
        // Reject the rare overflow tail (`bits - val + m < 0` in signed math).
        if bits.wrapping_sub(val).wrapping_add(m) >> 31 == 0 {
            return val;
        }
    }
}

/// Feature chunk inside one region (non-triangular path).
/// Mirrors upstream `getFeatureChunkInRegion`.
pub fn feature_chunk_in_region(
    config: &StructureConfig,
    seed: u64,
    reg_x: i32,
    reg_z: i32,
) -> (i32, i32) {
    let mut x_seed = (reg_x as u64)
        .wrapping_mul(REGION_X_FACTOR)
        .wrapping_add((reg_z as u64).wrapping_mul(REGION_Z_FACTOR))
        .wrapping_add(config.salt)
        .wrapping_add(seed);
    x_seed = (x_seed ^ LCG_A) & LCG_MASK;
    x_seed = x_seed.wrapping_mul(LCG_A).wrapping_add(LCG_B) & LCG_MASK;

    if config.chunk_range == 1 {
        // NOTE: upstream's `else` writes `pos.x` twice and never `pos.z`
        // (dead code — no config ships `chunk_range == 1`). This port takes
        // the sane branch: x and z, one LCG step each.
        let mut pos = (0i32, 0i32);
        pos.0 = ((x_seed >> 17) % config.chunk_range as u64) as i32;
        x_seed = x_seed.wrapping_mul(LCG_A).wrapping_add(LCG_B) & LCG_MASK;
        pos.1 = ((x_seed >> 17) % config.chunk_range as u64) as i32;
        return pos;
    }

    let pos_x = ((x_seed >> 17) % config.chunk_range as u64) as i32;
    x_seed = x_seed.wrapping_mul(LCG_A).wrapping_add(LCG_B) & LCG_MASK;
    let pos_z = ((x_seed >> 17) % config.chunk_range as u64) as i32;
    (pos_x, pos_z)
}

/// Triangular-path feature chunk (`isTriangular`).
/// Mirrors upstream `getLargeStructureChunkInRegion`.
pub fn large_feature_chunk_in_region(
    config: &StructureConfig,
    seed: u64,
    reg_x: i32,
    reg_z: i32,
) -> (i32, i32) {
    let mut x_seed = (reg_x as u64)
        .wrapping_mul(REGION_X_FACTOR)
        .wrapping_add((reg_z as u64).wrapping_mul(REGION_Z_FACTOR))
        .wrapping_add(config.salt)
        .wrapping_add(seed);
    x_seed = (x_seed ^ LCG_A) & LCG_MASK;

    x_seed = x_seed.wrapping_mul(LCG_A).wrapping_add(LCG_B) & LCG_MASK;
    let a = ((x_seed >> 17) % config.chunk_range as u64) as i32;
    x_seed = x_seed.wrapping_mul(LCG_A).wrapping_add(LCG_B) & LCG_MASK;
    let b = ((x_seed >> 17) % config.chunk_range as u64) as i32;
    let pos_x = (a + b) >> 1;

    x_seed = x_seed.wrapping_mul(LCG_A).wrapping_add(LCG_B) & LCG_MASK;
    let c = ((x_seed >> 17) % config.chunk_range as u64) as i32;
    x_seed = x_seed.wrapping_mul(LCG_A).wrapping_add(LCG_B) & LCG_MASK;
    let d = ((x_seed >> 17) % config.chunk_range as u64) as i32;
    let pos_z = (c + d) >> 1;

    (pos_x, pos_z)
}

/// Full block-space position for one region.
/// Mirrors upstream `getFeaturePosition` / `getLargeStructurePosition`.
pub fn feature_position(config: &StructureConfig, seed: u64, reg_x: i32, reg_z: i32) -> (i32, i32) {
    let offset = if config.triangular {
        large_feature_chunk_in_region(config, seed, reg_x, reg_z)
    } else {
        feature_chunk_in_region(config, seed, reg_x, reg_z)
    };
    let block_x = ((reg_x as i64 * config.region_size as i64 + offset.0 as i64) << 4) as i32;
    let block_z = ((reg_z as i64 * config.region_size as i64 + offset.1 as i64) << 4) as i32;
    (block_x, block_z)
}

/// Region indices whose offset window can cover `chunk`.
/// `chunk == reg * region_size + offset` with `offset in 0..chunk_range`
/// for both the uniform and the `(a + b) >> 1` triangular paths.
fn candidate_regions(config: &StructureConfig, chunk: i32) -> Vec<i32> {
    let rs = config.region_size as i64;
    let cr = config.chunk_range as i64;
    let lo = (chunk as i64 - cr + 1).div_euclid(rs);
    let hi = (chunk as i64).div_euclid(rs);
    (lo..=hi).map(|r| r as i32).collect()
}

/// All structure positions owned by one chunk, upstream-style.
///
/// For each of the 12 kinds this searches the region(s) overlapping the
/// chunk, recomputes the feature chunk with the Java LCG, and (for
/// PillagerOutpost) applies the extra `nextInt(5) === 0` roll. Pure and
/// deterministic: same `(seed, chunk)` always yields the same list, and no
/// voxel is read or written.
pub fn structure_positions_for_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> Vec<StructurePosition> {
    let seed_u = seed as u64;
    let mut out = Vec::new();
    for config in structure_configs() {
        for reg_x in candidate_regions(&config, chunk_x) {
            for reg_z in candidate_regions(&config, chunk_z) {
                let (fx, fz) = if config.triangular {
                    large_feature_chunk_in_region(&config, seed_u, reg_x, reg_z)
                } else {
                    feature_chunk_in_region(&config, seed_u, reg_x, reg_z)
                };
                let cx = reg_x * config.region_size + fx;
                let cz = reg_z * config.region_size + fz;
                if cx != chunk_x || cz != chunk_z {
                    continue;
                }
                if config.kind == StructureKind::PillagerOutpost {
                    let (bx, bz) = feature_position(&config, seed_u, reg_x, reg_z);
                    let attempt = set_attempt_seed(seed_u, bx >> 4, bz >> 4);
                    if next_int(attempt, 5) != 0 {
                        continue;
                    }
                }
                out.push(StructurePosition {
                    kind: config.kind,
                    chunk_x,
                    chunk_z,
                    block_x: chunk_x * 16,
                    block_z: chunk_z * 16,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_table_matches_upstream() {
        let table: Vec<(StructureKind, u64, i32, i32, bool)> = structure_configs()
            .iter()
            .map(|c| (c.kind, c.salt, c.region_size, c.chunk_range, c.triangular))
            .collect();
        assert_eq!(table.len(), 12);
        assert_eq!(
            table[0],
            (StructureKind::DesertPyramid, 14_357_617, 32, 24, false)
        );
        assert_eq!(
            table[4],
            (StructureKind::Village, 10_387_312, 34, 26, false)
        );
        assert_eq!(
            table[8],
            (StructureKind::OceanMonument, 10_387_313, 32, 27, true)
        );
        assert_eq!(table[9], (StructureKind::Mansion, 10_387_319, 80, 60, true));
        assert_eq!(
            table[10],
            (StructureKind::PillagerOutpost, 165_745_296, 32, 24, false)
        );
        assert_eq!(
            table[11],
            (StructureKind::AncientCity, 20_083_232, 24, 16, false)
        );
    }

    #[test]
    fn region_offsets_stay_in_range() {
        for config in structure_configs() {
            for reg_x in -3..=3 {
                for reg_z in -3..=3 {
                    let (fx, fz) = if config.triangular {
                        large_feature_chunk_in_region(&config, 0, reg_x, reg_z)
                    } else {
                        feature_chunk_in_region(&config, 0, reg_x, reg_z)
                    };
                    assert!(
                        (0..config.chunk_range).contains(&fx),
                        "{:?} fx={fx}",
                        config.kind
                    );
                    assert!(
                        (0..config.chunk_range).contains(&fz),
                        "{:?} fz={fz}",
                        config.kind
                    );
                }
            }
        }
    }

    #[test]
    fn chunk_lookup_is_deterministic() {
        let a = structure_positions_for_chunk(0, 0, 0);
        let b = structure_positions_for_chunk(0, 0, 0);
        assert_eq!(a, b);
    }
}
