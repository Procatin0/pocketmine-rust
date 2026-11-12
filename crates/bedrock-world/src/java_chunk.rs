//! Java Edition chunk column encoder: Bedrock chunk -> Java column bytes.
//!
//! Port of `plugins/JavaBridge/src/javabridge/ModernChunk.php` and
//! `ChunkBuilder.php` (JavaBridge), plus the `ModernProfile.php` chunk
//! tables those two files need: per-protocol block states, plains biome
//! index, and the three wire-format flags that change between 1.21.4
//! (769) and 26.1 (775):
//!
//! - `heightmaps_as_nbt`: 1.21.4 (769) only; 770+ send a typed list.
//! - `palette_data_has_length`: 1.21.4 (769) only; 770+ deduce the long
//!   count from bits-per-entry and the entry count.
//! - `sections_have_fluid_count`: 26.1 (775) only; the `fluidCount` u16
//!   before each section drives `LevelChunkSection.hasFluid()` — with 0
//!   the client never examines the blocks and the player falls through
//!   water as if it were air.
//!
//! The bridge asks the Rust server for the real Bedrock column
//! (`world.get_chunk`, Bedrock's paletted-subchunk network format) and
//! translates it here without approximating: every block drawn by the
//! Java client sits where Bedrock has it, including player edits.
//!
//! Layout: 24 sections of 16x16x16 from Y = -64 (`MIN_Y`) to Y = 319.
//! Bedrock indexes a section `x << 8 | z << 4 | y`; Java indexes
//! `y << 8 | z << 4 | x`. The encoder walks in Java order and reads the
//! Bedrock index with the axes reordered — reading the same index in
//! both orders is exactly how a bridge ends up with X and Y swapped.

use std::sync::OnceLock;

use crate::chunk::Chunk;

/// First Y of the Java column (same as Bedrock's `MIN_Y`).
pub const MIN_Y: i32 = -64;
/// Column height in blocks.
pub const HEIGHT: i32 = 384;
/// Number of 16x16x16 sections in a column.
pub const SECTIONS: usize = 24;
/// Entries in a section palette container (16x16x16).
pub const BLOCK_ENTRIES: usize = 4096;
/// Light sections: the 24 column sections plus one below and one above.
pub const LIGHT_SECTIONS: usize = SECTIONS + 2;

/// Global Java block states, identical in 1.21.4 - 26.1 (see
/// `ModernChunk.php`). These are the *default* states: 8 is
/// `grass_block[snowy=true]` and 136 is the X-laid oak log, so the
/// world would render snowy with sideways trees if they were used.
pub mod java_state {
    pub const AIR: i32 = 0;
    pub const STONE: i32 = 1;
    pub const GRASS: i32 = 9;
    pub const DIRT: i32 = 10;
    pub const COBBLESTONE: i32 = 14;
    pub const BEDROCK: i32 = 85;
    pub const WATER: i32 = 86;
    pub const SAND: i32 = 118;
    pub const GRAVEL: i32 = 124;
    pub const GOLD_ORE: i32 = 129;
    pub const IRON_ORE: i32 = 131;
    pub const COAL_ORE: i32 = 133;
    pub const OAK_LOG: i32 = 137;
    pub const SPRUCE_LOG: i32 = 140;
    pub const BIRCH_LOG: i32 = 143;
    pub const OAK_LOG_X: i32 = 136;
    pub const BIRCH_LOG_Z: i32 = 144;
    pub const OAK_LEAVES: i32 = 279;
    pub const SPRUCE_LEAVES: i32 = 307;
    pub const BIRCH_LEAVES: i32 = 335;
    pub const LAPIS_ORE: i32 = 563;
    pub const SANDSTONE: i32 = 578;
}

/// Bedrock network ids (FNV-1a hashes, see `block_id.rs`) paired with the
/// Java block name, in the exact order of `ModernChunk.php`
/// `BEDROCK_BLOCKS`. Order matters: `block_name_for_java_state` returns
/// the first match.
pub const BEDROCK_BLOCKS: &[(u32, &str)] = &[
    (3690217760, "air"),
    (2150698529, "stone"),
    (2186211206, "dirt"),
    (3727763636, "grass_block"),
    (4121722107, "bedrock"),
    (138639715, "sand"),
    (1529044762, "gravel"),
    (1211861802, "water"),
    (2754680827, "sandstone"),
    (825916963, "oak_log"),
    (2110714365, "oak_leaves"),
    (507450469, "spruce_log"),
    (1404704691, "spruce_leaves"),
    (1423805714, "birch_log"),
    (1740500456, "birch_leaves"),
    (685383673, "coal_ore"),
    (2032622302, "iron_ore"),
    (2144082742, "gold_ore"),
    (2930261704, "lapis_ore"),
    (1419890941, "diamond_ore"),
    (180213920, "redstone_ore"),
    (170572189, "emerald_ore"),
    (3699616834, "snow"),
    (2827903781, "short_grass"),
    (4175527249, "tall_grass"),
    (2383143704, "dandelion"),
    (3417975282, "poppy"),
];

/// Chunk-relevant slice of `ModernProfile.php`: protocol versions,
/// block-state tables and plains biome index. Packet ids, registries
/// and tags stay with the protocol server; they are not needed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModernProfile {
    /// Java protocol id (769 .. 775).
    pub protocol: i32,
    /// Version name, e.g. "26.1".
    pub name: &'static str,
}

