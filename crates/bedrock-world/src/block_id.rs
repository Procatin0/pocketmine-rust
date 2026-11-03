//! Canonical Block State Runtime IDs for Minecraft Bedrock 1.26.60
//!
//! Verified against canonical_block_states.nbt (23,960 block states).

pub const AIR: u32 = 18894;
pub const STONE: u32 = 3317;
pub const DIRT: u32 = 14825;
pub const GRASS_BLOCK: u32 = 16313;
pub const BEDROCK: u32 = 19770;
pub const SAND: u32 = 9807;
pub const SANDSTONE: u32 = 6994;
pub const GRAVEL: u32 = 23919;
pub const WATER: u32 = 14122;

// Ores
pub const COAL_ORE: u32 = 9891;
pub const IRON_ORE: u32 = 11794;
pub const GOLD_ORE: u32 = 4084;
pub const REDSTONE_ORE: u32 = 9931;
pub const LAPIS_ORE: u32 = 22174;
pub const DIAMOND_ORE: u32 = 10489;
pub const EMERALD_ORE: u32 = 21041;

// Trees
pub const OAK_LOG: u32 = 1899;
pub const OAK_LEAVES: u32 = 3596;
pub const SPRUCE_LOG: u32 = 9887;
pub const SPRUCE_LEAVES: u32 = 10613;
pub const BIRCH_LOG: u32 = 3320;
pub const BIRCH_LEAVES: u32 = 9659;

// Foliage & Surface
pub const SHORT_GRASS: u32 = 18720;
pub const TALL_GRASS: u32 = 18720;
pub const SNOW_LAYER: u32 = 1179;
pub const DANDELION: u32 = 23959;
pub const POPPY: u32 = 7303;

#[inline(always)]
pub fn is_transparent(id: u32) -> bool {
    matches!(
        id,
        AIR | WATER | SNOW_LAYER | SHORT_GRASS | DANDELION | POPPY | OAK_LEAVES | SPRUCE_LEAVES | BIRCH_LEAVES
    )
}

#[inline(always)]
pub fn is_solid(id: u32) -> bool {
    !matches!(
        id,
        AIR | WATER | SNOW_LAYER | SHORT_GRASS | DANDELION | POPPY
    )
}

#[inline(always)]
pub fn can_override_for_tree(id: u32) -> bool {
    matches!(
        id,
        AIR | SNOW_LAYER | SHORT_GRASS | DANDELION | POPPY | OAK_LEAVES | SPRUCE_LEAVES | BIRCH_LEAVES
    )
}

#[inline(always)]
pub fn can_be_flowed_into(id: u32) -> bool {
    matches!(id, SNOW_LAYER | SHORT_GRASS | DANDELION | POPPY | AIR)
}
