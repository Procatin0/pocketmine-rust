use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x6F (111): MoveActorDeltaPacket.
///
/// The interpolated movement channel: unlike `MoveActorAbsolutePacket`
/// (0x12), which snaps the entity to the given position on modern Bedrock
/// clients, `MoveActorDelta` carries per-component optional deltas plus a
/// `ticks` field telling the client how many ticks of interpolation to apply,
/// so a continuous 20 Hz position stream renders as sliding instead of
/// stepping/teleporting. This is the channel BDS and Geyser use for smooth
/// remote-player movement; PocketMine-MP keeps broadcasting movement with
/// `MoveActorAbsolutePacket` (`Entity::broadcastMovement`), so Bedrock ->
/// Bedrock motion in this server stays on the absolute path for parity.
///
/// Wire order (axolotl `MoveActorDeltaPacket::encodePayload`):
/// actorRuntimeId VarUInt64 (NOT zigzag — `CommonTypes::getActorRuntimeId`
/// reads `VarInt::readUnsignedLong`), then optional X/Y/Z (`bool` presence +
/// f32 LE), optional pitch/yaw/headYaw (`bool` presence + rotation byte),
/// onGround bool, teleport bool, forceMoveLocalEntity bool,
/// forceCompletion bool, ticks VarUInt64.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveActorDeltaPacket {
    pub actor_runtime_id: u64,
    pub x_pos: Option<f32>,
    pub y_pos: Option<f32>,
    pub z_pos: Option<f32>,
    pub pitch: Option<f32>,
    pub yaw: Option<f32>,
    pub head_yaw: Option<f32>,
    pub on_ground: bool,
    pub teleport: bool,
    pub force_move_local_entity: bool,
    pub force_completion: bool,
    /// Expected ticks before the next movement update (interpolation
    /// duration). `1` at the server's 20 Hz movement rate.
    pub ticks: u64,
}

impl MoveActorDeltaPacket {
    pub const NETWORK_ID: u32 = 0x6F;

    pub fn new(
        actor_runtime_id: u64,
        x_pos: Option<f32>,
        y_pos: Option<f32>,
        z_pos: Option<f32>,
        pitch: Option<f32>,
        yaw: Option<f32>,
        head_yaw: Option<f32>,
        on_ground: bool,
        teleport: bool,
        force_move_local_entity: bool,
        force_completion: bool,
        ticks: u64,
    ) -> Self {
        Self {
            actor_runtime_id,
            x_pos,
            y_pos,
            z_pos,
            pitch,
            yaw,
            head_yaw,
            on_ground,
            teleport,
            force_move_local_entity,
            force_completion,
            ticks,
        }
    }

    /// Full interpolated move (every component present), as used for
    /// Java -> Bedrock players: eye position (feet + 1.62) with body yaw as
    /// head yaw, no teleport snap, 1 tick of interpolation.
    pub fn full_move(
        actor_runtime_id: u64,
        position: crate::types::Vector3f,
        pitch: f32,
        yaw: f32,
        head_yaw: f32,
    ) -> Self {
        Self::new(
            actor_runtime_id,
            Some(position.x),
            Some(position.y),
            Some(position.z),
            Some(pitch),
            Some(yaw),
            Some(head_yaw),
            false,
            false,
            false,
            false,
            1,
        )
    }

    /// Snap move: same full components but with the teleport flag set so the
    /// client does not interpolate across a long jump.
    pub fn snap_move(
        actor_runtime_id: u64,
        position: crate::types::Vector3f,
        pitch: f32,
        yaw: f32,
        head_yaw: f32,
    ) -> Self {
        Self::new(
            actor_runtime_id,
            Some(position.x),
            Some(position.y),
            Some(position.z),
            Some(pitch),
            Some(yaw),
            Some(head_yaw),
            false,
            true,
            false,
            false,
            0,
        )
    }

    /// `CommonTypes::putRotationByte()`: 256 steps per full turn. The
    /// reference casts with `(int)` (truncation toward zero), not floor:
    /// negative rotations must truncate, not round down. Mirrors
    /// `MoveActorAbsolutePacket::write_rotation_byte`.
    fn write_rotation_byte(writer: &mut Writer, rotation: f32) {
        writer.write_byte(((rotation * (256.0 / 360.0)) as i32) as u8);
    }

    /// `CommonTypes::getRotationByte()`.
    fn read_rotation_byte(reader: &mut Reader) -> Result<f32, CodecError> {
        Ok(reader.read_byte()? as f32 * (360.0 / 256.0))
    }

    fn write_optional_coord(writer: &mut Writer, value: Option<f32>) {
        writer.write_bool(value.is_some());
        if let Some(v) = value {
            writer.write_f32_le(v);
        }
    }

    fn read_optional_coord(reader: &mut Reader) -> Result<Option<f32>, CodecError> {
        Ok(if reader.read_bool()? {
            Some(reader.read_f32_le()?)
        } else {
            None
        })
    }

    fn write_optional_rotation(writer: &mut Writer, value: Option<f32>) {
        writer.write_bool(value.is_some());
        if let Some(v) = value {
            Self::write_rotation_byte(writer, v);
        }
    }

