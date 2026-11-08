//! Integration tests ported from the pure (non-socket) sections of
//! `plugins/JavaBridge/tests/modern_test.php` (IDS, registry/tags/brand,
//! block states, update_time). The socket-lifecycle sections need the
//! JavaServer/JavaClient Rust ports (other agents) and are not
//! duplicated here.
//!
//! Helpers (`skip_nbt`, `parse_text_component`) mirror the PHP test's
//! field-by-field parsers.

use java_protocol::modern::ModernProfile;
use java_protocol::protocol as p;

/// Skip anonymous NBT (type + payload, no root name), like PHP
/// `skipNbt`/`skipNbtPayload` in modern_test.php.
fn skip_nbt(buf: &[u8], offset: &mut usize) {
    let kind = buf[*offset];
    *offset += 1;
    if kind == 0 {
        return;
    }
    skip_payload(buf, offset, kind);
}

fn skip_payload(buf: &[u8], offset: &mut usize, kind: u8) {
    match kind {
        1 => *offset += 1,
        2 => *offset += 2,
        3 | 5 => *offset += 4,
        4 | 6 => *offset += 8,
        7 => {
            let len = u32::from_be_bytes([
                buf[*offset],
                buf[*offset + 1],
                buf[*offset + 2],
                buf[*offset + 3],
            ]) as usize;
            *offset += 4 + len;
        }
        8 => {
            let len = u16::from_be_bytes([buf[*offset], buf[*offset + 1]]) as usize;
            *offset += 2 + len;
        }
        9 => {
            let elem = buf[*offset];
            *offset += 1;
            let count = u32::from_be_bytes([
                buf[*offset],
                buf[*offset + 1],
                buf[*offset + 2],
                buf[*offset + 3],
            ]) as usize;
            *offset += 4;
            for _ in 0..count {
                skip_payload(buf, offset, elem);
            }
        }
        10 => loop {
            let child = buf[*offset];
            *offset += 1;
            if child == 0 {
                break;
            }
            let len = u16::from_be_bytes([buf[*offset], buf[*offset + 1]]) as usize;
            *offset += 2 + len;
            skip_payload(buf, offset, child);
        },
        11 => {
            let len = u32::from_be_bytes([
                buf[*offset],
                buf[*offset + 1],
                buf[*offset + 2],
                buf[*offset + 3],
            ]) as usize;
            *offset += 4 + 4 * len;
        }
        12 => {
            let len = u32::from_be_bytes([
                buf[*offset],
                buf[*offset + 1],
                buf[*offset + 2],
                buf[*offset + 3],
            ]) as usize;
            *offset += 4 + 8 * len;
        }
        other => panic!("Tipo NBT desconocido: {other}"),
    }
}

/// Extract text from an anonymous `{"text": "..."}` component, like PHP
/// `parseTextComponent`.
fn parse_text_component(payload: &[u8]) -> String {
    assert!(!payload.is_empty() && payload[0] == 10);
    let mut offset = 1;
    assert_eq!(payload[offset], 8);
    offset += 1;
    let name_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
    offset += 2 + name_len;
    let len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
    offset += 2;
    payload[offset..offset + len]
        .to_vec()
        .into_iter()
        .map(char::from)
        .collect()
}

#[test]
fn update_time_ids_and_payloads() {
    // minecraft-data packet_update_time ids + JavaClient::sendTime shapes.
    for (proto, expected_id) in [(769, 107), (770, 106), (773, 111), (774, 111), (775, 113)] {
        let profile = ModernProfile::for_protocol(proto).unwrap();
        assert_eq!(profile.id("update_time"), expected_id);
        if proto >= 775 {
            let payload = profile.update_time_payload(6000, true);
            let mut offset = 0;
            assert_eq!(p::read_long(&payload, &mut offset), Some(0));
            assert_eq!(p::read_var_int(&payload, &mut offset), Some(1));
            assert_eq!(p::read_var_int(&payload, &mut offset), Some(0));
            assert_eq!(p::read_var_long(&payload, &mut offset), Some(6000));
            assert_eq!(p::read_float(&payload, &mut offset), Some(0.0));
            assert_eq!(p::read_float(&payload, &mut offset), Some(1.0));
            assert_eq!(offset, payload.len());
        } else {
            let expected = [p::long(0), p::long(6000), p::boolean(true)].concat();
            assert_eq!(profile.update_time_payload(6000, true), expected);
        }
    }
    // Frozen time: tickDayTime=false / rate=0.
    let frozen = ModernProfile::for_protocol(769)
        .unwrap()
        .update_time_payload(6000, false);
    assert_eq!(
        frozen,
        [p::long(0), p::long(6000), p::boolean(false)].concat()
    );
}

