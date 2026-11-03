use crate::binary::{CodecError, Reader, Writer};
use crate::types::{BlockPos, Vector3f};
use crate::Packet;

/// Settings describing world generation, spawn, rules, and gamemode for StartGamePacket.
/// Faithful to Bedrock protocol 2225 (1.26.60) / Axolotl LevelSettings.php.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelSettings {
    pub seed: u64,
    pub spawn_biome_type: i16,
    pub spawn_biome_name: String,
    pub dimension: i32,
    pub generator_type: i32,
    pub world_gamemode: i32,
    pub hardcore: bool,
    pub difficulty: i32,
    pub spawn_position: BlockPos,
    pub has_achievements_disabled: bool,
    pub editor_world_type: i32,
    pub created_in_editor_mode: bool,
    pub exported_from_editor_mode: bool,
    pub editor_level_migration_version: i8,
    pub time: i32,
    pub edu_edition_offer: u32,
    pub has_edu_features_enabled: bool,
    pub edu_product_uuid: String,
    pub rain_level: f32,
    pub lightning_level: f32,
    pub has_confirmed_platform_locked_content: bool,
    pub is_multiplayer_game: bool,
    pub has_lan_broadcast: bool,
    pub xbox_live_broadcast_mode: i32,
    pub platform_broadcast_mode: i32,
    pub commands_enabled: bool,
    pub is_texture_packs_required: bool,
    pub experiments: Vec<(String, bool)>,
    pub experiments_previously_used: bool,
    pub has_bonus_chest_enabled: bool,
    pub has_start_with_map_enabled: bool,
    pub default_player_permission: i8,
    pub server_chunk_tick_radius: i32,
    pub has_locked_behavior_pack: bool,
    pub has_locked_resource_pack: bool,
    pub is_from_locked_world_template: bool,
    pub use_msa_gamertags_only: bool,
    pub is_from_world_template: bool,
    pub is_world_template_option_locked: bool,
    pub only_spawn_v1_villagers: bool,
    pub disable_persona: bool,
    pub disable_custom_skins: bool,
    pub mute_emote_announcements: bool,
    pub vanilla_version: String,
    pub limited_world_width: i32,
    pub limited_world_length: i32,
    pub is_new_nether: bool,
    pub edu_resource_button_name: String,
    pub edu_resource_link_uri: String,
    pub experimental_gameplay_override: Option<bool>,
    pub chat_restriction_level: u8,
    pub disable_player_interactions: bool,
    pub server_editor_connection_policy: i32,
    pub allow_anonymous_block_drops_in_editor_worlds: bool,
}

impl Default for LevelSettings {
    fn default() -> Self {
        Self {
            seed: 0,
            spawn_biome_type: 0,
            spawn_biome_name: "minecraft:plains".to_string(),
            dimension: 0,
            generator_type: 1, // 1 = infinite overworld
            world_gamemode: 0, // 0 = survival
            hardcore: false,
            difficulty: 1,     // 1 = easy
            spawn_position: BlockPos::new(0, 64, 0),
            has_achievements_disabled: true,
            editor_world_type: 0,
            created_in_editor_mode: false,
            exported_from_editor_mode: false,
            editor_level_migration_version: 0,
            time: -1,
            edu_edition_offer: 0,
            has_edu_features_enabled: false,
            edu_product_uuid: String::new(),
            rain_level: 0.0,
            lightning_level: 0.0,
            has_confirmed_platform_locked_content: false,
            is_multiplayer_game: true,
            has_lan_broadcast: true,
            xbox_live_broadcast_mode: 6,
            platform_broadcast_mode: 6,
            commands_enabled: true,
            is_texture_packs_required: false,
            experiments: Vec::new(),
            experiments_previously_used: false,
            has_bonus_chest_enabled: false,
            has_start_with_map_enabled: false,
            default_player_permission: 1, // 1 = member
            server_chunk_tick_radius: 4,
            has_locked_behavior_pack: false,
            has_locked_resource_pack: false,
            is_from_locked_world_template: false,
            use_msa_gamertags_only: false,
            is_from_world_template: false,
            is_world_template_option_locked: false,
            only_spawn_v1_villagers: false,
            disable_persona: false,
            disable_custom_skins: false,
            mute_emote_announcements: false,
            vanilla_version: "1.26.60".to_string(),
            limited_world_width: 16,
            limited_world_length: 16,
            is_new_nether: true,
            edu_resource_button_name: String::new(),
            edu_resource_link_uri: String::new(),
            experimental_gameplay_override: None,
            chat_restriction_level: 0,
            disable_player_interactions: false,
            server_editor_connection_policy: 0,
            allow_anonymous_block_drops_in_editor_worlds: false,
        }
    }
}

