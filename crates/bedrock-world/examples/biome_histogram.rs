//! Prints the biome distribution of the BDS terrain generator over a block
//! area, using the real 1.18+ climate KD-tree pipeline (`generate_base_chunk`).

use bedrock_world::block_id;
use bedrock_world::generator::BDSTerrain;
use bedrock_world::generator::biome::get_biome_info;
use std::collections::BTreeMap;

fn block_name(id: u32) -> &'static str {
    match id {
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
        _ => "other",
    }
}

/// Surface block of a column: the first block from the top that is neither air,
/// plant nor leaf, so the trees do not hide the floor under them.
fn surface_block(chunk: &bedrock_world::chunk::Chunk, x: usize, z: usize) -> (u32, i32) {
    let Some(mut y) = chunk.get_highest_block_at(x, z) else {
        return (block_id::AIR, 0);
    };
    loop {
        let b = chunk.get_block(x, y, z);
        // Water and ice are the lake surface, not the ground: skip them so the
        // height a biome reports is the terrain it is standing on.
        if b != block_id::AIR
            && b != block_id::WATER
            && b != block_id::ICE
            && b != block_id::OAK_LEAVES
            && b != block_id::SPRUCE_LEAVES
            && b != block_id::BIRCH_LEAVES
            && b != block_id::OAK_LOG
            && b != block_id::SPRUCE_LOG
            && b != block_id::BIRCH_LOG
            && b != block_id::SHORT_GRASS
            && b != block_id::TALL_GRASS
        {
            return (b, y);
        }
        if y == 0 {
            return (block_id::AIR, 0);
        }
        y -= 1;
    }
}

fn main() {
    let bds = BDSTerrain::new(12345);

    // Half-extent in chunks, so `half * 2` chunks a side. The default 12x12
    // chunks centred on spawn (192x192 blocks) is small enough that a single
    // ocean can cover three quarters of it, so pass a bigger number for a
    // distribution that means something.
    let half: i32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(6);
    let mut counts: BTreeMap<u8, usize> = BTreeMap::new();
    let mut columns = 0usize;
    // What each biome actually puts on the ground, which is where the surface
    // rules show up: podzol in old growth taiga, red sand in the badlands, ice
    // over the frozen oceans.
    let mut covers: BTreeMap<(u8, u32), usize> = BTreeMap::new();
    // Surface heights per biome: the elevation a biome actually occupies is what
    // says whether it belongs there.
    let mut heights: BTreeMap<u8, Vec<i32>> = BTreeMap::new();
    // Per-column biome grid for the patch-size pass.
    let side = (half * 2) as usize * 16;
    let mut grid = vec![0u8; side * side];

    for cx in -half..half {
        for cz in -half..half {
            let chunk = bds.generate_base_chunk(cx, cz);
            for &b in chunk.biomes.iter() {
                *counts.entry(b).or_insert(0) += 1;
                columns += 1;
            }
            for x in 0..16usize {
                for z in 0..16usize {
                    let biome = chunk.biomes[x * 16 + z];
                    // The ground, not the water over it: a column under a lake
                    // reports the lake surface otherwise, which reads as a biome at
                    // sea level.
                    let (block, ground_y) = surface_block(&chunk, x, z);
                    *covers.entry((biome, block)).or_insert(0) += 1;
                    heights.entry(biome).or_default().push(ground_y);
                }
            }
            let ox = ((cx + half) * 16) as usize;
            let oz = ((cz + half) * 16) as usize;
            for x in 0..16usize {
                for z in 0..16usize {
                    grid[(oz + z) * side + ox + x] = chunk.biomes[x * 16 + z];
                }
            }
        }
    }

    println!("area: {}x{} chunks ({} columns)", half * 2, half * 2, columns);
    println!("biomes present: {}", counts.len());
    println!("{:>4} {:<24} {:>8} {:>7}", "id", "name", "cols", "%");
    for (id, n) in &counts {
        let info = get_biome_info(*id);
        println!("{:>4} {:<24} {:>8} {:>6.2}%", id, info.name, n, 100.0 * *n as f64 / columns as f64);
    }

    // Patch size: length of the stretches of one biome along two axes. Vanilla
    // biome regions are a few hundred blocks across, so this is the number that
    // says whether the scales are too broad (one biome forever) or too fine
    // (confetti).
    let mut runs: Vec<usize> = Vec::new();
    for row in [0i32, 1024, -1024] {
        let mut last = 255u8;
        let mut len = 0usize;
        for x in -2048..2048 {
            let chunk = bds.generate_base_chunk(x >> 4, row >> 4);
            let b = chunk.biomes[(x & 15) as usize * 16 + (row & 15) as usize];
            if b == last {
                len += 1;
            } else {
                if last != 255 {
                    runs.push(len);
                }
                last = b;
                len = 1;
            }
        }
        runs.push(len);
    }
    runs.sort_unstable();
    let mean = runs.iter().sum::<usize>() as f64 / runs.len() as f64;
    println!(
        "
biome patch length over 3 x 4096 blocks: mean {:.0} blocks, median {} , p90 {} , longest {} ({} patches)",
        mean,
        runs[runs.len() / 2],
        runs[runs.len() * 9 / 10],
        runs[runs.len() - 1],
        runs.len()
    );

    // Patch size, which is what a player sees as a "minibioma": flood fill every
    // blend of one biome and report the ones smaller than a 8x8 block square.
    let mut seen = vec![false; side * side];
    let mut small = 0usize;
    let mut small_area = 0usize;
    let mut small256 = 0usize;
    let mut small256_area = 0usize;
    let mut patches = 0usize;
    for start in 0..side * side {
        if seen[start] {
            continue;
        }
        let id = grid[start];
        let mut stack = vec![start];
        seen[start] = true;
        let mut area = 0usize;
        while let Some(i) = stack.pop() {
            area += 1;
            let (x, z) = (i % side, i / side);
            for (dx, dz) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                if nx < 0 || nz < 0 || nx as usize >= side || nz as usize >= side {
                    continue;
                }
                let j = nz as usize * side + nx as usize;
                if !seen[j] && grid[j] == id {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        patches += 1;
        if area < 64 {
            small += 1;
            small_area += area;
        }
        if area < 256 {
            small256 += 1;
            small256_area += area;
        }
    }
    println!(
        "biome patches: {patches}, smaller than 64 columns: {small} ({:.2} % of area), smaller than 256: {small256} ({:.2} % of area)",
        100.0 * small_area as f64 / (side * side) as f64,
        100.0 * small256_area as f64 / (side * side) as f64
    );

    // Per-biome surface composition, top three blocks of each biome that covers
    // at least half a percent of the area.
    println!("
surface blocks per biome:");
    let mut ids: Vec<u8> = counts.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        let biome_columns = counts[&id];
        if 100.0 * biome_columns as f64 / (columns as f64) < 0.5 {
            continue;
        }
        let mut parts: Vec<(u32, usize)> = covers
            .iter()
            .filter(|((b, _), _)| *b == id)
            .map(|((_, block), n)| (*block, *n))
            .collect();
        parts.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        let text: Vec<String> = parts
            .iter()
            .take(3)
            .map(|(block, n)| {
                format!(
                    "{} {:.0} %",
                    block_name(*block),
                    100.0 * *n as f64 / biome_columns as f64
                )
            })
            .collect();
        let h = heights.get_mut(&id).unwrap();
        h.sort_unstable();
        println!(
            "  {:>3} {:<24} y min {} p50 {} max {} | {}",
            id,
            get_biome_info(id).name,
            h[0],
            h[h.len() / 2],
            h[h.len() - 1],
            text.join(", ")
        );
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
