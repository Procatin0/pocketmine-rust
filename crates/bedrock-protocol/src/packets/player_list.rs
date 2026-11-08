use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// UUID codec matching `CommonTypes::putUUID/getUUID`: two little-endian
/// longs — bytes 7..0 followed by bytes 15..8.
pub fn write_uuid(writer: &mut Writer, uuid: &[u8; 16]) {
    let mut first = [0u8; 8];
    first.copy_from_slice(&uuid[0..8]);
    first.reverse();
    let mut second = [0u8; 8];
    second.copy_from_slice(&uuid[8..16]);
    second.reverse();
    writer.write_bytes(&first);
    writer.write_bytes(&second);
}

pub fn read_uuid(reader: &mut Reader) -> Result<[u8; 16], CodecError> {
    let p1 = reader.read_bytes(8)?;
    let p2 = reader.read_bytes(8)?;
    let mut out = [0u8; 16];
    for i in 0..8 {
        out[i] = p1[7 - i];
        out[8 + i] = p2[7 - i];
    }
    Ok(out)
}

/// Parses a hyphenated UUID string ("xxxxxxxx-....") into 16 raw bytes.
pub fn uuid_from_str(value: &str) -> Option<[u8; 16]> {
    let hex: String = value.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        let byte_str = &hex[i * 2..i * 2 + 2];
        out[i] = u8::from_str_radix(byte_str, 16).ok()?;
    }
    Some(out)
}