#[test]
fn registry_entries_roundtrip_through_skip_nbt() {
    for proto in [769, 770, 773, 774, 775] {
        let profile = ModernProfile::for_protocol(proto).unwrap();
        let regs = profile.registry_payloads().unwrap();
        assert!(!regs.is_empty());
        for payload in &regs {
            let mut offset = 0;
            p::read_string(payload, &mut offset).unwrap();
            let count = p::read_var_int(payload, &mut offset).unwrap() as usize;
            for _ in 0..count {
                p::read_string(payload, &mut offset).unwrap();
                assert_eq!(payload[offset], 1); // boolean(true)
                offset += 1;
                skip_nbt(payload, &mut offset);
            }
            assert_eq!(offset, payload.len(), "trailing bytes in registry {proto}");
        }
    }
}

#[test]
fn brand_and_tags_wire_shapes() {
    for proto in [769, 770, 773, 774, 775] {
        let profile = ModernProfile::for_protocol(proto).unwrap();
        assert_eq!(profile.id("config_brand"), 1);
        assert_eq!(profile.id("update_tags"), 13);
        assert_eq!(profile.id("tags"), 13);
        let brand = profile.brand_payload();
        let mut offset = 0;
        assert_eq!(
            p::read_string(&brand, &mut offset).as_deref(),
            Some("minecraft:brand")
        );
        assert_eq!(
            p::read_string(&brand, &mut offset).as_deref(),
            Some("PocketMine")
        );
        assert_eq!(offset, brand.len());
        // Tags: first registry in the file order that survives filtering.
        let tags = profile.tags_payload().unwrap();
        let mut toffset = 0;
        let registry_count = p::read_var_int(&tags, &mut toffset).unwrap();
        assert!(registry_count > 0);
        let mut seen_damage = false;
        for _ in 0..registry_count {
            let registry_id = p::read_string(&tags, &mut toffset).unwrap();
            if registry_id == "minecraft:damage_type" {
                seen_damage = true;
            }
            let tag_count = p::read_var_int(&tags, &mut toffset).unwrap();
            for _ in 0..tag_count {
                p::read_string(&tags, &mut toffset).unwrap();
                p::read_var_int_array(&tags, &mut toffset).unwrap();
            }
        }
        assert!(seen_damage);
        assert_eq!(toffset, tags.len());
    }
}

#[test]
fn text_component_parse_like_php() {
    let payload = java_protocol::nbt::text("plugin line here");
    assert_eq!(parse_text_component(&payload), "plugin line here");
}

#[test]
fn lp_vec3_zero_and_positioned_vectors() {
    assert_eq!(p::lp_vec3(0.0, 0.0, 0.0), vec![0x00]);
    // spawn_entity velocity header for 773+ is exactly lpVec3(0,0,0).
    let profile = ModernProfile::for_protocol(775).unwrap();
    assert!(profile.spawn_velocity_is_lp_vec3());
    let legacy = ModernProfile::for_protocol(769).unwrap();
    assert!(!legacy.spawn_velocity_is_lp_vec3());
}

#[test]
fn yaw_vectors_like_php() {
    assert!((p::bedrock_yaw_to_java(0.0) - 180.0).abs() < 0.001);
    assert!(p::bedrock_yaw_to_java(180.0).abs() < 0.001);
    assert!((p::bedrock_yaw_to_java(90.0) + 90.0).abs() < 0.001);
    assert!((p::bedrock_yaw_to_java(270.0) - 90.0).abs() < 0.001);
    assert!((p::java_yaw_to_bedrock(0.0) - 180.0).abs() < 0.001);
    assert!((p::bedrock_pitch_to_java(315.0) + 45.0).abs() < 0.001);
}