/// Global block states with the same id in 1.21.4 and 26.1
/// (`ModernProfile.php` `BLOCKS_STABLE`). Default states, not
/// `minStateId`: the first state of `grass_block` is `snowy=true` and
/// the first of `oak_log` is the X-laid trunk.
const BLOCKS_STABLE: &[(&str, i32)] = &[
    ("air", 0),
    ("stone", 1),
    ("granite", 2),
    ("diorite", 4),
    ("andesite", 6),
    ("grass_block", 9),
    ("dirt", 10),
    ("cobblestone", 14),
    ("bedrock", 85),
    ("water", 86),
    ("sand", 118),
    ("gravel", 124),
    ("gold_ore", 129),
    ("iron_ore", 131),
    ("coal_ore", 133),
    ("oak_log", 137),
    ("spruce_log", 140),
    ("birch_log", 143),
    ("oak_leaves", 279),
    ("spruce_leaves", 307),
    ("birch_leaves", 335),
    ("lapis_ore", 563),
    ("sandstone", 578),
];

/// Per-protocol overrides as `(protocol, name, state)` rows, kept flat
/// so the table stays readable next to the PHP original.
const BLOCKS_BY_PROTOCOL_ROWS: &[(i32, &str, i32)] = &[
    (769, "redstone_ore", 5904),
    (769, "diamond_ore", 4329),
    (769, "emerald_ore", 8285),
    (769, "short_grass", 2048),
    (769, "tall_grass", 11635),
    (769, "snow", 5941),
    (769, "dandelion", 2118),
    (769, "poppy", 2120),
    (770, "redstone_ore", 5913),
    (770, "diamond_ore", 4338),
    (770, "emerald_ore", 8295),
    (770, "short_grass", 2048),
    (770, "tall_grass", 11645),
    (770, "snow", 5950),
    (770, "dandelion", 2121),
    (770, "poppy", 2123),
    (773, "redstone_ore", 6681),
    (773, "diamond_ore", 5106),
    (773, "emerald_ore", 9372),
    (773, "short_grass", 2048),
    (773, "tall_grass", 12722),
    (773, "snow", 6718),
    (773, "dandelion", 2121),
    (773, "poppy", 2123),
    (774, "redstone_ore", 6681),
    (774, "diamond_ore", 5106),
    (774, "emerald_ore", 9372),
    (774, "short_grass", 2048),
    (774, "tall_grass", 12722),
    (774, "snow", 6718),
    (774, "dandelion", 2121),
    (774, "poppy", 2123),
    (775, "redstone_ore", 6882),
    (775, "diamond_ore", 5307),
    (775, "emerald_ore", 9573),
    (775, "short_grass", 2248),
    (775, "tall_grass", 12924),
    (775, "snow", 6919),
    (775, "dandelion", 2321),
    (775, "poppy", 2324),
];

impl ModernProfile {
    /// Profile for a protocol id, or `None` if the version is unknown.
    pub fn for_protocol(protocol: i32) -> Option<Self> {
        let name = match protocol {
            769 => "1.21.4",
            770 => "1.21.5",
            773 => "1.21.9",
            774 => "1.21.11",
            775 => "26.1",
            _ => return None,
        };
        Some(Self { protocol, name })
    }

    /// Whether this profile covers a protocol id.
    pub fn supports(protocol: i32) -> bool {
        Self::for_protocol(protocol).is_some()
    }

    /// Global Java state of a block by name for this version.
    /// `ModernProfile.php::blockState()`: version overrides first,
    /// then the stable table, unknown names fall back to air.
    pub fn block_state(&self, name: &str) -> i32 {
        // Full per-version block list first: the hand-written tables below
        // only name a few dozen blocks, and every other block coming from a
        // Bedrock player used to translate to air (invisible on the Java
        // client).
        if let Some(state) = crate::block_states::java_block_state(self.protocol, name) {
            return state;
        }
        for &(protocol, n, state) in BLOCKS_BY_PROTOCOL_ROWS {
            if protocol == self.protocol && n == name {
                return state;
            }
        }
        for &(n, state) in BLOCKS_STABLE {
            if n == name {
                return state;
            }
        }
        0
    }

    /// 1.21.4 sends heightmaps as NBT; 1.21.5+ as a typed list.
    pub fn heightmaps_as_nbt(&self) -> bool {
        self.protocol == 769
    }

    /// 1.21.5 removed the long-count prefix from palette data arrays:
    /// the client deduces it from bits-per-entry and entry count.
    pub fn palette_data_has_length(&self) -> bool {
        self.protocol == 769
    }

    /// 26.1 added a per-section fluid counter (drives `hasFluid()`).
    pub fn sections_have_fluid_count(&self) -> bool {
        self.protocol >= 775
    }

    /// Index of `minecraft:plains` in the `minecraft:worldgen/biome`
    /// registry of this version. The PHP original reads it from
    /// `resources/registry/<protocol>.json` (39 in 769-773, 40 in
    /// 774-775); index 0 is `minecraft:badlands`, so sending 0 would
    /// paint the world with badlands instead of plains.
    pub fn plains_biome(&self) -> i32 {
        if self.protocol >= 774 {
            40
        } else {
            39
        }
    }

    /// Index of `minecraft:badlands` (near `minecraft:plains` in the
    /// legacy id table: 37 vs 1, see `generator::biome`). Always 0:
    /// first registry entry in 769-775. Kept next to
    /// [`Self::plains_biome`] so the 0-is-badlands trap stays named.
    pub fn badlands_biome(&self) -> i32 {
        0
    }
}

/// Java Edition column encoder (1.18+ wire format).
pub struct ModernChunk;

