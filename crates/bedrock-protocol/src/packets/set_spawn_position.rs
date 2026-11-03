use crate::binary::{CodecError, Reader, Writer};
use crate::types::BlockPos;
use crate::Packet;

/// Packet ID 0x2B (43): SetSpawnPositionPacket
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetSpawnPositionPacket {
    pub spawn_type: i32,
    pub player_position: BlockPos,
    pub dimension_id: i32,
    pub world_position: BlockPos,
}

impl SetSpawnPositionPacket {
    pub const TYPE_PLAYER_SPAWN: i32 = 0;
    pub const TYPE_WORLD_SPAWN: i32 = 1;

    pub fn new_player_spawn(pos: BlockPos, dimension_id: i32) -> Self {
        Self {
            spawn_type: Self::TYPE_PLAYER_SPAWN,
            player_position: pos,
            dimension_id,
            world_position: pos,
        }
    }
}

impl Packet for SetSpawnPositionPacket {
    fn packet_id(&self) -> u32 {
        0x2B
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i32(self.spawn_type);
        self.player_position.write(writer);
        writer.write_var_i32(self.dimension_id);
        self.world_position.write(writer);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let spawn_type = reader.read_var_i32()?;
        let player_position = BlockPos::read(reader)?;
        let dimension_id = reader.read_var_i32()?;
        let world_position = BlockPos::read(reader)?;
        Ok(Self {
            spawn_type,
            player_position,
            dimension_id,
            world_position,
        })
    }
}
