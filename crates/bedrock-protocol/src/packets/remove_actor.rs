use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x0E (14): RemoveActorPacket
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemoveActorPacket {
    pub actor_unique_id: i64,
}

impl RemoveActorPacket {
    pub fn new(actor_unique_id: i64) -> Self {
        Self { actor_unique_id }
    }
}

impl Packet for RemoveActorPacket {
    fn packet_id(&self) -> u32 {
        0x0E
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        // `CommonTypes::putActorUniqueId()`: signed VarInt64.
        writer.write_var_i64(self.actor_unique_id);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            actor_unique_id: reader.read_var_i64()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_the_actor_unique_id() {
        for id in [0i64, 1, 62, -1, i64::MAX, i64::MIN] {
            let mut writer = Writer::new();
            RemoveActorPacket::new(id)
                .encode(&mut writer)
                .expect("encode");
            let bytes = writer.into_bytes();
            let mut reader = Reader::new(&bytes);
            let decoded = RemoveActorPacket::decode(&mut reader).expect("decode");
            assert_eq!(decoded.actor_unique_id, id);
            assert_eq!(reader.remaining(), 0);
        }
    }
}
