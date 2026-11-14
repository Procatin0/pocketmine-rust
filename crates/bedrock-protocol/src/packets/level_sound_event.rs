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

/// `LevelSoundEvent::FALL_SMALL` (PM `EntityShortFallSound`): landing after a
/// short fall that hurt (`Living::onHitGround()`, damage <= 4).
pub const LEVEL_SOUND_FALL_SMALL: &str = "fall.small";

/// `LevelSoundEvent::FALL_BIG` (PM `EntityLongFallSound`): the bone-breaker
/// crunch for a landing with damage > 4.
pub const LEVEL_SOUND_FALL_BIG: &str = "fall.big";

/// `LevelSoundEvent::LAND` (PM `EntityLandSound`): a landing that did not hurt.
pub const LEVEL_SOUND_LAND: &str = "land";

/// `LevelSoundEvent::BREAK` (PM `BlockBreakSound`, whose `extraData` is the
/// broken block's runtime id).
pub const LEVEL_SOUND_BREAK: &str = "break";

/// Encodes a `LevelSoundEventPacket` payload for a non-actor sound.
pub fn level_sound_event_wire(sound: &str, x: f32, y: f32, z: f32, extra_data: i32) -> Vec<u8> {
    level_sound_actor_event_wire(sound, x, y, z, extra_data, ":", -1)
}

/// Encodes a `LevelSoundEventPacket` payload for an actor sound.
///
/// PocketMine's `EntityShortFallSound`/`EntityLongFallSound` (and every other
/// sound taking an `Entity`) call
/// `LevelSoundEventPacket::create($sound, $pos, $extraData, $entity::getNetworkTypeId(), false, false, $actorId, null)`,
/// so `entity_type` is the actor's network type (`"minecraft:player"`) and
/// `actor_unique_id` its runtime id.
pub fn level_sound_actor_event_wire(
    sound: &str,
    x: f32,
    y: f32,
    z: f32,
    extra_data: i32,
    entity_type: &str,
    actor_unique_id: i64,
) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_string(sound);
    writer.write_f32_le(x);
    writer.write_f32_le(y);
    writer.write_f32_le(z);
    writer.write_var_i32(extra_data);
    writer.write_string(entity_type);
    writer.write_bool(false); // isBabyMob
    writer.write_bool(false); // disableRelativeVolume
    writer.write_i64_le(actor_unique_id);
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

    /// `EntityLongFallSound`: sound id, position, extraData -1, the actor's
    /// network type and its runtime id (`LevelSoundEventPacket::create`).
    #[test]
    fn actor_sound_wire_carries_the_entity_type_and_actor_id() {
        let wire = level_sound_actor_event_wire(
            LEVEL_SOUND_FALL_BIG,
            1.0,
            2.0,
            3.0,
            -1,
            "minecraft:player",
            4242,
        );
        assert_eq!(wire[0] as usize, "fall.big".len());
        assert_eq!(&wire[1..9], b"fall.big");
        // 3x f32 position, then extraData -1 (zigzag 0x01), then the entity
        // type string, the two flags, the actor id (i64 LE) and no firePos.
        assert_eq!(&wire[21..22], &[0x01]);
        assert_eq!(&wire[22..23], &[16]);
        assert_eq!(&wire[23..39], b"minecraft:player");
        assert_eq!(&wire[39..41], &[0x00, 0x00]);
        assert_eq!(&wire[41..49], &4242i64.to_le_bytes());
        assert_eq!(wire[49], 0);
        assert_eq!(wire.len(), 50);
    }
}
