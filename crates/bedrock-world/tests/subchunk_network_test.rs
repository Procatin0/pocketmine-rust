use bedrock_world::{
    deserialize_subchunk, serialize_subchunk, serialize_subchunk_version, SubChunk,
    SubChunkNetworkError,
};
use bedrock_protocol::{Reader, Writer};

#[test]
fn test_empty_subchunk_roundtrip() {
    let subchunk = SubChunk::new(0); // All air
    let bytes = subchunk.serialize_network();

    // Verify expected raw structure for single-entry empty subchunk:
    // [0] = version (8)
    // [1] = layer count (1)
    // [2] = palette header: (0 << 1) | 1 = 1
    // (no palette size count for bpb == 0 in Bedrock protocol)
    // [3] = block runtime id VarInt zigzag: 0 -> 0x00
    assert_eq!(bytes, vec![8, 1, 1, 0]);

    let deserialized = SubChunk::deserialize_network(&bytes).expect("Failed to deserialize empty subchunk");
    assert_eq!(deserialized.layers.len(), 1);

    for x in 0..16 {
        for y in 0..16 {
            for z in 0..16 {
                assert_eq!(deserialized.get_block(x, y, z, 0), 0);
            }
        }
    }
}

#[test]
fn test_dispersed_blocks_roundtrip() {
    let mut subchunk = SubChunk::new(0); // Fill with air (0)

    // Set various blocks at dispersed coordinates across the subchunk
    // Common Bedrock runtime IDs:
    const AIR: u32 = 0;
    const STONE: u32 = 1;
    const GRASS: u32 = 2;
    const DIRT: u32 = 3;
    const GOLD_BLOCK: u32 = 41;
    const OBSIDIAN: u32 = 49;
    const DIAMOND_BLOCK: u32 = 56;
    const BEDROCK: u32 = 7;

    subchunk.set_block(0, 0, 0, 0, STONE);
    subchunk.set_block(15, 15, 15, 0, DIAMOND_BLOCK);
    subchunk.set_block(0, 15, 0, 0, GRASS);
    subchunk.set_block(15, 0, 15, 0, DIRT);
    subchunk.set_block(7, 8, 9, 0, GOLD_BLOCK);
    subchunk.set_block(3, 12, 1, 0, OBSIDIAN);
    subchunk.set_block(10, 2, 14, 0, BEDROCK);
    subchunk.set_block(8, 8, 8, 0, AIR);

    // Serialize to network bytes
    let network_bytes = subchunk.serialize_network();
    assert!(!network_bytes.is_empty());

    // Deserialize back from network bytes
    let deserialized = SubChunk::deserialize_network(&network_bytes)
        .expect("Failed to deserialize network subchunk");

    // Check specific dispersed blocks
    assert_eq!(deserialized.get_block(0, 0, 0, 0), STONE);
    assert_eq!(deserialized.get_block(15, 15, 15, 0), DIAMOND_BLOCK);
    assert_eq!(deserialized.get_block(0, 15, 0, 0), GRASS);
    assert_eq!(deserialized.get_block(15, 0, 15, 0), DIRT);
    assert_eq!(deserialized.get_block(7, 8, 9, 0), GOLD_BLOCK);
    assert_eq!(deserialized.get_block(3, 12, 1, 0), OBSIDIAN);
    assert_eq!(deserialized.get_block(10, 2, 14, 0), BEDROCK);
    assert_eq!(deserialized.get_block(8, 8, 8, 0), AIR);

    // Check untouched coordinates remain AIR
    assert_eq!(deserialized.get_block(1, 1, 1, 0), AIR);
    assert_eq!(deserialized.get_block(14, 14, 14, 0), AIR);
    assert_eq!(deserialized.get_block(0, 0, 1, 0), AIR);
    assert_eq!(deserialized.get_block(5, 5, 5, 0), AIR);

    // Full 16x16x16 equivalence check
    for x in 0..16 {
        for y in 0..16 {
            for z in 0..16 {
                assert_eq!(
                    deserialized.get_block(x, y, z, 0),
                    subchunk.get_block(x, y, z, 0),
                    "Mismatch at ({}, {}, {})",
                    x, y, z
                );
            }
        }
    }
}