impl ModernChunk {
    pub const MIN_Y: i32 = MIN_Y;
    pub const HEIGHT: i32 = HEIGHT;
    pub const SECTIONS: usize = SECTIONS;
    pub const AIR: i32 = java_state::AIR;
    pub const STONE: i32 = java_state::STONE;
    pub const GRASS: i32 = java_state::GRASS;
    pub const DIRT: i32 = java_state::DIRT;
    pub const COBBLESTONE: i32 = java_state::COBBLESTONE;
    pub const BEDROCK: i32 = java_state::BEDROCK;
    pub const WATER: i32 = java_state::WATER;
    pub const SAND: i32 = java_state::SAND;
    pub const GRAVEL: i32 = java_state::GRAVEL;
    pub const GOLD_ORE: i32 = java_state::GOLD_ORE;
    pub const IRON_ORE: i32 = java_state::IRON_ORE;
    pub const COAL_ORE: i32 = java_state::COAL_ORE;
    pub const OAK_LOG: i32 = java_state::OAK_LOG;
    pub const SPRUCE_LOG: i32 = java_state::SPRUCE_LOG;
    pub const BIRCH_LOG: i32 = java_state::BIRCH_LOG;
    pub const OAK_LOG_X: i32 = java_state::OAK_LOG_X;
    pub const BIRCH_LOG_Z: i32 = java_state::BIRCH_LOG_Z;
    pub const OAK_LEAVES: i32 = java_state::OAK_LEAVES;
    pub const SPRUCE_LEAVES: i32 = java_state::SPRUCE_LEAVES;
    pub const BIRCH_LEAVES: i32 = java_state::BIRCH_LEAVES;
    pub const LAPIS_ORE: i32 = java_state::LAPIS_ORE;
    pub const SANDSTONE: i32 = java_state::SANDSTONE;

    /// Global state for a short kind string (`ModernChunk.php::state()`).
    pub fn state(kind: &str) -> i32 {
        match kind {
            "air" => Self::AIR,
            "stone" => Self::STONE,
            "grass" => Self::GRASS,
            "dirt" => Self::DIRT,
            "cobblestone" => Self::COBBLESTONE,
            "bedrock" => Self::BEDROCK,
            "water" => Self::WATER,
            "sand" => Self::SAND,
            "gravel" => Self::GRAVEL,
            "log" => Self::OAK_LOG,
            "leaves" => Self::OAK_LEAVES,
            "sandstone" => Self::SANDSTONE,
            _ => Self::AIR,
        }
    }

