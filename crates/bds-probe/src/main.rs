//! `bds-probe` — ground-truth tooling for Bedrock world-generation parity.
//!
//! Reads a BDS world directory (LevelDB) and exposes the biome data BDS really
//! wrote for a seed, so the Rust generator can be measured against it instead
//! of against guesses.
//!
//! Commands:
//!   scan <db>                      key/tag inventory
//!   dump <db> <cx> <cz>            per-section biome palettes of one chunk
//!   biomes <db> [--y N] [--ascii]  surface biome map + histogram
//!   export <db> <out.json> [--y N] machine-readable reference capture
//!   compare <db> <seed> [--y N]    parity of bedrock-world's generator vs BDS
//!   axes <db>                      which key field is X and which is Z

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod chunk;
mod leveldb;

use chunk::{decode_data3d, parse_chunk_key, tag, Data3d};
use leveldb::Db;

/// Default Y used to sample the "surface" biome of a column.
const DEFAULT_Y: i32 = 64;

struct World {
    chunks: BTreeMap<(i32, i32), Data3d>,
    other_dimensions: usize,
    key_count: usize,
}

fn load_world(dir: &Path) -> Result<World, String> {
    let db = Db::open(dir)?;
    let mut chunks = BTreeMap::new();
    let mut tags = BTreeMap::new();
    let mut other_dimensions = 0usize;
    let mut key_count = 0usize;
    for (key, value) in db.iter() {
        key_count += 1;
        let Some(ck) = parse_chunk_key(key) else { continue };
        *tags.entry(ck.tag).or_insert(0) += 1;
        if ck.dimension != 0 {
            other_dimensions += 1;
            continue;
        }
        if ck.tag != tag::DATA_3D {
            continue;
        }
        if let Some(data) = decode_data3d(value) {
            chunks.insert((ck.x, ck.z), data);
        }
    }
    if chunks.is_empty() {
        return Err(format!(
            "no Data3D records decoded in {} ({} keys, {} journal records, {} table files). \
             Generate chunks with a ticking area first.",
            dir.display(),
            db.len(),
            db.journal_records,
            db.table_files
        ));
    }
    Ok(World {
        chunks,
        other_dimensions,
        key_count,
    })
}

/// Short unique symbol per biome id, for terminal maps.
fn biome_symbol(id: u32) -> char {
    const TABLE: &[(u32, char)] = &[
        (0, '~'),
        (1, '.'),
        (2, ':'),
        (3, '^'),
        (4, 'F'),
        (5, 'T'),
        (6, 's'),
        (7, 'w'),
        (8, 'n'),
        (9, 'N'),
        (10, 'r'),
        (11, 'i'),
        (12, 'I'),
        (13, 'b'),
        (14, 'S'),
        (16, 'B'),
        (17, 'd'),
        (18, 'o'),
        (19, 'g'),
        (20, 'h'),
        (21, 'j'),
        (22, 'J'),
        (23, 'K'),
        (24, 'O'),
        (25, 'W'),
        (26, 'c'),
        (27, 'e'),
        (28, 'E'),
        (29, 'f'),
        (30, 'k'),
        (31, 'm'),
        (32, 'C'),
        (33, 'D'),
        (34, 'M'),
        (35, 'G'),
        (36, 'H'),
        (37, 'L'),
        (38, 'P'),
        (39, 'Q'),
        (40, 'y'),
        (41, 'Y'),
        (42, 'u'),
        (43, 'U'),
        (44, 'v'),
        (45, 'V'),
        (46, 'x'),
        (47, 'X'),
        (48, 'z'),
        (49, 'Z'),
        (129, 'p'),
        (130, 'q'),
        (131, 'a'),
        (132, 'A'),
        (133, 'R'),
        (140, '!'),
        (155, 'z'),
        (156, 'Z'),
        (157, 't'),
        (158, 'l'),
        (160, '&'),
        (162, '<'),
        (163, '>'),
        (164, '%'),
        (185, '?'),
        (187, '$'),
        (188, '@'),
        (189, '#'),
        (190, '*'),
        (191, '+'),
        (192, '='),
        (193, '-'),
        (194, '/'),
        (195, '|'),
        (196, ';'),
    ];
    TABLE
        .iter()
        .find(|(biome, _)| *biome == id)
        .map(|(_, symbol)| *symbol)
        .unwrap_or('?')
}

