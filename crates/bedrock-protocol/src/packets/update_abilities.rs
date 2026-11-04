use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0xBB (187): UpdateAbilitiesPacket
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateAbilitiesPacket {
    pub entity_unique_id: i64,
    pub player_permission: u8,
    pub command_permission: u8,
    pub layer_id: u16,
    pub set_abilities: u32,
    pub set_ability_values: u32,
    pub fly_speed: f32,
    pub vertical_fly_speed: f32,
    pub walk_speed: f32,
}

impl UpdateAbilitiesPacket {
    pub const ABILITY_BUILD: u32 = 1 << 0;
    pub const ABILITY_MINE: u32 = 1 << 1;
    pub const ABILITY_DOORS_AND_SWITCHES: u32 = 1 << 2;
    pub const ABILITY_OPEN_CONTAINERS: u32 = 1 << 3;
    pub const ABILITY_ATTACK_PLAYERS: u32 = 1 << 4;
    pub const ABILITY_ATTACK_MOBS: u32 = 1 << 5;
    pub const ABILITY_OPERATOR: u32 = 1 << 6;
    pub const ABILITY_TELEPORT: u32 = 1 << 7;
    pub const ABILITY_INVULNERABLE: u32 = 1 << 8;
    pub const ABILITY_FLYING: u32 = 1 << 9;
    pub const ABILITY_ALLOW_FLIGHT: u32 = 1 << 10;
    pub const ABILITY_INSTABUILD: u32 = 1 << 11;
    pub const ABILITY_FLY_SPEED: u32 = 1 << 13;
    pub const ABILITY_WALK_SPEED: u32 = 1 << 14;

    pub fn creative(entity_unique_id: i64, flying: bool) -> Self {
        let set = Self::ABILITY_BUILD
            | Self::ABILITY_MINE
            | Self::ABILITY_DOORS_AND_SWITCHES
            | Self::ABILITY_OPEN_CONTAINERS
            | Self::ABILITY_ATTACK_PLAYERS
            | Self::ABILITY_ATTACK_MOBS
            | Self::ABILITY_OPERATOR
            | Self::ABILITY_TELEPORT
            | Self::ABILITY_INVULNERABLE
            | Self::ABILITY_ALLOW_FLIGHT
            | Self::ABILITY_INSTABUILD
            | Self::ABILITY_FLYING
            | Self::ABILITY_FLY_SPEED
            | Self::ABILITY_WALK_SPEED;

        let mut values = Self::ABILITY_BUILD
            | Self::ABILITY_MINE
            | Self::ABILITY_DOORS_AND_SWITCHES
            | Self::ABILITY_OPEN_CONTAINERS
            | Self::ABILITY_ATTACK_PLAYERS
            | Self::ABILITY_ATTACK_MOBS
            | Self::ABILITY_OPERATOR
            | Self::ABILITY_TELEPORT
            | Self::ABILITY_INVULNERABLE
            | Self::ABILITY_ALLOW_FLIGHT
            | Self::ABILITY_INSTABUILD;

        if flying {
            values |= Self::ABILITY_FLYING;
        }

        Self {
            entity_unique_id,
            player_permission: 2, // Member
            command_permission: 1, // Operator / Commands
            layer_id: 1, // LAYER_BASE
            set_abilities: set,
            set_ability_values: values,
            fly_speed: 0.05,
            vertical_fly_speed: 0.05,
            walk_speed: 0.1,
        }
    }

    pub fn survival(entity_unique_id: i64) -> Self {
        let set = Self::ABILITY_BUILD
            | Self::ABILITY_MINE
            | Self::ABILITY_DOORS_AND_SWITCHES
            | Self::ABILITY_OPEN_CONTAINERS
            | Self::ABILITY_ATTACK_PLAYERS
            | Self::ABILITY_ATTACK_MOBS
            | Self::ABILITY_FLY_SPEED
            | Self::ABILITY_WALK_SPEED;

        let values = Self::ABILITY_BUILD
            | Self::ABILITY_MINE
            | Self::ABILITY_DOORS_AND_SWITCHES
            | Self::ABILITY_OPEN_CONTAINERS
            | Self::ABILITY_ATTACK_PLAYERS
            | Self::ABILITY_ATTACK_MOBS;

        Self {
            entity_unique_id,
            player_permission: 2, // Member
            command_permission: 0,
            layer_id: 1,
            set_abilities: set,
            set_ability_values: values,
            fly_speed: 0.05,
            vertical_fly_speed: 0.0,
            walk_speed: 0.1,
        }
    }
}

impl Packet for UpdateAbilitiesPacket {
    fn packet_id(&self) -> u32 {
        0xBB
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_i64_le(self.entity_unique_id);
        writer.write_byte(self.player_permission);
        writer.write_byte(self.command_permission);
        writer.write_byte(1); // 1 layer
        writer.write_u16_le(self.layer_id);
        writer.write_u32_le(self.set_abilities);
        writer.write_u32_le(self.set_ability_values);
        writer.write_f32_le(self.fly_speed);
        writer.write_f32_le(self.vertical_fly_speed);
        writer.write_f32_le(self.walk_speed);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let entity_unique_id = reader.read_i64_le()?;
        let player_permission = reader.read_byte()?;
        let command_permission = reader.read_byte()?;
        let layer_count = reader.read_byte()?;
        if layer_count == 0 {
            return Ok(Self {
                entity_unique_id,
                player_permission,
                command_permission,
                layer_id: 1,
                set_abilities: 0,
                set_ability_values: 0,
                fly_speed: 0.05,
                vertical_fly_speed: 0.0,
                walk_speed: 0.1,
            });
        }
        let layer_id = reader.read_u16_le()?;
        let set_abilities = reader.read_u32_le()?;
        let set_ability_values = reader.read_u32_le()?;
        let fly_speed = reader.read_f32_le()?;
        let vertical_fly_speed = reader.read_f32_le()?;
        let walk_speed = reader.read_f32_le()?;

        // Discard any extra layers if present
        for _ in 1..layer_count {
            let _ = reader.read_u16_le()?;
            let _ = reader.read_u32_le()?;
            let _ = reader.read_u32_le()?;
            let _ = reader.read_f32_le()?;
            let _ = reader.read_f32_le()?;
            let _ = reader.read_f32_le()?;
        }

        Ok(Self {
            entity_unique_id,
            player_permission,
            command_permission,
            layer_id,
            set_abilities,
            set_ability_values,
            fly_speed,
            vertical_fly_speed,
            walk_speed,
        })
    }
}
