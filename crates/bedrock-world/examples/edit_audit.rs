//! Audits `world.json` (generation plus a list of edits) against what the
//! generator places, to find the edits the fluid engine wrote by mistake:
//! water plants replaced by flowing water, and water cells turned into air.
//!
//!   cargo run -p bedrock-world --example edit_audit -- [world.json] [--clean]
//!
//! `--clean` copies the file to `<world>.bak` and drops those edits.

use std::collections::HashMap;
use std::fs;

use bedrock_world::block_id;
use bedrock_world::block_states::state_name;
use bedrock_world::generator::bds_terrain::BDSTerrain;
use bedrock_world::Chunk;

fn is_fluid_damage(block: u32) -> bool {
    block == block_id::AIR || state_name(block) == Some("flowing_water")
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "world.json".to_string());
    let clean = args.any(|arg| arg == "--clean");

    let text = fs::read_to_string(&path).expect("world.json");
    let world: serde_json::Value = serde_json::from_str(&text).expect("json");
    let seed = world["seed"].as_u64().unwrap_or(0);
    let generator = BDSTerrain::new(seed);

    let edits = world["edits"].as_array().expect("edits").clone();
    let mut chunks: HashMap<(i32, i32), Chunk> = HashMap::new();
    let mut by_plant = 0usize;
    let mut by_water = 0usize;
    let mut kept = Vec::with_capacity(edits.len());

    for edit in &edits {
        let (x, y, z) = (
            edit[0].as_i64().unwrap() as i32,
            edit[1].as_i64().unwrap() as i32,
            edit[2].as_i64().unwrap() as i32,
        );
        let block = edit[3].as_u64().unwrap() as u32;
        let key = (x >> 4, z >> 4);
        let chunk = chunks
            .entry(key)
            .or_insert_with(|| generator.generate_chunk_internal(key.0, key.1));
        let generated = chunk.get_block((x & 15) as usize, y, (z & 15) as usize);

        // Damage: the edit put air or flowing water where the generator has a
        // water plant or plain water. Generated water is always a source, so a
        // flowing water edit can only come from the fluid engine.
        let damage = is_fluid_damage(block)
            && (block_id::is_water_plant(generated) || generated == block_id::WATER);
        if damage {
            if block_id::is_water_plant(generated) {
                by_plant += 1;
            } else {
                by_water += 1;
            }
            continue;
        }
        kept.push(edit.clone());
    }

    println!("ediciones: {} | sobre una planta: {by_plant} | sobre agua: {by_water}", edits.len());
    println!("quedarian: {}", kept.len());

    if clean {
        let backup = format!("{path}.bak");
        fs::write(&backup, &text).expect("backup");
        let mut cleaned = world.clone();
        cleaned["edits"] = serde_json::Value::Array(kept);
        fs::write(&path, serde_json::to_string(&cleaned).expect("json")).expect("write");
        println!("limpiado; copia original en {backup}");
    }
}
