use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0xB8 (184): RequestAbilityPacket
#[derive(Debug, Clone, PartialEq)]
pub struct RequestAbilityPacket {
    pub ability_id: i32,
    pub value_type: u8,
    pub bool_value: bool,
    pub float_value: f32,
}

impl RequestAbilityPacket {
    pub const ABILITY_FLYING: i32 = 9;
    pub const ABILITY_NOCLIP: i32 = 17;

    pub fn new_bool(ability_id: i32, value: bool) -> Self {
        Self {
            ability_id,
            value_type: 1,
            bool_value: value,
            float_value: 0.0,
        }
    }
}

impl Packet for RequestAbilityPacket {
    fn packet_id(&self) -> u32 {
        0xB8
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i32(self.ability_id);
        writer.write_byte(self.value_type);
        writer.write_bool(self.bool_value);
        writer.write_f32_le(self.float_value);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let ability_id = reader.read_var_i32()?;
        let value_type = reader.read_byte()?;
        let bool_value = reader.read_bool()?;
        let float_value = reader.read_f32_le()?;
        Ok(Self {
            ability_id,
            value_type,
            bool_value,
            float_value,
        })
    }
}
