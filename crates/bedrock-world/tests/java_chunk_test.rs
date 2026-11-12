//! Port of `plugins/JavaBridge/tests/real_column_test.php` (plus the
//! `real_column_legacy_test.php` and synthetic-water cases of
//! `modern_test.php` / `chunk_api_test.php`) to Rust.
//!
//! The PHP version reads a real Bedrock column dumped by the Rust
//! server (`dump_column`: `<prefix>.bin` payload + `<prefix>.txt`
//! truth of every block) through the PHP runtime `Chunk` decoder and
//! checks the `ModernChunk::fromChunk()` Java column against it. Here
//! the "real column" is generated directly by `NormalGenerator`
//! (seed 0, the server seed) and the truth is every non-air block of
//! the generated `Chunk` — the same world the bridge would fetch via
//! `world.get_chunk`.
//!
//! Chunks covered:
//! - (0, 0): river/forest column, water surface at Y = 62 (225 blocks),
//!   section 7 fluidCount 812 (`hasFluid() == true` in 26.1).
//! - (29, 1): plains chunk (212 plains + 44 ocean biome cells) with its
//!   own water column: 66 blocks at Y = 62, section 7 fluidCount 424.

use bedrock_world::block_id;
use bedrock_world::java_chunk::{ChunkBuilder, ModernChunk, ModernProfile, MIN_Y, SECTIONS};
use bedrock_world::{Chunk, NormalGenerator};

/// One decoded Java section.
struct DecodedSection {
    block_count: u16,
    fluid_count: u16,
    /// State per Java entry (`y << 8 | z << 4 | x`).
    entries: Vec<i32>,
    biome: i32,
}

