//! Renders a top-down map of the generator to a PPM image.
//!
//! Each pixel is one block column, coloured by the block at its surface:
//! water (shaded by depth), ice, sand, red sand, gravel, grass, podzol, stone,
//! dirt, clay, snow and leaves. Height is used to darken the colour slightly,
//! so terraces show up.
//!
//! Usage: `map_render <seed> <blocks> <out.ppm> [x0] [z0]`
//!
//! `x0`/`z0` move the window (both multiples of 16), which is how a region with
//! badlands or a frozen ocean can be rendered without walking there from spawn.

use std::io::Write;

use bedrock_world::block_id;
use bedrock_world::generator::bds_terrain::{BDSTerrain, BDS_SEA_LEVEL};

fn colour_of(id: u32, depth: i32, height: i32) -> [u8; 3] {
    let base = match id {
        block_id::WATER => {
            // Deeper water is darker.
            let t = (depth as f32 / 30.0).clamp(0.0, 1.0);
            let r = 40.0 - 25.0 * t;
            let g = 110.0 - 70.0 * t;
            let b = 200.0 - 90.0 * t;
            return [r as u8, g as u8, b as u8];
        }
        block_id::ICE => [176.0, 214.0, 240.0],
        block_id::GRASS_BLOCK => [96.0, 158.0, 62.0],
        block_id::PODZOL => [104.0, 78.0, 40.0],
        block_id::COARSE_DIRT => [122.0, 86.0, 54.0],
        block_id::SAND => [226.0, 206.0, 140.0],
        block_id::RED_SAND => [206.0, 118.0, 56.0],
        block_id::SANDSTONE => [205.0, 184.0, 122.0],
        block_id::RED_SANDSTONE => [176.0, 96.0, 48.0],
        block_id::GRAVEL => [138.0, 130.0, 126.0],
        block_id::CLAY => [166.0, 176.0, 194.0],
        block_id::DIRT => [132.0, 92.0, 62.0],
        block_id::STONE => [122.0, 122.0, 122.0],
        block_id::SNOW_LAYER => [242.0, 246.0, 250.0],
        block_id::OAK_LEAVES | block_id::BIRCH_LEAVES | block_id::SPRUCE_LEAVES => {
            [46.0, 104.0, 48.0]
        }
        block_id::AIR => [255.0, 255.0, 255.0],
        _ => [220.0, 40.0, 200.0],
    };
    // Shade by height so slopes and terraces are visible.
    let shade = 0.72 + 0.28 * (((height - 40) as f32) / 50.0).clamp(0.0, 1.0);
    [
        (base[0] * shade).min(255.0) as u8,
        (base[1] * shade).min(255.0) as u8,
        (base[2] * shade).min(255.0) as u8,
    ]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: u64 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let size: i32 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(768);
    let out = args.next().unwrap_or_else(|| "world-map.ppm".to_string());
    let x0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0) & !15;
    let z0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0) & !15;

    let generator = BDSTerrain::new(seed);
    let sea = BDS_SEA_LEVEL;
    let chunks = size / 16;

    let mut pixels = vec![0u8; (size * size * 3) as usize];
    let mut water_mask = vec![false; (size * size) as usize];
    // Surface block behind every pixel, so the run reports what the biomes
    // actually put on the ground instead of only the picture.
    let mut surfaces: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    let mut heights: Vec<i32> = Vec::new();
    for cx in 0..chunks {
        for cz in 0..chunks {
            let chunk = generator.generate_chunk_internal(cx + x0 / 16, cz + z0 / 16);
            for x in 0..16usize {
                for z in 0..16usize {
                    let mut surface = None;
                    for y in (0..=(sea + 64)).rev() {
                        let b = chunk.get_block(x, y, z);
                        if b != block_id::AIR {
                            surface = Some((b, y));
                            break;
                        }
                    }
                    let (id, y) = surface.unwrap_or((block_id::AIR, 0));
                    let depth = (sea - y).max(0);
                    heights.push(y);
                    let index = ((cz * 16 + z as i32) * size + (cx * 16 + x as i32)) as usize;
                    water_mask[index] = id == block_id::WATER || id == block_id::ICE;
                    *surfaces.entry(id).or_insert(0) += 1;
                    let colour = colour_of(id, depth, y);
                    let px = (cz * 16 + z as i32) * size + (cx * 16 + x as i32);
                    let o = (px * 3) as usize;
                    pixels[o..o + 3].copy_from_slice(&colour);
                }
            }
        }
    }

    // A few numbers next to the picture: water share, height range and how
    // fragmented the land is.
    let total = water_mask.len();
    let water: usize = water_mask.iter().filter(|w| **w).count();
    heights.sort_unstable();
    println!(
        "water {:.1} %, height min {} p10 {} p50 {} p90 {} max {}",
        100.0 * water as f64 / total as f64,
        heights[0],
        heights[total / 10],
        heights[total / 2],
        heights[total * 9 / 10],
        heights[total - 1]
    );
    let name_of = |id: u32| match id {
        block_id::AIR => "air",
        block_id::WATER => "water",
        block_id::ICE => "ice",
        block_id::GRASS_BLOCK => "grass",
        block_id::PODZOL => "podzol",
        block_id::COARSE_DIRT => "coarse_dirt",
        block_id::DIRT => "dirt",
        block_id::SAND => "sand",
        block_id::RED_SAND => "red_sand",
        block_id::SANDSTONE => "sandstone",
        block_id::RED_SANDSTONE => "red_sandstone",
        block_id::GRAVEL => "gravel",
        block_id::STONE => "stone",
        block_id::CLAY => "clay",
        block_id::SNOW_LAYER => "snow_layer",
        block_id::OAK_LEAVES | block_id::BIRCH_LEAVES | block_id::SPRUCE_LEAVES => "leaves",
        _ => "other",
    };
    let mut ranked: Vec<(u32, usize)> = surfaces.into_iter().collect();
    ranked.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    let top: Vec<String> = ranked
        .iter()
        .take(8)
        .map(|(id, n)| format!("{} {:.1} %", name_of(*id), 100.0 * *n as f64 / total as f64))
        .collect();
    println!("surface blocks: {}", top.join(", "));

    for (label, want_water) in [("water", true), ("land", false)] {
        let mask: Vec<bool> = water_mask.iter().map(|w| *w == want_water).collect();
        let mut seen = vec![false; total];
        let mut stack = Vec::new();
        let mut sizes: Vec<usize> = Vec::new();
        for start in 0..total {
            if !mask[start] || seen[start] {
                continue;
            }
            seen[start] = true;
            stack.push(start);
            let mut count = 0usize;
            while let Some(i) = stack.pop() {
                count += 1;
                let (x, z) = (i % size as usize, i / size as usize);
                for (dx, dz) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                    if nx < 0 || nz < 0 || nx as usize >= size as usize || nz as usize >= size as usize {
                        continue;
                    }
                    let j = nz as usize * size as usize + nx as usize;
                    if mask[j] && !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
            sizes.push(count);
        }
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        let small: usize = sizes.iter().filter(|s| **s < 256).count();
        let small_area: usize = sizes.iter().filter(|s| **s < 256).sum();
        println!(
            "{label}: {} components, largest {:?}, <256 blocks: {small} ({:.2} % of area)",
            sizes.len(),
            &sizes[..sizes.len().min(3)],
            100.0 * small_area as f64 / total as f64
        );
    }

    let mut file = std::fs::File::create(&out).expect("create the map file");
    write!(file, "P6\n{size} {size}\n255\n").unwrap();
    file.write_all(&pixels).unwrap();
    println!("wrote {out} ({size}x{size}, seed {seed}, origin {x0},{z0})");
}
