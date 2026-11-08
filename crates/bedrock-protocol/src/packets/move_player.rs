use crate::binary::{CodecError, Reader, Writer};
use crate::types::Vector3f;
use crate::Packet;

/// Telemetry attached to a teleport-mode `MovePlayerPacket`
/// (`MovePlayerTeleportData` in axolotl BedrockProtocol).
///
/// On the wire both fields are unsigned 32-bit little-endian integers
/// (not VarInts): `LE::writeUnsignedInt(cause)`,
/// `LE::writeUnsignedInt(sourceActorType)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MovePlayerTeleportData {
    pub cause: u32,
    pub source_actor_type: u32,
}

impl MovePlayerTeleportData {
    pub fn new(cause: u32, source_actor_type: u32) -> Self {
        Self {
            cause,
            source_actor_type,
        }
    }

    pub fn write(&self, writer: &mut Writer) {
        writer.write_u32_le(self.cause);
        writer.write_u32_le(self.source_actor_type);
    }

    pub fn read(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            cause: reader.read_u32_le()?,
            source_actor_type: reader.read_u32_le()?,
        })
    }
}

/// Packet ID 0x13 (19): MovePlayerPacket.
///
/// This is the packet that moves the *local* player on their own client.
/// It is not `0x12`: that ID is MoveActorAbsolute, used to move other
/// entities in viewers' clients. Sending a MovePlayer payload under
/// `0x12` makes a Bedrock client decode it as MoveActorAbsolute and
/// disconnect with a `BinaryStream read() incomplete` packet violation
/// (`packetId: 18`).
///
/// Wire order (axolotl `MovePlayerPacket::encodePayload`, protocol 2225 /
/// Minecraft 1.26.60):
/// actorRuntimeId VarUInt64, position Vector3 (3x f32 LE), pitch/yaw/
/// headYaw f32 LE, mode u8, onGround bool, ridingActorRuntimeId VarUInt64,
/// telemetryData optional (bool + 2x u32 LE when present), tick VarUInt64.
#[derive(Debug, Clone, PartialEq)]
pub struct MovePlayerPacket {
    pub actor_runtime_id: u64,
    pub position: Vector3f,
    pub pitch: f32,
    pub yaw: f32,
    pub head_yaw: f32,
    pub mode: u8,
    pub on_ground: bool,
    pub riding_actor_runtime_id: u64,
    pub telemetry_data: Option<MovePlayerTeleportData>,
    pub tick: u64,
}

impl MovePlayerPacket {
    pub const NETWORK_ID: u32 = 0x13;

    pub const MODE_NORMAL: u8 = 0;
    pub const MODE_RESET: u8 = 1;
    pub const MODE_TELEPORT: u8 = 2;
    pub const MODE_PITCH: u8 = 3;

    /// Mirrors axolotl `MovePlayerPacket::simple()`: a teleport always
    /// carries telemetry data (`MovePlayerTeleportData(0, 0)`); every other
    /// mode carries none.
    pub fn simple(
        actor_runtime_id: u64,
        position: Vector3f,
        pitch: f32,
        yaw: f32,
        head_yaw: f32,
        mode: u8,
        on_ground: bool,
        riding_actor_runtime_id: u64,
        tick: u64,
    ) -> Self {
        Self {
            actor_runtime_id,
            position,
            pitch,
            yaw,
            head_yaw,
            mode,
            on_ground,
            riding_actor_runtime_id,
            telemetry_data: if mode == Self::MODE_TELEPORT {
                Some(MovePlayerTeleportData::new(0, 0))
            } else {
                None
            },
            tick,
        }
    }
}

