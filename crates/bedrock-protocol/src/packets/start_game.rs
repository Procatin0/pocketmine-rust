use crate::binary::{CodecError, Reader, Writer};
use crate::types::{BlockPos, Vector3f};
use crate::Packet;

/// Settings describing world generation, spawn, rules, and gamemode for StartGamePacket.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelSettings {
    pub seed: i64,
    pub spawn_position: BlockPos,
    pub generator_type: i32,
    pub world_gamemode: i32,
    pub hardcore: bool,
    pub difficulty: i32,
    pub dimension: i32,
    pub level_name: String,
}

impl Default for LevelSettings {
    fn default() -> Self {
        Self {
            seed: 0,
            spawn_position: BlockPos::new(0, 64, 0),
            generator_type: 1, // 1 = infinite
            world_gamemode: 0, // 0 = survival
            hardcore: false,
            difficulty: 1,     // 1 = normal
            dimension: 0,      // 0 = overworld
            level_name: "Bedrock World".to_string(),
        }
    }
}

impl LevelSettings {
    pub fn encode(&self, writer: &mut Writer) {
        writer.write_i64_le(self.seed);
        self.spawn_position.write(writer);
        writer.write_var_i32(self.generator_type);
        writer.write_var_i32(self.world_gamemode);
        writer.write_bool(self.hardcore);
        writer.write_var_i32(self.difficulty);
        writer.write_var_i32(self.dimension);
        writer.write_string(&self.level_name);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let seed = reader.read_i64_le()?;
        let spawn_position = BlockPos::read(reader)?;
        let generator_type = reader.read_var_i32()?;
        let world_gamemode = reader.read_var_i32()?;
        let hardcore = reader.read_bool()?;
        let difficulty = reader.read_var_i32()?;
        let dimension = reader.read_var_i32()?;
        let level_name = reader.read_string()?;

        Ok(Self {
            seed,
            spawn_position,
            generator_type,
            world_gamemode,
            hardcore,
            difficulty,
            dimension,
            level_name,
        })
    }
}

/// Packet ID 0x0B (11): StartGamePacket
#[derive(Debug, Clone, PartialEq)]
pub struct StartGamePacket {
    pub actor_unique_id: i64,
    pub actor_runtime_id: u64,
    pub player_gamemode: i32,
    pub player_position: Vector3f,
    pub pitch: f32,
    pub yaw: f32,
    pub level_settings: LevelSettings,
}

impl Packet for StartGamePacket {
    fn packet_id(&self) -> u32 {
        0x0B
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i64(self.actor_unique_id);
        writer.write_var_u64(self.actor_runtime_id);
        writer.write_var_i32(self.player_gamemode);
        self.player_position.write(writer);
        writer.write_f32_le(self.pitch);
        writer.write_f32_le(self.yaw);
        self.level_settings.encode(writer);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let actor_unique_id = reader.read_var_i64()?;
        let actor_runtime_id = reader.read_var_u64()?;
        let player_gamemode = reader.read_var_i32()?;
        let player_position = Vector3f::read(reader)?;
        let pitch = reader.read_f32_le()?;
        let yaw = reader.read_f32_le()?;
        let level_settings = LevelSettings::decode(reader)?;

        Ok(Self {
            actor_unique_id,
            actor_runtime_id,
            player_gamemode,
            player_position,
            pitch,
            yaw,
            level_settings,
        })
    }
}
