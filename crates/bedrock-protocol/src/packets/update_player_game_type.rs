use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x97 (151): UpdatePlayerGameTypePacket
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatePlayerGameTypePacket {
    pub gamemode: i32,
    pub player_actor_unique_id: i64,
    pub tick: u64,
}

impl UpdatePlayerGameTypePacket {
    pub fn new(gamemode: i32, player_actor_unique_id: i64, tick: u64) -> Self {
        Self {
            gamemode,
            player_actor_unique_id,
            tick,
        }
    }
}

impl Packet for UpdatePlayerGameTypePacket {
    fn packet_id(&self) -> u32 {
        0x97
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i32(self.gamemode);
        writer.write_var_i64(self.player_actor_unique_id);
        writer.write_var_u64(self.tick);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let gamemode = reader.read_var_i32()?;
        let player_actor_unique_id = reader.read_var_i64()?;
        let tick = reader.read_var_u64()?;
        Ok(Self {
            gamemode,
            player_actor_unique_id,
            tick,
        })
    }
}