fn biome_grid(world: &World, y: i32) -> BTreeMap<(i32, i32), [Option<u32>; 256]> {
    let mut grid = BTreeMap::new();
    for (&(cx, cz), data) in &world.chunks {
        let mut cells = [None; 256];
        for lz in 0..16 {
            for lx in 0..16 {
                let biome = data.biome_at(lx as i32, y, lz as i32);
                cells[lx | (lz << 4)] = biome;
            }
        }
        grid.insert((cx, cz), cells);
    }
    grid
}

fn histogram(grid: &BTreeMap<(i32, i32), [Option<u32>; 256]>) -> BTreeMap<u32, usize> {
    let mut hist = BTreeMap::new();
    for cells in grid.values() {
        for cell in cells.iter().flatten() {
            *hist.entry(*cell).or_insert(0) += 1;
        }
    }
    hist
}

fn print_map(grid: &BTreeMap<(i32, i32), [Option<u32>; 256]>) {
    let min_x = grid.keys().map(|k| k.0).min().unwrap_or(0);
    let max_x = grid.keys().map(|k| k.0).max().unwrap_or(0);
    let min_z = grid.keys().map(|k| k.1).min().unwrap_or(0);
    let max_z = grid.keys().map(|k| k.1).max().unwrap_or(0);
    for cz in min_z..=max_z {
        for lz in 0..16 {
            let mut line = String::new();
            for cx in min_x..=max_x {
                match grid.get(&(cx, cz)) {
                    Some(cells) => {
                        for lx in 0..16 {
                            let symbol = cells[lx | (lz << 4)]
                                .map(biome_symbol)
                                .unwrap_or(' ');
                            line.push(symbol);
                        }
                    }
                    None => line.push_str("                "),
                }
            }
            println!("{line}");
        }
    }
}

fn parse_args(args: &[String]) -> (Vec<String>, BTreeMap<String, String>) {
    let mut positional = Vec::new();
    let mut flags = BTreeMap::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(name) = arg.strip_prefix("--") {
            if let Some((name, value)) = name.split_once('=') {
                flags.insert(name.to_string(), value.to_string());
            } else if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                flags.insert(name.to_string(), args[i + 1].clone());
                i += 1;
            } else {
                flags.insert(name.to_string(), "true".to_string());
            }
        } else {
            positional.push(arg.clone());
        }
        i += 1;
    }
    (positional, flags)
}