    fn read_optional_rotation(reader: &mut Reader) -> Result<Option<f32>, CodecError> {
        Ok(if reader.read_bool()? {
            Some(Self::read_rotation_byte(reader)?)
        } else {
            None
        })
    }
}

impl Packet for MoveActorDeltaPacket {
    fn packet_id(&self) -> u32 {
        Self::NETWORK_ID
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_u64(self.actor_runtime_id);
        Self::write_optional_coord(writer, self.x_pos);
        Self::write_optional_coord(writer, self.y_pos);
        Self::write_optional_coord(writer, self.z_pos);
        Self::write_optional_rotation(writer, self.pitch);
        Self::write_optional_rotation(writer, self.yaw);
        Self::write_optional_rotation(writer, self.head_yaw);
        writer.write_bool(self.on_ground);
        writer.write_bool(self.teleport);
        writer.write_bool(self.force_move_local_entity);
        writer.write_bool(self.force_completion);
        writer.write_var_u64(self.ticks);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            actor_runtime_id: reader.read_var_u64()?,
            x_pos: Self::read_optional_coord(reader)?,
            y_pos: Self::read_optional_coord(reader)?,
            z_pos: Self::read_optional_coord(reader)?,
            pitch: Self::read_optional_rotation(reader)?,
            yaw: Self::read_optional_rotation(reader)?,
            head_yaw: Self::read_optional_rotation(reader)?,
            on_ground: reader.read_bool()?,
            teleport: reader.read_bool()?,
            force_move_local_entity: reader.read_bool()?,
            force_completion: reader.read_bool()?,
            ticks: reader.read_var_u64()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_wire(packet: &MoveActorDeltaPacket) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(packet.packet_id());
        packet.encode(&mut writer).expect("encode");
        writer.into_bytes().to_vec()
    }

    #[test]
    fn packet_id_is_move_actor_delta() {
        let packet = MoveActorDeltaPacket::full_move(
            1,
            crate::types::Vector3f::new(0.0, 64.0, 0.0),
            0.0,
            0.0,
            0.0,
        );
        assert_eq!(packet.packet_id(), 0x6F);
        assert_eq!(MoveActorDeltaPacket::NETWORK_ID, 0x6F);
        // 0x12 is MoveActorAbsolutePacket in this crate.
        assert_ne!(packet.packet_id(), 0x12);
    }

    #[test]
    fn runtime_id_is_unsigned_varint_not_zigzag() {
        // `CommonTypes::putActorRuntimeId` writes VarUInt64: id 1 is the
        // single byte 0x01, not zigzag 0x02. `MoveActorAbsolutePacket`
        // (precedent) uses signed zigzag; the delta channel must use the
        // reference unsigned form so the id matches `AddPlayerPacket`
        // (also unsigned).
        let packet = MoveActorDeltaPacket::full_move(
            1,
            crate::types::Vector3f::new(0.0, 64.0, 0.0),
            0.0,
            0.0,
            0.0,
        );
        let bytes = encode_wire(&packet);
        assert_eq!(bytes[0], 0x6F); // packet header varu32
        assert_eq!(bytes[1], 0x01); // actorRuntimeId varu64
    }

    #[test]
    fn full_move_roundtrips_and_consumes_the_whole_payload() {
        let packet = MoveActorDeltaPacket::full_move(
            62,
            crate::types::Vector3f::new(-12.5, 70.25, 8.75),
            // Rotation bytes cover [0, 360) in 256 steps, like PM's
            // `getRotationByte()`, so only in-range values round-trip.
            45.0,
            180.0,
            180.0,
        );
        let bytes = encode_wire(&packet);
        let mut reader = Reader::new(&bytes);
        assert_eq!(reader.read_var_u32().unwrap(), 0x6F);
        let decoded = MoveActorDeltaPacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn partial_move_with_some_none_fields_roundtrips() {
        // Yaw-only delta (head/body rotation without a position change):
        // absent coords must encode as a lone presence byte.
        let packet = MoveActorDeltaPacket::new(
            7,
            None,
            None,
            None,
            Some(0.0),
            Some(90.0),
            Some(90.0),
            false,
            false,
            false,
            false,
            1,
        );
        let bytes = encode_wire(&packet);
        // header(1) + id(1) + 3x absent(1) + pitch present+byte(2)
        // + yaw present+byte(2) + headYaw present+byte(2)
        // + 4 bools(4) + ticks(1) = 16
        assert_eq!(bytes.len(), 16);
        let mut reader = Reader::new(&bytes);
        assert_eq!(reader.read_var_u32().unwrap(), 0x6F);
        let decoded = MoveActorDeltaPacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn yaw_90_is_step_64_of_256() {
        let packet = MoveActorDeltaPacket::full_move(
            1,
            crate::types::Vector3f::new(0.0, 64.0, 0.0),
            0.0,
            90.0,
            90.0,
        );
        let bytes = encode_wire(&packet);
        // header(1) + id(1) + 3 coords present(3x(1 bool + 4)) + pitch
        // present+byte (0->0) + yaw present, then the yaw byte.
        let yaw_byte_index = 1 + 1 + 3 * 5 + 2 + 1;
        assert_eq!(bytes[yaw_byte_index], 64);
    }
}
