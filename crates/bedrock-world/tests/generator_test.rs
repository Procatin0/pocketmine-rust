use bedrock_world::{FlatGenerator, NormalGenerator, WorldGenerator};

#[test]
fn test_flat_generator_contract() {
    let generator = FlatGenerator::new();
    assert_eq!(generator.generator_type(), 1); // FLAT
    assert_eq!(generator.spawn_position(), (0, 4, 0));
    assert_eq!(generator.player_position(), (0.5, 5.62, 0.5));

    let (sub_chunk_count, payload) = generator.generate_chunk(0, 0);
    assert_eq!(sub_chunk_count, 5);
    assert!(!payload.is_empty());

    // Verify subchunks -4..-1 are empty [0x08, 0x00]
    // 4 empty subchunks * 2 bytes = 8 bytes prefix: [8, 0, 8, 0, 8, 0, 8, 0]
    assert_eq!(&payload[0..8], &[8, 0, 8, 0, 8, 0, 8, 0]);

    // Chunk (10, -5) has identical payload in flat world
    let (sc2, p2) = generator.generate_chunk(10, -5);
    assert_eq!(sc2, 5);
    assert_eq!(payload, p2);
}

#[test]
fn test_normal_generator_contract() {
    let generator = NormalGenerator::new(12345);
    assert_eq!(generator.generator_type(), 2); // DEFAULT / INFINITE

    let (spawn_x, spawn_y, spawn_z) = generator.spawn_position();
    assert_eq!(spawn_x, 0);
    assert_eq!(spawn_z, 0);
    assert!(spawn_y >= NormalGenerator::SEA_LEVEL);

    let (player_x, player_y, player_z) = generator.player_position();
    assert_eq!(player_x, 0.5);
    assert_eq!(player_z, 0.5);
    assert_eq!(player_y, spawn_y as f32 + 1.62);

    let (sub_chunk_count, payload) = generator.generate_chunk(0, 0);
    assert!(sub_chunk_count >= 5);
    assert!(!payload.is_empty());

    // Verify that subchunks -4..-1 are empty in normal world payload [8, 0, 8, 0, 8, 0, 8, 0]
    assert_eq!(&payload[0..8], &[8, 0, 8, 0, 8, 0, 8, 0]);
}

#[test]
fn test_biome_selector() {
    use bedrock_world::generator::biome::BiomeSelector;
    use bedrock_world::generator::random::Random;

    let mut random = Random::new(12345);
    let selector = BiomeSelector::new(&mut random);
    let b = selector.pick_biome(0, 0);
    assert!(b <= 30);
}