impl Packet for MovePlayerPacket {
    fn packet_id(&self) -> u32 {
        Self::NETWORK_ID
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_u64(self.actor_runtime_id);
        self.position.write(writer);
        writer.write_f32_le(self.pitch);
        writer.write_f32_le(self.yaw);
        writer.write_f32_le(self.head_yaw);
        writer.write_byte(self.mode);
        writer.write_bool(self.on_ground);
        writer.write_var_u64(self.riding_actor_runtime_id);
        // CommonTypes::writeOptional: a presence bool always comes first,
        // even when the data is absent.
        writer.write_bool(self.telemetry_data.is_some());
        if let Some(data) = &self.telemetry_data {
            data.write(writer);
        }
        writer.write_var_u64(self.tick);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let actor_runtime_id = reader.read_var_u64()?;
        let position = Vector3f::read(reader)?;
        let pitch = reader.read_f32_le()?;
        let yaw = reader.read_f32_le()?;
        let head_yaw = reader.read_f32_le()?;
        let mode = reader.read_byte()?;
        let on_ground = reader.read_bool()?;
        let riding_actor_runtime_id = reader.read_var_u64()?;
        let telemetry_data = if reader.read_bool()? {
            Some(MovePlayerTeleportData::read(reader)?)
        } else {
            None
        };
        let tick = reader.read_var_u64()?;
        Ok(Self {
            actor_runtime_id,
            position,
            pitch,
            yaw,
            head_yaw,
            mode,
            on_ground,
            riding_actor_runtime_id,
            telemetry_data,
            tick,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_wire(packet: &MovePlayerPacket) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(packet.packet_id());
        packet.encode(&mut writer).expect("encode");
        writer.into_bytes().to_vec()
    }

    #[test]
    fn packet_id_is_move_player_not_move_actor_absolute() {
        let packet = MovePlayerPacket::simple(
            1,
            Vector3f::new(0.0, 101.62, 0.0),
            0.0,
            0.0,
            0.0,
            MovePlayerPacket::MODE_TELEPORT,
            true,
            0,
            0,
        );
        assert_eq!(packet.packet_id(), 0x13);
        assert_eq!(MovePlayerPacket::NETWORK_ID, 0x13);
        // 0x12 is MoveActorAbsolutePacket in this crate.
        assert_ne!(packet.packet_id(), 0x12);
    }

    #[test]
    fn teleport_to_0_100_0_encodes_byte_for_byte() {
        // `/tp 0 100 0` with eye offset 1.62: position (0, 101.62, 0),
        // yaw/pitch 0, mode teleport, onGround true, riding 0,
        // telemetry present (cause 0, source 0), tick 0.
        let packet = MovePlayerPacket::simple(
            1,
            Vector3f::new(0.0, 101.62, 0.0),
            0.0,
            0.0,
            0.0,
            MovePlayerPacket::MODE_TELEPORT,
            true,
            0,
            0,
        );
        let bytes = encode_wire(&packet);

        let mut expected = vec![0x13u8, 0x01]; // header, actorRuntimeId VarUInt64
        expected.extend_from_slice(&0.0f32.to_le_bytes()); // x
        expected.extend_from_slice(&101.62f32.to_le_bytes()); // eye y
        expected.extend_from_slice(&0.0f32.to_le_bytes()); // z
        expected.extend_from_slice(&0.0f32.to_le_bytes()); // pitch
        expected.extend_from_slice(&0.0f32.to_le_bytes()); // yaw
        expected.extend_from_slice(&0.0f32.to_le_bytes()); // headYaw
        expected.push(0x02); // mode: one unsigned byte, not a VarInt
        expected.push(0x01); // onGround
        expected.push(0x00); // ridingActorRuntimeId VarUInt64
        expected.push(0x01); // telemetryData present (optional bool)
        expected.extend_from_slice(&0u32.to_le_bytes()); // cause, u32 LE
        expected.extend_from_slice(&0u32.to_le_bytes()); // sourceActorType, u32 LE
        expected.push(0x00); // tick VarUInt64

        assert_eq!(bytes, expected);
        // 1 header + 1 actor + 12 position + 12 rotation + 1 mode
        // + 1 onGround + 1 riding + 1 optional + 8 telemetry + 1 tick.
        assert_eq!(bytes.len(), 39);
    }

    #[test]
    fn teleport_roundtrips_and_consumes_the_whole_payload() {
        let packet = MovePlayerPacket::simple(
            42,
            Vector3f::new(0.0, 101.62, 0.0),
            -10.0,
            90.0,
            90.0,
            MovePlayerPacket::MODE_TELEPORT,
            true,
            0,
            1234,
        );
        let bytes = encode_wire(&packet);
        let mut reader = Reader::new(&bytes);
        assert_eq!(reader.read_var_u32().unwrap(), 0x13);
        let decoded = MovePlayerPacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn non_teleport_modes_still_write_the_optional_bool_and_tick() {
        // The old hand-rolled encoder in session.rs wrote the optional
        // telemetry and the tick only for mode 2, truncating every other
        // mode. The spec writes both unconditionally.
        let packet = MovePlayerPacket::simple(
            7,
            Vector3f::new(1.0, 65.0, 2.0),
            0.0,
            180.0,
            180.0,
            MovePlayerPacket::MODE_NORMAL,
            false,
            0,
            99,
        );
        assert_eq!(packet.telemetry_data, None);
        let bytes = encode_wire(&packet);
        let mut reader = Reader::new(&bytes);
        assert_eq!(reader.read_var_u32().unwrap(), 0x13);
        let decoded = MovePlayerPacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(decoded.tick, 99);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn telemetry_fields_are_u32_le_not_varints() {
        // Non-zero values prove the width: VarUInt32(0x01020304) would be
        // 5 bytes, a u32 LE is exactly 4.
        let packet = MovePlayerPacket {
            actor_runtime_id: 1,
            position: Vector3f::new(0.0, 0.0, 0.0),
            pitch: 0.0,
            yaw: 0.0,
            head_yaw: 0.0,
            mode: MovePlayerPacket::MODE_TELEPORT,
            on_ground: true,
            riding_actor_runtime_id: 0,
            telemetry_data: Some(MovePlayerTeleportData::new(0x0102_0304, 0x0506_0708)),
            tick: 0,
        };
        let bytes = encode_wire(&packet);
        // Tail: optional bool + cause (4) + source (4) + tick (1).
        let tail = &bytes[bytes.len() - 10..];
        assert_eq!(
            tail,
            &[0x01, 0x04, 0x03, 0x02, 0x01, 0x08, 0x07, 0x06, 0x05, 0x00]
        );

        let mut reader = Reader::new(&bytes);
        let _ = reader.read_var_u32().unwrap();
        let decoded = MovePlayerPacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded.telemetry_data, packet.telemetry_data);
        assert_eq!(reader.remaining(), 0);
    }
}