impl LevelSettings {
    pub fn encode(&self, writer: &mut Writer) {
        writer.write_u64_le(self.seed);
        writer.write_i16_le(self.spawn_biome_type);
        writer.write_string(&self.spawn_biome_name);
        writer.write_var_i32(self.dimension);
        writer.write_var_i32(self.generator_type);
        writer.write_var_i32(self.world_gamemode);
        writer.write_bool(self.hardcore);
        writer.write_var_i32(self.difficulty);
        self.spawn_position.write(writer);
        writer.write_bool(self.has_achievements_disabled);
        writer.write_var_i32(self.editor_world_type);
        writer.write_bool(self.created_in_editor_mode);
        writer.write_bool(self.exported_from_editor_mode);
        writer.write_i8(self.editor_level_migration_version);
        writer.write_var_i32(self.time);
        writer.write_var_u32(self.edu_edition_offer);
        writer.write_bool(self.has_edu_features_enabled);
        writer.write_string(&self.edu_product_uuid);
        writer.write_f32_le(self.rain_level);
        writer.write_f32_le(self.lightning_level);
        writer.write_bool(self.has_confirmed_platform_locked_content);
        writer.write_bool(self.is_multiplayer_game);
        writer.write_bool(self.has_lan_broadcast);
        writer.write_var_i32(self.xbox_live_broadcast_mode);
        writer.write_var_i32(self.platform_broadcast_mode);
        writer.write_bool(self.commands_enabled);
        writer.write_bool(self.is_texture_packs_required);
        // Game rules count (0 = none)
        writer.write_var_u32(0);
        // Experiments
        writer.write_u32_le(self.experiments.len() as u32);
        for (name, enabled) in &self.experiments {
            writer.write_string(name);
            writer.write_bool(*enabled);
        }
        writer.write_bool(self.experiments_previously_used);
        writer.write_bool(self.has_bonus_chest_enabled);
        writer.write_bool(self.has_start_with_map_enabled);
        writer.write_i8(self.default_player_permission);
        writer.write_i32_le(self.server_chunk_tick_radius);
        writer.write_bool(self.has_locked_behavior_pack);
        writer.write_bool(self.has_locked_resource_pack);
        writer.write_bool(self.is_from_locked_world_template);
        writer.write_bool(self.use_msa_gamertags_only);
        writer.write_bool(self.is_from_world_template);
        writer.write_bool(self.is_world_template_option_locked);
        writer.write_bool(self.only_spawn_v1_villagers);
        writer.write_bool(self.disable_persona);
        writer.write_bool(self.disable_custom_skins);
        writer.write_bool(self.mute_emote_announcements);
        writer.write_string(&self.vanilla_version);
        writer.write_i32_le(self.limited_world_width);
        writer.write_i32_le(self.limited_world_length);
        writer.write_bool(self.is_new_nether);
        writer.write_string(&self.edu_resource_button_name);
        writer.write_string(&self.edu_resource_link_uri);
        match self.experimental_gameplay_override {
            Some(v) => {
                writer.write_bool(true);
                writer.write_bool(v);
            }
            None => writer.write_bool(false),
        }
        writer.write_byte(self.chat_restriction_level);
        writer.write_bool(self.disable_player_interactions);
        writer.write_var_i32(self.server_editor_connection_policy);
        writer.write_bool(self.allow_anonymous_block_drops_in_editor_worlds);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let seed = reader.read_u64_le()?;
        let spawn_biome_type = reader.read_i16_le()?;
        let spawn_biome_name = reader.read_string()?;
        let dimension = reader.read_var_i32()?;
        let generator_type = reader.read_var_i32()?;
        let world_gamemode = reader.read_var_i32()?;
        let hardcore = reader.read_bool()?;
        let difficulty = reader.read_var_i32()?;
        let spawn_position = BlockPos::read(reader)?;
        let has_achievements_disabled = reader.read_bool()?;
        let editor_world_type = reader.read_var_i32()?;
        let created_in_editor_mode = reader.read_bool()?;
        let exported_from_editor_mode = reader.read_bool()?;
        let editor_level_migration_version = reader.read_i8()?;
        let time = reader.read_var_i32()?;
        let edu_edition_offer = reader.read_var_u32()?;
        let has_edu_features_enabled = reader.read_bool()?;
        let edu_product_uuid = reader.read_string()?;
        let rain_level = reader.read_f32_le()?;
        let lightning_level = reader.read_f32_le()?;
        let has_confirmed_platform_locked_content = reader.read_bool()?;
        let is_multiplayer_game = reader.read_bool()?;
        let has_lan_broadcast = reader.read_bool()?;
        let xbox_live_broadcast_mode = reader.read_var_i32()?;
        let platform_broadcast_mode = reader.read_var_i32()?;
        let commands_enabled = reader.read_bool()?;
        let is_texture_packs_required = reader.read_bool()?;
        let _rules_count = reader.read_var_u32()?;
        let exp_count = reader.read_u32_le()?;
        let mut experiments = Vec::with_capacity(exp_count as usize);
        for _ in 0..exp_count {
            let name = reader.read_string()?;
            let enabled = reader.read_bool()?;
            experiments.push((name, enabled));
        }
        let experiments_previously_used = reader.read_bool()?;
        let has_bonus_chest_enabled = reader.read_bool()?;
        let has_start_with_map_enabled = reader.read_bool()?;
        let default_player_permission = reader.read_i8()?;
        let server_chunk_tick_radius = reader.read_i32_le()?;
        let has_locked_behavior_pack = reader.read_bool()?;
        let has_locked_resource_pack = reader.read_bool()?;
        let is_from_locked_world_template = reader.read_bool()?;
        let use_msa_gamertags_only = reader.read_bool()?;
        let is_from_world_template = reader.read_bool()?;
        let is_world_template_option_locked = reader.read_bool()?;
        let only_spawn_v1_villagers = reader.read_bool()?;
        let disable_persona = reader.read_bool()?;
        let disable_custom_skins = reader.read_bool()?;
        let mute_emote_announcements = reader.read_bool()?;
        let vanilla_version = reader.read_string()?;
        let limited_world_width = reader.read_i32_le()?;
        let limited_world_length = reader.read_i32_le()?;
        let is_new_nether = reader.read_bool()?;
        let edu_resource_button_name = reader.read_string()?;
        let edu_resource_link_uri = reader.read_string()?;
        let has_override = reader.read_bool()?;
        let experimental_gameplay_override = if has_override {
            Some(reader.read_bool()?)
        } else {
            None
        };
        let chat_restriction_level = reader.read_byte()?;
        let disable_player_interactions = reader.read_bool()?;
        let server_editor_connection_policy = reader.read_var_i32()?;
        let allow_anonymous_block_drops_in_editor_worlds = reader.read_bool()?;

        Ok(Self {
            seed,
            spawn_biome_type,
            spawn_biome_name,
            dimension,
            generator_type,
            world_gamemode,
            hardcore,
            difficulty,
            spawn_position,
            has_achievements_disabled,
            editor_world_type,
            created_in_editor_mode,
            exported_from_editor_mode,
            editor_level_migration_version,
            time,
            edu_edition_offer,
            has_edu_features_enabled,
            edu_product_uuid,
            rain_level,
            lightning_level,
            has_confirmed_platform_locked_content,
            is_multiplayer_game,
            has_lan_broadcast,
            xbox_live_broadcast_mode,
            platform_broadcast_mode,
            commands_enabled,
            is_texture_packs_required,
            experiments,
            experiments_previously_used,
            has_bonus_chest_enabled,
            has_start_with_map_enabled,
            default_player_permission,
            server_chunk_tick_radius,
            has_locked_behavior_pack,
            has_locked_resource_pack,
            is_from_locked_world_template,
            use_msa_gamertags_only,
            is_from_world_template,
            is_world_template_option_locked,
            only_spawn_v1_villagers,
            disable_persona,
            disable_custom_skins,
            mute_emote_announcements,
            vanilla_version,
            limited_world_width,
            limited_world_length,
            is_new_nether,
            edu_resource_button_name,
            edu_resource_link_uri,
            experimental_gameplay_override,
            chat_restriction_level,
            disable_player_interactions,
            server_editor_connection_policy,
            allow_anonymous_block_drops_in_editor_worlds,
        })
    }
}

