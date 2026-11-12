//! Java `boss_bar` packets (apiBossBar, Java side) using this crate's own
//! `wire` primitives (byte-identical to `java-protocol::bossbar`, which
//! `java-bridge` does not depend on — same duplication split as `wire` vs
//! `java-protocol::protocol`). See `java-protocol/src/bossbar.rs` for the
//! format rationale; the packet ids are:
//!
//! | protocol | version | id |
//! |----------|---------|----:|
//! | 340      | 1.12.2  | `0x0C` |
//! | 769      | 1.21.4  | `0x0A` |
//! | 770-775  | 1.21.5+ | `0x09` |
//!
//! Payload: `UUID(16) + VarInt(action)` then per-action fields
//! (`0 ADD: title, health f32, color, dividers, flags`,
//! `1 REMOVE: —`, `2 UPDATE_HEALTH: health`,
//! `3 UPDATE_TITLE: title`, `4 UPDATE_STYLE: color, dividers`,
//! `5 UPDATE_FLAGS: flags`). Legacy titles are plain strings, modern
//! titles are anonymous-NBT `{"text": ...}` (`wire::nbt_text`).

use crate::wire;

/// Legacy 1.12.2 / 340 `boss_bar` id (the "0x0C java?" of the task).
pub const S2C_BOSS_BAR_LEGACY: i32 = 0x0C;
/// 1.21.4 / 769 id.
pub const S2C_BOSS_BAR_769: i32 = 0x0A;
/// 1.21.5+ / 770-775 id.
pub const S2C_BOSS_BAR_770_PLUS: i32 = 0x09;

pub const ACTION_ADD: i32 = 0;
pub const ACTION_REMOVE: i32 = 1;
pub const ACTION_UPDATE_HEALTH: i32 = 2;
pub const ACTION_UPDATE_TITLE: i32 = 3;
pub const ACTION_UPDATE_STYLE: i32 = 4;
pub const ACTION_UPDATE_FLAGS: i32 = 5;

/// Java colours `0 pink .. 6 white`.
pub const COLOR_PINK: i32 = 0;
pub const COLOR_BLUE: i32 = 1;
pub const COLOR_RED: i32 = 2;
pub const COLOR_GREEN: i32 = 3;
pub const COLOR_YELLOW: i32 = 4;
pub const COLOR_PURPLE: i32 = 5;
pub const COLOR_WHITE: i32 = 6;

pub const FLAG_DARKEN_SCREEN: u8 = 0x01;
pub const FLAG_PLAY_MUSIC: u8 = 0x02;
pub const FLAG_CREATE_WORLD_FOG: u8 = 0x04;

/// `boss_bar` id for `protocol`, or `None` when unknown.
pub fn boss_bar_packet_id(protocol: i32) -> Option<i32> {
    match protocol {
        340 => Some(S2C_BOSS_BAR_LEGACY),
        769 => Some(S2C_BOSS_BAR_769),
        770 | 773 | 774 | 775 => Some(S2C_BOSS_BAR_770_PLUS),
        _ => None,
    }
}

/// Bedrock colour byte (0..7) -> Java colour (0..6); rebecca purple
/// falls back to purple.
pub fn bedrock_color_to_java(bedrock: u8) -> i32 {
    match bedrock {
        0 => COLOR_PINK,
        1 => COLOR_BLUE,
        2 => COLOR_RED,
        3 => COLOR_GREEN,
        4 => COLOR_YELLOW,
        5 | 6 => COLOR_PURPLE,
        _ => COLOR_WHITE,
    }
}

