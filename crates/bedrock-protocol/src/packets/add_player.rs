use crate::binary::{CodecError, Reader, Writer};
use crate::packets::item_stack::ItemStack;
use crate::packets::player_list::{read_uuid, write_uuid};
use crate::packets::set_actor_data::{
    decode_metadata, encode_metadata, MetadataEntry, PropertySyncData,
};
use crate::types::Vector3f;
use crate::Packet;

#[derive(Debug, Clone, PartialEq)]
pub struct AbilitiesLayer {
    pub layer_id: u16,
    pub set_mask: u32,
    pub value_mask: u32,
    pub fly_speed: f32,
    pub vertical_fly_speed: f32,
    pub walk_speed: f32,
}

impl AbilitiesLayer {
    pub const LAYER_BASE: u16 = 1;

    pub fn new(
        layer_id: u16,
        set_mask: u32,
        value_mask: u32,
        fly_speed: f32,
        vertical_fly_speed: f32,
        walk_speed: f32,
    ) -> Self {
        Self {
            layer_id,
            set_mask,
            value_mask,
            fly_speed,
            vertical_fly_speed,
            walk_speed,
        }
    }

    pub fn visitor() -> Self {
        Self::new(Self::LAYER_BASE, 0, 0, 0.0, 0.0, 0.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AbilitiesData {
    pub command_permission: u8,
    pub player_permission: u8,
    /// Little-endian long on the wire, NOT a var-long (Mojang).
    pub target_actor_unique_id: i64,
    pub layers: Vec<AbilitiesLayer>,
}

impl AbilitiesData {
    pub fn visitor(target_actor_unique_id: i64) -> Self {
        Self {
            command_permission: 0, // NORMAL
            player_permission: 0,  // VISITOR
            target_actor_unique_id,
            layers: vec![AbilitiesLayer::visitor()],
        }
    }

    pub fn encode(&self, writer: &mut Writer) {
        writer.write_i64_le(self.target_actor_unique_id);
        writer.write_byte(self.player_permission);
        writer.write_byte(self.command_permission);
        writer.write_byte(self.layers.len() as u8);
        for layer in &self.layers {
            writer.write_u16_le(layer.layer_id);
            writer.write_u32_le(layer.set_mask);
            writer.write_u32_le(layer.value_mask);
            writer.write_f32_le(layer.fly_speed);
            writer.write_f32_le(layer.vertical_fly_speed);
            writer.write_f32_le(layer.walk_speed);
        }
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let target_actor_unique_id = reader.read_i64_le()?;
        let player_permission = reader.read_byte()?;
        let command_permission = reader.read_byte()?;
        let layer_count = reader.read_byte()?;
        let mut layers = Vec::with_capacity(layer_count as usize);
        for _ in 0..layer_count {
            let layer_id = reader.read_u16_le()?;
            let set_mask = reader.read_u32_le()?;
            let value_mask = reader.read_u32_le()?;
            let fly_speed = reader.read_f32_le()?;
            let vertical_fly_speed = reader.read_f32_le()?;
            let walk_speed = reader.read_f32_le()?;
            layers.push(AbilitiesLayer::new(
                layer_id,
                set_mask,
                value_mask,
                fly_speed,
                vertical_fly_speed,
                walk_speed,
            ));
        }
        Ok(Self {
            command_permission,
            player_permission,
            target_actor_unique_id,
            layers,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityLink {
    pub from_actor_unique_id: i64,
    pub to_actor_unique_id: i64,
    pub link_type: u8,
    pub immediate: bool,
    pub caused_by_rider: bool,
    pub vehicle_angular_velocity: f32,
}

impl EntityLink {
    fn encode(&self, writer: &mut Writer) {
        writer.write_var_i64(self.from_actor_unique_id);
        writer.write_var_i64(self.to_actor_unique_id);
        writer.write_byte(self.link_type);
        writer.write_bool(self.immediate);
        writer.write_bool(self.caused_by_rider);
        writer.write_f32_le(self.vehicle_angular_velocity);
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            from_actor_unique_id: reader.read_var_i64()?,
            to_actor_unique_id: reader.read_var_i64()?,
            link_type: reader.read_byte()?,
            immediate: reader.read_bool()?,
            caused_by_rider: reader.read_bool()?,
            vehicle_angular_velocity: reader.read_f32_le()?,
        })
    }
}

/// Packet ID 0x0C (12): AddPlayerPacket, mirroring
/// `Human::sendSpawnPacket` — UUID, name, runtime id, position, motion,
/// rotation, held item, gamemode, entity metadata, synced properties,
/// abilities, links, device/build platform.
#[derive(Debug, Clone, PartialEq)]
pub struct AddPlayerPacket {
    pub uuid: [u8; 16],
    pub username: String,
    pub actor_runtime_id: u64,
    pub platform_chat_id: String,
    pub position: Vector3f,
    pub motion: Option<Vector3f>,
    pub pitch: f32,
    pub yaw: f32,
    pub head_yaw: f32,
    pub item: ItemStack,
    pub game_mode: i32,
    pub metadata: Vec<MetadataEntry>,
    pub synced_properties: PropertySyncData,
    pub abilities: AbilitiesData,
    pub links: Vec<EntityLink>,
    pub device_id: String,
    pub build_platform: i32,
}

impl AddPlayerPacket {
    pub const PACKET_ID: u32 = 0x0C;

    pub fn new(
        uuid: [u8; 16],
        username: impl Into<String>,
        actor_runtime_id: u64,
        position: Vector3f,
        pitch: f32,
        yaw: f32,
        head_yaw: f32,
        item: ItemStack,
        game_mode: i32,
        metadata: Vec<MetadataEntry>,
        abilities: AbilitiesData,
    ) -> Self {
        Self {
            uuid,
            username: username.into(),
            actor_runtime_id,
            platform_chat_id: String::new(),
            position,
            motion: None,
            pitch,
            yaw,
            head_yaw,
            item,
            game_mode,
            metadata,
            synced_properties: PropertySyncData::empty(),
            abilities,
            links: Vec::new(),
            device_id: String::new(),
            build_platform: -1,
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        write_uuid(writer, &self.uuid);
        writer.write_string(&self.username);
        writer.write_var_u64(self.actor_runtime_id);
        writer.write_string(&self.platform_chat_id);
        self.position.write(writer);
        match self.motion {
            Some(motion) => motion.write(writer),
            None => {
                writer.write_f32_le(0.0);
                writer.write_f32_le(0.0);
                writer.write_f32_le(0.0);
            }
        }
        writer.write_f32_le(self.pitch);
        writer.write_f32_le(self.yaw);
        writer.write_f32_le(self.head_yaw);
        self.item.encode(writer);
        writer.write_var_i32(self.game_mode);
        encode_metadata(&self.metadata, writer);
        self.synced_properties.encode(writer);
        self.abilities.encode(writer);
        writer.write_var_u32(self.links.len() as u32);
        for link in &self.links {
            link.encode(writer);
        }
        // passengerBlockData: optional, absent
        writer.write_bool(false);
        writer.write_string(&self.device_id);
        writer.write_i32_le(self.build_platform);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let uuid = read_uuid(reader)?;
        let username = reader.read_string()?;
        let actor_runtime_id = reader.read_var_u64()?;
        let platform_chat_id = reader.read_string()?;
        let position = Vector3f::read(reader)?;
        let motion_x = reader.read_f32_le()?;
        let motion_y = reader.read_f32_le()?;
        let motion_z = reader.read_f32_le()?;
        let pitch = reader.read_f32_le()?;
        let yaw = reader.read_f32_le()?;
        let head_yaw = reader.read_f32_le()?;
        let item = ItemStack::decode(reader)?;
        let game_mode = reader.read_var_i32()?;
        let metadata = decode_metadata(reader)?;
        let synced_properties = PropertySyncData::decode(reader)?;
        let abilities = AbilitiesData::decode(reader)?;
        let link_count = reader.read_var_u32()?;
        let mut links = Vec::with_capacity(link_count as usize);
        for _ in 0..link_count {
            links.push(EntityLink::decode(reader)?);
        }
        let has_passenger_block = reader.read_bool()?;
        if has_passenger_block {
            // PassengerOfBlockData layout is not modelled yet; a packet
            // carrying it is not produced by this server.
            return Err(CodecError::Custom(
                "PassengerOfBlockData in AddPlayerPacket is not supported".to_string(),
            ));
        }
        let device_id = reader.read_string()?;
        let build_platform = reader.read_i32_le()?;
        let motion = if motion_x == 0.0 && motion_y == 0.0 && motion_z == 0.0 && false {
            None
        } else {
            Some(Vector3f::new(motion_x, motion_y, motion_z))
        };
        Ok(Self {
            uuid,
            username,
            actor_runtime_id,
            platform_chat_id,
            position,
            motion,
            pitch,
            yaw,
            head_yaw,
            item,
            game_mode,
            metadata,
            synced_properties,
            abilities,
            links,
            device_id,
            build_platform,
        })
    }
}

impl Packet for AddPlayerPacket {
    fn packet_id(&self) -> u32 {
        Self::PACKET_ID
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        self.encode_payload(writer);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Self::decode_payload(reader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::set_actor_data::MetadataValue;

    #[test]
    fn roundtrips_a_spawn() {
        let packet = AddPlayerPacket::new(
            [9u8; 16],
            "Steve",
            7,
            Vector3f::new(10.5, 65.0, -3.25),
            0.0,
            90.0,
            90.0,
            ItemStack::empty(),
            0,
            vec![MetadataEntry::new(
                4,
                MetadataValue::String("Steve".to_string()),
            )],
            AbilitiesData::visitor(7),
        );
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = AddPlayerPacket::decode_payload(&mut reader).expect("decode");
        // Motion encodes as three zeroes when absent, which decodes as Some(0,0,0);
        // normalize before comparing.
        let mut expected = packet.clone();
        expected.motion = Some(Vector3f::new(0.0, 0.0, 0.0));
        assert_eq!(decoded, expected);
        assert_eq!(decoded.actor_runtime_id, 7);
        assert_eq!(decoded.username, "Steve");
        assert_eq!(reader.remaining(), 0);
    }
}
