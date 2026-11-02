use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourcePackInfoEntry {
    pub pack_id: String,
    pub version: String,
    pub pack_size: u64,
    pub content_key: String,
    pub sub_pack_name: String,
    pub content_identity: String,
    pub has_scripts: bool,
    pub rtx_enabled: bool,
}

impl ResourcePackInfoEntry {
    pub fn encode(&self, writer: &mut Writer) {
        writer.write_string(&self.pack_id);
        writer.write_string(&self.version);
        writer.write_u64_le(self.pack_size);
        writer.write_string(&self.content_key);
        writer.write_string(&self.sub_pack_name);
        writer.write_string(&self.content_identity);
        writer.write_bool(self.has_scripts);
        writer.write_bool(self.rtx_enabled);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            pack_id: reader.read_string()?,
            version: reader.read_string()?,
            pack_size: reader.read_u64_le()?,
            content_key: reader.read_string()?,
            sub_pack_name: reader.read_string()?,
            content_identity: reader.read_string()?,
            has_scripts: reader.read_bool()?,
            rtx_enabled: reader.read_bool()?,
        })
    }
}

/// Packet ID 0x06: ResourcePacksInfoPacket
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourcePacksInfoPacket {
    pub must_accept: bool,
    pub has_scripts: bool,
    pub force_server_packs: bool,
    pub behavior_packs: Vec<ResourcePackInfoEntry>,
    pub resource_packs: Vec<ResourcePackInfoEntry>,
}

impl Packet for ResourcePacksInfoPacket {
    fn packet_id(&self) -> u32 {
        0x06
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_bool(self.must_accept);
        writer.write_bool(self.has_scripts);
        writer.write_bool(self.force_server_packs);

        writer.write_u16_le(self.behavior_packs.len() as u16);
        for entry in &self.behavior_packs {
            entry.encode(writer);
        }

        writer.write_u16_le(self.resource_packs.len() as u16);
        for entry in &self.resource_packs {
            entry.encode(writer);
        }

        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let must_accept = reader.read_bool()?;
        let has_scripts = reader.read_bool()?;
        let force_server_packs = reader.read_bool()?;

        let behavior_count = reader.read_u16_le()? as usize;
        let mut behavior_packs = Vec::with_capacity(behavior_count);
        for _ in 0..behavior_count {
            behavior_packs.push(ResourcePackInfoEntry::decode(reader)?);
        }

        let resource_count = reader.read_u16_le()? as usize;
        let mut resource_packs = Vec::with_capacity(resource_count);
        for _ in 0..resource_count {
            resource_packs.push(ResourcePackInfoEntry::decode(reader)?);
        }

        Ok(Self {
            must_accept,
            has_scripts,
            force_server_packs,
            behavior_packs,
            resource_packs,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourcePackStackEntry {
    pub pack_id: String,
    pub version: String,
    pub sub_pack_name: String,
}

impl ResourcePackStackEntry {
    pub fn encode(&self, writer: &mut Writer) {
        writer.write_string(&self.pack_id);
        writer.write_string(&self.version);
        writer.write_string(&self.sub_pack_name);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            pack_id: reader.read_string()?,
            version: reader.read_string()?,
            sub_pack_name: reader.read_string()?,
        })
    }
}

/// Packet ID 0x07: ResourcePackStackPacket
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourcePackStackPacket {
    pub must_accept: bool,
    pub behavior_packs: Vec<ResourcePackStackEntry>,
    pub resource_packs: Vec<ResourcePackStackEntry>,
    pub game_version: String,
    pub experiments_previously_enabled: bool,
}

impl Packet for ResourcePackStackPacket {
    fn packet_id(&self) -> u32 {
        0x07
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_bool(self.must_accept);

        writer.write_var_u32(self.behavior_packs.len() as u32);
        for entry in &self.behavior_packs {
            entry.encode(writer);
        }

        writer.write_var_u32(self.resource_packs.len() as u32);
        for entry in &self.resource_packs {
            entry.encode(writer);
        }

        writer.write_string(&self.game_version);
        writer.write_bool(self.experiments_previously_enabled);

        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let must_accept = reader.read_bool()?;

        let behavior_count = reader.read_var_u32()? as usize;
        let mut behavior_packs = Vec::with_capacity(behavior_count);
        for _ in 0..behavior_count {
            behavior_packs.push(ResourcePackStackEntry::decode(reader)?);
        }

        let resource_count = reader.read_var_u32()? as usize;
        let mut resource_packs = Vec::with_capacity(resource_count);
        for _ in 0..resource_count {
            resource_packs.push(ResourcePackStackEntry::decode(reader)?);
        }

        let game_version = reader.read_string()?;
        let experiments_previously_enabled = reader.read_bool()?;

        Ok(Self {
            must_accept,
            behavior_packs,
            resource_packs,
            game_version,
            experiments_previously_enabled,
        })
    }
}

/// Packet ID 0x08: ResourcePackClientResponsePacket
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePackClientResponsePacket {
    pub response_status: u8,
    pub pack_ids: Vec<String>,
}

impl ResourcePackClientResponsePacket {
    pub const STATUS_REFUSED: u8 = 1;
    pub const STATUS_SEND_PACKS: u8 = 2;
    pub const STATUS_HAVE_ALL_PACKS: u8 = 3;
    pub const STATUS_COMPLETED: u8 = 4;

    pub fn new(response_status: u8, pack_ids: Vec<String>) -> Self {
        Self {
            response_status,
            pack_ids,
        }
    }
}

impl Packet for ResourcePackClientResponsePacket {
    fn packet_id(&self) -> u32 {
        0x08
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_byte(self.response_status);
        writer.write_u16_le(self.pack_ids.len() as u16);
        for id in &self.pack_ids {
            writer.write_string(id);
        }
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let response_status = reader.read_byte()?;
        let count = reader.read_u16_le()? as usize;
        let mut pack_ids = Vec::with_capacity(count);
        for _ in 0..count {
            pack_ids.push(reader.read_string()?);
        }
        Ok(Self {
            response_status,
            pack_ids,
        })
    }
}