#[test]
fn test_multi_layer_waterlogged() {
    let mut subchunk = SubChunk::new(0);

    const OAK_STAIRS: u32 = 53;
    const WATER: u32 = 9;

    // Layer 0: stairs
    subchunk.set_block(4, 2, 8, 0, OAK_STAIRS);
    // Layer 1: water in the same block position
    subchunk.set_block(4, 2, 8, 1, WATER);

    let bytes = subchunk.serialize_network();
    let deserialized = SubChunk::deserialize_network(&bytes)
        .expect("Failed to deserialize 2-layer subchunk");

    assert_eq!(deserialized.layers.len(), 2);
    assert_eq!(deserialized.get_block(4, 2, 8, 0), OAK_STAIRS);
    assert_eq!(deserialized.get_block(4, 2, 8, 1), WATER);

    // Coordinate where no water was placed
    assert_eq!(deserialized.get_block(0, 0, 0, 1), 0);
}

#[test]
fn test_multiple_palette_sizes_and_packing() {
    // Tests packing when palette size requires 3 bits per block (5-8 unique block IDs),
    // which tests non-power-of-two words packing (10 blocks per u32 word).
    let mut subchunk = SubChunk::new(0);

    let block_ids = [100, 200, 300, 400, 500, 600];
    for (i, &bid) in block_ids.iter().enumerate() {
        subchunk.set_block(i, i, i, 0, bid);
    }

    let bytes = subchunk.serialize_network();
    let deserialized = SubChunk::deserialize_network(&bytes)
        .expect("Failed to deserialize 3-bpb subchunk");

    for (i, &bid) in block_ids.iter().enumerate() {
        assert_eq!(deserialized.get_block(i, i, i, 0), bid);
    }
}

#[test]
fn test_writer_reader_stream_functions() {
    let mut subchunk = SubChunk::new(0);
    subchunk.set_block(2, 3, 4, 0, 99);

    let mut writer = Writer::new();
    serialize_subchunk(&subchunk, &mut writer);
    let bytes = writer.into_bytes();

    let mut reader = Reader::new(&bytes);
    let deserialized = deserialize_subchunk(&mut reader).expect("Failed to deserialize with Reader");
    assert_eq!(deserialized.get_block(2, 3, 4, 0), 99);
}

#[test]
fn test_version_9_support() {
    let mut subchunk = SubChunk::new(0);
    subchunk.set_block(1, 2, 3, 0, 42);

    let mut writer = Writer::new();
    serialize_subchunk_version(&subchunk, &mut writer, 9);
    let bytes = writer.into_bytes();

    let mut reader = Reader::new(&bytes);
    let deserialized = deserialize_subchunk(&mut reader).expect("Version 9 should be accepted");
    assert_eq!(deserialized.get_block(1, 2, 3, 0), 42);
}

#[test]
fn test_unsupported_version_rejected() {
    let mut writer = Writer::new();
    writer.write_byte(1); // Legacy version 1
    writer.write_byte(1); // 1 layer

    let bytes = writer.into_bytes();
    let result = SubChunk::deserialize_network(&bytes);
    match result {
        Err(SubChunkNetworkError::UnsupportedVersion(1)) => {}
        other => panic!("Expected UnsupportedVersion(1), got {:?}", other),
    }
}

#[test]
fn test_non_runtime_palette_rejected() {
    let mut writer = Writer::new();
    writer.write_byte(8); // Version 8
    writer.write_byte(1); // 1 layer
    writer.write_byte(0); // Palette header: is_runtime = 0 (disk NBT palette)

    let bytes = writer.into_bytes();
    let result = SubChunk::deserialize_network(&bytes);
    assert_eq!(result, Err(SubChunkNetworkError::NonRuntimePalette));
}

#[test]
fn test_truncated_data_error() {
    let bytes = vec![8]; // Only version byte, missing layer count
    let result = SubChunk::deserialize_network(&bytes);
    assert!(result.is_err());
}
