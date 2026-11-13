//! Benchmark chunk generation performance

use bedrock_world::generator::{create_generator, BDSTerrain, WorldGenerator};
use std::time::Instant;

fn main() {
    println!("=== Chunk Generation Performance Benchmark ===\n");
    
    // Test with BDSTerrain directly
    let bds = BDSTerrain::new(12345);
    
    // Warm up
    println!("Warming up...");
    for _ in 0..5 {
        let _ = bds.generate_chunk_internal(0, 0);
    }
    
    // Benchmark single chunk generation
    println!("\nBenchmarking single chunk generation (base chunk)...");
    let iterations = 50u32;
    let start = Instant::now();
    for i in 0..iterations {
        let _ = bds.generate_base_chunk(i as i32, 0);
    }
    let elapsed = start.elapsed();
    let per_chunk = elapsed / iterations;
    println!("  {} iterations in {:.2?}", iterations, elapsed);
    println!("  Per chunk: {:.2?} ({:.2} chunks/sec)", per_chunk, iterations as f64 / elapsed.as_secs_f64());
    
    // Benchmark full chunk with population
    println!("\nBenchmarking full chunk generation (with population)...");
    let start = Instant::now();
    for i in 0..iterations {
        let _ = bds.generate_chunk_internal(i as i32, 100);
    }
    let elapsed = start.elapsed();
    let per_chunk = elapsed / iterations;
    println!("  {} iterations in {:.2?}", iterations, elapsed);
    println!("  Per chunk: {:.2?} ({:.2} chunks/sec)", per_chunk, iterations as f64 / elapsed.as_secs_f64());
    
    // Benchmark via factory (WorldGenerator trait)
    println!("\nBenchmarking via create_generator(\"bds\")...");
    let gen = create_generator("bds", 12345);
    let start = Instant::now();
    for i in 0..iterations {
        let _ = gen.generate_chunk(i as i32, 200);
    }
    let elapsed = start.elapsed();
    let per_chunk = elapsed / iterations;
    println!("  {} iterations in {:.2?}", iterations, elapsed);
    println!("  Per chunk: {:.2?} ({:.2} chunks/sec)", per_chunk, iterations as f64 / elapsed.as_secs_f64());
    
    // Benchmark height_at calls
    println!("\nBenchmarking height_at (1000 calls)...");
    let height_iterations = 1000u32;
    let start = Instant::now();
    for i in 0..height_iterations {
        let _ = bds.height_at(i as i32 * 10, 0);
    }
    let elapsed = start.elapsed();
    println!("  {} height_at calls in {:.2?}", height_iterations, elapsed);
    println!("  Per call: {:.2?} ({:.2} calls/sec)", elapsed / height_iterations, height_iterations as f64 / elapsed.as_secs_f64());
    
    // Test cached column access
    println!("\nBenchmarking cached column access...");
    let start = Instant::now();
    for i in 0..100 {
        let _ = bds.block_at(i, 64, 0);
    }
    let elapsed = start.elapsed();
    println!("  100 block_at calls (cached): {:.2?}", elapsed);
    
    println!("\n=== Done ===");
}