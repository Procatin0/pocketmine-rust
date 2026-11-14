use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x11 (17): TakeItemActorPacket, broadcast when a player picks
/// an item entity up (`onPickUpItem`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TakeItemActorPacket {
    pub item_actor_runtime_id: u64,
    pub taker_actor_runtime_id: u64,
}

impl TakeItemActorPacket {
    pub const PACKET_ID: u32 = 0x11;

    pub fn new(item_actor_runtime_id: u64, taker_actor_runtime_id: u64) -> Self {
        Self {
            item_actor_runtime_id,
            taker_actor_runtime_id,
        }
    }
}

impl Packet for TakeItemActorPacket {
    fn packet_id(&self) -> u32 {
        Self::PACKET_ID
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_u64(self.item_actor_runtime_id);
        writer.write_var_u64(self.taker_actor_runtime_id);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            item_actor_runtime_id: reader.read_var_u64()?,
            taker_actor_runtime_id: reader.read_var_u64()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_item_then_taker() {
        let packet = TakeItemActorPacket::new(4242, 7);
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = TakeItemActorPacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }
}
