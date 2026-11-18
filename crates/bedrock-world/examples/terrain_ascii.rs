//! Prints a small ASCII height window so stair-stepping can be read directly.
//!
//!   cargo run -p bedrock-world --example terrain_ascii -- <seed> [x0] [z0] [w] [h]
//!
//! Without x0/z0 it looks for a pond near the origin (deep water with land
//! within a few blocks) and prints the window around it. Digits are blocks above
//! sea level (9 = nine or more), `#` is land above that, `~` is water and `*` is
//! the water surface at sea level.

use bedrock_world::block_id;
use bedrock_world::generator::bds_terrain::{BDSTerrain, BDS_SEA_LEVEL};

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: u64 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let g = BDSTerrain::new(seed);

    let given: Vec<i32> = args.filter_map(|s| s.parse().ok()).collect();
    let (x0, z0, w, h) = if given.len() >= 2 {
        (
            given[0],
            given[1],
            *given.get(2).unwrap_or(&64),
            *given.get(3).unwrap_or(&28),
        )
    } else {
        find_pond(&g)
    };

    println!("window {w}x{h} at ({x0}, {z0}) seed {seed}");
    let mut depth_hist = [0usize; 8];
    let mut land = 0usize;
    let mut chunk_x = i32::MIN;
    let mut chunk_z = i32::MIN;
    let mut chunk = None;
    for z in z0..z0 + h {
        let mut line = String::new();
        for x in x0..x0 + w {
            let (cx, cz) = (x >> 4, z >> 4);
            if cx != chunk_x || cz != chunk_z {
                chunk_x = cx;
                chunk_z = cz;
                chunk = Some(g.generate_base_chunk(cx, cz));
            }
            let chunk = chunk.as_ref().unwrap();
            let (lx, lz) = ((x & 15) as usize, (z & 15) as usize);
            let mut top = 0i32;
            let mut water = None;
            let mut ice = None;
            for y in (1..=self_top()).rev() {
                let b = chunk.get_block(lx, y, lz);
                if b == block_id::ICE {
                    if ice.is_none() {
                        ice = Some(y);
                    }
                    continue;
                }
                if b == block_id::WATER {
                    // Highest water block: the level, not the bottom of the column.
                    if water.is_none() {
                        water = Some(y);
                    }
                    continue;
                }
                if b != block_id::AIR && !block_id::is_transparent(b) {
                    top = y;
                    break;
                }
            }
            let c = if ice.is_some() {
                'I'
            } else {
                match water {
                Some(level) => {
                    let depth = level - top;
                    let d = depth.min(7) as usize;
                    depth_hist[d] += 1;
                    // `a`, `b`, `c` are water 1, 2 and 3 deep; digits are land.
                    match depth {
                        1 => 'a',
                        2 => 'b',
                        3 => 'c',
                        _ => '~',
                    }
                }
                None => {
                    land += 1;
                    let over = top - BDS_SEA_LEVEL;
                    if over == 0 {
                        '='
                    } else if over < 0 {
                        ':'
                    } else if over <= 9 {
                        char::from(b'0' + over as u8)
                    } else {
                        '#'
                    }
                }
                }
            };
            line.push(c);
        }
        println!("{line}");
    }
    if h == 1 {
        // A single row is a transect: spell the bed heights out so a cliff can be
        // read as numbers, not guessed from the glyphs.
        let mut row = String::new();
        for x in x0..x0 + w {
            let cx = x >> 4;
            let chunk = g.generate_base_chunk(cx, z0 >> 4);
            let mut top = 0i32;
            for y in (1..=127).rev() {
                let b = chunk.get_block((x & 15) as usize, y, (z0 & 15) as usize);
                if b != block_id::AIR && !block_id::is_transparent(b) && b != block_id::ICE {
                    top = y;
                    break;
                }
            }
            row.push_str(&format!("{top:>4}"));
        }
        println!("heights:{row}");
    }

    println!(
        "depth 1: {}, 2: {}, 3: {}, 4: {}, 5: {}, 6: {}, 7+: {}, land: {}",
        depth_hist[1],
        depth_hist[2],
        depth_hist[3],
        depth_hist[4],
        depth_hist[5],
        depth_hist[6],
        depth_hist[7],
        land
    );
}

fn self_top() -> i32 {
    127
}

/// A window with water in it and land close by, so a shore can be inspected.
fn find_pond(g: &BDSTerrain) -> (i32, i32, i32, i32) {
    for r in (0..480).step_by(8) {
        for a in 0..16 {
            let ang = a as f64 * std::f64::consts::TAU / 16.0;
            let x = (ang.cos() * r as f64).round() as i32;
            let z = (ang.sin() * r as f64).round() as i32;
            let h = g.height_at(x, z);
            if h > BDS_SEA_LEVEL - 3 {
                continue;
            }
            // Land on both sides tight enough that this is a pond, not the sea.
            if g.height_at(x + 6, z) > BDS_SEA_LEVEL + 2
                && g.height_at(x - 6, z) > BDS_SEA_LEVEL + 2
                && g.height_at(x, z + 6) > BDS_SEA_LEVEL + 2
                && g.height_at(x, z - 6) > BDS_SEA_LEVEL + 2
            {
                return (x - 24, z - 10, 48, 20);
            }
        }
    }
    (0, 0, 48, 20)
}
