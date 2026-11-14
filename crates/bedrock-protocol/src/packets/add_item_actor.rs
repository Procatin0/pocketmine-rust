use crate::binary::{CodecError, Reader, Writer};
use crate::packets::item_stack::ItemStack;
use crate::packets::set_actor_data::{decode_metadata, encode_metadata, MetadataEntry};
use crate::types::Vector3f;
use crate::Packet;

/// Packet ID 0x0F (15): AddItemActorPacket, sent by `ItemEntity::sendSpawnPacket`.
///
/// The item travels in the `ItemStackWrapper` layout, which is byte-identical
/// to a plain `ItemStack` (`putItemStackWrapper` writes the stack fields in
/// the same order as `ItemStack::write`).
#[derive(Debug, Clone, PartialEq)]
pub struct AddItemActorPacket {
    pub actor_unique_id: i64,
    pub actor_runtime_id: u64,
    pub item: ItemStack,
    pub position: Vector3f,
    pub motion: Option<Vector3f>,
    pub metadata: Vec<MetadataEntry>,
    pub is_from_fishing: bool,
}

impl AddItemActorPacket {
    pub const PACKET_ID: u32 = 0x0F;

    pub fn new(
        actor_unique_id: i64,
        actor_runtime_id: u64,
        item: ItemStack,
        position: Vector3f,
        motion: Option<Vector3f>,
        metadata: Vec<MetadataEntry>,
    ) -> Self {
        Self {
            actor_unique_id,
            actor_runtime_id,
            item,
            position,
            motion,
            metadata,
            is_from_fishing: false,
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_i64(self.actor_unique_id);
        writer.write_var_u64(self.actor_runtime_id);
        self.item.encode(writer);
        self.position.write(writer);
        match self.motion {
            Some(motion) => motion.write(writer),
            None => {
                writer.write_f32_le(0.0);
                writer.write_f32_le(0.0);
                writer.write_f32_le(0.0);
            }
        }
        encode_metadata(&self.metadata, writer);
        writer.write_bool(self.is_from_fishing);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            actor_unique_id: reader.read_var_i64()?,
            actor_runtime_id: reader.read_var_u64()?,
            item: ItemStack::decode(reader)?,
            position: Vector3f::read(reader)?,
            motion: Some(Vector3f::read(reader)?),
            metadata: decode_metadata(reader)?,
            is_from_fishing: reader.read_bool()?,
        })
    }
}

impl Packet for AddItemActorPacket {
    fn packet_id(&self) -> u32 {
        Self::PACKET_ID
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        self.encode_payload(writer);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Self::decode_payload(reader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::set_actor_data::MetadataValue;

    #[test]
    fn roundtrips_the_item_and_motion() {
        let packet = AddItemActorPacket::new(
            7,
            99,
            ItemStack {
                network_id: 1,
                count: 3,
                ..ItemStack::empty()
            },
            Vector3f::new(0.5, 64.0, 0.5),
            Some(Vector3f::new(0.0, 0.0, 0.0)),
            vec![MetadataEntry::new(0, MetadataValue::Long(0))],
        );
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = AddItemActorPacket::decode_payload(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }
}
