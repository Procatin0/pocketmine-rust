//! Integration tests ported from `plugins/JavaBridge/tests/protocol_selftest.php`
//! (the `Protocol` + primitive parts). Socket/loop sections of
//! `modern_test.php` need the JavaServer/JavaClient Rust ports (other
//! agents); they are covered in `modern_test.rs` at the pure
//! protocol/profile level.

use java_protocol::protocol as p;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn var_int_roundtrip_like_php() {
    // PHP: foreach([0, 1, 127, 128, 300, 25565, -1, 2147483647] as $value)
    for value in [0, 1, 127, 128, 300, 25565, -1, 2147483647] {
        let encoded = p::var_int(value);
        let mut offset = 0;
        assert_eq!(p::read_var_int(&encoded, &mut offset), Some(value));
        assert_eq!(offset, encoded.len());
    }
}

#[test]
fn var_int_incomplete_like_php() {
    let buffer = [0x80u8];
    let mut offset = 0;
    assert_eq!(p::read_var_int(&buffer, &mut offset), None);
    assert_eq!(offset, 0);
}

#[test]
fn string_roundtrip_like_php() {
    let encoded = p::string("Steve");
    let mut offset = 0;
    assert_eq!(
        p::read_string(&encoded, &mut offset).as_deref(),
        Some("Steve")
    );
}

#[test]
fn identifier_is_length_prefixed_like_php() {
    let encoded = p::identifier("minecraft:water");
    let mut offset = 0;
    assert_eq!(
        p::read_string(&encoded, &mut offset).as_deref(),
        Some("minecraft:water")
    );
    assert_eq!(offset, encoded.len());
}

#[test]
fn var_int_array_like_php() {
    for values in [&[][..], &[1], &[1, 2], &[0, 40, 482, 1167]] {
        let encoded = p::var_int_array(values);
        let mut offset = 0;
        assert_eq!(
            p::read_var_int_array(&encoded, &mut offset).unwrap(),
            values
        );
        assert_eq!(offset, encoded.len());
    }
    let buffer = [0x02u8, 0x01];
    let mut offset = 0;
    assert_eq!(p::read_var_int_array(&buffer, &mut offset), None);
    assert_eq!(offset, 0);
}

#[test]
fn frame_carries_length_and_id_like_php() {
    let frame = p::frame(0x00, &p::string("hello"));
    let mut offset = 0;
    let length = p::read_var_int(&frame, &mut offset).unwrap();
    assert_eq!(length as usize, frame.len() - offset);
    let body = &frame[offset..offset + length as usize];
    let mut body_offset = 0;
    assert_eq!(p::read_var_int(body, &mut body_offset), Some(0x00));
    assert_eq!(&body[body_offset..], p::string("hello").as_slice());
}

#[test]
fn position_roundtrip_like_php() {
    for pos in [(0, 4, 0), (-1, 64, 200), (1_000_000, 255, -1_000_000)] {
        let encoded = p::position(pos.0, pos.1, pos.2);
        let mut offset = 0;
        assert_eq!(p::read_position(&encoded, &mut offset), Some(pos));
    }
}

#[test]
fn offline_uuid_like_php() {
    let uuid = p::offline_uuid("Steve");
    assert_eq!(hex(&uuid.to_vec()), "5627dd98e6be3c21b8a8e92344183641");
    assert_eq!(uuid, p::offline_uuid("Steve"));
    assert_ne!(uuid, p::offline_uuid("Alex"));
    assert_eq!(p::uuid_string(&uuid).len(), 36);
    assert_eq!(uuid[6] & 0xF0, 0x30);
}

#[test]
fn chat_json_like_php() {
    assert_eq!(p::chat_json("a\"b"), "{\"text\":\"a\\\"b\"}");
}

#[test]
fn var_long_agrees_with_var_int_for_non_negative_ticks() {
    // 26.1 update_time totalTicks: varLong == varInt en positivo.
    for value in [0i64, 1, 127, 128, 300, 6000, 13000] {
        assert_eq!(p::var_long(value), p::var_int(value as i32));
    }
}

#[test]
fn take_frames_helper_like_php_test() {
    // Same helper style as modern_test.php takeFrames().
    let mut buffer = p::frame(0x00, &p::string("world"));
    buffer.extend_from_slice(&p::frame(0x01, &p::var_int(42)));
    let mut frames = Vec::new();
    while !buffer.is_empty() {
        let mut offset = 0;
        let Some(length) = p::read_var_int(&buffer, &mut offset) else {
            break;
        };
        let length = length as usize;
        if buffer.len() - offset < length {
            break;
        }
        let body = buffer[offset..offset + length].to_vec();
        buffer = buffer[offset + length..].to_vec();
        let mut body_offset = 0;
        let id = p::read_var_int(&body, &mut body_offset).unwrap();
        frames.push((id, body[body_offset..].to_vec()));
    }
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].0, 0);
    assert_eq!(frames[1].1, p::var_int(42));
}
