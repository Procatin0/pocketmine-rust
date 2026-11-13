//! Prints the biome distribution of the BDS terrain generator over a block
//! area, using the real 1.18+ climate KD-tree pipeline (`generate_base_chunk`).

use bedrock_world::generator::BDSTerrain;
use bedrock_world::generator::biome::get_biome_info;
use std::collections::BTreeMap;

fn main() {
    let bds = BDSTerrain::new(12345);

    // 12x12 chunks centred on spawn: 192x192 blocks.
    let half = 6;
    let mut counts: BTreeMap<u8, usize> = BTreeMap::new();
    let mut columns = 0usize;

    for cx in -half..half {
        for cz in -half..half {
            let chunk = bds.generate_base_chunk(cx, cz);
            for &b in chunk.biomes.iter() {
                *counts.entry(b).or_insert(0) += 1;
                columns += 1;
            }
        }
    }

    println!("area: {}x{} chunks ({} columns)", half * 2, half * 2, columns);
    println!("{:>4} {:<24} {:>8} {:>7}", "id", "name", "cols", "%");
    for (id, n) in &counts {
        let info = get_biome_info(*id);
        println!("{:>4} {:<24} {:>8} {:>6.2}%", id, info.name, n, 100.0 * *n as f64 / columns as f64);
    }

    // Also show a strip of biomes along the x axis at z=0 for a quick look at
    // how the map behaves away from spawn.
    println!("\nbiome ids along z=0, x in [-2048, 2048] step 64:");
    for x in (-2048..=2048).step_by(64) {
        let chunk = bds.generate_base_chunk(x >> 4, 0);
        print!("{:>3} ", chunk.biomes[(x & 15) as usize * 16]);
    }
    println!();
}
