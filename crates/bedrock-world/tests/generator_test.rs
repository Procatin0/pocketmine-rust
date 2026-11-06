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
    assert!(spawn_y >= NormalGenerator::SEA_LEVEL);

    let (player_x, player_y, player_z) = generator.player_position();
    assert_eq!(player_x, spawn_x as f32 + 0.5);
    assert_eq!(player_z, spawn_z as f32 + 0.5);
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

#[test]
fn test_world_generation_verification_suite() {
    // This suite compares against a PHP exporter fixture. A clean checkout does
    // not ship it, so skip rather than reporting NaN rates as a failure.
    let exporter_available = std::path::Path::new("/tmp/export_chunk.php").exists()
        && std::process::Command::new("php")
            .arg("-v")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
    if !exporter_available {
        eprintln!("Skipping world generation verification: PHP exporter fixture not available");
        return;
    }

    let seeds = [12345i64, 67890, 42, 1337];
    let chunks = [(0, 0), (1, 0), (0, 1), (-1, -1), (2, -3)];

    let mut total_blocks = 0;
    let mut total_exact_matches = 0;
    let mut total_height_matches = 0;
    let mut total_biome_matches = 0;
    let mut total_columns = 0;

    println!("\n=======================================================");
    println!("     POCKETMINE-RUST WORLD GENERATION VERIFICATION     ");
    println!("=======================================================");

    for &seed in &seeds {
        let gen = NormalGenerator::new(seed);

        for &(cx, cz) in &chunks {
            let chunk_bin = format!("/tmp/chunk_{}_{}_{}.bin", seed, cx, cz);
            // Run PHP export
            let status = std::process::Command::new("php")
                .arg("/tmp/export_chunk.php")
                .arg(seed.to_string())
                .arg(cx.to_string())
                .arg(cz.to_string())
                .arg("1") // with populator
                .arg(&chunk_bin)
                .status();

            if status.is_err() || !status.unwrap().success() {
                eprintln!("Failed to run PHP exporter for seed {} chunk ({}, {})", seed, cx, cz);
                continue;
            }

            let data = match std::fs::read(&chunk_bin) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Failed to read {}: {}", chunk_bin, e);
                    continue;
                }
            };

            let rust_chunk = gen.generate_chunk_internal(cx, cz);

            let php_biomes = &data[0..256];
            let php_dominant = data[256];
            let php_highest = &data[257..513];
            let php_blocks = &data[513..33281];

            let mut chunk_exact = 0;
            let mut chunk_height = 0;
            let mut chunk_biome = 0;

            for x in 0..16 {
                for z in 0..16 {
                    let col_idx = x * 16 + z;
                    total_columns += 1;

                    if rust_chunk.biomes[col_idx] == php_biomes[col_idx] {
                        chunk_biome += 1;
                        total_biome_matches += 1;
                    }

                    let rust_h = rust_chunk.get_highest_block_at(x, z).unwrap_or(0) as u8;
                    let php_h = php_highest[col_idx];
                    if rust_h == php_h {
                        chunk_height += 1;
                        total_height_matches += 1;
                    }

                    for y in 0..128 {
                        total_blocks += 1;
                        let block_idx = col_idx * 128 + y;
                        let php_b = php_blocks[block_idx];
                        let rust_b = map_rust_block(rust_chunk.get_block(x, y as i32, z));
                        if rust_b == php_b {
                            chunk_exact += 1;
                            total_exact_matches += 1;
                        }
                    }
                }
            }

            let block_rate = chunk_exact as f64 / (16.0 * 16.0 * 128.0) * 100.0;
            let height_rate = chunk_height as f64 / 256.0 * 100.0;
            let biome_rate = chunk_biome as f64 / 256.0 * 100.0;

            println!(
                "Seed {:>5} Chunk ({:>2}, {:>2}): Biome {:>5.1}% (dom Rust={} PHP={}), Height {:>5.1}%, Blocks {:>6.2}%",
                seed, cx, cz, biome_rate, rust_chunk.biomes[7 * 16 + 7], php_dominant, height_rate, block_rate
            );
        }
    }

    let overall_block_rate = total_exact_matches as f64 / total_blocks as f64 * 100.0;
    let overall_height_rate = total_height_matches as f64 / total_columns as f64 * 100.0;
    let overall_biome_rate = total_biome_matches as f64 / total_columns as f64 * 100.0;

    println!("-------------------------------------------------------");
    println!("OVERALL RESULTS (across {} chunks, {} blocks):", chunks.len() * seeds.len(), total_blocks);
    println!("Biome Consistency:       {:>6.2}%", overall_biome_rate);
    println!("Elevation Height Match:  {:>6.2}%", overall_height_rate);
    println!("Exact Block Alignment:   {:>6.2}%", overall_block_rate);
    println!("=======================================================\n");

    assert!(overall_biome_rate >= 99.99, "Biome consistency must be 100%");
    assert!(overall_height_rate >= 99.99, "Elevation height match rate must be 100%");
    assert!(overall_block_rate >= 99.99, "Exact block alignment rate must be 100%");
}

