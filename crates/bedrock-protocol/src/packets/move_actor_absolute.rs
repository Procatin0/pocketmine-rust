use crate::binary::{CodecError, Reader, Writer};
use crate::types::Vector3f;
use crate::Packet;

/// Packet ID 0x12 (18): MoveActorAbsolutePacket
#[derive(Debug, Clone, PartialEq)]
pub struct MoveActorAbsolutePacket {
    pub actor_runtime_id: i64,
    pub flags: u8,
    pub position: Vector3f,
    pub pitch: f32,
    pub yaw: f32,
    /// Always zero for non-mobs.
    pub head_yaw: f32,
}

impl MoveActorAbsolutePacket {
    pub const FLAG_GROUND: u8 = 0x01;
    pub const FLAG_TELEPORT: u8 = 0x02;
    pub const FLAG_FORCE_MOVE_LOCAL_ENTITY: u8 = 0x04;
    pub const FLAG_FORCE_COMPLETION: u8 = 0x08;

    pub fn new(
        actor_runtime_id: i64,
        position: Vector3f,
        pitch: f32,
        yaw: f32,
        head_yaw: f32,
        flags: u8,
    ) -> Self {
        Self {
            actor_runtime_id,
            flags,
            position,
            pitch,
            yaw,
            head_yaw,
        }
    }

    /// `CommonTypes::putRotationByte()`: 256 steps per full turn. The
    /// reference casts with `(int)` (truncation toward zero), not floor:
    /// negative rotations must truncate, not round down.
    fn write_rotation_byte(writer: &mut Writer, rotation: f32) {
        writer.write_byte(((rotation * (256.0 / 360.0)) as i32) as u8);
    }

    /// `CommonTypes::getRotationByte()`.
    fn read_rotation_byte(reader: &mut Reader) -> Result<f32, CodecError> {
        Ok(reader.read_byte()? as f32 * (360.0 / 256.0))
    }
}

impl Packet for MoveActorAbsolutePacket {
    fn packet_id(&self) -> u32 {
        0x12
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i64(self.actor_runtime_id);
        writer.write_byte(self.flags);
        self.position.write(writer);
        Self::write_rotation_byte(writer, self.pitch);
        Self::write_rotation_byte(writer, self.yaw);
        Self::write_rotation_byte(writer, self.head_yaw);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            actor_runtime_id: reader.read_var_i64()?,
            flags: reader.read_byte()?,
            position: Vector3f::read(reader)?,
            pitch: Self::read_rotation_byte(reader)?,
            yaw: Self::read_rotation_byte(reader)?,
            head_yaw: Self::read_rotation_byte(reader)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_the_flag_byte_right_after_the_runtime_id() {
        let packet = MoveActorAbsolutePacket::new(
            1,
            Vector3f::new(0.0, 64.0, 0.0),
            0.0,
            90.0,
            0.0,
            MoveActorAbsolutePacket::FLAG_TELEPORT | MoveActorAbsolutePacket::FLAG_GROUND,
        );
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        // VarInt64 zigzags 1 into 2, then the unsigned flag byte.
        assert_eq!(bytes[0], 0x02);
        assert_eq!(
            bytes[1],
            MoveActorAbsolutePacket::FLAG_TELEPORT | MoveActorAbsolutePacket::FLAG_GROUND
        );
        // Yaw 90 degrees is step 64 of 256.
        assert_eq!(bytes[1 + 1 + 12 + 1], 64);
    }

    #[test]
    fn roundtrips_positions_and_whole_rotation_steps() {
        let packet = MoveActorAbsolutePacket::new(
            62,
            Vector3f::new(-12.5, 70.25, 8.75),
            // Rotation bytes cover [0, 360) in 256 steps, like PM's
            // `getRotationByte()`, so only in-range values round-trip.
            45.0,
            180.0,
            0.0,
            MoveActorAbsolutePacket::FLAG_GROUND,
        );
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = MoveActorAbsolutePacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }
}
