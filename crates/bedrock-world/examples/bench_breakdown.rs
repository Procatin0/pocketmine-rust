//! Breakdown benchmark for the chunk generation pipeline.
//!
//! Times the pieces the server pays per column: base terrain, population,
//! network serialization and the per-column climate/height sampling, for both
//! generators. Run with `--release`.

use std::time::Instant;

use bedrock_world::generator::{create_generator, BDSTerrain};

fn per_chunk(total: std::time::Duration, count: u32) -> f64 {
    total.as_secs_f64() * 1000.0 / f64::from(count)
}

fn main() {
    let seed = 12345u64;
    let count = 64u32;

    let bds = BDSTerrain::new(seed);
    println!("=== BDSTerrain (default level-type) ===");

    // Cold base terrain: 64 distinct columns.
    let start = Instant::now();
    for i in 0..count {
        let _ = bds.generate_base_chunk(400 + i as i32, 400);
    }
    let base = start.elapsed();
    println!("  base        {:7.2} ms/chunk", per_chunk(base, count));

    // Per-column climate + height only (no fill, no biome map).
    let start = Instant::now();
    for i in 0..count {
        let x = i as i32 * 16;
        for z in 0..16 {
            let _ = bds.height_at(x, z);
        }
    }
    let heights = start.elapsed();
    println!(
        "  height_at   {:7.2} ms/chunk (256 calls)",
        per_chunk(heights, count)
    );

    // Full column on one warm coordinate: population + 9 base clones only.
    let start = Instant::now();
    for _ in 0..count {
        let _ = bds.generate_chunk_internal(400, 400);
    }
    let warm = start.elapsed();
    println!(
        "  full warm   {:7.2} ms/chunk (same column, base cached)",
        per_chunk(warm, count)
    );

    // Full column: base + population (base cache warm from the pass above).
    let start = Instant::now();
    for i in 0..count {
        let _ = bds.generate_chunk_internal(400 + i as i32, 400);
    }
    let full = start.elapsed();
    println!("  full        {:7.2} ms/chunk", per_chunk(full, count));

    // Network payload for an already-built chunk.
    let chunk = bds.generate_base_chunk(0, 0);
    let start = Instant::now();
    for _ in 0..count {
        std::hint::black_box(chunk.clone());
    }
    let clone = start.elapsed();
    println!("  clone       {:7.2} ms/chunk", per_chunk(clone, count));
    let dominant = chunk.biomes[7 * 16 + 7];
    let start = Instant::now();
    for _ in 0..count {
        let _ = chunk.serialize_network_payload(dominant);
    }
    let serialize = start.elapsed();
    println!(
        "  serialize   {:7.2} ms/chunk",
        per_chunk(serialize, count)
    );

    // Full trait path (what the pipeline calls).
    let gen = create_generator("bds", seed);
    let start = Instant::now();
    for i in 0..count {
        let _ = gen.generate_chunk(900 + i as i32, 900);
    }
    let trait_full = start.elapsed();
    println!(
        "  generate_chunk (trait) {:7.2} ms/chunk \
         ({:.0} chunks/s/core)",
        per_chunk(trait_full, count),
        f64::from(count) / trait_full.as_secs_f64()
    );

    // Region generation: 16x16 columns with one base build per neighbour.
    let start = Instant::now();
    let region = bds.generate_region(4000, 4000, 16, 16);
    let region_elapsed = start.elapsed();
    println!(
        "  region 16x16 {:7.2} ms/chunk ({} columns in {:.2?})",
        region_elapsed.as_secs_f64() * 1000.0 / region.len() as f64,
        region.len(),
        region_elapsed
    );
    std::hint::black_box(&region);

    // LOD tile: one 16x16-block tile, one sample every 4 blocks (4x4
    // samples), height + dominant biome. No fill, no populate.
    let tiles = 512usize;
    let start = Instant::now();
    let mut lod_bytes = 0usize;
    for i in 0..tiles {
        let tx = 8000 + (i % 32) as i32 * 16;
        let tz = 8000 + (i / 32) as i32 * 16;
        let mut tile = Vec::with_capacity(16);
        for sx in 0..4 {
            for sz in 0..4 {
                let x = tx + sx * 4;
                let z = tz + sz * 4;
                tile.push((bds.height_at(x, z) as u16, u32::from(bds.biome_at(x, z))));
            }
        }
        lod_bytes += tile.len() * 6;
        std::hint::black_box(&tile);
    }
    let lod = start.elapsed();
    let per_tile = lod.as_secs_f64() * 1000.0 / tiles as f64;
    println!(
        "  lod tile 16x16 {:7.3} ms/tile (4x4 samples, {} B/tile)",
        per_tile, lod_bytes / tiles
    );
    println!(
        "  1M tiles: {:.1} s single-thread, {:.1} s at 16 threads",
        per_tile * 1_000_000.0 / 1000.0,
        per_tile * 1_000_000.0 / 1000.0 / 16.0
    );

    // Fill micro-benchmark: 256 columns x 120 blocks of stone.
    use bedrock_world::block_id::STONE;
    use bedrock_world::chunk::Chunk;
    let mut filled = Chunk::new(1);
    let start = Instant::now();
    for x in 0..16usize {
        for z in 0..16usize {
            filled.fill_column(x, z, 1, 120, STONE);
        }
    }
    println!("  fill_column {:7.2} ms/256 cols", start.elapsed().as_secs_f64() * 1000.0);
    let mut set = Chunk::new(1);
    let start = Instant::now();
    for x in 0..16usize {
        for z in 0..16usize {
            for y in 1..=120 {
                set.set_block(x, y, z, STONE);
            }
        }
    }
    println!("  set_block   {:7.2} ms/256 cols", start.elapsed().as_secs_f64() * 1000.0);
    std::hint::black_box(&filled);
    std::hint::black_box(&set);

    println!("\n=== NormalGenerator (level-type DEFAULT) ===");
    let normal = create_generator("normal", seed);
    let start = Instant::now();
    for i in 0..count {
        let _ = normal.generate_chunk(1200 + i as i32, 1200);
    }
    let normal_full = start.elapsed();
    println!(
        "  generate_chunk {:7.2} ms/chunk ({:.0} chunks/s/core)",
        per_chunk(normal_full, count),
        f64::from(count) / normal_full.as_secs_f64()
    );

    // Parallel throughput, per-column jobs (what the server pipeline does).
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    println!("\n=== Parallel per-column jobs (cores: {cores}) ===");
    for threads in [1usize, 4, 8, cores] {
        let start = Instant::now();
        let per_thread = 32;
        std::thread::scope(|scope| {
            for t in 0..threads {
                let gen = create_generator("bds", seed);
                scope.spawn(move || {
                    for i in 0..per_thread {
                        let _ = gen.generate_chunk(
                            2000 + (t as i32) * 64 + i as i32,
                            2000,
                        );
                    }
                });
            }
        });
        let elapsed = start.elapsed();
        let total = threads as u32 * per_thread;
        println!(
            "  {threads:2} threads: {:.1} chunks/s ({:.2} ms/chunk)",
            f64::from(total) / elapsed.as_secs_f64(),
            elapsed.as_secs_f64() * 1000.0 * threads as f64 / f64::from(total)
        );
    }

    // Parallel region jobs (16x16 tiles): one base build per neighbour.
    println!("\n=== Parallel region jobs (16x16) ===");
    for threads in [1usize, 4, 8, cores] {
        let start = Instant::now();
        std::thread::scope(|scope| {
            for t in 0..threads {
                let gen = create_generator("bds", seed);
                scope.spawn(move || {
                    let _ = gen.generate_region(5000 + (t as i32) * 32, 5000, 16, 16);
                });
            }
        });
        let elapsed = start.elapsed();
        let total = threads as u32 * 256;
        println!(
            "  {threads:2} threads: {:.1} chunks/s ({:.2} ms/chunk)",
            f64::from(total) / elapsed.as_secs_f64(),
            elapsed.as_secs_f64() * 1000.0 * threads as f64 / f64::from(total)
        );
    }
}
