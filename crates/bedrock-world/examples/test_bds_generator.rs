//! Test BDS Generator integration

use bedrock_world::generator::{create_generator, bedrock_string_seed_to_u64, BDSTerrain, WorldGenerator};

fn main() {
    println!("=== Testing BDS Generator Integration ===\n");

    // Test 1: BDS Generator with numeric seed via factory
    println!("1. BDS Generator with numeric seed (12345) via factory:");
    let gen = create_generator("bds", 12345);
    println!("   Seed: {}", gen.seed());
    println!("   Generator type: {}", gen.generator_type());
    println!("   Spawn position: {:?}", gen.spawn_position());
    
    let (sub_chunks, payload) = gen.generate_chunk(0, 0);
    println!("   Chunk (0,0): {} subchunks, {} bytes", sub_chunks, payload.len());
    
    // Test 2: String seed via MurmurHash3 (Bedrock style)
    println!("\n2. BDS Generator with string seed ('testworld') via factory:");
    let string_seed = bedrock_string_seed_to_u64("testworld");
    println!("   String seed 'testworld' -> {}", string_seed);
    
    let gen2 = create_generator("bds", string_seed);
    println!("   Seed: {}", gen2.seed());
    println!("   Generator type: {}", gen2.generator_type());
    println!("   Spawn position: {:?}", gen2.spawn_position());
    
    // Test 3: Direct BDSTerrain for detailed analysis
    println!("\n3. Direct BDSTerrain analysis (seed 12345):");
    let bds = BDSTerrain::new(12345);
    let h = bds.height_at(0, 0);
    println!("   Height at (0,0): {}", h);
    
    // Sample a few heights
    for x in [-100, -50, 0, 50, 100] {
        let h = bds.height_at(x, 0);
        println!("   Height at ({}, 0): {}", x, h);
    }
    
    // Test 4: Verify different seeds produce different terrain
    println!("\n4. Seed variation test:");
    let bds_a = BDSTerrain::new(100);
    let bds_b = BDSTerrain::new(200);
    
    let mut heights_a = Vec::new();
    let mut heights_b = Vec::new();
    
    for x in 0..20 {
        heights_a.push(bds_a.height_at(x * 10, 0));
        heights_b.push(bds_b.height_at(x * 10, 0));
    }
    println!("   Seed 100 heights: {:?}", heights_a);
    println!("   Seed 200 heights: {:?}", heights_b);
    assert_ne!(heights_a, heights_b, "Different seeds should produce different terrain!");
    println!("   ✓ Different seeds produce different terrain");
    
    // Test 5: Biome variation
    println!("\n5. Biome variation test (using block_at at surface):");
    let bds_c = BDSTerrain::new(42);
    let mut biomes = std::collections::HashSet::new();
    for x in -50..=50 {
        for z in -50..=50 {
            // Use block_at at surface level to infer biome
            let h = bds_c.height_at(x, z);
            let block = bds_c.block_at(x, h + 1, z);
            biomes.insert(block);
        }
    }
    println!("   Unique surface blocks in 101x101 area: {} (block IDs: {:?})", biomes.len(), biomes);
    
    // Test 6: Test flat generator still works
    println!("\n6. Flat generator (regression test):");
    let flat = create_generator("flat", 0);
    println!("   Generator type: {}", flat.generator_type());
    let (sub_chunks, payload) = flat.generate_chunk(0, 0);
    println!("   Chunk (0,0): {} subchunks, {} bytes", sub_chunks, payload.len());
    
    // Test 7: Test normal generator still works
    println!("\n7. Normal generator (regression test):");
    let normal = create_generator("normal", 12345);
    println!("   Generator type: {}", normal.generator_type());
    let (sub_chunks, payload) = normal.generate_chunk(0, 0);
    println!("   Chunk (0,0): {} subchunks, {} bytes", sub_chunks, payload.len());
    
    // Test 8: Verify sea level constant
    println!("\n8. Sea level verification:");
    println!("   BDS_SEA_LEVEL = {}", bedrock_world::generator::BDS_SEA_LEVEL);
    assert_eq!(bedrock_world::generator::BDS_SEA_LEVEL, 62, "Bedrock sea level must be 62");
    println!("   ✓ Sea level is correctly 62");
    
    println!("\n=== All tests passed! ===");
}