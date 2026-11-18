//! Canonical Block State Network IDs (FNV-1a 32-bit Hashes) for Bedrock 1.26.60 (protocol 2225)
//!
//! When `block_network_ids_are_hashes: true` in StartGamePacket, Bedrock client resolves
//! blocks using signed 32-bit FNV-1a hashes of the canonical state NBT tag.
//! Stored as `u32` in PalettedBlockArray and written as zigzag `i32` in subchunk palette.

pub const AIR: u32 = -604749536i32 as u32;
pub const STONE: u32 = -2144268767i32 as u32;
pub const DIRT: u32 = -2108756090i32 as u32;
pub const GRASS_BLOCK: u32 = -567203660i32 as u32;
pub const BEDROCK: u32 = -173245189i32 as u32;
pub const SAND: u32 = 138639715i32 as u32;
pub const SANDSTONE: u32 = -1540286469i32 as u32;
pub const GRAVEL: u32 = 1529044762i32 as u32;
pub const WATER: u32 = 1211861802i32 as u32;
/// Derived, not copied: `state_hash("minecraft:clay", &[])`, the same function
/// the rest of these constants come from. `block_states` asserts every entry.
pub const CLAY: u32 = 666874214u32;

// Derived the same way, from the block's canonical state with no properties:
// `state_hash("minecraft:podzol", &[])` and so on. `block_states` asserts each
// of them against the function, so none of these is a transcribed number.
pub const PODZOL: u32 = 1711067891u32;
pub const COARSE_DIRT: u32 = 1884368513u32;
pub const RED_SAND: u32 = -66861741i32 as u32;
pub const RED_SANDSTONE: u32 = 1502365015u32;
pub const ICE: u32 = 107547877u32;

// Ores
pub const COAL_ORE: u32 = 685383673i32 as u32;
pub const IRON_ORE: u32 = 2032622302i32 as u32;
pub const GOLD_ORE: u32 = 2144082742i32 as u32;
pub const REDSTONE_ORE: u32 = 180213920i32 as u32;
pub const LAPIS_ORE: u32 = -1364705592i32 as u32;
pub const DIAMOND_ORE: u32 = 1419890941i32 as u32;
pub const EMERALD_ORE: u32 = 170572189i32 as u32;

// Trees
pub const OAK_LOG: u32 = 825916963i32 as u32;
pub const OAK_LEAVES: u32 = 2110714365i32 as u32;
pub const SPRUCE_LOG: u32 = 507450469i32 as u32;
pub const SPRUCE_LEAVES: u32 = 1404704691i32 as u32;
pub const BIRCH_LOG: u32 = 1423805714i32 as u32;
pub const BIRCH_LEAVES: u32 = 1740500456i32 as u32;

// Foliage & Surface
pub const SHORT_GRASS: u32 = -1467063515i32 as u32;
pub const TALL_GRASS: u32 = -119440047i32 as u32;
pub const SNOW_LAYER: u32 = -595350462i32 as u32;
pub const DANDELION: u32 = -1911823592i32 as u32;
pub const POPPY: u32 = -876992014i32 as u32;

// Underwater plants. Ids derived from `data/canonical_block_states.nbt`:
// `seagrass` carries `sea_grass_type`, `kelp` carries `kelp_age` (0..=25) and
// `sea_pickle` carries `cluster_count` plus `dead_bit`.
pub const SEAGRASS: u32 = 274823543u32;
pub const SEAGRASS_DOUBLE_BOT: u32 = 3466686788u32;
pub const SEAGRASS_DOUBLE_TOP: u32 = 518857582u32;

/// Canonical kelp ids: `kelp_age` 0 at the base of the stalk, 25 at the tip.
const KELP_AGES: [u32; 26] = [
    3657091594, 3595616771, 3780041240, 3718566417, 3411192302, 3349717479, 3534141948,
    3472667125, 4148890178, 4087415355, 4271839824, 4210365001, 3902990886, 3841516063,
    4025940532, 3964465709, 345721466, 284246643, 468671112, 407196289, 99822174, 38347351,
    222771820, 161296997, 837520050, 776045227,
];

pub const SEA_PICKLE_1: u32 = 2891842849u32;
pub const SEA_PICKLE_2: u32 = 570293998u32;
pub const SEA_PICKLE_3: u32 = 3111906463u32;

/// Kelp with `kelp_age` clamped to the canonical range.
#[inline]
pub fn kelp(age: u8) -> u32 {
    KELP_AGES[(age as usize).min(KELP_AGES.len() - 1)]
}

/// Sea pickle with one to three pickles in the cluster.
#[inline]
pub fn sea_pickle(cluster_count: u8) -> u32 {
    match cluster_count {
        1 => SEA_PICKLE_1,
        2 => SEA_PICKLE_2,
        _ => SEA_PICKLE_3,
    }
}

#[inline]
pub fn is_seagrass(id: u32) -> bool {
    matches!(id, SEAGRASS | SEAGRASS_DOUBLE_BOT | SEAGRASS_DOUBLE_TOP)
}

#[inline]
pub fn is_kelp(id: u32) -> bool {
    KELP_AGES.contains(&id)
}

/// Any water plant. Deliberately not part of [`is_solid`] or [`is_transparent`]:
/// the surface cover runs before the underwater decoration, and no populator
/// asks about a water column afterwards, so a 26 id scan would only cost time on
/// the hot path.
#[inline]
pub fn is_water_plant(id: u32) -> bool {
    is_seagrass(id)
        || is_kelp(id)
        || matches!(id, SEA_PICKLE_1 | SEA_PICKLE_2 | SEA_PICKLE_3)
}

#[inline(always)]
pub fn is_transparent(id: u32) -> bool {
    matches!(
        id,
        AIR | WATER
            | SNOW_LAYER
            | SHORT_GRASS
            | TALL_GRASS
            | DANDELION
            | POPPY
            | OAK_LEAVES
            | SPRUCE_LEAVES
            | BIRCH_LEAVES
    )
}

#[inline(always)]
pub fn is_solid(id: u32) -> bool {
    !matches!(
        id,
        AIR | WATER | SNOW_LAYER | SHORT_GRASS | TALL_GRASS | DANDELION | POPPY
    )
}

#[inline(always)]
pub fn can_override_for_tree(id: u32) -> bool {
    matches!(
        id,
        AIR | WATER
            | SNOW_LAYER
            | SHORT_GRASS
            | TALL_GRASS
            | DANDELION
            | POPPY
            | OAK_LEAVES
            | SPRUCE_LEAVES
            | BIRCH_LEAVES
    )
}

#[inline(always)]
pub fn can_be_flowed_into(id: u32) -> bool {
    matches!(
        id,
        SNOW_LAYER | SHORT_GRASS | TALL_GRASS | DANDELION | POPPY | AIR
    )
}
