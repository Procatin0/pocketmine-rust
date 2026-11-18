//! Looks for seeds whose vanilla (cubiomes) map has desert or badlands near the
//! origin: `seed_scan <from> <to> <radius>` samples a window of that radius around
//! 0,0 and prints the seeds with the most sand biome.

use bedrock_world::generator::bds_biome_tree::java_to_bedrock;
use bedrock_world::generator::biome;
use bedrock_world::generator::cubiomes::Cubiomes;

fn main() {
    let mut args = std::env::args().skip(1);
    let from: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let to: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(64);
    let radius: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(512);

    let mut rows: Vec<(f64, u64, f64, f64)> = Vec::new();
    for seed in from..to {
        let Some(generator) = Cubiomes::new(seed) else {
            continue;
        };
        let (mut sand, mut total) = (0usize, 0usize);
        let (mut badlands, mut desert) = (0usize, 0usize);
        let mut x = -radius;
        while x < radius {
            let mut z = -radius;
            while z < radius {
                total += 1;
                let id = generator
                    .biome_at(x, 63, z)
                    .map(java_to_bedrock)
                    .unwrap_or(0);
                match id {
                    biome::DESERT => {
                        desert += 1;
                        sand += 1;
                    }
                    biome::BADLANDS
                    | biome::BADLANDS_PLATEAU
                    | biome::WOODED_BADLANDS_PLATEAU
                    | biome::ERODED_BADLANDS
                    | biome::MODIFIED_BADLANDS_PLATEAU
                    | biome::MODIFIED_WOODED_BADLANDS_PLATEAU => {
                        badlands += 1;
                        sand += 1;
                    }
                    _ => {}
                }
                z += 16;
            }
            x += 16;
        }
        let pct = sand as f64 / total as f64 * 100.0;
        rows.push((
            pct,
            seed,
            desert as f64 / total as f64 * 100.0,
            badlands as f64 / total as f64 * 100.0,
        ));
    }
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!("radius {radius} blocks around 0,0; seeds {}..{}", from, to);
    println!("  seed   sand%  desert%  badlands%");
    for (pct, seed, desert, badlands) in rows.iter().take(12) {
        println!("{seed:>6} {pct:>7.1} {desert:>8.1} {badlands:>10.1}");
    }
}
