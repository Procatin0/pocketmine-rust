//! LevelSoundEventPacket (0x7b) — `target/axolotl-bedrock-protocol/src/LevelSoundEventPacket.php`.
//!
//! Wire (`encodePayload`): `sound` (varint-prefixed string, a `LevelSoundEvent`
//! identifier such as `"place"`), `position` (3x f32 LE, no presence byte),
//! `extraData` (zigzag varint, the block runtime id for block sounds),
//! `entityType` (string, `":"` for non-actor sounds), `isBabyMob` (bool),
//! `disableRelativeVolume` (bool), `actorUniqueId` (signed int64 **LE**, not a
//! varint) and an optional `firePosition` (presence byte + vector3).
//!
//! PocketMine's `BlockPlaceSound` sends exactly one of these per successful
//! placement (`LevelSoundEventPacket::nonActorSound(LevelSoundEvent::PLACE, ...)`)
//! which is what makes the client play the block's place sound — without it the
//! block just appears.

use crate::binary::Writer;

/// `ProtocolInfo::LEVEL_SOUND_EVENT_PACKET = 0x7b`.
pub const LEVEL_SOUND_EVENT_PACKET_ID: u32 = 0x7b;

/// `LevelSoundEvent::PLACE`.
pub const LEVEL_SOUND_PLACE: &str = "place";

/// `LevelSoundEvent::POWER_ON` (PM `RedstonePowerOnSound`): button pressed,
/// lever switched on.
pub const LEVEL_SOUND_POWER_ON: &str = "power.on";

/// `LevelSoundEvent::POWER_OFF` (PM `RedstonePowerOffSound`): button released,
/// lever switched off.
pub const LEVEL_SOUND_POWER_OFF: &str = "power.off";

/// Encodes a `LevelSoundEventPacket` payload for a non-actor sound.
pub fn level_sound_event_wire(sound: &str, x: f32, y: f32, z: f32, extra_data: i32) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_string(sound);
    writer.write_f32_le(x);
    writer.write_f32_le(y);
    writer.write_f32_le(z);
    writer.write_var_i32(extra_data);
    writer.write_string(":");
    writer.write_bool(false); // isBabyMob
    writer.write_bool(false); // disableRelativeVolume
    writer.write_i64_le(-1); // actorUniqueId
    writer.write_byte(0); // no firePosition
    writer.into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `place` -> varint len 5 + bytes, position as f32 LEs, extraData zigzag,
    /// `":"` entity type, two false bytes, -1 as i64 LE and the absent optional.
    #[test]
    fn place_sound_wire_is_reference_shaped() {
        let wire = level_sound_event_wire(LEVEL_SOUND_PLACE, 1.0, 2.0, 3.0, 0x1234);
        assert_eq!(&wire[..6], &[0x05, b'p', b'l', b'a', b'c', b'e']);
        // 1.0f32 LE, 2.0f32 LE, 3.0f32 LE
        assert_eq!(
            &wire[6..18],
            &[0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x40, 0x40]
        );
        // zigzag(0x6031F1A8) as varint, then `":"` (len 1 + ':'), two false, -1 i64 LE.
        assert_eq!(
            &wire[18..],
            &[
                0xE8, 0x48, // extraData 0x1234 zigzag -> 0x2468 -> varint
                0x01, b':', // entityType ":"
                0x00, 0x00, // isBabyMob, disableRelativeVolume
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // actorUniqueId -1
                0x00, // no firePosition
            ]
        );
    }
}
