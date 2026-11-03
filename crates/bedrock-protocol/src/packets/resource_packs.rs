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
    pub has_addons: bool,
    pub has_scripts: bool,
    pub disable_vibrant_visuals: bool,
    pub world_template_uuid: [u8; 16],
    pub world_template_version: String,
    pub behavior_packs: Vec<ResourcePackInfoEntry>,
    pub resource_packs: Vec<ResourcePackInfoEntry>,
}

impl Packet for ResourcePacksInfoPacket {
    fn packet_id(&self) -> u32 {
        0x06
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_bool(self.must_accept);
        writer.write_bool(self.has_addons);
        writer.write_bool(self.has_scripts);
        writer.write_bool(self.disable_vibrant_visuals);

        writer.write_bytes(&self.world_template_uuid);
        writer.write_string(&self.world_template_version);

        // texture_packs count (varint)
        writer.write_var_u32(self.resource_packs.len() as u32);
        for entry in &self.resource_packs {
            entry.encode(writer);
        }

        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let must_accept = reader.read_bool()?;
        let has_addons = reader.read_bool()?;
        let has_scripts = reader.read_bool()?;
        let disable_vibrant_visuals = reader.read_bool()?;

        let mut world_template_uuid = [0u8; 16];
        let uuid_bytes = reader.read_bytes(16)?;
        world_template_uuid.copy_from_slice(uuid_bytes);
        let world_template_version = reader.read_string()?;

        let resource_count = reader.read_var_u32()? as usize;
        let mut resource_packs = Vec::with_capacity(resource_count);
        for _ in 0..resource_count {
            resource_packs.push(ResourcePackInfoEntry::decode(reader)?);
        }

        Ok(Self {
            must_accept,
            has_addons,
            has_scripts,
            disable_vibrant_visuals,
            world_template_uuid,
            world_template_version,
            behavior_packs: Vec::new(),
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
    pub has_editor_packs: bool,
}

impl Packet for ResourcePackStackPacket {
    fn packet_id(&self) -> u32 {
        0x07
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_bool(self.must_accept);

        writer.write_var_u32(self.resource_packs.len() as u32);
        for entry in &self.resource_packs {
            writer.write_string(&entry.pack_id);
            writer.write_string(&entry.version);
            writer.write_string(&entry.sub_pack_name);
        }

        writer.write_string(&self.game_version);
        writer.write_i32_le(0); // experiments count (li32 0)
        writer.write_bool(self.experiments_previously_enabled);
        writer.write_bool(self.has_editor_packs);

        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let must_accept = reader.read_bool()?;

        let resource_count = reader.read_var_u32()? as usize;
        let mut resource_packs = Vec::with_capacity(resource_count);
        for _ in 0..resource_count {
            resource_packs.push(ResourcePackStackEntry::decode(reader)?);
        }

        let game_version = reader.read_string()?;
        let _experiments_count = reader.read_i32_le().unwrap_or(0);
        let experiments_previously_enabled = reader.read_bool().unwrap_or(false);
        let has_editor_packs = reader.read_bool().unwrap_or(false);

        Ok(Self {
            must_accept,
            behavior_packs: Vec::new(),
            resource_packs,
            game_version,
            experiments_previously_enabled,
            has_editor_packs,
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
    pub const STATUS_REFUSED: u8 = 0;
    pub const STATUS_SEND_PACKS: u8 = 1;
    pub const STATUS_HAVE_ALL_PACKS: u8 = 2;
    pub const STATUS_COMPLETED: u8 = 3;

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
        let response_status = match reader.read_var_u32() {
            Ok(s) => s as u8,
            Err(_) => reader.read_byte()?,
        };
        let count = match reader.read_u16_le() {
            Ok(c) => c as usize,
            Err(_) => 0,
        };
        let mut pack_ids = Vec::with_capacity(count);
        for _ in 0..count {
            if let Ok(id) = reader.read_string() {
                pack_ids.push(id);
            }
        }
        Ok(Self {
            response_status,
            pack_ids,
        })
    }
}
