//! Canonical Block State Runtime IDs (FNV-1a 32-bit Hashes) for Minecraft Bedrock 1.26.60
//!
//! When `block_network_ids_are_hashes` is true in StartGamePacket (standard for 1.26+),
//! Bedrock subchunk palette runtime IDs are signed 32-bit FNV-1a hashes of the
//! canonical NBT compound { name: "minecraft:...", states: { ...sorted... } }.

pub const AIR: u32 = (-604749536i32) as u32;            // 0xdbf44120
pub const STONE: u32 = (-2144268767i32) as u32;         // 0x80310e21
pub const DIRT: u32 = (-2108756090i32) as u32;          // 0x824eef86
pub const GRASS_BLOCK: u32 = (-567203660i32) as u32;    // 0xde3128b4
pub const BEDROCK: u32 = (-173245189i32) as u32;        // 0xf5ac7cfb
pub const SAND: u32 = 138639715;                         // 0x08437963
pub const SANDSTONE: u32 = (-1540286469i32) as u32;     // 0xa43117fb
pub const GRAVEL: u32 = 1529044762;                      // 0x5b235f1a
pub const WATER: u32 = 1211861802;                       // 0x483b8b2a

// Ores
pub const COAL_ORE: u32 = 685383673;                     // 0x28da1ff9
pub const IRON_ORE: u32 = 2032622302;                    // 0x79275ade
pub const GOLD_ORE: u32 = 2144082742;                    // 0x7fcc1b36
pub const REDSTONE_ORE: u32 = 180213920;                 // 0x0abdd8a0
pub const LAPIS_ORE: u32 = (-1364705592i32) as u32;     // 0xaea83ec8
pub const DIAMOND_ORE: u32 = 1419890941;                 // 0x54a1d0fd
pub const EMERALD_ORE: u32 = 170572189;                  // 0x0a2ab99d

// Trees
pub const OAK_LOG: u32 = 825916963;                      // 0x313a7e23
pub const OAK_LEAVES: u32 = 2110714365;                  // 0x7dcef1fd
pub const SPRUCE_LOG: u32 = 507450469;                   // 0x1e3f1465
pub const SPRUCE_LEAVES: u32 = 1404704691;               // 0x53ba17b3
pub const BIRCH_LOG: u32 = 1423805714;                   // 0x54dd8d12
pub const BIRCH_LEAVES: u32 = 1740500456;                // 0x67bdede8

// Foliage & Surface
pub const SHORT_GRASS: u32 = (-1467063515i32) as u32;   // 0xa88e6325
pub const TALL_GRASS: u32 = (-119440047i32) as u32;     // 0xf8e17d51
pub const SNOW_LAYER: u32 = (-595350462i32) as u32;     // 0xdc83ac42
pub const DANDELION: u32 = (-1911823592i32) as u32;     // 0x8e0be318
pub const POPPY: u32 = (-876992014i32) as u32;          // 0xcbba29f2

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