#[test]
fn test_terrain_column_consistency() {
    let seeds = [12345i64, 67890, 42, 1337];
    let chunks = [(0, 0), (1, 0), (0, 1), (-1, -1), (2, -3)];

    println!("\n=== TERRAIN COLUMN CONSISTENCY (Stone/Dirt/Water) ===");
    for &seed in &seeds {
        let gen = NormalGenerator::new(seed);
        for &(cx, cz) in &chunks {
            let chunk_bin = format!("/tmp/chunk_{}_{}_{}.bin", seed, cx, cz);
            let data = match std::fs::read(&chunk_bin) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let rust_chunk = gen.generate_chunk_internal(cx, cz);
            let php_blocks = &data[513..33281];

            let mut matched_columns = 0;
            for x in 0..16 {
                for z in 0..16 {
                    let col_idx = x * 16 + z;
                    // Find highest stone, highest dirt, highest water in PHP
                    let mut php_stone_h = -1;
                    let mut php_dirt_h = -1;
                    let mut php_water_h = -1;
                    for y in 0..128 {
                        let b = php_blocks[col_idx * 128 + y];
                        if b == 1 { php_stone_h = y as i32; }
                        else if b == 2 || b == 3 { php_dirt_h = y as i32; }
                        else if b == 5 { php_water_h = y as i32; }
                    }

                    // In Rust
                    let mut rust_stone_h = -1;
                    let mut rust_dirt_h = -1;
                    let mut rust_water_h = -1;
                    for y in 0..128 {
                        let b = map_rust_block(rust_chunk.get_block(x, y as i32, z));
                        if b == 1 { rust_stone_h = y as i32; }
                        else if b == 2 || b == 3 { rust_dirt_h = y as i32; }
                        else if b == 5 { rust_water_h = y as i32; }
                    }

                    if php_stone_h == rust_stone_h && php_dirt_h == rust_dirt_h && php_water_h == rust_water_h {
                        matched_columns += 1;
                    } else {
                        println!("  Mismatch at ({}, {}): PHP stone={} dirt={} water={} | Rust stone={} dirt={} water={}",
                            x, z, php_stone_h, php_dirt_h, php_water_h, rust_stone_h, rust_dirt_h, rust_water_h);
                    }
                }
            }
            let col_match_rate = matched_columns as f64 / 256.0 * 100.0;
            println!("Seed {:>5} Chunk ({:>2}, {:>2}): Terrain Column Match: {:>5.1}%", seed, cx, cz, col_match_rate);
        }
    }
}

#[test]
fn test_dump_chunk_12345() {
    let gen = NormalGenerator::new(12345);
    let chunk = gen.generate_chunk_internal(0, 0);

    let bin_path = "/tmp/test_chunk.bin";
    if let Ok(data) = std::fs::read(bin_path) {
        let php_biomes = &data[0..256];
        let php_dominant = data[256];
        let php_highest = &data[257..513];
        let php_blocks = &data[513..33281];

        let mut exact_matches = 0;
        let mut height_matches = 0;
        let mut biome_matches = 0;
        let total_blocks = 16 * 16 * 128;

        for x in 0..16 {
            for z in 0..16 {
                let col_idx = x * 16 + z;
                let rust_biome = chunk.biomes[col_idx];
                if rust_biome == php_biomes[col_idx] {
                    biome_matches += 1;
                }

                let rust_h = chunk.get_highest_block_at(x, z).unwrap_or(0) as u8;
                let php_h = php_highest[col_idx];
                if rust_h == php_h {
                    height_matches += 1;
                }

                for y in 0..128 {
                    let block_idx = col_idx * 128 + y;
                    let php_b = php_blocks[block_idx];
                    let rust_b = map_rust_block(chunk.get_block(x, y as i32, z));
                    if rust_b == php_b {
                        exact_matches += 1;
                    } else {
                        println!("Diff at ({}, {}, {}): Rust={}, PHP={}", x, y, z, rust_b, php_b);
                    }
                }
            }
        }

        println!("=== Comparison for Seed 12345 Chunk (0,0) ===");
        println!("Dominant Biome: Rust={}, PHP={}", chunk.biomes[7 * 16 + 7], php_dominant);
        println!("Biome Match: {}/256 ({:.2}%)", biome_matches, biome_matches as f64 / 256.0 * 100.0);
        println!("Height Match: {}/256 ({:.2}%)", height_matches, height_matches as f64 / 256.0 * 100.0);
        println!("Exact Block Match: {}/{} ({:.2}%)", exact_matches, total_blocks, exact_matches as f64 / total_blocks as f64 * 100.0);
    }
}

fn map_rust_block(id: u32) -> u8 {
    use bedrock_world::block_id;
    match id {
        block_id::AIR => 0,
        block_id::STONE => 1,
        block_id::DIRT => 2,
        block_id::GRASS_BLOCK => 3,
        block_id::BEDROCK => 4,
        block_id::WATER => 5,
        block_id::SAND => 6,
        block_id::SANDSTONE => 7,
        block_id::GRAVEL => 8,
        block_id::COAL_ORE => 9,
        block_id::IRON_ORE => 10,
        block_id::GOLD_ORE => 11,
        block_id::REDSTONE_ORE => 12,
        block_id::LAPIS_ORE => 13,
        block_id::DIAMOND_ORE => 14,
        block_id::OAK_LOG => 15,
        block_id::OAK_LEAVES => 16,
        block_id::SPRUCE_LOG => 17,
        block_id::SPRUCE_LEAVES => 18,
        block_id::BIRCH_LOG => 19,
        block_id::BIRCH_LEAVES => 20,
        block_id::SNOW_LAYER => 21,
        block_id::SHORT_GRASS | block_id::TALL_GRASS => 22,
        block_id::EMERALD_ORE => 23,
        _ => 255,
    }
}
