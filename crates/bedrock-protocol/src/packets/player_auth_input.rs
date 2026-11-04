use crate::binary::{CodecError, Reader, Writer};
use crate::types::Vector3f;
use crate::Packet;

/// Packet ID 0x90 (144): PlayerAuthInputPacket
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerAuthInputPacket {
    pub pitch: f32,
    pub yaw: f32,
    pub position: Vector3f,
    pub move_vec_x: f32,
    pub move_vec_z: f32,
    pub head_yaw: f32,
    pub input_data: Vec<u8>,
}

impl PlayerAuthInputPacket {
    pub fn new(
        pitch: f32,
        yaw: f32,
        position: Vector3f,
        move_vec_x: f32,
        move_vec_z: f32,
        head_yaw: f32,
    ) -> Self {
        Self {
            pitch,
            yaw,
            position,
            move_vec_x,
            move_vec_z,
            head_yaw,
            input_data: Vec::new(),
        }
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Self::decode(reader)
    }
}

impl Packet for PlayerAuthInputPacket {
    fn packet_id(&self) -> u32 {
        0x90
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_f32_le(self.pitch);
        writer.write_f32_le(self.yaw);
        self.position.write(writer);
        writer.write_f32_le(self.move_vec_x);
        writer.write_f32_le(self.move_vec_z);
        writer.write_f32_le(self.head_yaw);
        if !self.input_data.is_empty() {
            writer.write_bytes(&self.input_data);
        }
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let pitch = reader.read_f32_le()?;
        let yaw = reader.read_f32_le()?;
        let position = Vector3f::read(reader)?;
        let move_vec_x = reader.read_f32_le()?;
        let move_vec_z = reader.read_f32_le()?;
        let head_yaw = reader.read_f32_le()?;
        let input_data = reader.read_remaining().to_vec();

        Ok(Self {
            pitch,
            yaw,
            position,
            move_vec_x,
            move_vec_z,
            head_yaw,
            input_data,
        })
    }
}