pub fn uuid_to_string(uuid: &[u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        uuid[0], uuid[1], uuid[2], uuid[3], uuid[4], uuid[5], uuid[6], uuid[7], uuid[8],
        uuid[9], uuid[10], uuid[11], uuid[12], uuid[13], uuid[14], uuid[15]
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkinImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl SkinImage {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            data,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkinAnimation {
    pub image: SkinImage,
    pub anim_type: u32,
    pub frames: f32,
    pub expression_type: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonaSkinPiece {
    pub piece_id: String,
    pub piece_type: u32,
    pub pack_id: [u8; 16],
    pub is_default_piece: bool,
    pub product_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonaPieceTintColor {
    pub piece_type: String,
    pub colors: [u32; 4],
}

/// Vanilla classic Steve geometry (`geometry.humanoid.custom`).
///
/// The Bedrock client validates the skin geometry file: `geometry_data`
/// must be a geometry JSON whose `description.identifier` matches the name
/// referenced by `resource_patch` (`{"geometry":{"default":...}}`), and
/// `geometry_version` must match its `format_version`. Sending `"{}"` made
/// the client log `Required child geometry.[...] not found` and
/// `geometry file didn't validate`.
///
/// Payload is Mojang's vanilla `humanoid.custom.geo.json`
/// (bedrock-samples, `format_version` 1.21.0), minified. The 64x32
/// placeholder texture uses the base layer only (head/body/arms/legs);
/// the overlay bones (hat/sleeves/pants/jacket) sample outside the legacy
/// texture and render transparent, matching vanilla 64x32 Steve.
pub const STEVE_GEOMETRY_NAME: &str = "geometry.humanoid.custom";
pub const STEVE_GEOMETRY_VERSION: &str = "1.21.0";
pub const STEVE_RESOURCE_PATCH: &str =
    r#"{"geometry":{"default":"geometry.humanoid.custom"}}"#;
pub const STEVE_GEOMETRY_DATA: &str = r#"{"format_version":"1.21.0","minecraft:geometry":[{"description":{"identifier":"geometry.humanoid.custom","visible_bounds_width":1,"visible_bounds_height":2,"visible_bounds_offset":[0,1,0]},"bones":[{"name":"root","pivot":[0.0,0.0,0.0]},{"name":"body","parent":"waist","pivot":[0.0,24.0,0.0],"cubes":[{"origin":[-4.0,12.0,-2.0],"size":[8,12,4],"uv":[16,16]}]},{"name":"waist","parent":"root","pivot":[0.0,12.0,0.0]},{"name":"head","parent":"body","pivot":[0.0,24.0,0.0],"cubes":[{"origin":[-4.0,24.0,-4.0],"size":[8,8,8],"uv":[0,0]}]},{"name":"cape","pivot":[0.0,24,3.0],"parent":"body"},{"name":"hat","parent":"head","pivot":[0.0,24.0,0.0],"cubes":[{"origin":[-4.0,24.0,-4.0],"size":[8,8,8],"uv":[32,0],"inflate":0.5}]},{"name":"leftArm","parent":"body","pivot":[5.0,22.0,0.0],"cubes":[{"origin":[4.0,12.0,-2.0],"size":[4,12,4],"uv":[32,48]}]},{"name":"leftSleeve","parent":"leftArm","pivot":[5.0,22.0,0.0],"cubes":[{"origin":[4.0,12.0,-2.0],"size":[4,12,4],"uv":[48,48],"inflate":0.25}]},{"name":"leftItem","pivot":[6.0,15.0,1.0],"parent":"leftArm"},{"name":"rightArm","parent":"body","pivot":[-5.0,22.0,0.0],"cubes":[{"origin":[-8.0,12.0,-2.0],"size":[4,12,4],"uv":[40,16]}]},{"name":"rightSleeve","parent":"rightArm","pivot":[-5.0,22.0,0.0],"cubes":[{"origin":[-8.0,12.0,-2.0],"size":[4,12,4],"uv":[40,32],"inflate":0.25}]},{"name":"rightItem","pivot":[-6,15,1],"locators":{"lead_hold":[-6,15,1]},"parent":"rightArm"},{"name":"leftLeg","parent":"root","pivot":[1.9,12.0,0.0],"cubes":[{"origin":[-0.1,0.0,-2.0],"size":[4,12,4],"uv":[16,48]}]},{"name":"leftPants","parent":"leftLeg","pivot":[1.9,12.0,0.0],"cubes":[{"origin":[-0.1,0.0,-2.0],"size":[4,12,4],"uv":[0,48],"inflate":0.25}]},{"name":"rightLeg","parent":"root","pivot":[-1.9,12.0,0.0],"cubes":[{"origin":[-3.9,0.0,-2.0],"size":[4,12,4],"uv":[0,16]}]},{"name":"rightPants","parent":"rightLeg","pivot":[-1.9,12.0,0.0],"cubes":[{"origin":[-3.9,0.0,-2.0],"size":[4,12,4],"uv":[0,32],"inflate":0.25}]},{"name":"jacket","parent":"body","pivot":[0.0,24.0,0.0],"cubes":[{"origin":[-4.0,12.0,-2.0],"size":[8,12,4],"uv":[16,32],"inflate":0.25}]}]}]}"#;

/// Mirror of BedrockProtocol `SkinData` for protocol 2225.
#[derive(Debug, Clone, PartialEq)]
pub struct SkinData {
    pub skin_id: String,
    pub resource_patch: String,
    pub skin_image: SkinImage,
    pub animations: Vec<SkinAnimation>,
    pub cape_image: SkinImage,
    pub geometry_data: String,
    pub geometry_version: String,
    pub animation_data: String,
    pub cape_id: String,
    pub full_skin_id: String,
    pub arm_size: u8,
    pub skin_color: u32,
    pub persona_pieces: Vec<PersonaSkinPiece>,
    pub piece_tint_colors: Vec<PersonaPieceTintColor>,
    pub premium: bool,
    pub persona: bool,
    pub cape_on_classic: bool,
    pub is_primary_user: bool,
    pub override_appearance: bool,
    pub trusted_skin_flag: String,
    pub profile_hash: String,
}

impl SkinData {
    /// Placeholder Steve-like skin. 64x32 is the legacy Steve size
    /// (`SkinImage::fromLegacy`). Filling it with opaque pixels keeps every
    /// client happy until real skins arrive; the full skin id is unique per
    /// player so clients never mix two players' appearances up. The
    /// geometry is the vanilla `geometry.humanoid.custom` so the client
    /// geometry file validates (see `STEVE_GEOMETRY_DATA`).
    pub fn default_for(uuid: &[u8; 16]) -> Self {
        let width = 64u32;
        let height = 32u32;
        let mut data = vec![0u8; (width * height * 4) as usize];
        // Classic Steve palette approximation: skin tone base, brown hair on
        // the head rows, teal torso, blue legs. Exact texels do not matter for
        // parity; being opaque 64x32 RGBA does.
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                let (r, g, b) = if y < 8 {
                    // Head / hair
                    if y < 4 && x >= 8 && x < 56 {
                        (88, 58, 28)
                    } else {
                        (198, 145, 112)
                    }
                } else if y < 20 {
                    // Torso (teal)
                    (0, 168, 168)
                } else {
                    // Legs (blue)
                    (40, 48, 168)
                };
                data[idx] = r;
                data[idx + 1] = g;
                data[idx + 2] = b;
                data[idx + 3] = 0xFF;
            }
        }
        Self {
            skin_id: "pm_rust_steve".to_string(),
            resource_patch: STEVE_RESOURCE_PATCH.to_string(),
            skin_image: SkinImage::new(width, height, data),
            animations: Vec::new(),
            cape_image: SkinImage::new(0, 0, Vec::new()),
            geometry_data: STEVE_GEOMETRY_DATA.to_string(),
            geometry_version: STEVE_GEOMETRY_VERSION.to_string(),
            animation_data: String::new(),
            cape_id: String::new(),
            full_skin_id: uuid_to_string(uuid),
            arm_size: 1, // wide (Steve)
            skin_color: 0xFFFFC890,
            persona_pieces: Vec::new(),
            piece_tint_colors: Vec::new(),
            premium: false,
            persona: false,
            cape_on_classic: false,
            is_primary_user: true,
            override_appearance: true,
            trusted_skin_flag: "true".to_string(),
            profile_hash: String::new(),
        }
    }

    pub fn encode(&self, writer: &mut Writer) {
        writer.write_string(&self.skin_id);
        writer.write_string(&self.resource_patch);
        Self::write_skin_image(writer, &self.skin_image);
        writer.write_var_u32(self.animations.len() as u32);
        for anim in &self.animations {
            Self::write_skin_image(writer, &anim.image);
            writer.write_var_u32(anim.anim_type);
            writer.write_f32_le(anim.frames);
            writer.write_var_u32(anim.expression_type);
        }
        Self::write_skin_image(writer, &self.cape_image);
        writer.write_string(&self.geometry_data);
        writer.write_string(&self.geometry_version);
        writer.write_string(&self.animation_data);
        writer.write_string(&self.cape_id);
        writer.write_string(&self.full_skin_id);
        writer.write_byte(self.arm_size);
        writer.write_u32_le(self.skin_color);
        writer.write_var_u32(self.persona_pieces.len() as u32);
        for piece in &self.persona_pieces {
            writer.write_string(&piece.piece_id);
            writer.write_u32_le(piece.piece_type);
            write_uuid(writer, &piece.pack_id);
            writer.write_bool(piece.is_default_piece);
            writer.write_string(&piece.product_id);
        }
        writer.write_var_u32(self.piece_tint_colors.len() as u32);
        for tint in &self.piece_tint_colors {
            writer.write_string(&tint.piece_type);
            for color in tint.colors {
                writer.write_u32_le(color);
            }
        }
        writer.write_bool(self.premium);
        writer.write_bool(self.persona);
        writer.write_bool(self.cape_on_classic);
        writer.write_bool(self.is_primary_user);
        writer.write_bool(self.override_appearance);
        writer.write_string(&self.trusted_skin_flag);
        writer.write_string(&self.profile_hash);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let skin_id = reader.read_string()?;
        let resource_patch = reader.read_string()?;
        let skin_image = Self::read_skin_image(reader)?;
        let anim_count = reader.read_var_u32()?;
        let mut animations = Vec::with_capacity(anim_count as usize);
        for _ in 0..anim_count {
            let image = Self::read_skin_image(reader)?;
            let anim_type = reader.read_var_u32()?;
            let frames = reader.read_f32_le()?;
            let expression_type = reader.read_var_u32()?;
            animations.push(SkinAnimation {
                image,
                anim_type,
                frames,
                expression_type,
            });
        }
        let cape_image = Self::read_skin_image(reader)?;
        let geometry_data = reader.read_string()?;
        let geometry_version = reader.read_string()?;
        let animation_data = reader.read_string()?;
        let cape_id = reader.read_string()?;
        let full_skin_id = reader.read_string()?;
        let arm_size = reader.read_byte()?;
        let skin_color = reader.read_u32_le()?;
        let persona_count = reader.read_var_u32()?;
        let mut persona_pieces = Vec::with_capacity(persona_count as usize);
        for _ in 0..persona_count {
            let piece_id = reader.read_string()?;
            let piece_type = reader.read_u32_le()?;
            let pack_id = read_uuid(reader)?;
            let is_default_piece = reader.read_bool()?;
            let product_id = reader.read_string()?;
            persona_pieces.push(PersonaSkinPiece {
                piece_id,
                piece_type,
                pack_id,
                is_default_piece,
                product_id,
            });
        }
        let tint_count = reader.read_var_u32()?;
        let mut piece_tint_colors = Vec::with_capacity(tint_count as usize);
        for _ in 0..tint_count {
            let piece_type = reader.read_string()?;
            let mut colors = [0u32; 4];
            for color in &mut colors {
                *color = reader.read_u32_le()?;
            }
            piece_tint_colors.push(PersonaPieceTintColor {
                piece_type,
                colors,
            });
        }
        let premium = reader.read_bool()?;
        let persona = reader.read_bool()?;
        let cape_on_classic = reader.read_bool()?;
        let is_primary_user = reader.read_bool()?;
        let override_appearance = reader.read_bool()?;
        let trusted_skin_flag = reader.read_string()?;
        let profile_hash = reader.read_string()?;
        Ok(Self {
            skin_id,
            resource_patch,
            skin_image,
            animations,
            cape_image,
            geometry_data,
            geometry_version,
            animation_data,
            cape_id,
            full_skin_id,
            arm_size,
            skin_color,
            persona_pieces,
            piece_tint_colors,
            premium,
            persona,
            cape_on_classic,
            is_primary_user,
            override_appearance,
            trusted_skin_flag,
            profile_hash,
        })
    }

    fn write_skin_image(writer: &mut Writer, image: &SkinImage) {
        writer.write_u32_le(image.width);
        writer.write_u32_le(image.height);
        writer.write_var_u32(image.data.len() as u32);
        writer.write_bytes(&image.data);
    }

    fn read_skin_image(reader: &mut Reader) -> Result<SkinImage, CodecError> {
        let width = reader.read_u32_le()?;
        let height = reader.read_u32_le()?;
        let len = reader.read_var_u32()? as usize;
        let data = reader.read_bytes(len)?.to_vec();
        Ok(SkinImage {
            width,
            height,
            data,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerListEntry {
    pub entry_type: u32,
    pub uuid: [u8; 16],
    pub actor_unique_id: i64,
    pub username: String,
    pub xbox_user_id: String,
    pub playfab_id: String,
    pub platform_chat_id: String,
    pub build_platform: i32,
    pub skin_data: SkinData,
    pub is_teacher: bool,
    pub is_host: bool,
    pub is_sub_client: bool,
    pub color: u32,
}

impl PlayerListEntry {
    pub fn addition(
        uuid: [u8; 16],
        actor_unique_id: i64,
        username: impl Into<String>,
        skin_data: SkinData,
        xbox_user_id: impl Into<String>,
    ) -> Self {
        Self {
            entry_type: PlayerListPacket::TYPE_ADD,
            uuid,
            actor_unique_id,
            username: username.into(),
            xbox_user_id: xbox_user_id.into(),
            playfab_id: String::new(),
            platform_chat_id: String::new(),
            build_platform: -1,
            skin_data,
            is_teacher: false,
            is_host: false,
            is_sub_client: false,
            color: 0xFFFFFFFF,
        }
    }

    pub fn removal(uuid: [u8; 16]) -> Self {
        Self {
            entry_type: PlayerListPacket::TYPE_REMOVE,
            uuid,
            actor_unique_id: 0,
            username: String::new(),
            xbox_user_id: String::new(),
            playfab_id: String::new(),
            platform_chat_id: String::new(),
            build_platform: -1,
            skin_data: SkinData::default_for(&uuid),
            is_teacher: false,
            is_host: false,
            is_sub_client: false,
            color: 0xFFFFFFFF,
        }
    }
}

/// Packet ID 0x3F (63): PlayerListPacket.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlayerListPacket {
    pub entries: Vec<PlayerListEntry>,
}

impl PlayerListPacket {
    pub const PACKET_ID: u32 = 0x3F;
    pub const TYPE_REMOVE: u32 = 0;
    pub const TYPE_ADD: u32 = 1;

    pub fn addition(entry: PlayerListEntry) -> Self {
        Self {
            entries: vec![entry],
        }
    }

    pub fn with_additions(entries: Vec<PlayerListEntry>) -> Self {
        Self { entries }
    }

    pub fn removal(uuid: [u8; 16]) -> Self {
        Self {
            entries: vec![PlayerListEntry::removal(uuid)],
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u32(self.entries.len() as u32);
        for entry in &self.entries {
            writer.write_var_u32(entry.entry_type);
            let inner: u8 = match entry.entry_type {
                Self::TYPE_ADD => 0,
                Self::TYPE_REMOVE => 1,
                _ => 0,
            };
            writer.write_byte(inner);
            if entry.entry_type == Self::TYPE_ADD {
                write_uuid(writer, &entry.uuid);
                writer.write_var_i64(entry.actor_unique_id);
                writer.write_string(&entry.username);
                writer.write_string(&entry.xbox_user_id);
                writer.write_string(&entry.playfab_id);
                writer.write_string(&entry.platform_chat_id);
                writer.write_i32_le(entry.build_platform);
                entry.skin_data.encode(writer);
                writer.write_bool(entry.is_teacher);
                writer.write_bool(entry.is_host);
                writer.write_bool(entry.is_sub_client);
                writer.write_u32_le(entry.color);
            } else {
                write_uuid(writer, &entry.uuid);
            }
        }
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let count = reader.read_var_u32()?;
        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let entry_type = reader.read_var_u32()?;
            let _inner = reader.read_byte()?;
            if entry_type == Self::TYPE_ADD {
                let uuid = read_uuid(reader)?;
                let actor_unique_id = reader.read_var_i64()?;
                let username = reader.read_string()?;
                let xbox_user_id = reader.read_string()?;
                let playfab_id = reader.read_string()?;
                let platform_chat_id = reader.read_string()?;
                let build_platform = reader.read_i32_le()?;
                let skin_data = SkinData::decode(reader)?;
                let is_teacher = reader.read_bool()?;
                let is_host = reader.read_bool()?;
                let is_sub_client = reader.read_bool()?;
                let color = reader.read_u32_le()?;
                entries.push(PlayerListEntry {
                    entry_type,
                    uuid,
                    actor_unique_id,
                    username,
                    xbox_user_id,
                    playfab_id,
                    platform_chat_id,
                    build_platform,
                    skin_data,
                    is_teacher,
                    is_host,
                    is_sub_client,
                    color,
                });
            } else if entry_type == Self::TYPE_REMOVE {
                let uuid = read_uuid(reader)?;
                entries.push(PlayerListEntry::removal(uuid));
            } else {
                return Err(CodecError::Custom(format!(
                    "Unknown player list entry type {entry_type}"
                )));
            }
        }
        Ok(Self { entries })
    }
}

impl Packet for PlayerListPacket {
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
    fn uuid_codec_roundtrips() {
        let uuid = uuid_from_str("12345678-1234-1234-1234-1234567890ab").unwrap();
        let mut writer = Writer::new();
        write_uuid(&mut writer, &uuid);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(read_uuid(&mut reader).unwrap(), uuid);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn addition_roundtrips() {
        let uuid = [1u8; 16];
        let skin = SkinData::default_for(&uuid);
        let packet = PlayerListPacket::addition(PlayerListEntry::addition(
            uuid,
            42,
            "Steve",
            skin.clone(),
            "2535",
        ));
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = PlayerListPacket::decode_payload(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(decoded.entries[0].username, "Steve");
        assert_eq!(decoded.entries[0].actor_unique_id, 42);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn removal_roundtrips() {
        let uuid = [7u8; 16];
        let packet = PlayerListPacket::removal(uuid);
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = PlayerListPacket::decode_payload(&mut reader).expect("decode");
        assert_eq!(decoded.entries.len(), 1);
        assert_eq!(decoded.entries[0].entry_type, PlayerListPacket::TYPE_REMOVE);
        assert_eq!(decoded.entries[0].uuid, uuid);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn default_skin_geometry_validates() {
        // Bedrock validates the skin geometry file: `geometry_data` must be
        // a geometry JSON whose identifier matches the name referenced by
        // `resource_patch`, with `geometry_version` matching
        // `format_version`. This blocks the "geometry file didn't validate"
        // client error caused by the old `"{}"` placeholder.
        let skin = SkinData::default_for(&[3u8; 16]);

        let patch: serde_json::Value =
            serde_json::from_str(&skin.resource_patch).expect("resource_patch parses");
        assert_eq!(
            patch["geometry"]["default"],
            serde_json::Value::String(STEVE_GEOMETRY_NAME.to_string())
        );

        let geometry: serde_json::Value =
            serde_json::from_str(&skin.geometry_data).expect("geometry_data parses");
        assert_eq!(
            geometry["format_version"],
            serde_json::Value::String(skin.geometry_version.clone())
        );
        let entries = geometry["minecraft:geometry"]
            .as_array()
            .expect("minecraft:geometry is an array");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0]["description"]["identifier"],
            serde_json::Value::String(STEVE_GEOMETRY_NAME.to_string())
        );
        let bones = entries[0]["bones"].as_array().expect("bones");
        let names: Vec<&str> = bones
            .iter()
            .map(|bone| bone["name"].as_str().expect("bone has name"))
            .collect();
        for required in ["head", "body", "leftArm", "rightArm", "leftLeg", "rightLeg"] {
            assert!(names.contains(&required), "missing bone {required}");
        }
        // Pivot-only bones (root, waist, cape, item holders) carry no cubes;
        // every rendered bone must define at least one well-formed cube.
        let mut cubed_bones = 0;
        for bone in bones {
            if let Some(cubes) = bone.get("cubes").and_then(|c| c.as_array()) {
                assert!(!cubes.is_empty(), "bone must define at least one cube");
                cubed_bones += 1;
                for cube in cubes {
                    assert!(cube["origin"].is_array());
                    assert!(cube["size"].is_array());
                    assert!(cube["uv"].is_array());
                }
            }
        }
        assert!(cubed_bones >= 6, "core bones must render");
        // Legacy Steve texture: opaque 64x32.
        assert_eq!((skin.skin_image.width, skin.skin_image.height), (64, 32));
        assert_eq!(skin.skin_image.data.len(), 64 * 32 * 4);
    }
}
