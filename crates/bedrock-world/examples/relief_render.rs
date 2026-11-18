//! Renders a relief (height) map of the generator to a PPM image.
//!
//! Unlike `map_render`, which colours the surface block, this colours the height
//! of the ground: blue for water (shaded by depth), then a hypsometric ramp from
//! the shoreline through lowland green, hills, rock and snow. Every pixel is also
//! hillshaded from a light in the north-west, so slopes, ridges and the shape of
//! the coast are visible.
//!
//! Usage: `relief_render <seed> <blocks> <out.ppm>`

use std::io::Write;

use bedrock_world::block_id;
use bedrock_world::generator::bds_terrain::{BDSTerrain, BDS_SEA_LEVEL};

/// Height ramp for dry land: the colour of a column that stands `h` blocks above
/// sea level.
fn land_colour(h: i32) -> [f32; 3] {
    const STOPS: [(i32, [f32; 3]); 6] = [
        (0, [214.0, 200.0, 148.0]),  // sand at the waterline
        (3, [150.0, 190.0, 96.0]),   // meadow
        (12, [96.0, 158.0, 62.0]),   // lowland grass
        (24, [122.0, 148.0, 72.0]),  // highland, drier
        (36, [138.0, 122.0, 96.0]),  // rock and gravel
        (52, [232.0, 236.0, 240.0]), // snow
    ];
    if h <= STOPS[0].0 {
        return STOPS[0].1;
    }
    for pair in STOPS.windows(2) {
        let (h0, c0) = pair[0];
        let (h1, c1) = pair[1];
        if h <= h1 {
            let t = (h - h0) as f32 / (h1 - h0) as f32;
            return [
                c0[0] + (c1[0] - c0[0]) * t,
                c0[1] + (c1[1] - c0[1]) * t,
                c0[2] + (c1[2] - c0[2]) * t,
            ];
        }
    }
    STOPS[STOPS.len() - 1].1
}