    /// Java block name for a Bedrock network id, or `None` if unknown.
    ///
    /// The hand-written table above only names a few dozen blocks, so every
    /// other id falls back to the full canonical-state table
    /// ([`crate::block_states`]): without it a block placed by a Bedrock
    /// player arrived as `kind=block` / Java state 0 (air) and was invisible.
    pub fn block_name(bedrock_id: u32) -> Option<&'static str> {
        // Canonical table first: one hash lookup covers every real block, so
        // the chunk hot path (one call per block) never walks the short
        // hand-written list; that list is only the fallback for the few ids
        // that are not canonical states.
        crate::block_states::state_name(bedrock_id).or_else(|| {
            BEDROCK_BLOCKS
                .iter()
                .find(|(id, _)| *id == bedrock_id)
                .map(|(_, name)| *name)
        })
    }

    /// Java state equivalent to a Bedrock network id for this version.
    /// Unknown ids map to `air`, like the PHP original.
    pub fn bedrock_state(bedrock_id: u32, profile: &ModernProfile) -> i32 {
        // One name lookup per block; connectable blocks (fences, panes, bars,
        // walls) then map to the exact Java variant with its connections
        // (`data/java/connections/<protocol>.json`) instead of the block
        // default, which is what made a placed fence render without arms.
        let name = Self::block_name(bedrock_id).unwrap_or("air");
        if let Some(state) =
            crate::block_states::java_connection_state_for_name(profile.protocol, bedrock_id, name)
        {
            return state;
        }
        profile.block_state(name)
    }

    /// Bedrock network id for a Java block name, or `None`.
    ///
    /// Falls back to the canonical-state table for blocks outside the
    /// hand-written one, so a Java player can place any block.
    pub fn bedrock_network_id_for_name(name: &str) -> Option<u32> {
        BEDROCK_BLOCKS
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(id, _)| *id)
            .or_else(|| crate::block_states::network_id(name))
    }

    /// Java block name for a global Java state under this version, or
    /// `None`: the inverse of `ModernProfile::block_state`, used to
    /// translate what a modern client wants to place.
    pub fn block_name_for_java_state(state: i32, profile: &ModernProfile) -> Option<&'static str> {
        BEDROCK_BLOCKS
            .iter()
            .find(|(_, name)| profile.block_state(name) == state)
            .map(|(_, name)| *name)
    }

    /// Bedrock network id for a global Java state, or `None`.
    pub fn bedrock_network_id_for_java_state(state: i32, profile: &ModernProfile) -> Option<u32> {
        let name = Self::block_name_for_java_state(state, profile)?;
        Self::bedrock_network_id_for_name(name)
    }

    /// Whether a global Java state carries a non-empty fluid (water
    /// 86-101 or lava 102-117, in every version).
    ///
    /// In 26.1 the client decides if a section has fluid from the
    /// packet's `fluidCount` (`LevelChunkSection.hasFluid()`): with 0
    /// it never examines the blocks and the player falls through water
    /// as if it were air, even when the palette is correct.
    ///
    /// Number spaces: `86..=117` are *block states*. The login tags
    /// `minecraft:fluid` `water == [1, 2]` / `lava == [3, 4]` (see
    /// `java-protocol` `ModernProfile::tag_members`) are *fluid
    /// registry* ids (`empty=0, water=1, flowing_water=2, lava=3,
    /// flowing_lava=4`) — different space, so never mix them into a
    /// palette or a `fluidCount`.
    pub fn is_fluid(state: i32) -> bool {
        (86..=117).contains(&state)
    }

    /// Java `hasFluid()` for a section: `fluidCount > 0`.
    pub fn has_fluid(fluid_count: u16) -> bool {
        fluid_count > 0
    }

    /// Chunk data: per section, block counter, fluid counter (26.1
    /// only), translated state palette and plains biome container. No
    /// block entities.
    pub fn from_chunk(chunk: &Chunk, profile: &ModernProfile) -> Vec<u8> {
        let (data, _report) = Self::from_chunk_with_fluid_report(chunk, profile);
        data
    }

    /// Like [`Self::from_chunk`], plus the per-section fluid report the
    /// bridge logs for debugging (`fluidCount` must be > 0 where there
    /// is water and the client's `hasFluid()` looks at it).
    pub fn from_chunk_with_fluid_report(
        chunk: &Chunk,
        profile: &ModernProfile,
    ) -> (Vec<u8>, [u16; SECTIONS]) {
        let fluid_count = profile.sections_have_fluid_count();
        let data_length = profile.palette_data_has_length();
        let plains = profile.plains_biome();
        let mut report = [0u16; SECTIONS];
        let mut data = Vec::new();

        for section in 0..SECTIONS {
            let base_y = MIN_Y + (section as i32) * 16;
            let Some(payload) = section_payload(chunk, base_y) else {
                data.extend_from_slice(&0u16.to_be_bytes());
                if fluid_count {
                    data.extend_from_slice(&0u16.to_be_bytes());
                }
                data.extend_from_slice(&single(Self::AIR, data_length));
                data.extend_from_slice(&biomes(data_length, plains));
                continue;
            };

            // Translate the Bedrock palette to Java states, keeping
            // first-seen order; `slots` maps each Bedrock palette index
            // to its Java palette slot.
            let mut states: Vec<i32> = Vec::with_capacity(payload.palette.len());
            let mut java_palette: Vec<i32> = Vec::new();
            let mut slots: Vec<usize> = Vec::with_capacity(payload.palette.len());
            for &id in &payload.palette {
                let state = Self::bedrock_state(id, profile);
                states.push(state);
                let slot = match java_palette.iter().position(|&s| s == state) {
                    Some(slot) => slot,
                    None => {
                        java_palette.push(state);
                        java_palette.len() - 1
                    }
                };
                slots.push(slot);
            }

            if java_palette.len() == 1 {
                let only = java_palette[0];
                let fluid_single = if Self::is_fluid(only) {
                    BLOCK_ENTRIES as u16
                } else {
                    0
                };
                report[section] = fluid_single;
                let block_count = if only == Self::AIR {
                    0u16
                } else {
                    BLOCK_ENTRIES as u16
                };
                data.extend_from_slice(&block_count.to_be_bytes());
                if fluid_count {
                    data.extend_from_slice(&fluid_single.to_be_bytes());
                }
                data.extend_from_slice(&single(only, data_length));
                data.extend_from_slice(&biomes(data_length, plains));
                continue;
            }

            let mut out_bits = 4usize;
            while (1usize << out_bits) < java_palette.len() {
                out_bits += 1;
            }
            let in_per_word = (32usize / (payload.bits.max(1) as usize)).max(1);
            let in_mask: u64 = if payload.bits == 0 {
                0
            } else {
                (1u64 << payload.bits) - 1
            };
            let out_per_long = 64 / out_bits;
            let longs = BLOCK_ENTRIES.div_ceil(out_per_long);
            let mut packed = vec![0u64; longs];
            let mut count: u16 = 0;
            let mut fluids: u16 = 0;

            // Walk in Java order (`y << 8 | z << 4 | x`); each Java entry
            // reads the Bedrock index with the axes reordered
            // (`x << 8 | z << 4 | y`).
            for entry in 0..BLOCK_ENTRIES {
                let source = ((entry & 0x0F) << 8) | (entry & 0xF0) | ((entry >> 8) & 0x0F);
                let slot_index = if payload.bits == 0 {
                    0
                } else {
                    let word = payload.words[source / in_per_word] as u64;
                    let per = (source % in_per_word) * payload.bits as usize;
                    ((word >> per) & in_mask) as usize
                };
                let state = states.get(slot_index).copied().unwrap_or(Self::AIR);
                if state != Self::AIR {
                    count += 1;
                }
                if Self::is_fluid(state) {
                    fluids += 1;
                }
                packed[entry / out_per_long] |=
                    (slots[slot_index] as u64) << ((entry % out_per_long) * out_bits);
            }

            report[section] = fluids;
            data.extend_from_slice(&count.to_be_bytes());
            if fluid_count {
                data.extend_from_slice(&fluids.to_be_bytes());
            }
            data.push(out_bits as u8);
            data.extend_from_slice(&var_int(java_palette.len() as i32));
            for &state in &java_palette {
                data.extend_from_slice(&var_int(state));
            }
            if data_length {
                data.extend_from_slice(&var_int(longs as i32));
            }
            for &word in &packed {
                data.extend_from_slice(&word.to_be_bytes());
            }
            data.extend_from_slice(&biomes(data_length, plains));
        }

        (data, report)
    }

    /// Real heights of the 256 columns, for the heightmaps.
    /// Index `z*16+x`, in Bedrock Y.
    pub fn heights_from_chunk(chunk: &Chunk) -> [i32; 256] {
        let mut heights = [MIN_Y; 256];
        for z in 0..16 {
            for x in 0..16 {
                heights[(z << 4) | x] = chunk.get_highest_block_at(x, z).unwrap_or(MIN_Y);
            }
        }
        heights
    }

    /// Heightmaps: MOTION_BLOCKING with the real height of each column
    /// (9 bits per entry, 7 per long, 37 longs for the overworld
    /// heights). The client uses this map to recompute sky light.
    pub fn heightmaps(heights: &[i32; 256], profile: &ModernProfile) -> Vec<u8> {
        let mut values = [0i32; 256];
        for i in 0..256 {
            // Height above MIN_Y of the first free block.
            values[i] = (heights[i] + 1 - MIN_Y).clamp(0, 511);
        }
        let longs = pack_values(&values, 9);
        if profile.heightmaps_as_nbt() {
            return nbt_long_array_compound("MOTION_BLOCKING", &longs);
        }
        let mut out = Vec::new();
        out.extend_from_slice(&var_int(1));
        out.extend_from_slice(&var_int(4));
        out.extend_from_slice(&var_int(longs.len() as i32));
        for &l in &longs {
            out.extend_from_slice(&l.to_be_bytes());
        }
        out
    }

    /// Light fields of `map_chunk`: sky light 15 in every section and
    /// empty block light. The 55 KB sky payload is identical in every
    /// column, so it is built once.
    pub fn light() -> Vec<u8> {
        static CACHE: OnceLock<Vec<u8>> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                let mask = (1u64 << LIGHT_SECTIONS) - 1;
                let full = vec![0xFFu8; 2048];
                let mut out = Vec::new();
                out.extend_from_slice(&var_int(1));
                out.extend_from_slice(&mask.to_be_bytes()); // sky light mask
                out.extend_from_slice(&var_int(0)); // block light mask
                out.extend_from_slice(&var_int(0)); // empty sky light mask
                out.extend_from_slice(&var_int(1));
                out.extend_from_slice(&mask.to_be_bytes()); // empty block light mask
                out.extend_from_slice(&var_int(LIGHT_SECTIONS as i32));
                for _ in 0..LIGHT_SECTIONS {
                    out.extend_from_slice(&var_int(2048));
                    out.extend_from_slice(&full);
                }
                out.extend_from_slice(&var_int(0)); // block light arrays
                out
            })
            .clone()
    }
}

