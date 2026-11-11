//! Block queries and edit-aware chunk generation across generators.
//!
//! Edits passed to `generate_chunk_with_edits` use ABSOLUTE world coordinates.
//! `block_at` only describes generated terrain: persistent edits belong to the
//! server's chunk/world store, not to the generator.

use bedrock_world::{block_id, FlatGenerator, NormalGenerator, WorldGenerator};

#[test]
fn flat_block_at_returns_the_expected_layers() {
    let generator = FlatGenerator::new();

    assert_eq!(generator.block_at(0, 0, 0), block_id::BEDROCK);
    assert_eq!(generator.block_at(0, 1, 0), block_id::DIRT);
    assert_eq!(generator.block_at(0, 2, 0), block_id::DIRT);
    assert_eq!(generator.block_at(0, 3, 0), block_id::GRASS_BLOCK);
    assert_eq!(generator.block_at(0, 4, 0), block_id::AIR);

    // The flat terrain is identical for every column and chunk.
    assert_eq!(generator.block_at(-7, 3, 42), block_id::GRASS_BLOCK);
    assert_eq!(generator.block_at(1000, 0, -1000), block_id::BEDROCK);
    // Outside the world height range there is no terrain.
    assert_eq!(generator.block_at(0, -1, 0), block_id::AIR);
    assert_eq!(generator.block_at(0, 320, 0), block_id::AIR);
}

#[test]
fn flat_payload_is_byte_identical_to_the_legacy_manual_serialization() {
    // Reconstruct the pre-refactor payload by hand to guarantee the Chunk-based
    // generator keeps the exact same wire format (no worldgen/protocol drift).
    use bedrock_protocol::Writer;
    use bedrock_world::SubChunk;

    let mut writer = Writer::new();
    let empty_subchunk = SubChunk::empty();
    for _ in 0..4 {
        empty_subchunk.serialize_network_to(&mut writer);
    }

    let mut subchunk = SubChunk::new(block_id::AIR);
    for x in 0..16 {
        for z in 0..16 {
            subchunk.set_block(x, 0, z, 0, block_id::BEDROCK);
            subchunk.set_block(x, 1, z, 0, block_id::DIRT);
            subchunk.set_block(x, 2, z, 0, block_id::DIRT);
            subchunk.set_block(x, 3, z, 0, block_id::GRASS_BLOCK);
        }
    }
    subchunk.serialize_network_to(&mut writer);

    for _ in 0..24 {
        writer.write_byte(1);
        writer.write_var_i32(1); // Plains
    }
    writer.write_byte(0); // border blocks
    writer.write_var_u32(0); // tiles count (0)

    let legacy_payload = writer.into_bytes().to_vec();
    let generator = FlatGenerator::new();
    assert_eq!(
        generator.generate_chunk(0, 0),
        (5, legacy_payload),
        "flat generator payload must remain byte-identical after refactor"
    );
}

#[test]
fn flat_edits_change_payload_but_not_block_at() {
    let generator = FlatGenerator::new();

    let (base_subchunks, base_payload) = generator.generate_chunk(0, 0);
    let edits = [(1, 4, 1, block_id::STONE)];
    let (edited_subchunks, edited_payload) = generator.generate_chunk_with_edits(0, 0, &edits);

    assert_eq!(edited_subchunks, base_subchunks);
    assert_ne!(edited_payload, base_payload);

    // `block_at` describes the unedited generated terrain. `generate_chunk_with_edits`
    // does not mutate the generator; the server stores the edits alongside the
    // chunk and is responsible for returning them from `world.get_block`.
    assert_eq!(generator.block_at(1, 4, 1), block_id::AIR);
    // And the generator is still stateless afterwards.
    assert_eq!(
        generator.generate_chunk(0, 0),
        (base_subchunks, base_payload)
    );
}

#[test]
fn flat_edits_use_absolute_coordinates_including_negative_chunks() {
    let generator = FlatGenerator::new();

    let (_, base_payload) = generator.generate_chunk(-1, -1);
    let (_, edited_payload) =
        generator.generate_chunk_with_edits(-1, -1, &[(-1, 4, -1, block_id::STONE)]);
    assert_ne!(edited_payload, base_payload);

    // (-17, y, -1) belongs to chunk (-2, -1), so it must not alter chunk (-1, -1).
    let (_, untouched_payload) =
        generator.generate_chunk_with_edits(-1, -1, &[(-17, 4, -1, block_id::STONE)]);
    assert_eq!(untouched_payload, base_payload);
}

#[test]
fn edits_outside_the_chunk_are_ignored() {
    let outside = [
        (16, 4, 0, block_id::STONE),
        (0, 4, -1, block_id::STONE),
        (0, 4, 16, block_id::STONE),
    ];

    let flat = FlatGenerator::new();
    let (flat_subchunks, flat_payload) = flat.generate_chunk(0, 0);
    assert_eq!(
        flat.generate_chunk_with_edits(0, 0, &outside),
        (flat_subchunks, flat_payload)
    );

    let normal = NormalGenerator::new(12345);
    let normal_payload = normal.generate_chunk(0, 0);
    assert_eq!(
        normal.generate_chunk_with_edits(0, 0, &outside),
        normal_payload
    );
}

#[test]
fn normal_block_at_matches_the_generated_chunk() {
    let generator = NormalGenerator::new(12345);
    let (spawn_x, spawn_y, spawn_z) = generator.spawn_position();

    // Spawn query must not panic and must return a valid network id.
    let spawn_block = generator.block_at(spawn_x, spawn_y, spawn_z);
    assert_ne!(spawn_block, 0);

    // Local coherence: block_at(x, y, z) reads the same chunk cell as the
    // generated chunk at local coordinates.
    let chunk = generator.generate_chunk_internal(spawn_x >> 4, spawn_z >> 4);
    let expected = chunk.get_block((spawn_x & 15) as usize, spawn_y, (spawn_z & 15) as usize);
    assert_eq!(spawn_block, expected);

    // Far-away chunks must also be queryable without panicking.
    let far_x = 10000 * 16 + 3;
    let far_z = -10000 * 16 + 5;
    let far_block = generator.block_at(far_x, 70, far_z);
    let far_chunk = generator.generate_chunk_internal(10000, -10000);
    assert_eq!(far_block, far_chunk.get_block(3, 70, (far_z & 15) as usize));
}

#[test]
fn normal_edits_change_only_their_chunk() {
    let generator = NormalGenerator::new(12345);

    let base_payload = generator.generate_chunk(0, 0);
    let edits = [
        (1, 70, 1, block_id::STONE),
        (16, 70, 0, block_id::STONE), // chunk (1, 0), ignored here
    ];
    let edited_payload = generator.generate_chunk_with_edits(0, 0, &edits);
    assert_ne!(edited_payload, base_payload);

    // Serializing again without edits is deterministic and unaffected.
    assert_eq!(generator.generate_chunk(0, 0), base_payload);
}