/// Deterministic boss-bar UUID for a Bedrock boss actor id (SplitMix64,
/// version-3/variant bits like `wire::offline_uuid`).
pub fn bossbar_uuid(boss_actor_unique_id: i64) -> [u8; 16] {
    fn splitmix(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    let high = splitmix(boss_actor_unique_id as u64);
    let low = splitmix(high ^ 0xB055_BA2B_1A1C_3E5D);
    let mut out = high.to_be_bytes().to_vec();
    out.extend_from_slice(&low.to_be_bytes());
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(&out);
    uuid[6] = (uuid[6] & 0x0F) | 0x30;
    uuid[8] = (uuid[8] & 0x3F) | 0x80;
    uuid
}

fn title_for(protocol: i32, title: &str) -> Vec<u8> {
    if protocol == 340 {
        wire::string(title)
    } else {
        wire::nbt_text(title)
    }
}

fn header(uuid: &[u8; 16], action: i32) -> Vec<u8> {
    let mut out = uuid.to_vec();
    out.extend_from_slice(&wire::varint(action));
    out
}

pub fn add_payload(
    uuid: &[u8; 16],
    protocol: i32,
    title: &str,
    health: f32,
    color: i32,
    dividers: i32,
    flags: u8,
) -> Vec<u8> {
    let mut out = header(uuid, ACTION_ADD);
    out.extend_from_slice(&title_for(protocol, title));
    out.extend_from_slice(&wire::float(health.clamp(0.0, 1.0)));
    out.extend_from_slice(&wire::varint(color));
    out.extend_from_slice(&wire::varint(dividers));
    out.extend_from_slice(&wire::ubyte(flags as i32));
    out
}

pub fn remove_payload(uuid: &[u8; 16]) -> Vec<u8> {
    header(uuid, ACTION_REMOVE)
}

pub fn update_health_payload(uuid: &[u8; 16], health: f32) -> Vec<u8> {
    let mut out = header(uuid, ACTION_UPDATE_HEALTH);
    out.extend_from_slice(&wire::float(health.clamp(0.0, 1.0)));
    out
}

pub fn update_title_payload(uuid: &[u8; 16], protocol: i32, title: &str) -> Vec<u8> {
    let mut out = header(uuid, ACTION_UPDATE_TITLE);
    out.extend_from_slice(&title_for(protocol, title));
    out
}

pub fn update_style_payload(uuid: &[u8; 16], color: i32, dividers: i32) -> Vec<u8> {
    let mut out = header(uuid, ACTION_UPDATE_STYLE);
    out.extend_from_slice(&wire::varint(color));
    out.extend_from_slice(&wire::varint(dividers));
    out
}

pub fn update_flags_payload(uuid: &[u8; 16], flags: u8) -> Vec<u8> {
    let mut out = header(uuid, ACTION_UPDATE_FLAGS);
    out.extend_from_slice(&wire::ubyte(flags as i32));
    out
}

pub fn add_frame(
    uuid: &[u8; 16],
    protocol: i32,
    title: &str,
    health: f32,
    color: i32,
    dividers: i32,
    flags: u8,
) -> Option<Vec<u8>> {
    let id = boss_bar_packet_id(protocol)?;
    Some(wire::frame(
        id,
        &add_payload(uuid, protocol, title, health, color, dividers, flags),
    ))
}

pub fn remove_frame(uuid: &[u8; 16], protocol: i32) -> Option<Vec<u8>> {
    let id = boss_bar_packet_id(protocol)?;
    Some(wire::frame(id, &remove_payload(uuid)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_minecraft_data() {
        assert_eq!(boss_bar_packet_id(340), Some(0x0C));
        assert_eq!(boss_bar_packet_id(769), Some(0x0A));
        assert_eq!(boss_bar_packet_id(775), Some(0x09));
        assert_eq!(boss_bar_packet_id(1), None);
    }

    #[test]
    fn add_frame_layout() {
        let uuid = bossbar_uuid(7);
        let frame = add_frame(&uuid, 340, "Hi", 0.5, COLOR_RED, 0, 0).unwrap();
        let (_, payload, _) = wire::try_split_frame(&frame).unwrap();
        let add = add_payload(&uuid, 340, "Hi", 0.5, COLOR_RED, 0, 0);
        assert_eq!(payload, add);
        assert_eq!(&add[0..16], &uuid);
        assert_eq!(add[16], 0x00);
        assert_eq!(&add[18..20], b"Hi");
        // Modern title is NBT, not a plain string.
        let modern = add_payload(&uuid, 775, "Hi", 0.5, COLOR_RED, 0, 0);
        assert_eq!(modern[17], 10);
        assert!(add_frame(&uuid, 999, "Hi", 1.0, 0, 0, 0).is_none());
    }
}
