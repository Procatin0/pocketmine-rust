//! Audits which biomes cubiomes actually produces against our own tables.
//!
//! `biome_audit [seeds] [radius]` samples the overworld surface of several seeds
//! and reports every id that appears, plus the ones our `get_biome_info` does not
//! know (those fall back to the ocean/gravel entry).

use std::collections::BTreeSet;

use bedrock_world::block_id;
use bedrock_world::generator::bds_biome_tree::java_to_bedrock;
use bedrock_world::generator::biome::get_biome_info;
use bedrock_world::generator::cubiomes::Cubiomes;

fn main() {
    let mut args = std::env::args().skip(1);
    let seeds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(24);
    let radius: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1024);

    let mut seen: BTreeSet<u8> = BTreeSet::new();
    for seed in 0..seeds {
        let Some(generator) = Cubiomes::new(seed) else { continue };
        let mut x = -radius;
        while x < radius {
            let mut z = -radius;
            while z < radius {
                if let Some(java) = generator.biome_at(x, 63, z) {
                    seen.insert(java);
                }
                z += 16;
            }
            x += 16;
        }
    }

    println!("cubiomes produced {} distinct ids over {seeds} seeds", seen.len());
    let mut missing = 0;
    for java in &seen {
        let bedrock = java_to_bedrock(*java);
        let info = get_biome_info(bedrock);
        let known = !(info.name == "Ocean" && bedrock != 0);
        if !known {
            missing += 1;
        }
        let cover = match info.ground_cover.first() {
            Some(&b) if b == block_id::GRASS_BLOCK => "grass",
            Some(&b) if b == block_id::SAND => "sand",
            Some(&b) if b == block_id::RED_SAND => "red_sand",
            Some(&b) if b == block_id::SNOW_LAYER => "snow",
            Some(&b) if b == block_id::GRAVEL => "gravel",
            Some(&b) if b == block_id::STONE => "stone",
            Some(&b) if b == block_id::PODZOL => "podzol",
            Some(&b) if b == block_id::CLAY => "clay",
            Some(&b) if b == block_id::ICE => "ice",
            Some(&b) if b == block_id::DIRT => "dirt",
            Some(_) => "other",
            None => "none",
        };
        println!(
            "java {java:>3} {:<24} -> bedrock {bedrock:>3}  {} {:<22} {}",
            Cubiomes::biome_name(*java),
            if known { "ok  " } else { "MISS" },
            info.name,
            cover
        );
    }
    println!("missing from our tables: {missing}");
}
