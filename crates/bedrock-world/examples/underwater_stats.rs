//! Counts the underwater decoration the generator places and prints single
//! columns, so the seagrass / kelp / sea pickle rules can be checked without
//! logging into the world.
//!
//!   cargo run -p bedrock-world --example underwater_stats -- <seed> <cx> <cz> <chunks> [x z]

use bedrock_world::block_id;
use bedrock_world::generator::bds_terrain::BDSTerrain;

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let chunks: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(4);
    let column: Vec<i32> = args.filter_map(|s| s.parse().ok()).collect();

    let generator = BDSTerrain::new(seed);
    let (mut short, mut tall, mut kelp, mut pickles) = (0usize, 0usize, 0usize, 0usize);

    for x in cx..cx + chunks {
        for z in cz..cz + chunks {
            let chunk = generator.generate_chunk_internal(x, z);
            for lx in 0..16 {
                for lz in 0..16 {
                    for y in 0..128 {
                        let id = chunk.get_block(lx, y, lz);
                        if id == block_id::SEAGRASS {
                            short += 1;
                        } else if block_id::is_seagrass(id) {
                            tall += 1;
                        } else if block_id::is_kelp(id) {
                            kelp += 1;
                        } else if matches!(
                            id,
                            block_id::SEA_PICKLE_1 | block_id::SEA_PICKLE_2 | block_id::SEA_PICKLE_3
                        ) {
                            pickles += 1;
                        }
                    }
                }
            }
        }
    }

    println!(
        "chunks {chunks}x{chunks} from ({cx}, {cz}): seagrass {short}, tall seagrass {tall}, kelp {kelp}, sea pickles {pickles}"
    );

    // `map x0 z0 w h`: top solid block height and name per column, to find
    // anything that sits above the water surface on its own.
    if std::env::var("TOPMAP").is_ok() && column.len() >= 4 {
        let (x0, z0, w, h) = (column[0], column[1], column[2], column[3]);
        for z in z0..z0 + h {
            let mut line = String::new();
            for x in x0..x0 + w {
                let chunk = generator.generate_chunk_internal(x >> 4, z >> 4);
                let (lx, lz) = ((x & 15) as usize, (z & 15) as usize);
                let mut top = 0;
                for y in (0..128).rev() {
                    let id = chunk.get_block(lx, y, lz);
                    if id != block_id::AIR && id != block_id::WATER {
                        top = y;
                        break;
                    }
                }
                line.push_str(&format!("{top:>3}"));
            }
            println!("z={z:<4}{line}");
        }
    }

    if column.len() >= 2 {
        let (wx, wz) = (column[0], column[1]);
        let chunk = generator.generate_chunk_internal(wx >> 4, wz >> 4);
        let (lx, lz) = ((wx & 15) as usize, (wz & 15) as usize);
        println!("column ({wx}, {wz}):");
        for y in (0..128).rev() {
            let id = chunk.get_block(lx, y, lz);
            if id != block_id::AIR {
                println!("  y={y:<4} {id}");
            }
        }
    }
}