fn read_var_int(data: &[u8], offset: &mut usize) -> i32 {
    let mut value: i32 = 0;
    let mut shift = 0;
    while *offset < data.len() {
        let byte = data[*offset];
        *offset += 1;
        value |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    value
}

/// Decodes the Java column exactly like the PHP `decodeJavaSections`
/// helper: per section, counters, palette container, plains biome
/// container. Also verifies, per section, that the leading counters
/// agree with the decoded entries (fluids = states 86..=117).
fn decode_java_sections(data: &[u8], profile: &ModernProfile) -> Vec<DecodedSection> {
    let fluid = profile.sections_have_fluid_count();
    let data_length = profile.palette_data_has_length();
    let plains = profile.plains_biome();
    let mut offset = 0usize;
    let mut sections = Vec::with_capacity(SECTIONS);
    for section in 0..SECTIONS {
        let block_count = u16::from_be_bytes(data[offset..offset + 2].try_into().unwrap());
        offset += 2;
        let fluid_count = if fluid {
            let v = u16::from_be_bytes(data[offset..offset + 2].try_into().unwrap());
            offset += 2;
            v
        } else {
            0
        };
        let (entries, bits, _) = decode_container(data, &mut offset, 4096, data_length);
        let (biome_entries, biome_bits, _) = decode_container(data, &mut offset, 64, data_length);
        if biome_bits != 0 || biome_entries.iter().any(|&b| b != plains) {
            panic!(
                "sección {section}: bioma {biome_entries:?} (bits {biome_bits}) != plains {plains}"
            );
        }
        if fluid {
            let expected_fluids = entries.iter().filter(|&&s| (86..=117).contains(&s)).count();
            assert_eq!(
                fluid_count as usize, expected_fluids,
                "sección {section}: fluidCount {fluid_count} != fluidos reales {expected_fluids}"
            );
            let expected_blocks = entries.iter().filter(|&&s| s != 0).count();
            assert_eq!(
                block_count as usize, expected_blocks,
                "sección {section}: blockCount {block_count} != bloques reales {expected_blocks}"
            );
        }
        let _ = bits;
        sections.push(DecodedSection {
            block_count,
            fluid_count,
            entries,
            biome: biome_entries[0],
        });
    }
    sections
}

/// Decodes one palette container (blocks or biomes) like the PHP
/// `parseContainer` helper. Returns the per-entry values.
fn decode_container(
    data: &[u8],
    offset: &mut usize,
    count: usize,
    data_length: bool,
) -> (Vec<i32>, u8, Vec<i32>) {
    let bits = data[*offset];
    *offset += 1;
    if bits == 0 {
        let value = read_var_int(data, offset);
        if data_length {
            let length = read_var_int(data, offset);
            assert_eq!(length, 0, "single-value container with data length");
        }
        return (vec![value; count], bits, vec![value]);
    }
    let palette_size = read_var_int(data, offset) as usize;
    let mut palette = Vec::with_capacity(palette_size);
    for _ in 0..palette_size {
        palette.push(read_var_int(data, offset));
    }
    let per_long = 64 / bits as usize;
    let longs = if data_length {
        read_var_int(data, offset) as usize
    } else {
        count.div_ceil(per_long)
    };
    let mask = (1u64 << bits) - 1;
    let raw = &data[*offset..*offset + longs * 8];
    *offset += longs * 8;
    let mut entries = vec![0i32; count];
    for entry in 0..count {
        let word = u64::from_be_bytes(
            raw[(entry / per_long) * 8..(entry / per_long) * 8 + 8]
                .try_into()
                .unwrap(),
        );
        let slot = ((word >> ((entry % per_long) * bits as usize)) & mask) as usize;
        entries[entry] = palette[slot];
    }
    (entries, bits, palette)
}

/// Truth of a real column: every non-air Bedrock block.
fn truth_blocks(chunk: &Chunk) -> Vec<(usize, i32, usize, u32)> {
    let mut blocks = Vec::new();
    for x in 0..16 {
        for z in 0..16 {
            for y in MIN_Y..320 {
                let id = chunk.get_block(x, y, z);
                if id != block_id::AIR && id != 0 {
                    blocks.push((x, y, z, id));
                }
            }
        }
    }
    blocks
}

fn java_state_at(sections: &[DecodedSection], x: usize, y: i32, z: usize) -> i32 {
    let relative = y + 64;
    let section = (relative >> 4) as usize;
    let index = (((relative & 15) << 8) | (((z & 15) as i32) << 4) | (x as i32)) as usize;
    sections[section].entries[index]
}

/// The full `real_column_test.php` parity check for one column.
/// Returns the decoded sections so the caller can assert specifics.
fn check_real_column(chunk: &Chunk, profile: &ModernProfile) -> Vec<DecodedSection> {
    let truth = truth_blocks(chunk);
    assert!(!truth.is_empty(), "la columna de verdad está vacía");

    let (data, report) = ModernChunk::from_chunk_with_fluid_report(chunk, profile);
    let sections = decode_java_sections(&data, profile);

    // The fluid report agrees with the wire counters (the bridge logs it).
    if profile.sections_have_fluid_count() {
        for (section, decoded) in sections.iter().enumerate() {
            assert_eq!(
                report[section], decoded.fluid_count,
                "sección {section}: informe != cable"
            );
        }
    }

    // (2) Same block in the same coordinate.
    for &(x, y, z, id) in &truth {
        let expected = ModernChunk::bedrock_state(id, profile);
        assert_eq!(
            java_state_at(&sections, x, y, z),
            expected,
            "({x},{y},{z}) id {id} esperado {expected}"
        );
    }

    // (3) No invented blocks.
    let java_non_air: usize = sections
        .iter()
        .flat_map(|s| &s.entries)
        .filter(|&&state| state != ModernChunk::AIR)
        .count();
    assert_eq!(
        java_non_air,
        truth.len(),
        "java={java_non_air} bedrock={}",
        truth.len()
    );

    // (4) Identical surface column by column.
    for z in 0..16 {
        for x in 0..16 {
            let bedrock_surface = chunk.get_highest_block_at(x, z).unwrap_or(MIN_Y);
            let mut java_surface = MIN_Y;
            'top: for section in (0..SECTIONS).rev() {
                for local_y in (0..16).rev() {
                    let index = (local_y << 8) | (z << 4) | x;
                    if sections[section].entries[index] != ModernChunk::AIR {
                        java_surface = (section as i32) * 16 + local_y as i32 + MIN_Y;
                        break 'top;
                    }
                }
            }
            assert_eq!(
                java_surface, bedrock_surface,
                "superficie ({x},{z}) bedrock {bedrock_surface} java {java_surface}"
            );
        }
    }

    sections
}