/// Packet ID 0x0B (11): StartGamePacket
/// Full Bedrock 1.26.60 / Axolotl-PM StartGamePacket implementation.
#[derive(Debug, Clone, PartialEq)]
pub struct StartGamePacket {
    pub actor_unique_id: i64,
    pub actor_runtime_id: u64,
    pub player_gamemode: i32,
    pub player_position: Vector3f,
    pub pitch: f32,
    pub yaw: f32,
    pub level_settings: LevelSettings,
    pub level_id: String,
    pub world_name: String,
    pub premium_world_template_id: String,
    pub is_trial: bool,
    pub rewind_history_size: i32,
    pub server_authoritative_block_breaking: bool,
    pub current_tick: u64,
    pub enchantment_seed: i32,
    pub multiplayer_correlation_id: String,
    pub enable_new_inventory_system: bool,
    pub server_software_version: String,
    pub player_actor_properties: Vec<u8>,
    pub block_palette_checksum: u64,
    pub world_template_id: [u8; 16],
    pub enable_client_side_chunk_generation: bool,
    pub block_network_ids_are_hashes: bool,
    pub disable_client_sounds: bool,
    pub server_identifier: String,
    pub scenario_identifier: String,
    pub world_identifier: String,
    pub owner_identifier: String,
}

impl Default for StartGamePacket {
    fn default() -> Self {
        Self {
            actor_unique_id: 1,
            actor_runtime_id: 1,
            player_gamemode: 0,
            player_position: Vector3f::new(0.0, 64.0, 0.0),
            pitch: 0.0,
            yaw: 0.0,
            level_settings: LevelSettings::default(),
            level_id: "Bedrock level".to_string(),
            world_name: "PocketMine-Rust World".to_string(),
            premium_world_template_id: String::new(),
            is_trial: false,
            rewind_history_size: 40,
            server_authoritative_block_breaking: true,
            current_tick: 0,
            enchantment_seed: 0,
            multiplayer_correlation_id: String::new(),
            enable_new_inventory_system: true,
            server_software_version: "PocketMine-Rust 1.0.0 (Axolotl-PM)".to_string(),
            player_actor_properties: vec![0x0a, 0x00, 0x00],
            block_palette_checksum: 0, // 0 skips client strict checksum validation (prevents error Boar)
            world_template_id: [0u8; 16],
            enable_client_side_chunk_generation: false,
            block_network_ids_are_hashes: false,
            disable_client_sounds: true,
            server_identifier: String::new(),
            scenario_identifier: String::new(),
            world_identifier: String::new(),
            owner_identifier: String::new(),
        }
    }
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
        writer.write_string(&self.level_id);
        writer.write_string(&self.world_name);
        writer.write_string(&self.premium_world_template_id);
        writer.write_bool(self.is_trial);
        writer.write_var_i32(self.rewind_history_size);
        writer.write_bool(self.server_authoritative_block_breaking);
        writer.write_u64_le(self.current_tick);
        writer.write_var_i32(self.enchantment_seed);
        // Block palette count (0)
        writer.write_var_u32(0);
        writer.write_string(&self.multiplayer_correlation_id);
        writer.write_bool(self.enable_new_inventory_system);
        writer.write_string(&self.server_software_version);
        // Empty Compound NBT
        writer.write_bytes(&self.player_actor_properties);
        writer.write_u64_le(self.block_palette_checksum);
        writer.write_bytes(&self.world_template_id);
        writer.write_bool(self.enable_client_side_chunk_generation);
        writer.write_bool(self.block_network_ids_are_hashes);
        writer.write_bool(self.disable_client_sounds);
        // server_join_info: false
        writer.write_bool(false);
        writer.write_string(&self.server_identifier);
        writer.write_string(&self.scenario_identifier);
        writer.write_string(&self.world_identifier);
        writer.write_string(&self.owner_identifier);
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
        let level_id = reader.read_string()?;
        let world_name = reader.read_string()?;
        let premium_world_template_id = reader.read_string()?;
        let is_trial = reader.read_bool()?;
        let rewind_history_size = reader.read_var_i32()?;
        let server_authoritative_block_breaking = reader.read_bool()?;
        let current_tick = reader.read_u64_le()?;
        let enchantment_seed = reader.read_var_i32()?;
        let palette_count = reader.read_var_u32()?;
        for _ in 0..palette_count {
            let _name = reader.read_string()?;
            // skip nbt compound
        }
        let multiplayer_correlation_id = reader.read_string()?;
        let enable_new_inventory_system = reader.read_bool()?;
        let server_software_version = reader.read_string()?;
        // NBT: 3 bytes minimum
        let nbt_b0 = reader.read_byte()?;
        let nbt_b1 = reader.read_byte()?;
        let nbt_b2 = reader.read_byte()?;
        let player_actor_properties = vec![nbt_b0, nbt_b1, nbt_b2];
        let block_palette_checksum = reader.read_u64_le()?;
        let mut world_template_id = [0u8; 16];
        world_template_id.copy_from_slice(reader.read_bytes(16)?);
        let enable_client_side_chunk_generation = reader.read_bool()?;
        let block_network_ids_are_hashes = reader.read_bool()?;
        let disable_client_sounds = reader.read_bool()?;
        let has_join_info = reader.read_bool()?;
        if has_join_info {
            let _has_gathering = reader.read_bool()?;
        }
        let server_identifier = reader.read_string()?;
        let scenario_identifier = reader.read_string()?;
        let world_identifier = reader.read_string()?;
        let owner_identifier = reader.read_string()?;

        Ok(Self {
            actor_unique_id,
            actor_runtime_id,
            player_gamemode,
            player_position,
            pitch,
            yaw,
            level_settings,
            level_id,
            world_name,
            premium_world_template_id,
            is_trial,
            rewind_history_size,
            server_authoritative_block_breaking,
            current_tick,
            enchantment_seed,
            multiplayer_correlation_id,
            enable_new_inventory_system,
            server_software_version,
            player_actor_properties,
            block_palette_checksum,
            world_template_id,
            enable_client_side_chunk_generation,
            block_network_ids_are_hashes,
            disable_client_sounds,
            server_identifier,
            scenario_identifier,
            world_identifier,
            owner_identifier,
        })
    }
}
