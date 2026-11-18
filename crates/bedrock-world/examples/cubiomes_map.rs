//! Renders the biome map cubiomes itself reports for a window, in the same
//! colours as `map_render`, so the two images can be compared directly.
//!
//! `cubiomes_map <seed> <size> <out.ppm> [x0] [z0]`

use bedrock_world::generator::bds_biome_tree::java_to_bedrock;
use bedrock_world::generator::biome;
use bedrock_world::generator::cubiomes::Cubiomes;
use std::io::Write;

fn colour(id: u8) -> [u8; 3] {
    const OCEANS: [u8; 9] = [0, 24, 40, 41, 42, 43, 44, 45, 46];
    const BADLANDS: [u8; 6] = [37, 38, 39, 165, 166, 167];
    if OCEANS.contains(&id) || id == 47 {
        return [60, 120, 200];
    }
    if BADLANDS.contains(&id) {
        return [206, 118, 56];
    }
    match id {
        biome::RIVER | biome::FROZEN_RIVER => [80, 130, 210],
        biome::DESERT => [226, 206, 140],
        biome::BEACH => [226, 214, 170],
        biome::SNOWY_BEACH | biome::ICE_PLAINS | biome::ICE_SPIKES => [242, 246, 250],
        biome::SNOWY_TAIGA => [150, 180, 190],
        biome::JUNGLE | biome::JUNGLE_EDGE | biome::BAMBOO_JUNGLE => [40, 96, 40],
        biome::SAVANNA | biome::SAVANNA_PLATEAU => [150, 150, 70],
        biome::BADLANDS_PLATEAU => [206, 118, 56],
        biome::STONE_SHORE => [122, 122, 122],
        _ => [96, 158, 62],
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let size: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(768);
    let out = args.next().unwrap_or_else(|| "cubiomes.ppm".to_string());
    let x0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let z0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);

    let generator = Cubiomes::new(seed).expect("cubiomes");
    let mut ppm = Vec::with_capacity(size * size * 3 + 32);
    write!(ppm, "P6\n{size} {size}\n255\n").unwrap();
    let mut counts: std::collections::BTreeMap<u8, usize> = Default::default();
    let mut names: std::collections::BTreeMap<u8, u8> = Default::default();
    for j in 0..size {
        for i in 0..size {
            let x = x0 + i as i32;
            let z = z0 + j as i32;
            // Vanilla's own biome scale: one sample per 4 blocks.
            let java = generator.biome_at(x & !3, 63, z & !3).unwrap_or(0);
            let id = java_to_bedrock(java);
            *counts.entry(id).or_default() += 1;
            *names.entry(id).or_insert(java);
            ppm.extend_from_slice(&colour(id));
        }
    }
    std::fs::write(&out, ppm).unwrap();
    let total = (size * size) as f64;
    let mut top: Vec<_> = counts.into_iter().collect();
    top.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    for (id, n) in top.iter().take(12) {
        println!(
            "{id:>4} {:>9} {:>6.2}%  {}",
            n,
            *n as f64 / total * 100.0,
            {
                let _ = names;
                bedrock_world::generator::cubiomes::Cubiomes::biome_name(*id)
            }
        );
    }
    println!("wrote {out} ({size}x{size}, seed {seed}, origin {x0},{z0})");
}
