//! Java creative items must resolve to a Bedrock block, otherwise a Java
//! placement silently does nothing ("I can't place doors"). Bedrock kept legacy
//! names for a few dozen families (`wooden_door` for `oak_door`, `web` for
//! `cobweb`, `normal_stone_slab` for `stone_slab`, ...), so the name alone is
//! not enough: `block_states::java_to_bedrock_block_name` bridges them.

use bedrock_world::block_states;
use bedrock_world::java_chunk::ModernChunk;

/// Every block item of both supported Java versions maps to a real Bedrock
/// block. The creative-only test blocks have no Bedrock counterpart and are the
/// only allowed holes.
#[test]
fn every_java_block_item_resolves_to_a_bedrock_block() {
    let mut unresolved = Vec::new();
    for protocol in [769, 775] {
        for id in 1..4000 {
            let Some(name) = block_states::java_item_block_name(protocol, id) else {
                continue;
            };
            let Some(bedrock) = block_states::java_to_bedrock_block_name(&name) else {
                if name != "test_block" && name != "test_instance_block" {
                    unresolved.push((protocol, id, name));
                }
                continue;
            };
            if ModernChunk::bedrock_network_id_for_name(bedrock.as_ref()).is_none() {
                unresolved.push((protocol, id, name));
            }
        }
    }
    assert!(unresolved.is_empty(), "unresolved Java block items: {unresolved:?}");
}

/// The diverging families resolve to the Bedrock name that carries the same
/// block, and doors keep the properties `Door::place()` writes.
#[test]
fn legacy_java_names_map_to_their_bedrock_blocks() {
    let cases = [
        ("oak_door", "wooden_door"),
        ("spruce_door", "spruce_door"),
        ("oak_trapdoor", "trapdoor"),
        ("oak_fence_gate", "fence_gate"),
        ("oak_button", "wooden_button"),
        ("oak_pressure_plate", "wooden_pressure_plate"),
        ("oak_sign", "standing_sign"),
        ("spruce_sign", "spruce_standing_sign"),
        ("dark_oak_sign", "darkoak_standing_sign"),
        ("red_bed", "bed"),
        ("blue_banner", "standing_banner"),
        ("bricks", "brick_block"),
        ("stone_slab", "normal_stone_slab"),
        ("cobblestone_stairs", "stone_stairs"),
        ("end_stone_bricks", "end_bricks"),
        ("cobweb", "web"),
        ("sugar_cane", "reeds"),
        ("terracotta", "hardened_clay"),
        ("chain", "iron_chain"),
        ("light", "light_block_15"),
    ];
    for (java, bedrock) in cases {
        assert_eq!(
            block_states::java_to_bedrock_block_name(java).as_deref(),
            Some(bedrock),
            "{java} should resolve to {bedrock}"
        );
    }

    let door = block_states::network_id("wooden_door").unwrap();
    assert_eq!(block_states::state_name(door), Some("wooden_door"));
    assert!(block_states::with_property(door, "minecraft:cardinal_direction", "north").is_some());
    assert!(block_states::with_property(door, "upper_block_bit", "true").is_some());
    assert!(block_states::with_property(door, "door_hinge_bit", "true").is_some());
}