/// Decoded first layer of a Bedrock subchunk: `bits`, `words`,
/// `palette` — the Rust equivalent of what the PHP bridge's
/// `Chunk::getSectionPayload(baseY)` returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionPayload {
    pub bits: u8,
    pub words: Vec<u32>,
    pub palette: Vec<u32>,
}

/// Payload of the section with base `base_y` (e.g. `-48`), or `None`
/// when the section is air / does not exist.
pub fn section_payload(chunk: &Chunk, base_y: i32) -> Option<SectionPayload> {
    let section = ((base_y - MIN_Y) >> 4) as usize;
    let subchunk = chunk.subchunks.get(section)?;
    if subchunk.is_empty() {
        return None;
    }
    let layer = subchunk.layers.first()?;
    let palette = layer.palette().to_vec();
    if palette.is_empty() {
        return None;
    }
    Some(SectionPayload {
        bits: layer.bits_per_block(),
        words: layer.words().to_vec(),
        palette,
    })
}

/// Single-value palette: no longs and, since 1.21.5, no length.
fn single(state: i32, data_length: bool) -> Vec<u8> {
    let mut out = vec![0u8];
    out.extend_from_slice(&var_int(state));
    if data_length {
        out.extend_from_slice(&var_int(0));
    }
    out
}

/// Biome container of a section: single-value (plains) palette, no
/// data array. With 1 bit per entry the client would expect 64 longs
/// (4096 entries) and eat 504 bytes of the next section; with 0 bits
/// the palette is one value and there is nothing to read. In 1.21.4
/// every container's data array also carries its length (0 longs
/// here); from 1.21.5 the length does not travel.
fn biomes(data_length: bool, plains: i32) -> Vec<u8> {
    single(plains, data_length)
}

/// Packs `bits`-wide values into 64-bit longs, little-endian inside
/// each long (same layout as the palette data array).
fn pack_values(values: &[i32], bits: usize) -> Vec<u64> {
    let per_long = 64 / bits;
    let mask = (1u64 << bits) - 1;
    let mut longs = Vec::with_capacity(values.len() / per_long + 1);
    for chunk in values.chunks(per_long) {
        let mut word = 0u64;
        for (entry, &value) in chunk.iter().enumerate() {
            word |= ((value as u64) & mask) << (entry * bits);
        }
        longs.push(word);
    }
    longs
}

/// `varInt` of Java `Protocol.php`: plain LEB128 masked to 32 bits
/// (no zigzag — that is Bedrock's `write_var_i32`).
pub fn var_int(value: i32) -> Vec<u8> {
    let mut value = value as u32;
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
    out
}

