//! Legacy (pre-1.13) metadata of a Bedrock block state.
//!
//! `data/block_state_meta_map.json` is the BDS map PM uses in
//! `BlockStateDictionary`: one entry per canonical state, in the order of
//! `data/canonical_block_states.nbt` (which is why the state needs its palette
//! position, not its network hash). BDS writes `-1` for states it has no legacy
//! equivalent for.
//!
//! The map only feeds the legacy `Block::getMeta()` alias; nothing in the
//! server decides behaviour from it.

use std::sync::OnceLock;

const META_MAP: &str = include_str!("../../../data/block_state_meta_map.json");

fn meta_map() -> &'static Vec<i64> {
    static MAP: OnceLock<Vec<i64>> = OnceLock::new();
    MAP.get_or_init(|| serde_json::from_str(META_MAP).unwrap_or_default())
}

/// Legacy meta for a Bedrock runtime state id.
///
/// `None` when:
///
/// - the id is not a canonical state (custom or unknown block),
/// - the palette and the map disagree on length,
/// - BDS reports no legacy meta (`-1`).
pub fn legacy_meta(state_id: u32) -> Option<i32> {
    let index = bedrock_world::block_states::state_index(state_id)?;
    let meta = *meta_map().get(index)?;
    i32::try_from(meta).ok().filter(|meta| *meta >= 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bedrock_world::block_states::network_id;

    #[test]
    fn air_and_stone_have_meta_zero() {
        assert_eq!(legacy_meta(bedrock_world::block_id::AIR), Some(0));
        assert_eq!(legacy_meta(network_id("stone").expect("stone")), Some(0));
    }

    #[test]
    fn single_state_blocks_report_meta_zero() {
        // A block with no properties has one state and therefore meta 0; the
        // legacy `<id>:<meta>` id of e.g. wool lives in the id, not here.
        assert_eq!(legacy_meta(network_id("stone").expect("stone")), Some(0));
        assert_eq!(legacy_meta(network_id("white_wool").expect("white wool")), Some(0));
        assert_eq!(legacy_meta(network_id("dirt").expect("dirt")), Some(0));
    }

    #[test]
    fn property_variants_report_their_legacy_meta() {
        use bedrock_world::block_states::with_property;

        // `RichBlockState::getMeta()`: the meta follows the state variant.
        let log = network_id("oak_log").expect("oak log");
        assert_eq!(legacy_meta(log), Some(0), "default axis");
        let x = with_property(log, "pillar_axis", "x").expect("axis x");
        let z = with_property(log, "pillar_axis", "z").expect("axis z");
        assert_eq!(legacy_meta(x), Some(1));
        assert_eq!(legacy_meta(z), Some(2));

        let sign = network_id("birch_standing_sign").expect("birch sign");
        let turned = with_property(sign, "ground_sign_direction", "3").expect("direction 3");
        assert_eq!(legacy_meta(turned), Some(3));
    }

    #[test]
    fn unknown_state_has_no_meta() {
        assert_eq!(legacy_meta(0xDEAD_BEEF), None);
    }
}