fn y_from(flags: &BTreeMap<String, String>) -> i32 {
    flags
        .get("y")
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_Y)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: bds-probe <scan|dump|biomes|export|compare|axes> <db> [args]");
        return ExitCode::from(2);
    }
    let command = args[0].clone();
    let (positional, flags) = parse_args(&args[1..]);
    let result = match command.as_str() {
        "scan" => cmd_scan(&positional),
        "dump" => cmd_dump(&positional),
        "biomes" => cmd_biomes(&positional, &flags),
        "export" => cmd_export(&positional, &flags),
        "compare" => cmd_compare(&positional, &flags),
        "axes" => cmd_axes(&positional),
        other => Err(format!("unknown command {other}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_scan(positional: &[String]) -> Result<(), String> {
    let dir = positional.first().ok_or("scan needs a world db directory")?;
    let db = Db::open(Path::new(dir))?;
    println!("keys: {}", db.len());
    println!("journal records: {}", db.journal_records);
    println!("table files: {}", db.table_files);
    let mut tags: BTreeMap<u8, usize> = BTreeMap::new();
    let mut chunks = 0usize;
    for (key, _) in db.iter() {
        if let Some(ck) = parse_chunk_key(key) {
            *tags.entry(ck.tag).or_insert(0) += 1;
            if ck.tag == tag::DATA_3D {
                chunks += 1;
            }
        }
    }
    println!("chunk records (all dimensions): {chunks}");
    for (tag, count) in tags {
        println!("  tag 0x{tag:02x}: {count}");
    }
    Ok(())
}

fn cmd_dump(positional: &[String]) -> Result<(), String> {
    if positional.len() < 3 {
        return Err("dump needs <db> <cx> <cz>".to_string());
    }
    let dir = PathBuf::from(&positional[0]);
    let cx: i32 = positional[1].parse().map_err(|_| "bad cx")?;
    let cz: i32 = positional[2].parse().map_err(|_| "bad cz")?;
    let db = Db::open(&dir)?;
    let key = chunk::data3d_key(cx, cz);
    let value = db.get(&key).ok_or_else(|| format!("no Data3D for {cx},{cz}"))?;
    let data = decode_data3d(value).ok_or("malformed Data3D")?;
    println!("chunk ({cx},{cz}) sections:");
    for (i, cells) in data.sections.iter().enumerate() {
        let y0 = chunk::MIN_SECTION_Y + 16 * i as i32;
        let mut hist: BTreeMap<u32, usize> = BTreeMap::new();
        for cell in cells {
            *hist.entry(*cell).or_insert(0) += 1;
        }
        let summary: Vec<String> = hist.iter().map(|(b, n)| format!("{b}:{n}")).collect();
        let marker = if data.stored[i] { " " } else { "~" };
        println!("{marker} y {y0:>4}..{:>4}: {}", y0 + 15, summary.join(" "));
    }
    println!(
        "heightmap: min {:?} max {:?}",
        data.heightmap.iter().min(),
        data.heightmap.iter().max()
    );
    Ok(())
}

fn cmd_biomes(positional: &[String], flags: &BTreeMap<String, String>) -> Result<(), String> {
    let dir = positional.first().ok_or("biomes needs a world db directory")?;
    let world = load_world(Path::new(dir))?;
    let y = y_from(flags);
    let grid = biome_grid(&world, y);
    println!(
        "chunks: {} (y={y})  other-dimension records: {}  total keys: {}",
        world.chunks.len(),
        world.other_dimensions,
        world.key_count
    );
    let hist = histogram(&grid);
    let total: usize = hist.values().sum();
    let mut entries: Vec<_> = hist.into_iter().collect();
    entries.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (biome, count) in entries.iter().take(12) {
        let pct = 100.0 * *count as f64 / total.max(1) as f64;
        println!("  biome {biome:>3} [{}] {count:>6} ({pct:5.1}%)", biome_symbol(*biome));
    }
    if flags.contains_key("ascii") {
        print_map(&grid);
    }
    Ok(())
}

fn cmd_export(positional: &[String], flags: &BTreeMap<String, String>) -> Result<(), String> {
    if positional.len() < 2 {
        return Err("export needs <db> <out.json>".to_string());
    }
    let world = load_world(Path::new(&positional[0]))?;
    let y = y_from(flags);
    let grid = biome_grid(&world, y);
    let mut chunks = Vec::new();
    for (&(cx, cz), cells) in &grid {
        let mut runs: Vec<[i64; 2]> = Vec::new();
        let mut previous: Option<u32> = None;
        for cell in cells.iter() {
            let id = cell.map(|v| v as i64).unwrap_or(-1);
            match runs.last_mut() {
                Some(last) if previous == Some(id as u32) => last[1] += 1,
                _ => runs.push([id, 1]),
            }
            previous = Some(id as u32);
        }
        let heightmap: Vec<i16> = world.chunks[&(cx, cz)].heightmap.to_vec();
        chunks.push(serde_json::json!({
            "x": cx,
            "z": cz,
            "y": y,
            "runs": runs,
            "heightmap": heightmap,
        }));
    }
    let document = serde_json::json!({
        "source": "bds-1.26.63.1",
        "y": y,
        "chunks": chunks,
    });
    let out = PathBuf::from(&positional[1]);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&out, serde_json::to_string(&document).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    println!("wrote {} chunks to {}", chunks.len(), out.display());
    Ok(())
}

/// Measures the parity of `bedrock_world`'s biome map against the capture.
fn cmd_compare(positional: &[String], flags: &BTreeMap<String, String>) -> Result<(), String> {
    if positional.len() < 2 {
        return Err("compare needs <db> <seed>".to_string());
    }
    let seed: u64 = positional[1].parse().map_err(|_| "bad seed")?;
    let world = load_world(Path::new(&positional[0]))?;
    let y = y_from(flags);
    let grid = biome_grid(&world, y);

    // `--impl java` swaps the Bedrock noise port for the Java 1.18+ climate
    // port (`java_biomes`), which reproduces vanilla's biome placement.
    // `--impl java` swaps the Bedrock noise port for the Java 1.18+ climate
    // port (`java_biomes`), which reproduces vanilla's biome placement;
    // `java4` additionally samples per 4x4x4 cell like the vanilla biome
    // storage does.
    let implementation = flags.get("impl").map(String::as_str).unwrap_or("bds");
    let generator = bedrock_world::generator::bds_terrain::BDSTerrain::new(seed);
    let java_generator = (implementation != "bds")
        .then(|| bedrock_world::generator::java_biomes::JavaBiomes::new(seed));
    let cell = implementation == "java4";
    let sample = |x: i32, z: i32| -> u32 {
        match &java_generator {
            Some(java_generator) => {
                let (x, y, z) = if cell {
                    (
                        (x >> 2) * 4 + 2,
                        (y >> 2) * 4 + 2,
                        (z >> 2) * 4 + 2,
                    )
                } else {
                    (x, y, z)
                };
                java_generator.biome_at(x, y, z) as u32
            }
            None => generator.biome_at_bds(x, z) as u32,
        }
    };

    let mut total = 0usize;
    let mut agree = 0usize;
    let mut confusion: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for (&(cx, cz), cells) in &grid {
        for lz in 0..16 {
            for lx in 0..16 {
                let Some(expected) = cells[lx | (lz << 4)] else { continue };
                let x = cx * 16 + lx as i32;
                let z = cz * 16 + lz as i32;
                let got = sample(x, z);
                total += 1;
                if got == expected {
                    agree += 1;
                } else {
                    *confusion.entry((expected, got)).or_insert(0) += 1;
                }
            }
        }
    }
    if total == 0 {
        return Err("no comparable columns".to_string());
    }
    let pct = 100.0 * agree as f64 / total as f64;
    println!("columns compared: {total}");
    println!("biome agreement: {agree}/{total} = {pct:.2}%");
    let mut top: Vec<_> = confusion.into_iter().collect();
    top.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("top mismatches (bds -> generated):");
    for ((expected, got), count) in top.iter().take(10) {
        println!("  {expected:>3} -> {got:>3}: {count}");
    }
    let mut summary = String::new();
    let _ = write!(summary, "seed {seed} y {y}: {pct:.2}% of {total} columns");
    println!("{summary}");
    Ok(())
}

/// Verifies which of the two leading key fields is X and which is Z.
///
/// Both chunks and heightmaps would still decode if the axes were swapped, so
/// the check compares the height continuity across neighbouring chunks: the
/// correct interpretation has far smaller edge deltas.
fn cmd_axes(positional: &[String]) -> Result<(), String> {
    let dir = positional.first().ok_or("axes needs a world db directory")?;
    let world = load_world(Path::new(dir))?;
    // `swap == false`: the first key field is world X. `swap == true`: it is
    // world Z. Neighbours are looked up in key space, so the pair of edges
    // being compared changes between the two hypothesis.
    let score = |swap: bool| -> (u64, usize) {
        let mut total = 0u64;
        let mut pairs = 0usize;
        for (&(ka, kb), data) in &world.chunks {
            for step in 0..2 {
                let neighbour = if step == 0 {
                    world.chunks.get(&(ka + 1, kb))
                } else {
                    world.chunks.get(&(ka, kb + 1))
                };
                let Some(neighbour) = neighbour else { continue };
                // `step == 0` advances the first key field.
                let first_field_is_x = !swap;
                let advances_x = (step == 0) == first_field_is_x;
                for i in 0..16usize {
                    let (here, there) = if advances_x {
                        (data.height_at(15, i), neighbour.height_at(0, i))
                    } else {
                        (data.height_at(i, 15), neighbour.height_at(i, 0))
                    };
                    total += (here as i64 - there as i64).unsigned_abs();
                    pairs += 1;
                }
            }
        }
        (total, pairs)
    };
    let (direct, pairs) = score(false);
    let (swapped, _) = score(true);
    println!(
        "key field 0 = X: mean |height delta| {:.2} over {pairs} column pairs",
        direct as f64 / pairs.max(1) as f64
    );
    println!(
        "key field 0 = Z: mean |height delta| {:.2}",
        swapped as f64 / pairs.max(1) as f64
    );
    Ok(())
}