/// Water colour by depth below sea level: shallow turquoise to deep navy.
fn water_colour(depth: i32) -> [f32; 3] {
    let t = (depth as f32 / 24.0).clamp(0.0, 1.0);
    [
        72.0 - 48.0 * t,
        158.0 - 96.0 * t,
        196.0 - 66.0 * t,
    ]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let size: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(768);
    let out = args.next().unwrap_or_else(|| "world-relief.ppm".to_string());

    let generator = BDSTerrain::new(seed);
    let sea = BDS_SEA_LEVEL;
    let chunks = size / 16;
    let side = size as usize;

    // Ground height per column, and whether there is water standing on it.
    let mut ground = vec![0i32; side * side];
    let mut water = vec![false; side * side];
    for cx in 0..chunks {
        for cz in 0..chunks {
            let chunk = generator.generate_chunk_internal(cx, cz);
            for x in 0..16usize {
                for z in 0..16usize {
                    let mut height = 0i32;
                    let mut wet = false;
                    for y in (0..=(sea + 96)).rev() {
                        let b = chunk.get_block(x, y, z);
                        if b == block_id::WATER {
                            wet = true;
                            continue;
                        }
                        // A frozen surface is still water underneath: counting
                        // the ice as ground would paint frozen oceans as land
                        // sitting exactly at sea level.
                        if b == block_id::ICE {
                            wet = true;
                            continue;
                        }
                        // Plants and snow sit on the ground rather than being it,
                        // and leaves would add a one block bump over every tree.
                        if b != block_id::AIR
                            && block_id::is_solid(b)
                            && b != block_id::OAK_LEAVES
                            && b != block_id::SPRUCE_LEAVES
                            && b != block_id::BIRCH_LEAVES
                            && b != block_id::SNOW_LAYER
                            && b != block_id::OAK_LOG
                            && b != block_id::SPRUCE_LOG
                            && b != block_id::BIRCH_LOG
                        {
                            height = y;
                            break;
                        }
                    }
                    let pixel = (cz * 16 + z as i32) as usize * side + (cx * 16 + x as i32) as usize;
                    ground[pixel] = height;
                    water[pixel] = wet;
                }
            }
        }
    }

    // Shading works off a smoothed copy of the height field: the raw field has a
    // block of noise on every pixel, and shading that directly turns the map into
    // crumpled paper instead of showing the shape of the land.
    let mut smooth = vec![0f32; side * side];
    for z in 0..side {
        for x in 0..side {
            let mut sum = 0.0;
            let mut count = 0.0;
            for dz in -2i32..=2 {
                for dx in -2i32..=2 {
                    let nx = (x as i32 + dx).clamp(0, side as i32 - 1) as usize;
                    let nz = (z as i32 + dz).clamp(0, side as i32 - 1) as usize;
                    sum += ground[nz * side + nx] as f32;
                    count += 1.0;
                }
            }
            smooth[z * side + x] = sum / count;
        }
    }

    // Hillshade: the gradient of the smoothed field, lit from the north-west.
    let mut pixels = vec![0u8; side * side * 3];
    for z in 0..side {
        for x in 0..side {
            let i = z * side + x;
            let at = |dx: i32, dz: i32| -> f32 {
                let nx = (x as i32 + dx).clamp(0, side as i32 - 1) as usize;
                let nz = (z as i32 + dz).clamp(0, side as i32 - 1) as usize;
                smooth[nz * side + nx]
            };
            let dhx = at(-2, 0) - at(2, 0);
            let dhz = at(0, -2) - at(0, 2);
            // Light from the north-west, 45 degrees up.
            let shade = 1.0 + (dhx + dhz) * 0.055;
            let shade = shade.clamp(0.6, 1.4);

            let base = if water[i] {
                water_colour((sea - 1 - ground[i]).max(1))
            } else {
                land_colour(ground[i] - sea)
            };
            let o = i * 3;
            for c in 0..3 {
                pixels[o + c] = (base[c] * shade).clamp(0.0, 255.0) as u8;
            }
        }
    }

    let mut heights: Vec<i32> = ground.iter().map(|h| h - sea).collect();
    heights.sort_unstable();
    let pct = |p: usize| heights[heights.len() * p / 100];
    println!(
        "heights above sea level: min {} p10 {} p50 {} p90 {} max {}",
        heights[0],
        pct(10),
        pct(50),
        pct(90),
        heights[heights.len() - 1]
    );
    // Isolated towers and pits: a column two or more blocks above (or below)
    // every one of its four neighbours is a spike, which is what one block wide
    // needles on a hillside look like from the inside of the world.
    let mut spikes = 0usize;
    let mut pits = 0usize;
    for z in 1..side - 1 {
        for x in 1..side - 1 {
            let h = ground[z * side + x];
            let around = [
                ground[z * side + x - 1],
                ground[z * side + x + 1],
                ground[(z - 1) * side + x],
                ground[(z + 1) * side + x],
            ];
            let hi = *around.iter().max().unwrap();
            let lo = *around.iter().min().unwrap();
            if h >= hi + 2 {
                spikes += 1;
            }
            if h <= lo - 2 {
                pits += 1;
            }
        }
    }
    let shelf = ground
        .iter()
        .zip(&water)
        .filter(|(h, w)| !**w && **h == sea)
        .count();
    println!(
        "dry columns sitting exactly at sea level: {shelf} ({:.2} % of the map)",
        100.0 * shelf as f64 / (side * side) as f64
    );
    let land = ground.iter().zip(&water).filter(|(_, w)| !**w).count();
    println!(
        "water {} columns ({:.1} %), land {land} ({:.1} %)",
        side * side - land,
        100.0 * (side * side - land) as f64 / (side * side) as f64,
        100.0 * land as f64 / (side * side) as f64
    );

    let mut jumps = [0usize; 4];
    let mut dry_walls = [0usize; 4];
    for z in 0..side {
        for x in 1..side {
            let i = z * side + x;
            let d = (ground[i] - ground[i - 1]).unsigned_abs() as usize;
            jumps[d.min(3)] += 1;
            if !water[i] && !water[i - 1] {
                dry_walls[d.min(3)] += 1;
            }
        }
    }
    let dry_total: usize = dry_walls.iter().sum();
    // Terrace steps: a two block jump with flat ground on both sides, which is
    // what a "wall on a plain" looks like from the inside of the world.
    let mut terraces = 0usize;
    let mut flat_steps = 0usize;
    for z in 0..side {
        for x in 2..side - 2 {
            let i = z * side + x;
            let d = (ground[i] - ground[i - 1]).unsigned_abs();
            if d >= 2 && ground[i - 1] == ground[i - 2] && ground[i + 1] == ground[i + 2] {
                terraces += 1;
            }
            if d >= 2 {
                flat_steps += 1;
            }
        }
    }
    println!(
        "steps of 2+ with flat ground on both sides (terraces): {terraces} of {flat_steps}          steps ({:.1} %)",
        100.0 * terraces as f64 / flat_steps.max(1) as f64
    );
    println!(
        "dry land only: 2 block steps {:.2} %, 3+ {:.2} %",
        100.0 * dry_walls[2] as f64 / dry_total as f64,
        100.0 * dry_walls[3] as f64 / dry_total as f64
    );
    let total_jumps: usize = jumps.iter().sum();
    println!(
        "step to the next column: same {} ({:.1} %), 1 block {} ({:.1} %), 2 blocks {} ({:.2} %), 3+ {} ({:.2} %)",
        jumps[0], 100.0 * jumps[0] as f64 / total_jumps as f64,
        jumps[1], 100.0 * jumps[1] as f64 / total_jumps as f64,
        jumps[2], 100.0 * jumps[2] as f64 / total_jumps as f64,
        jumps[3], 100.0 * jumps[3] as f64 / total_jumps as f64
    );
    println!(
        "spikes (2+ blocks above every neighbour): {spikes} ({:.2} %), pits: {pits} ({:.2} %)",
        100.0 * spikes as f64 / (side * side) as f64,
        100.0 * pits as f64 / (side * side) as f64
    );

    let mut file = std::fs::File::create(&out).expect("create the relief map file");
    write!(file, "P6\n{size} {size}\n255\n").unwrap();
    file.write_all(&pixels).unwrap();
    println!("wrote {out} ({size}x{size}, seed {seed})");
}