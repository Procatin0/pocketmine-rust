//! LevelEventPacket (0x19) — `target/BedrockProtocol/src/LevelEventPacket.php`.
//!
//! Wire (`encodePayload` / `decodePayload`): `eventId` (signed varint),
//! `position` (`CommonTypes::putVector3Nullable` — 3x f32 LE, zeros when
//! absent: no presence byte), `eventData` (signed varint).
//!
//! Dig-progress events (`LevelEvent.php`): BLOCK_START_BREAK = 3600,
//! BLOCK_STOP_BREAK = 3601, BLOCK_BREAK_SPEED = 3602. PM
//! `SurvivalBlockBreakHandler` uses them to drive the client's crack
//! rendering in server-auth block breaking (`broadcastPacketToViewers`):
//! the client finishing its local crack sends `PREDICT_DESTROY_BLOCK`,
//! which is the authoritative break.

use crate::binary::Writer;

/// `ProtocolInfo::LEVEL_EVENT_PACKET = 0x19`.
pub const LEVEL_EVENT_PACKET_ID: u32 = 0x19;

/// `LevelEvent::PARTICLE_DESTROY_BLOCK` (2001).
/// Spawns block destruction particles and plays block break sound at position.
pub const LEVEL_EVENT_PARTICLE_DESTROY_BLOCK: i32 = 2001;
/// `LevelEvent::PARTICLE_PUNCH_BLOCK` (2014).
pub const LEVEL_EVENT_PARTICLE_PUNCH_BLOCK: i32 = 2014;
/// `LevelEvent::BLOCK_START_BREAK`.
pub const LEVEL_EVENT_BLOCK_START_BREAK: i32 = 3600;
/// `LevelEvent::BLOCK_STOP_BREAK`.
pub const LEVEL_EVENT_BLOCK_STOP_BREAK: i32 = 3601;
/// `LevelEvent::BLOCK_BREAK_SPEED`.
pub const LEVEL_EVENT_BLOCK_BREAK_SPEED: i32 = 3602;
/// `LevelEvent::SOUND_DOOR` (PM `DoorSound`, played by fence gates).
pub const LEVEL_EVENT_SOUND_DOOR: i32 = 1003;
/// `LevelEvent::SOUND_ANVIL_FALL` (1022), PM `AnvilFallSound`: played when a
/// falling anvil lands (`Anvil::getLandSound()`).
pub const LEVEL_EVENT_SOUND_ANVIL_FALL: i32 = 1022;

/// Encodes a `LevelEventPacket` payload with the exact reference wire
/// (eventId zigzag varint, position 3x f32 LE, eventData zigzag varint).
pub fn level_event_wire(event_id: i32, event_data: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    level_event_wire_at(event_id, event_data, x as f32, y as f32, z as f32)
}

/// `level_event_wire` for fractional positions, which is what
/// `World::addParticle()`/`World::addSound()` take: PocketMine encodes the
/// break particle at `$pos->add(0.5, 0.5, 0.5)`, the block's centre.
pub fn level_event_wire_at(event_id: i32, event_data: i32, x: f32, y: f32, z: f32) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_i32(event_id);
    writer.write_f32_le(x);
    writer.write_f32_le(y);
    writer.write_f32_le(z);
    writer.write_var_i32(event_data);
    writer.into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3600 zigzag-encodes as 7200 unsigned -> varint [0xA0, 0x38];
    /// position (1,2,3) as three f32 LEs; eventData -1 zigzag -> 1 -> [0x01].
    #[test]
    fn fractional_positions_keep_their_half_block_offset() {
        use crate::binary::Reader;
        let wire = level_event_wire_at(LEVEL_EVENT_PARTICLE_DESTROY_BLOCK, 42, 1.5, 2.5, 3.5);
        let mut reader = Reader::new(&wire);
        assert_eq!(reader.read_var_i32().unwrap(), LEVEL_EVENT_PARTICLE_DESTROY_BLOCK);
        assert_eq!(reader.read_f32_le().unwrap(), 1.5);
        assert_eq!(reader.read_f32_le().unwrap(), 2.5);
        assert_eq!(reader.read_f32_le().unwrap(), 3.5);
        assert_eq!(reader.read_var_i32().unwrap(), 42);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn start_break_wire_is_zigzag_eventid_f32_pos_zigzag_data() {
        let wire = level_event_wire(3600, -1, 1, 2, 3);
        assert_eq!(
            wire,
            vec![
                0xA0, 0x38, // eventId 3600 signed->zigzag 7200 varint
                0x00, 0x00, 0x80, 0x3F, // x = 1.0
                0x00, 0x00, 0x00, 0x40, // y = 2.0
                0x00, 0x00, 0x40, 0x40, // z = 3.0
                0x01, // eventData -1
            ]
        );
    }

    /// BLOCK_STOP_BREAK carries data 0 (PM `LevelEvent::BLOCK_STOP_BREAK, 0`).
    #[test]
    fn stop_break_event_ids_are_reference_values() {
        assert_eq!(LEVEL_EVENT_BLOCK_START_BREAK, 3600);
        assert_eq!(LEVEL_EVENT_BLOCK_STOP_BREAK, 3601);
        assert_eq!(LEVEL_EVENT_BLOCK_BREAK_SPEED, 3602);
        assert_eq!(LEVEL_EVENT_PACKET_ID, 0x19);
        // sanity: encode does not panic and splits position bytes.
        let wire = level_event_wire(LEVEL_EVENT_BLOCK_STOP_BREAK, 0, -5, 64, 9);
        assert_eq!(wire.len(), 2 + 12 + 1);
    }
}
