//! Measures the ground under every column, so a plate of land with empty space
//! beneath it (the "floating land" report) can be confirmed or ruled out with
//! numbers instead of a guess.
//!
//!   cargo run -p bedrock-world --example column_dump -- <seed> <x0> <z0> <w> <h>
//!
//! For each column it finds the surface block (the highest one that is not a
//! plant), then counts how many solid blocks sit under it before air or water:
//! a thin crust is what makes a surface look like it floats. Leaves, grass and
//! water plants are ignored, otherwise a tree canopy reads as a hole.

use std::collections::HashMap;

use bedrock_world::block_id;
use bedrock_world::block_states::state_name;
use bedrock_world::generator::bds_terrain::BDSTerrain;
use bedrock_world::Chunk;

fn name(id: u32) -> &'static str {
    state_name(id).unwrap_or("?")
}

fn is_decoration(id: u32) -> bool {
    block_id::is_water_plant(id)
        || matches!(
            id,
            block_id::SHORT_GRASS
                | block_id::TALL_GRASS
                | block_id::DANDELION
                | block_id::POPPY
                | block_id::SNOW_LAYER
                | block_id::OAK_LEAVES
                | block_id::SPRUCE_LEAVES
                | block_id::BIRCH_LEAVES
        )
}

/// Highest non-decoration block of the column, and how many solid blocks sit
/// under it before air or water.
fn surface(chunk: &Chunk, lx: usize, lz: usize) -> Option<(i32, i32)> {
    let top = (0..128)
        .rev()
        .find(|&y| {
            let id = chunk.get_block(lx, y, lz);
            id != block_id::AIR && id != block_id::WATER && id != block_id::ICE && !is_decoration(id)
        })?;
    let mut thickness = 0;
    let mut y = top;
    while y >= 0 {
        let id = chunk.get_block(lx, y, lz);
        if id == block_id::AIR || id == block_id::WATER {
            break;
        }
        thickness += 1;
        y -= 1;
    }
    Some((top, thickness))
}

fn dump(chunk: &Chunk, x: i32, z: i32) {
    let (lx, lz) = ((x & 15) as usize, (z & 15) as usize);
    println!("--- columna ({x}, {z}) ---");
    let mut y = 127;
    while y >= 0 {
        let id = chunk.get_block(lx, y, lz);
        let mut low = y;
        while low > 0 && chunk.get_block(lx, low - 1, lz) == id {
            low -= 1;
        }
        println!("  {low:>3}-{y:>3} {}", name(id));
        y = low - 1;
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let x0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(612);
    let z0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(-76);
    let w: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(72);
    let h: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(72);
    let asked: Vec<i32> = args.filter_map(|s| s.parse().ok()).collect();

    let generator = BDSTerrain::new(seed);
    let mut chunks: HashMap<(i32, i32), Chunk> = HashMap::new();
    let mut thin = Vec::new();
    let mut columns = 0usize;
    let mut min_thickness = i32::MAX;

    for z in z0..z0 + h {
        for x in x0..x0 + w {
            let key = (x >> 4, z >> 4);
            let chunk = chunks
                .entry(key)
                .or_insert_with(|| generator.generate_chunk_internal(key.0, key.1));
            let (lx, lz) = ((x & 15) as usize, (z & 15) as usize);
            let Some((top, thickness)) = surface(chunk, lx, lz) else {
                continue;
            };
            columns += 1;
            min_thickness = min_thickness.min(thickness);
            if thickness <= 2 {
                thin.push((x, z, top, thickness, name(chunk.get_block(lx, top, lz))));
            }
        }
    }

    println!(
        "columnas: {columns} | costra minima: {min_thickness} | con costra de 2 bloques o menos: {}",
        thin.len()
    );

    // Where the water plants sit: they must be inside the water, with water (or
    // more plant) above and below, never in the air.
    let mut plants = 0usize;
    let mut shown = 0usize;
    for (key, chunk) in chunks.iter() {
        for lx in 0..16usize {
            for lz in 0..16usize {
                let (x, z) = (key.0 * 16 + lx as i32, key.1 * 16 + lz as i32);
                for y in 1..127 {
                    if !block_id::is_water_plant(chunk.get_block(lx, y, lz)) {
                        continue;
                    }
                    plants += 1;
                    if shown < 5 {
                        shown += 1;
                        println!(
                            "  planta ({x}, {y}, {z}) {} | arriba: {} | abajo: {}",
                            name(chunk.get_block(lx, y, lz)),
                            name(chunk.get_block(lx, y + 1, lz)),
                            name(chunk.get_block(lx, y - 1, lz))
                        );
                    }
                }
            }
        }
    }
    println!("plantas acuaticas en la ventana: {plants}");
    for (x, z, top, thickness, block) in thin.iter().take(12) {
        println!("  ({x}, {z}) superficie={top} {block} costra={thickness}");
    }

    let mut targets = vec![(644, -44), (643, -44), (645, -44), (644, -45), (644, -43), (649, -44)];
    for pair in asked.chunks(2) {
        if let [x, z] = pair {
            targets.push((*x, *z));
        }
    }
    for (x, z) in targets {
        if let Some(chunk) = chunks.get(&(x >> 4, z >> 4)) {
            dump(chunk, x, z);
        }
    }
}
