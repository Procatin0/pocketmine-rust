use crate::binary::{CodecError, Reader, Writer};
use crate::types::Vector3f;
use crate::Packet;

/// Entity metadata property value, mirroring BedrockProtocol's
/// `MetadataProperty` subclasses. Type ids are `EntityMetadataTypes`:
/// BYTE=0, SHORT=1, INT=2, FLOAT=3, STRING=4, COMPOUND_TAG=5, POS=6,
/// LONG=7, VECTOR3F=8.
#[derive(Debug, Clone, PartialEq)]
pub enum MetadataValue {
    Byte(i8),
    Short(i16),
    Int(i32),
    Float(f32),
    String(String),
    CompoundTag(Vec<u8>),
    BlockPos(i32, i32, i32),
    Long(i64),
    Vector3(Vector3f),
}

impl MetadataValue {
    pub fn type_id(&self) -> u32 {
        match self {
            Self::Byte(_) => 0,
            Self::Short(_) => 1,
            Self::Int(_) => 2,
            Self::Float(_) => 3,
            Self::String(_) => 4,
            Self::CompoundTag(_) => 5,
            Self::BlockPos(_, _, _) => 6,
            Self::Long(_) => 7,
            Self::Vector3(_) => 8,
        }
    }

    pub fn encode_value(&self, writer: &mut Writer) {
        match self {
            Self::Byte(v) => writer.write_i8(*v),
            Self::Short(v) => writer.write_i16_le(*v),
            Self::Int(v) => writer.write_var_i32(*v),
            Self::Float(v) => writer.write_f32_le(*v),
            Self::String(s) => writer.write_string(s),
            Self::CompoundTag(bytes) => writer.write_byte_array(bytes),
            Self::BlockPos(x, y, z) => {
                writer.write_var_i32(*x);
                writer.write_var_i32(*y);
                writer.write_var_i32(*z);
            }
            Self::Long(v) => writer.write_var_i64(*v),
            Self::Vector3(v) => v.write(writer),
        }
    }

    pub fn decode_value(type_id: u32, reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(match type_id {
            0 => Self::Byte(reader.read_i8()?),
            1 => Self::Short(reader.read_i16_le()?),
            2 => Self::Int(reader.read_var_i32()?),
            3 => Self::Float(reader.read_f32_le()?),
            4 => Self::String(reader.read_string()?),
            5 => Self::CompoundTag(reader.read_byte_array()?),
            6 => Self::BlockPos(reader.read_var_i32()?, reader.read_var_i32()?, reader.read_var_i32()?),
            7 => Self::Long(reader.read_var_i64()?),
            8 => Self::Vector3(Vector3f::read(reader)?),
            other => {
                return Err(CodecError::Custom(format!(
                    "Unknown entity metadata type {other}"
                )))
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetadataEntry {
    pub key: u32,
    pub value: MetadataValue,
}

impl MetadataEntry {
    pub fn new(key: u32, value: MetadataValue) -> Self {
        Self { key, value }
    }

    pub fn encode(&self, writer: &mut Writer) {
        writer.write_var_u32(self.key);
        writer.write_var_u32(self.value.type_id());
        writer.write_byte(self.value.type_id() as u8);
        self.value.encode_value(writer);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let key = reader.read_var_u32()?;
        let type_id = reader.read_var_u32()?;
        let inner = reader.read_byte()?;
        if inner as u32 != type_id {
            return Err(CodecError::Custom(format!(
                "Entity metadata type mismatch: expected {type_id}, got {inner}"
            )));
        }
        let value = MetadataValue::decode_value(type_id, reader)?;
        Ok(Self { key, value })
    }
}

pub fn encode_metadata(entries: &[MetadataEntry], writer: &mut Writer) {
    writer.write_var_u32(entries.len() as u32);
    for entry in entries {
        entry.encode(writer);
    }
}

pub fn decode_metadata(reader: &mut Reader) -> Result<Vec<MetadataEntry>, CodecError> {
    let count = reader.read_var_u32()?;
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        entries.push(MetadataEntry::decode(reader)?);
    }
    Ok(entries)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropertySyncData {
    pub int_properties: Vec<(u32, i32)>,
    pub float_properties: Vec<(u32, f32)>,
}

impl PropertySyncData {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn encode(&self, writer: &mut Writer) {
        writer.write_var_u32(self.int_properties.len() as u32);
        for (key, value) in &self.int_properties {
            writer.write_var_u32(*key);
            writer.write_var_i32(*value);
        }
        writer.write_var_u32(self.float_properties.len() as u32);
        for (key, value) in &self.float_properties {
            writer.write_var_u32(*key);
            writer.write_f32_le(*value);
        }
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let int_count = reader.read_var_u32()?;
        let mut int_properties = Vec::with_capacity(int_count as usize);
        for _ in 0..int_count {
            let key = reader.read_var_u32()?;
            let value = reader.read_var_i32()?;
            int_properties.push((key, value));
        }
        let float_count = reader.read_var_u32()?;
        let mut float_properties = Vec::with_capacity(float_count as usize);
        for _ in 0..float_count {
            let key = reader.read_var_u32()?;
            let value = reader.read_f32_le()?;
            float_properties.push((key, value));
        }
        Ok(Self {
            int_properties,
            float_properties,
        })
    }
}

/// Packet ID 0x27 (39): SetActorDataPacket.
#[derive(Debug, Clone, PartialEq)]
pub struct SetActorDataPacket {
    pub actor_runtime_id: u64,
    pub metadata: Vec<MetadataEntry>,
    pub synced_properties: PropertySyncData,
    pub tick: u64,
}

impl SetActorDataPacket {
    pub const PACKET_ID: u32 = 0x27;

    pub fn new(
        actor_runtime_id: u64,
        metadata: Vec<MetadataEntry>,
        synced_properties: PropertySyncData,
        tick: u64,
    ) -> Self {
        Self {
            actor_runtime_id,
            metadata,
            synced_properties,
            tick,
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u64(self.actor_runtime_id);
        encode_metadata(&self.metadata, writer);
        self.synced_properties.encode(writer);
        writer.write_var_u64(self.tick);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let actor_runtime_id = reader.read_var_u64()?;
        let metadata = decode_metadata(reader)?;
        let synced_properties = PropertySyncData::decode(reader)?;
        let tick = reader.read_var_u64()?;
        Ok(Self {
            actor_runtime_id,
            metadata,
            synced_properties,
            tick,
        })
    }
}

impl Packet for SetActorDataPacket {
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

    #[test]
    fn metadata_roundtrips() {
        let entries = vec![
            MetadataEntry::new(0, MetadataValue::Long((1 << 48) | (1 << 49))),
            MetadataEntry::new(4, MetadataValue::String("Steve".to_string())),
            MetadataEntry::new(7, MetadataValue::Short(300)),
            MetadataEntry::new(38, MetadataValue::Float(1.0)),
        ];
        let mut writer = Writer::new();
        encode_metadata(&entries, &mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = decode_metadata(&mut reader).expect("decode");
        assert_eq!(decoded, entries);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn set_actor_data_roundtrips() {
        let packet = SetActorDataPacket::new(
            42,
            vec![MetadataEntry::new(4, MetadataValue::String("Alex".to_string()))],
            PropertySyncData::empty(),
            0,
        );
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = SetActorDataPacket::decode_payload(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }
}
