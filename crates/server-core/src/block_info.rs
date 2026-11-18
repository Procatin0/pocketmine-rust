//! Break info per block, port of PocketMine `BlockBreakInfo`.
//!
//! PM `BlockBreakInfo::getBreakTime` computes
//! `hardness * (isToolCompatible ? 1.5 : 5.0)`. The bare hand is compatible
//! for blocks with `toolHarvestLevel == 0` and incompatible for tool-required
//! blocks (stone family, ores). We keep no item/tool model yet, so the bare
//! hand is the faithful default.
//!
//! Only the blocks the generator emits have real values; hashed ids outside
//! that set fall back to stone, a limitation shared with the dig handler.

/// The `BlockBreakInfo` fields the plugin API exposes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreakInfo {
    /// Seconds to break with an empty hand.
    pub seconds: f32,
    /// `BlockBreakInfo::getHardness()`.
    pub hardness: f32,
    /// `isToolCompatible()` with an empty hand (`toolHarvestLevel === 0`).
    pub bare_hand_compatible: bool,
    /// `breaksInstantly()`: hardness `0` (tall grass, flowers, snow layer).
    pub instant: bool,
}

/// `None` when `isBreakable()` is false (air, bedrock, water).
pub fn break_info(block_id: u32) -> Option<BreakInfo> {
    use bedrock_world::block_id as bid;
    if block_id == bid::AIR {
        return None;
    }
    let (hardness, bare_hand_compatible) = match block_id {
        bid::STONE => (1.5, false),
        bid::DIRT => (0.5, true),
        bid::GRASS_BLOCK => (0.6, true),
        bid::SAND => (0.5, true),
        bid::SANDSTONE => (0.8, false),
        bid::GRAVEL => (0.6, true),
        bid::COAL_ORE | bid::IRON_ORE | bid::GOLD_ORE | bid::REDSTONE_ORE
        | bid::LAPIS_ORE | bid::DIAMOND_ORE | bid::EMERALD_ORE => (3.0, false),
        bid::OAK_LOG | bid::SPRUCE_LOG | bid::BIRCH_LOG => (2.0, true),
        bid::OAK_LEAVES | bid::SPRUCE_LEAVES | bid::BIRCH_LEAVES => (0.2, true),
        bid::SHORT_GRASS | bid::TALL_GRASS | bid::DANDELION | bid::POPPY
        | bid::SNOW_LAYER => (0.0, true),
        // Unbreakable (hardness < 0 / not breakable): bedrock, water.
        bid::BEDROCK | bid::WATER => return None,
        // Unknown hashed id: conservative stone default.
        _ => (1.5, false),
    };
    let seconds = if bare_hand_compatible {
        hardness * 1.5
    } else {
        hardness * 5.0
    };
    Some(BreakInfo {
        seconds,
        hardness,
        bare_hand_compatible,
        instant: hardness <= 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stone_hand_and_unbreakable_blocks() {
        let stone = break_info(bedrock_world::block_id::STONE).expect("stone breaks");
        assert_eq!(stone.hardness, 1.5);
        assert!(!stone.bare_hand_compatible);
        assert_eq!(stone.seconds, 7.5);
        assert!(!stone.instant);

        let dirt = break_info(bedrock_world::block_id::DIRT).expect("dirt breaks");
        assert_eq!(dirt.seconds, 0.75);

        let grass = break_info(bedrock_world::block_id::SHORT_GRASS).expect("grass breaks");
        assert!(grass.instant);

        assert!(break_info(bedrock_world::block_id::AIR).is_none());
        assert!(break_info(bedrock_world::block_id::BEDROCK).is_none());
    }
}
