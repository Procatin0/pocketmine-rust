//! Bedrock network block id (FNV-1a hash) -> Java 1.12.2 block state.
//!
//! In 1.12.2 a block state is `(numeric block id << 4) | metadata`, so the
//! translation is a two-level mapping: Bedrock runtime id to the legacy Java
//! pair. Only the blocks the world generators actually produce are mapped;
//! anything unknown becomes stone so a Java client never falls through
//! invisible-but-solid geometry.

use bedrock_world::block_id;

/// Java air block state.
pub const JAVA_AIR: u16 = 0;

/// Java block state for an unmapped Bedrock block: stone.
pub const JAVA_FALLBACK: u16 = 1 << 4;

#[inline]
const fn java(id: u16, meta: u16) -> u16 {
    (id << 4) | meta
}

/// Translates a Bedrock network block id into a Java 1.12.2 block state.
///
/// `0` is the "empty layer" sentinel used by the Bedrock subchunk code, so it
/// maps to air as well.
pub fn java_state_for_bedrock(id: u32) -> u16 {
    match id {
        0 => JAVA_AIR,
        block_id::AIR => JAVA_AIR,
        block_id::STONE => java(1, 0),
        block_id::GRASS_BLOCK => java(2, 0),
        block_id::DIRT => java(3, 0),
        block_id::BEDROCK => java(7, 0),
        block_id::WATER => java(9, 0),
        block_id::SAND => java(12, 0),
        block_id::GRAVEL => java(13, 0),
        block_id::SANDSTONE => java(24, 0),
        block_id::COAL_ORE => java(16, 0),
        block_id::IRON_ORE => java(15, 0),
        block_id::GOLD_ORE => java(14, 0),
        block_id::REDSTONE_ORE => java(73, 0),
        block_id::LAPIS_ORE => java(21, 0),
        block_id::DIAMOND_ORE => java(56, 0),
        block_id::EMERALD_ORE => java(129, 0),
        block_id::OAK_LOG => java(17, 0),
        block_id::SPRUCE_LOG => java(17, 1),
        block_id::BIRCH_LOG => java(17, 2),
        block_id::OAK_LEAVES => java(18, 0),
        block_id::SPRUCE_LEAVES => java(18, 1),
        block_id::BIRCH_LEAVES => java(18, 2),
        block_id::SHORT_GRASS => java(31, 1),
        block_id::TALL_GRASS => java(31, 2),
        block_id::SNOW_LAYER => java(78, 0),
        block_id::DANDELION => java(37, 0),
        block_id::POPPY => java(38, 0),
        _ => JAVA_FALLBACK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_stays_air_and_unknown_becomes_stone() {
        assert_eq!(java_state_for_bedrock(block_id::AIR), JAVA_AIR);
        assert_eq!(java_state_for_bedrock(0), JAVA_AIR);
        assert_eq!(java_state_for_bedrock(123_456_789), JAVA_FALLBACK);
    }

    #[test]
    fn grass_block_matches_java_id_two() {
        assert_eq!(java_state_for_bedrock(block_id::GRASS_BLOCK), 2 << 4);
        assert_eq!(java_state_for_bedrock(block_id::BEDROCK), 7 << 4);
    }
}