#[test]
fn real_column_chunk_0_0_ocean_water_y62_fluidcount_812() {
    let generator = NormalGenerator::new(0);
    let chunk = generator.generate_chunk_internal(0, 0);
    let profile = ModernProfile::for_protocol(775).unwrap(); // 26.1

    let sections = check_real_column(&chunk, &profile);

    // Ocean-level water: surface at the sea level Y = 62 in the decoded
    // column (225 water blocks at Y62 in Bedrock, seed 0).
    let truth = truth_blocks(&chunk);
    let water_y62: Vec<_> = truth
        .iter()
        .filter(|&&(_, y, _, id)| y == 62 && id == block_id::WATER)
        .collect();
    assert!(!water_y62.is_empty(), "chunk 0,0 debería tener agua en Y62");
    assert_eq!(water_y62.len(), 225, "agua Bedrock en Y62 (0,0)");
    for &&(x, _, z, _) in &water_y62 {
        assert_eq!(
            java_state_at(&sections, x, 62, z),
            ModernChunk::WATER,
            "agua decodificada en ({x},62,{z})"
        );
    }

    // 26.1: the fluid counter is real, so hasFluid() is true and the
    // client can swim instead of falling through the water.
    assert_eq!(sections[7].fluid_count, 812, "fluidCount sección 7");
    assert_eq!(sections[7].block_count, 3866);
    assert!(ModernChunk::has_fluid(sections[7].fluid_count));
    assert!(sections
        .iter()
        .enumerate()
        .all(|(s, _)| s == 7 || sections[s].fluid_count == 0));
}

#[test]
fn real_column_chunk_29_1() {
    let generator = NormalGenerator::new(0);
    let chunk = generator.generate_chunk_internal(29, 1);
    let profile = ModernProfile::for_protocol(775).unwrap(); // 26.1

    let sections = check_real_column(&chunk, &profile);

    // Plains-dominated column (dominant cell = PLAINS 1); the biome
    // histogram also holds 44 ocean cells, which is where its water
    // comes from.
    assert_eq!(chunk.biomes[7 * 16 + 7], 1, "bioma dominante (29,1)");

    // This chunk also has water at Y62 (66 blocks, seed 0), with its
    // own 26.1 fluid counter — and no other section carries fluid.
    let truth = truth_blocks(&chunk);
    let water_y62 = truth
        .iter()
        .filter(|&&(_, y, _, id)| y == 62 && id == block_id::WATER)
        .count();
    assert_eq!(water_y62, 66, "agua Bedrock en Y62 (29,1)");
    assert_eq!(sections[7].fluid_count, 424, "fluidCount sección 7 (29,1)");
    assert!(ModernChunk::has_fluid(sections[7].fluid_count));
    assert!(sections
        .iter()
        .enumerate()
        .all(|(s, _)| s == 7 || sections[s].fluid_count == 0));
}

#[test]
fn real_column_decodes_under_every_modern_profile() {
    let generator = NormalGenerator::new(0);
    let chunk = generator.generate_chunk_internal(0, 0);
    for protocol in [769, 770, 773, 774, 775] {
        let profile = ModernProfile::for_protocol(protocol).unwrap();
        let sections = check_real_column(&chunk, &profile);
        assert_eq!(sections[7].block_count, 3866, "protocolo {protocol}");
        assert_eq!(
            sections[7].biome,
            profile.plains_biome(),
            "protocolo {protocol}"
        );
    }
}

