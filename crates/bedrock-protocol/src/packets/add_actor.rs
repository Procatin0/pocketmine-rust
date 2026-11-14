use crate::binary::{CodecError, Reader, Writer};
use crate::packets::add_player::EntityLink;
use crate::packets::set_actor_data::{
    decode_metadata, encode_metadata, MetadataEntry, PropertySyncData,
};
use crate::types::Vector3f;
use crate::Packet;

/// `Attribute` in `AddActorPacket`: identifier plus min/current/max.
#[derive(Debug, Clone, PartialEq)]
pub struct ActorAttribute {
    pub id: String,
    pub min: f32,
    pub current: f32,
    pub max: f32,
}

impl ActorAttribute {
    pub fn new(id: impl Into<String>, min: f32, current: f32, max: f32) -> Self {
        Self {
            id: id.into(),
            min,
            current,
            max,
        }
    }

    fn encode(&self, writer: &mut Writer) {
        writer.write_string(&self.id);
        writer.write_f32_le(self.min);
        writer.write_f32_le(self.current);
        writer.write_f32_le(self.max);
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            id: reader.read_string()?,
            min: reader.read_f32_le()?,
            current: reader.read_f32_le()?,
            max: reader.read_f32_le()?,
        })
    }
}

/// Packet ID 0x0D (13): AddActorPacket, mirroring `Entity::sendSpawnPacket`
/// for non-player actors: unique id, runtime id, network type, position,
/// motion, the four rotations, attributes, metadata, synced properties,
/// links and the optional `PassengerOfBlockData`.
#[derive(Debug, Clone, PartialEq)]
pub struct AddActorPacket {
    pub actor_unique_id: i64,
    pub actor_runtime_id: u64,
    pub type_id: String,
    pub position: Vector3f,
    pub motion: Option<Vector3f>,
    pub pitch: f32,
    pub yaw: f32,
    pub head_yaw: f32,
    pub body_yaw: f32,
    pub attributes: Vec<ActorAttribute>,
    pub metadata: Vec<MetadataEntry>,
    pub synced_properties: PropertySyncData,
    pub links: Vec<EntityLink>,
}

impl AddActorPacket {
    pub const PACKET_ID: u32 = 0x0D;

    pub fn new(
        actor_unique_id: i64,
        actor_runtime_id: u64,
        type_id: impl Into<String>,
        position: Vector3f,
        motion: Option<Vector3f>,
        pitch: f32,
        yaw: f32,
        metadata: Vec<MetadataEntry>,
    ) -> Self {
        Self {
            actor_unique_id,
            actor_runtime_id,
            type_id: type_id.into(),
            position,
            motion,
            pitch,
            yaw,
            head_yaw: yaw,
            body_yaw: yaw,
            attributes: Vec::new(),
            metadata,
            synced_properties: PropertySyncData::empty(),
            links: Vec::new(),
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        // `putActorUniqueId` is a signed VarInt64, `putActorRuntimeId` an
        // unsigned VarInt64.
        writer.write_var_i64(self.actor_unique_id);
        writer.write_var_u64(self.actor_runtime_id);
        writer.write_string(&self.type_id);
        self.position.write(writer);
        // `putVector3Nullable` writes three zero floats for `null`.
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
        writer.write_f32_le(self.body_yaw);
        writer.write_var_u32(self.attributes.len() as u32);
        for attribute in &self.attributes {
            attribute.encode(writer);
        }
        encode_metadata(&self.metadata, writer);
        self.synced_properties.encode(writer);
        writer.write_var_u32(self.links.len() as u32);
        for link in &self.links {
            link.encode_payload(writer);
        }
        // passengerBlockData: optional, absent
        writer.write_bool(false);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let actor_unique_id = reader.read_var_i64()?;
        let actor_runtime_id = reader.read_var_u64()?;
        let type_id = reader.read_string()?;
        let position = Vector3f::read(reader)?;
        let motion = Some(Vector3f::read(reader)?);
        let pitch = reader.read_f32_le()?;
        let yaw = reader.read_f32_le()?;
        let head_yaw = reader.read_f32_le()?;
        let body_yaw = reader.read_f32_le()?;
        let attribute_count = reader.read_var_u32()?;
        let mut attributes = Vec::with_capacity(attribute_count as usize);
        for _ in 0..attribute_count {
            attributes.push(ActorAttribute::decode(reader)?);
        }
        let metadata = decode_metadata(reader)?;
        let synced_properties = PropertySyncData::decode(reader)?;
        let link_count = reader.read_var_u32()?;
        let mut links = Vec::with_capacity(link_count as usize);
        for _ in 0..link_count {
            links.push(EntityLink::decode_payload(reader)?);
        }
        let has_passenger_block = reader.read_bool()?;
        if has_passenger_block {
            return Err(CodecError::Custom(
                "PassengerOfBlockData in AddActorPacket is not supported".to_string(),
            ));
        }
        Ok(Self {
            actor_unique_id,
            actor_runtime_id,
            type_id,
            position,
            motion,
            pitch,
            yaw,
            head_yaw,
            body_yaw,
            attributes,
            metadata,
            synced_properties,
            links,
        })
    }
}

impl Packet for AddActorPacket {
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

    fn sample() -> AddActorPacket {
        let mut packet = AddActorPacket::new(
            62,
            4242,
            "minecraft:item",
            Vector3f::new(1.5, 64.0, -2.5),
            Some(Vector3f::new(0.0, -0.04, 0.0)),
            0.0,
            90.0,
            vec![MetadataEntry::new(0, MetadataValue::Long(0))],
        );
        packet.attributes = vec![ActorAttribute::new("minecraft:health", 0.0, 5.0, 5.0)];
        packet
    }

    #[test]
    fn roundtrips_every_field() {
        let packet = sample();
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = AddActorPacket::decode_payload(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn starts_with_var_actor_ids_then_the_network_type() {
        let packet = sample();
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        // VarInt64(62) zigzags to 124 = 0x7C, runtime id 4242 = 0x92 0x21.
        assert_eq!(bytes[0], 124);
        assert_eq!(&bytes[1..3], &[0x92, 0x21]);
        // The network type string follows the two ids.
        let len = bytes[3] as usize;
        assert_eq!(
            std::str::from_utf8(&bytes[4..4 + len]).unwrap(),
            "minecraft:item"
        );
    }
}
