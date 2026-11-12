//! Packet ID 0x21 (33): InteractPacket.
//! Client -> server: interact with an actor (or open the inventory for self).
//!
//! Payload layout (faithful to `BedrockProtocol/src/InteractPacket.php`):
//! - `action`: unsigned byte (`Byte::readUnsigned`), not a varint.
//! - `target_actor_runtime_id`: unsigned varlong (`CommonTypes::getActorRuntimeId`
//!   -> `VarInt::readUnsignedLong`).
//! - `position`: optional Vector3 (`CommonTypes::readOptional`: bool present
//!   flag + 3 little-endian floats when present, otherwise absent).

use crate::binary::{CodecError, Reader, Writer};
use crate::types::Vector3f;

/// Leave a vehicle.
pub const ACTION_LEAVE_VEHICLE: u8 = 3;
/// Mouse-over an actor.
pub const ACTION_MOUSEOVER: u8 = 4;
/// Open an NPC dialog.
pub const ACTION_OPEN_NPC: u8 = 5;
/// Open the player inventory screen.
pub const ACTION_OPEN_INVENTORY: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct InteractPacket {
    pub action: u8,
    pub target_actor_runtime_id: u64,
    pub position: Option<Vector3f>,
}

impl InteractPacket {
    pub const PACKET_ID: u32 = 0x21;

    pub fn new(action: u8, target_actor_runtime_id: u64, position: Option<Vector3f>) -> Self {
        Self { action, target_actor_runtime_id, position }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_byte(self.action);
        writer.write_var_u64(self.target_actor_runtime_id);
        writer.write_bool(self.position.is_some());
        if let Some(position) = self.position {
            position.write(writer);
        }
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let action = reader.read_byte()?;
        let target_actor_runtime_id = reader.read_var_u64()?;
        let position = if reader.read_bool()? {
            Some(Vector3f::read(reader)?)
        } else {
            None
        };
        Ok(Self { action, target_actor_runtime_id, position })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(packet: &InteractPacket) -> Vec<u8> {
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        writer.into_bytes().to_vec()
    }

    fn roundtrip(packet: InteractPacket) {
        let bytes = encode(&packet);
        // The action must be a single raw byte (Byte::readUnsigned), never a varint.
        assert_eq!(bytes[0], packet.action);
        let mut reader = Reader::new(&bytes);
        assert_eq!(InteractPacket::decode_payload(&mut reader).unwrap(), packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn roundtrips_open_inventory_without_position() {
        let packet = InteractPacket::new(ACTION_OPEN_INVENTORY, 5, None);
        // action byte + varu64 target + false present flag.
        assert_eq!(encode(&packet), vec![6, 5, 0]);
        roundtrip(packet);
    }

    #[test]
    fn roundtrips_open_inventory_with_position() {
        let packet = InteractPacket::new(
            ACTION_OPEN_INVENTORY,
            5,
            Some(Vector3f::new(1.0, 2.0, 3.0)),
        );
        // action byte + varu64 target + true present flag + 12 float bytes.
        let bytes = encode(&packet);
        assert_eq!(bytes.len(), 15);
        assert_eq!(&bytes[..3], &[6, 5, 1]);
        roundtrip(packet);
    }
}