#[test]
fn heightmaps_carry_the_real_column_heights() {
    let generator = NormalGenerator::new(0);
    let chunk = generator.generate_chunk_internal(0, 0);
    let heights = ModernChunk::heights_from_chunk(&chunk);

    // Typed form (775): 3 varints, then 37 longs of 9-bit values.
    let profile = ModernProfile::for_protocol(775).unwrap();
    let typed = ModernChunk::heightmaps(&heights, &profile);
    let mut offset = 0;
    assert_eq!(read_var_int(&typed, &mut offset), 1);
    assert_eq!(read_var_int(&typed, &mut offset), 4);
    assert_eq!(read_var_int(&typed, &mut offset), 37);
    let mut values = Vec::with_capacity(256);
    for i in 0..37 {
        let word = u64::from_be_bytes(typed[offset..offset + 8].try_into().unwrap());
        offset += 8;
        for entry in 0..7 {
            if values.len() < 256 {
                values.push(((word >> (entry * 9)) & 0x1FF) as i32);
            }
        }
        let _ = i;
    }
    assert_eq!(offset, typed.len());
    for i in 0..256 {
        let expected = (heights[i] + 1 - MIN_Y).clamp(0, 511);
        assert_eq!(values[i], expected, "altura {i}");
    }

    // NBT form (769): compound, longArray key, 37 longs, end tag.
    let nbt = ModernChunk::heightmaps(&heights, &ModernProfile::for_protocol(769).unwrap());
    assert_eq!(nbt[0], 10);
    assert_eq!(nbt[1], 12);
    assert_eq!(&nbt[4..19], b"MOTION_BLOCKING");
    assert_eq!(&nbt[19..23], &37u32.to_be_bytes());
    assert_eq!(*nbt.last().unwrap(), 0);
}

// --- `real_column_legacy_test.php` / `ChunkBuilder` -------------------------

/// Decodes `map_chunk` (1.12.2) data, skipping the light arrays like the
/// PHP `decodeLegacy` helper.
fn decode_legacy(data: &[u8], mask: u16) -> Vec<Option<Vec<i32>>> {
    let mut offset = 0usize;
    let mut sections: Vec<Option<Vec<i32>>> = vec![None; 16];
    for section in 0..16 {
        if (mask >> section) & 1 == 0 {
            continue;
        }
        let bits = data[offset];
        offset += 1;
        let palette_size = read_var_int(data, &mut offset) as usize;
        let mut palette = Vec::with_capacity(palette_size);
        for _ in 0..palette_size {
            palette.push(read_var_int(data, &mut offset));
        }
        let longs = read_var_int(data, &mut offset) as usize;
        let per_long = 64 / bits as usize;
        let mask_bits = if bits == 64 {
            u64::MAX
        } else {
            (1u64 << bits) - 1
        };
        let mut entries = vec![0i32; 4096];
        for entry in 0..4096 {
            let word = u64::from_be_bytes(
                data[offset + (entry / per_long) * 8..offset + (entry / per_long) * 8 + 8]
                    .try_into()
                    .unwrap(),
            );
            let slot = ((word >> ((entry % per_long) * bits as usize)) & mask_bits) as usize;
            entries[entry] = palette[slot];
        }
        offset += longs * 8;
        offset += 2048 + 2048; // block light + sky light
        sections[section] = Some(entries);
    }
    sections
}

fn check_legacy_column(chunk: &Chunk) {
    let truth = truth_blocks(chunk);
    let (mask, encoded) = ChunkBuilder::from_chunk(chunk);
    let java = decode_legacy(&encoded, mask);
    assert!(mask != 0, "la máscara legacy no debería ser cero");

    // Biome tail: plains (1) in the 256 trailing bytes.
    assert_eq!(&encoded[encoded.len() - 256..], &[1u8; 256]);

    // Same block in the same coordinate (Y + 64 shift).
    let mut expected = 0usize;
    for &(x, y, z, id) in &truth {
        assert!(ChunkBuilder::is_mapped(id), "id {id} sin traducción legacy");
        let state = ChunkBuilder::java_state(id);
        if state != ChunkBuilder::JAVA_AIR {
            expected += 1;
        }
        let relative = y + 64;
        if !(0..=255).contains(&relative) {
            continue;
        }
        let section = (relative >> 4) as usize;
        let index = (((relative & 15) << 8) | (((z & 15) as i32) << 4) | (x as i32)) as usize;
        let got = java[section].as_ref().map_or(-1, |entries| entries[index]);
        assert_eq!(got, state, "({x},{y},{z}) id {id}");
    }

    // No invented blocks.
    let java_non_air: usize = java
        .iter()
        .flatten()
        .flat_map(|entries| entries.iter())
        .filter(|&&s| s != ChunkBuilder::JAVA_AIR)
        .count();
    assert_eq!(java_non_air, expected);
}