/// Minimal NBT writer for the heightmaps of 1.21.4: network NBT
/// ("anonymous": type + payload, no root name), a compound with a
/// single long array (`Nbt.php::longArrayCompound`).
fn nbt_long_array_compound(key: &str, values: &[u64]) -> Vec<u8> {
    let mut out = vec![10]; // compound root
    out.push(12); // longArray
    out.extend_from_slice(&(key.len() as u16).to_be_bytes());
    out.extend_from_slice(key.as_bytes());
    out.extend_from_slice(&(values.len() as u32).to_be_bytes());
    for &v in values {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out.push(0); // TAG_END
    out
}

// ---------------------------------------------------------------------------
// Legacy Java 1.12.2 (`ChunkBuilder.php`, protocol 340)
// ---------------------------------------------------------------------------

pub const JAVA_AIR: i32 = 0;
pub const JAVA_STONE: i32 = 1 << 4;
pub const JAVA_GRASS: i32 = 2 << 4;
pub const JAVA_DIRT: i32 = 3 << 4;
pub const JAVA_BEDROCK: i32 = 7 << 4;

/// Bedrock network ids to Java 1.12.2 states (`(id legacy << 4) | meta`),
/// in `ChunkBuilder.php` `BEDROCK_STATES` order. Blocks whose id changes
/// between Java versions (diamond, redstone, emerald) keep their 1.12.2
/// ids; anything without a translation renders as stone in the PHP
/// comment — the code itself maps unknown ids to air.
pub const BEDROCK_STATES: &[(u32, i32)] = &[
    (3690217760, JAVA_AIR),
    (2150698529, JAVA_STONE),
    (2186211206, JAVA_DIRT),
    (3727763636, JAVA_GRASS),
    (4121722107, JAVA_BEDROCK),
    (138639715, 12 << 4),
    (1529044762, 13 << 4),
    (1211861802, 9 << 4),
    (2754680827, 24 << 4),
    (825916963, 17 << 4),
    (2110714365, 18 << 4),
    (507450469, (17 << 4) | 1),
    (1404704691, (18 << 4) | 1),
    (1423805714, (17 << 4) | 2),
    (1740500456, (18 << 4) | 2),
    (685383673, 16 << 4),
    (2032622302, 15 << 4),
    (2144082742, 14 << 4),
    (2930261704, 21 << 4),
    (180213920, 73 << 4),
    (1419890941, 56 << 4),
    (170572189, 129 << 4),
    (3699616834, 78 << 4),
    (2827903781, (31 << 4) | 1),
    (4175527249, (31 << 4) | 1),
    (2383143704, 37 << 4),
    (3417975282, 38 << 4),
];

/// Java 1.12 / PocketMine legacy ids by block name
/// (`ChunkBuilder.php` `NAME_TO_LEGACY`), in PHP order so that
/// `name_for_legacy` returns the first name per id, like the PHP loop.
pub const NAME_TO_LEGACY: &[(&str, i32)] = &[
    ("air", 0),
    ("stone", 1),
    ("grass_block", 2),
    ("dirt", 3),
    ("cobblestone", 4),
    ("bedrock", 7),
    ("water", 9),
    ("sand", 12),
    ("gravel", 13),
    ("gold_ore", 14),
    ("iron_ore", 15),
    ("coal_ore", 16),
    ("oak_log", 17),
    ("oak_leaves", 18),
    ("lapis_ore", 21),
    ("sandstone", 24),
    ("short_grass", 31),
    ("tall_grass", 31),
    ("dandelion", 37),
    ("poppy", 38),
    ("diamond_ore", 56),
    ("redstone_ore", 73),
    ("snow", 78),
    ("emerald_ore", 129),
];

/// Java 1.12.2 column builder (protocol 340).
pub struct ChunkBuilder;

impl ChunkBuilder {
    pub const JAVA_AIR: i32 = JAVA_AIR;
    pub const JAVA_STONE: i32 = JAVA_STONE;
    pub const JAVA_GRASS: i32 = JAVA_GRASS;
    pub const JAVA_DIRT: i32 = JAVA_DIRT;
    pub const JAVA_BEDROCK: i32 = JAVA_BEDROCK;

    const MIN_BITS: usize = 4;
    const BLOCK_LIGHT_BYTES: usize = 2048;
    const SKY_LIGHT_BYTES: usize = 2048;

    /// 1.12.2 state for a Bedrock network id; unknown ids become air.
    pub fn java_state(bedrock_id: u32) -> i32 {
        BEDROCK_STATES
            .iter()
            .find(|(id, _)| *id == bedrock_id)
            .map(|(_, state)| *state)
            .unwrap_or(JAVA_AIR)
    }

    /// Whether a Bedrock network id has a 1.12.2 translation. An
    /// untranslated id renders as air, so unmapped terrain vanishes.
    pub fn is_mapped(bedrock_id: u32) -> bool {
        BEDROCK_STATES.iter().any(|(id, _)| *id == bedrock_id)
    }

    /// Legacy id (the one `Block::setBlock()` expects) for a block
    /// name, or `None` without a translation.
    pub fn legacy_id_for_name(name: &str) -> Option<i32> {
        NAME_TO_LEGACY
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, id)| *id)
    }

    /// Java 1.12 name for a legacy id, or `None`.
    pub fn name_for_legacy(legacy_id: i32) -> Option<&'static str> {
        NAME_TO_LEGACY
            .iter()
            .find(|(_, id)| *id == legacy_id)
            .map(|(name, _)| *name)
    }

    /// Legacy id (`state >> 4`) for a Bedrock network id, or `None`
    /// without a translation.
    pub fn legacy_id_for_bedrock(bedrock_id: u32) -> Option<i32> {
        BEDROCK_STATES
            .iter()
            .find(|(id, _)| *id == bedrock_id)
            .map(|(_, state)| state >> 4)
    }

    /// Bedrock network id for a legacy id (+ meta), or `None` when the
    /// combined state is not in the table.
    pub fn bedrock_id_for_legacy(legacy_id: i32, meta: i32) -> Option<u32> {
        let wanted = (legacy_id << 4) | (meta & 0x0F);
        BEDROCK_STATES
            .iter()
            .find(|(_, state)| *state == wanted)
            .map(|(id, _)| *id)
    }

    /// Builds the 1.12.2 column from the real Bedrock chunk:
    /// `(section_bitmap, data)`.
    ///
    /// Bedrock has 24 subchunks from Y=-64; 1.12.2 only draws Y=0..255,
    /// so the world shifts 64 blocks up: Bedrock bedrock at -64 lands
    /// at Y=0.
    pub fn from_chunk(chunk: &Chunk) -> (u16, Vec<u8>) {
        let mut states = [JAVA_AIR; 16 * 16 * 256];
        for section in 0..24i32 {
            let base_y = -64 + section * 16;
            if base_y + 64 > 255 {
                break;
            }
            let Some(payload) = section_payload(chunk, base_y) else {
                continue;
            };
            let legacy: Vec<i32> = payload
                .palette
                .iter()
                .map(|&id| Self::java_state(id))
                .collect();
            let per_word = (32usize / (payload.bits.max(1) as usize)).max(1);
            let mask: u32 = if payload.bits == 0 {
                0
            } else {
                (1u32 << payload.bits) - 1
            };
            for x in 0..16 {
                for z in 0..16 {
                    for y in 0..16 {
                        let world_y = base_y + y as i32 + 64;
                        if !(0..=255).contains(&world_y) {
                            continue;
                        }
                        let entry = (x << 8) | (z << 4) | y;
                        let index = if payload.bits == 0 {
                            0
                        } else {
                            let word = payload.words[entry / per_word];
                            ((word >> ((entry % per_word) * payload.bits as usize)) & mask) as usize
                        };
                        states[((world_y as usize) << 8) | (z << 4) | x] =
                            legacy.get(index).copied().unwrap_or(JAVA_AIR);
                    }
                }
            }
        }
        Self::encode(&states)
    }

    /// Encodes 65536 states (index `(y << 8) | (z << 4) | x`) as
    /// `map_chunk` data for protocol 340, ground-up continuous: per
    /// present section, `bits_per_block`, palette, longs, block light
    /// (2048 zero bytes) and sky light (2048 `0xFF` bytes); then 256
    /// plains biome bytes.
    ///
    /// 1.12.2 uses non-compact packing: a value never crosses two
    /// longs (enforced by flooring `64 / bits`, exactly like the
    /// modern encoder's `out_per_long` here).
    pub fn encode(states: &[i32]) -> (u16, Vec<u8>) {
        assert!(states.len() >= 16 * 16 * 256);
        let mut data = Vec::new();
        let mut bit_map: u16 = 0;
        for section in 0..16 {
            let base = section * 4096;
            let slice = &states[base..base + 4096];
            if slice.iter().max().copied().unwrap_or(JAVA_AIR) == JAVA_AIR {
                continue;
            }
            bit_map |= 1 << section;

            // Unique palette, first-seen order.
            let mut palette: Vec<i32> = Vec::new();
            for &state in slice {
                if !palette.contains(&state) {
                    palette.push(state);
                }
            }
            let mut bits = Self::MIN_BITS;
            while (1usize << bits) < palette.len() {
                bits += 1;
            }
            if bits > 8 {
                bits = 8;
                palette.truncate(256);
            }

            data.push(bits as u8);
            data.extend_from_slice(&var_int(palette.len() as i32));
            for &state in &palette {
                data.extend_from_slice(&var_int(state));
            }

            let per_long = 64 / bits;
            let long_count = 4096 / per_long;
            data.extend_from_slice(&var_int(long_count as i32));

            let mut words = vec![0u64; long_count];
            for (index, &state) in slice.iter().enumerate() {
                let slot = palette.iter().position(|&p| p == state).unwrap_or(0) as u64;
                words[index / per_long] |= slot << ((index % per_long) * bits);
            }
            for &word in &words {
                data.extend_from_slice(&word.to_be_bytes());
            }
            data.extend(std::iter::repeat_n(0, Self::BLOCK_LIGHT_BYTES));
            data.extend(std::iter::repeat_n(0xFF, Self::SKY_LIGHT_BYTES));
        }
        // Plains biome (legacy id 1) in every column.
        data.extend(std::iter::repeat_n(1, 256));
        (bit_map, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bedrock_state_uses_default_states() {
        let p775 = ModernProfile::for_protocol(775).unwrap();
        let p769 = ModernProfile::for_protocol(769).unwrap();
        // Default states, not minStateId: no snowy grass, upright log.
        assert_eq!(ModernChunk::bedrock_state(3727763636, &p775), 9);
        assert_eq!(ModernChunk::bedrock_state(825916963, &p775), 137);
        assert_eq!(ModernChunk::bedrock_state(1211861802, &p775), 86);
        // Ores/plants that change ids between versions.
        assert_eq!(ModernChunk::bedrock_state(180213920, &p769), 5904);
        assert_eq!(ModernChunk::bedrock_state(180213920, &p775), 6882);
        assert_eq!(ModernChunk::bedrock_state(2827903781, &p769), 2048);
        assert_eq!(ModernChunk::bedrock_state(2827903781, &p775), 2248);
        // Unknown ids become air, like the PHP lookup fallback.
        assert_eq!(ModernChunk::bedrock_state(12345, &p775), 0);
    }

    #[test]
    fn bedrock_name_maps_roundtrip() {
        let p775 = ModernProfile::for_protocol(775).unwrap();
        assert_eq!(ModernChunk::block_name(1211861802), Some("water"));
        assert_eq!(ModernChunk::block_name(12345), None);
        assert_eq!(
            ModernChunk::bedrock_network_id_for_name("water"),
            Some(1211861802)
        );
        assert_eq!(
            ModernChunk::block_name_for_java_state(6882, &p775),
            Some("redstone_ore")
        );
        assert_eq!(
            ModernChunk::bedrock_network_id_for_java_state(6882, &p775),
            Some(180213920)
        );
        // Grass: 9 under 775, but 8 (snowy) is not claimed by any name.
        assert_eq!(
            ModernChunk::block_name_for_java_state(9, &p775),
            Some("grass_block")
        );
        assert_eq!(ModernChunk::block_name_for_java_state(8, &p775), None);
    }

    #[test]
    fn fluid_range_is_water_and_lava_only() {
        assert!(ModernChunk::is_fluid(86)); // water source
        assert!(ModernChunk::is_fluid(101)); // flowing water end
        assert!(ModernChunk::is_fluid(102)); // lava source
        assert!(ModernChunk::is_fluid(117)); // flowing lava end
        assert!(!ModernChunk::is_fluid(85)); // bedrock
        assert!(!ModernChunk::is_fluid(118)); // sand
        assert!(!ModernChunk::is_fluid(0));
        assert!(ModernChunk::has_fluid(812));
        assert!(!ModernChunk::has_fluid(0));
    }

    #[test]
    fn profile_wire_flags_match_protocol() {
        let p769 = ModernProfile::for_protocol(769).unwrap();
        let p770 = ModernProfile::for_protocol(770).unwrap();
        let p775 = ModernProfile::for_protocol(775).unwrap();
        assert!(p769.heightmaps_as_nbt());
        assert!(!p770.heightmaps_as_nbt());
        assert!(p769.palette_data_has_length());
        assert!(!p770.palette_data_has_length());
        assert!(!ModernProfile::for_protocol(774)
            .unwrap()
            .sections_have_fluid_count());
        assert!(p775.sections_have_fluid_count());
        assert_eq!(p769.plains_biome(), 39);
        assert_eq!(p775.plains_biome(), 40);
        assert!(ModernProfile::for_protocol(769).is_some());
        assert!(ModernProfile::for_protocol(999).is_none());
    }

    #[test]
    fn biome_registry_indexes_plains_vs_badlands() {
        // Legacy biome ids: plains 1 / badlands 37 (niduy `BIOME`).
        // Worldgen registry indexes: plains 39/40, badlands 0 — sending
        // 0 as "plains" would paint badlands. All 5 profiles agree on 0.
        for protocol in [769, 770, 773, 774, 775] {
            let profile = ModernProfile::for_protocol(protocol).unwrap();
            assert_eq!(profile.badlands_biome(), 0, "protocolo {protocol}");
            assert_ne!(profile.plains_biome(), profile.badlands_biome());
            assert_eq!(
                profile.plains_biome(),
                if protocol >= 774 { 40 } else { 39 },
                "protocolo {protocol}"
            );
        }
    }

    #[test]
    fn legacy_maps_match_php_tables() {
        assert_eq!(ChunkBuilder::java_state(2150698529), 16); // stone
        assert_eq!(ChunkBuilder::java_state(3727763636), 32); // grass
        assert_eq!(ChunkBuilder::java_state(3699616834), 78 << 4); // snow_layer
        assert_eq!(ChunkBuilder::java_state(12345), 0); // unknown -> air
        assert!(ChunkBuilder::is_mapped(1211861802));
        assert!(!ChunkBuilder::is_mapped(12345));
        assert_eq!(ChunkBuilder::legacy_id_for_name("grass_block"), Some(2));
        assert_eq!(ChunkBuilder::legacy_id_for_name("cobblestone"), Some(4));
        assert_eq!(ChunkBuilder::name_for_legacy(31), Some("short_grass"));
        assert_eq!(ChunkBuilder::legacy_id_for_bedrock(4121722107), Some(7));
        assert_eq!(ChunkBuilder::bedrock_id_for_legacy(9, 0), Some(1211861802));
        // `state()` helper names.
        assert_eq!(ModernChunk::state("grass"), 9);
        assert_eq!(ModernChunk::state("log"), 137);
        assert_eq!(ModernChunk::state("nope"), 0);
    }

    #[test]
    fn light_is_sky_full_and_block_empty() {
        let light = ModernChunk::light();
        // 1 + 8 + 1 + 1 + (1 + 8) + 1 + 26*(2 + 2048) + 1
        assert_eq!(light.len(), 1 + 8 + 1 + 1 + 9 + 1 + 26 * 2050 + 1);
        assert_eq!(light[0], 1);
        let mask = u64::from_be_bytes(light[1..9].try_into().unwrap());
        assert_eq!(mask, (1u64 << LIGHT_SECTIONS) - 1);
        assert_eq!(ModernChunk::light(), light);
    }

    #[test]
    fn heightmaps_typed_and_nbt_forms() {
        let heights = [MIN_Y; 256];
        let typed = ModernChunk::heightmaps(&heights, &ModernProfile::for_protocol(775).unwrap());
        assert_eq!(typed[0], 1);
        assert_eq!(typed[1], 4);
        // 37 longs of 9-bit ones: value above MIN_Y is 1 for an all-air column.
        assert_eq!(typed.len(), 3 + 37 * 8);
        let first_long = u64::from_be_bytes(typed[3..11].try_into().unwrap());
        assert_eq!(first_long, 0x0040_2010_0804_0201);

        let nbt = ModernChunk::heightmaps(&heights, &ModernProfile::for_protocol(769).unwrap());
        assert_eq!(nbt[0], 10); // compound
        assert_eq!(nbt[1], 12); // longArray
        assert_eq!(&nbt[2..4], &(15u16).to_be_bytes());
        assert_eq!(&nbt[4..19], b"MOTION_BLOCKING");
        assert_eq!(&nbt[19..23], &37u32.to_be_bytes());
        assert_eq!(*nbt.last().unwrap(), 0);
        assert_eq!(nbt.len(), 1 + 1 + 2 + 15 + 4 + 37 * 8 + 1);
    }

    #[test]
    fn legacy_encode_empty_is_only_biomes() {
        let states = [JAVA_AIR; 16 * 16 * 256];
        let (mask, data) = ChunkBuilder::encode(&states);
        assert_eq!(mask, 0);
        assert_eq!(data, vec![1u8; 256]);
    }
}
