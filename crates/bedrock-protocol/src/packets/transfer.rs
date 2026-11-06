use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x55 (85): TransferPacket
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransferPacket {
    pub address: String,
    pub port: u16,
    pub reload_world: bool,
}

impl TransferPacket {
    pub fn new(address: impl Into<String>, port: u16, reload_world: bool) -> Self {
        Self {
            address: address.into(),
            port,
            reload_world,
        }
    }
}

impl Packet for TransferPacket {
    fn packet_id(&self) -> u32 {
        0x55
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_string(&self.address);
        writer.write_u16_le(self.port);
        writer.write_bool(self.reload_world);
        // `gatheringsConfig` is optional and `TransferPacket::create($ip, $port,
        // false, null)` sends null, which is a single false byte.
        writer.write_bool(false);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let address = reader.read_string()?;
        let port = reader.read_u16_le()?;
        let reload_world = reader.read_bool()?;
        if reader.read_bool()? {
            return Err(CodecError::Custom(
                "TransferPacket gatheringsConfig is not supported".to_string(),
            ));
        }
        Ok(Self {
            address,
            port,
            reload_world,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_the_port_little_endian() {
        let mut writer = Writer::new();
        TransferPacket::new("play.example.net", 19132, false)
            .encode(&mut writer)
            .expect("encode");
        let bytes = writer.into_bytes();
        assert_eq!(bytes[0], 16, "address length prefix");
        assert_eq!(&bytes[1..17], b"play.example.net");
        assert_eq!(&bytes[17..19], &19132u16.to_le_bytes());
        assert_eq!(bytes[19], 0, "reloadWorld");
        assert_eq!(bytes[20], 0, "null gatheringsConfig");
    }

    #[test]
    fn roundtrips_the_transfer_target() {
        let packet = TransferPacket::new("127.0.0.1", 19132, true);
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = TransferPacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }
}