#[test]
fn real_column_legacy_chunk_0_0() {
    let generator = NormalGenerator::new(0);
    let chunk = generator.generate_chunk_internal(0, 0);
    check_legacy_column(&chunk);
}

#[test]
fn real_column_legacy_chunk_29_1() {
    let generator = NormalGenerator::new(0);
    let chunk = generator.generate_chunk_internal(29, 1);
    check_legacy_column(&chunk);
}

// --- Synthetic cases from `modern_test.php` / `chunk_api_test.php` -----------

#[test]
fn synthetic_half_water_section_has_fluid_2048() {
    // Section 7 (Y 48-63), lower half water: 2048 water cells.
    let mut chunk = Chunk::new(1);
    for x in 0..16 {
        for z in 0..16 {
            for y in 48..56 {
                chunk.set_block(x, y, z, block_id::WATER);
            }
        }
    }

    for protocol in [769, 775] {
        let profile = ModernProfile::for_protocol(protocol).unwrap();
        let sections = decode_java_sections(&ModernChunk::from_chunk(&chunk, &profile), &profile);
        assert_eq!(sections[7].block_count, 2048, "protocolo {protocol}");
        if profile.sections_have_fluid_count() {
            assert_eq!(sections[7].fluid_count, 2048, "protocolo {protocol}");
            assert!(ModernChunk::has_fluid(sections[7].fluid_count));
        }
        // Java entry (y=7,z=0,x=0) is water; (y=8,z=0,x=0) is air.
        assert_eq!(sections[7].entries[7 << 8], 86, "protocolo {protocol}");
        assert_eq!(sections[7].entries[8 << 8], 0, "protocolo {protocol}");
    }
}

#[test]
fn synthetic_bedrock_stone_dirt_grass_column() {
    // Mirrors `chunk_api_test.php`: single bedrock section at Y=-64 and
    // stone (<10), dirt (<14), grass (14), air above in Y=-48.
    let mut chunk = Chunk::new(1);
    chunk.subchunks[0] = bedrock_world::SubChunk::new(block_id::BEDROCK);
    for x in 0..16 {
        for z in 0..16 {
            for local_y in 0..16 {
                let y = -48 + local_y;
                let id = if local_y < 10 {
                    block_id::STONE
                } else if local_y < 14 {
                    block_id::DIRT
                } else if local_y == 14 {
                    block_id::GRASS_BLOCK
                } else {
                    block_id::AIR
                };
                chunk.set_block(x, y, z, id);
            }
        }
    }

    let profile = ModernProfile::for_protocol(775).unwrap();
    let sections = decode_java_sections(&ModernChunk::from_chunk(&chunk, &profile), &profile);
    assert_eq!(java_state_at(&sections, 0, -64, 0), ModernChunk::BEDROCK);
    assert_eq!(java_state_at(&sections, 3, -48, 5), ModernChunk::STONE);
    assert_eq!(java_state_at(&sections, 3, -38, 5), ModernChunk::DIRT);
    assert_eq!(java_state_at(&sections, 3, -34, 5), ModernChunk::GRASS);
    assert_eq!(java_state_at(&sections, 3, -33, 5), ModernChunk::AIR);
    assert_eq!(java_state_at(&sections, 3, 100, 5), ModernChunk::AIR);
    assert_eq!(chunk.get_highest_block_at(3, 5), Some(-34));

    // Legacy twin: bedrock at Y=0 after the +64 shift.
    let (mask, encoded) = ChunkBuilder::from_chunk(&chunk);
    let legacy = decode_legacy(&encoded, mask);
    let at = |y: i32, x: usize, z: usize| {
        let relative = y + 64;
        let section = (relative >> 4) as usize;
        let index = (((relative & 15) << 8) | (((z & 15) as i32) << 4) | (x as i32)) as usize;
        legacy[section].as_ref().map(|e| e[index])
    };
    assert_eq!(at(-64, 0, 0), Some(ChunkBuilder::JAVA_BEDROCK));
    assert_eq!(at(-48, 3, 5), Some(ChunkBuilder::JAVA_STONE));
    assert_eq!(at(-34, 3, 5), Some(ChunkBuilder::JAVA_GRASS));
}
