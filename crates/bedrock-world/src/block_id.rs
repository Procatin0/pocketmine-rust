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

#[inline(always)]
pub fn is_transparent(id: u32) -> bool {
    matches!(
        id,
        AIR | WATER | SNOW_LAYER | SHORT_GRASS | TALL_GRASS | DANDELION | POPPY | OAK_LEAVES | SPRUCE_LEAVES | BIRCH_LEAVES
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
        AIR | WATER | SNOW_LAYER | SHORT_GRASS | TALL_GRASS | DANDELION | POPPY | OAK_LEAVES | SPRUCE_LEAVES | BIRCH_LEAVES
    )
}

#[inline(always)]
pub fn can_be_flowed_into(id: u32) -> bool {
    matches!(id, SNOW_LAYER | SHORT_GRASS | TALL_GRASS | DANDELION | POPPY | AIR)
}
