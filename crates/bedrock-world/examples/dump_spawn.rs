//! Diagnóstico `Block / ClientDisconnection-90`: vuelca el payload EXACTO que
//! el server envía al cliente para el chunk de spawn (seed 0, chunk 1,0) y
//! describe su orden byte a byte.
//!
//! Uso: `cargo run -p bedrock-world --example dump_spawn`

use std::collections::HashMap;

use bedrock_world::block_id;
use bedrock_world::generator::normal::NormalGenerator;
use bedrock_world::WorldGenerator;

fn name_of(id: u32) -> &'static str {
    match id {
        x if x == block_id::AIR => "AIR",
        x if x == block_id::STONE => "STONE",
        x if x == block_id::DIRT => "DIRT",
        x if x == block_id::GRASS_BLOCK => "GRASS_BLOCK",
        x if x == block_id::BEDROCK => "BEDROCK",
        x if x == block_id::SAND => "SAND",
        x if x == block_id::SANDSTONE => "SANDSTONE",
        x if x == block_id::GRAVEL => "GRAVEL",
        x if x == block_id::WATER => "WATER",
        x if x == block_id::COAL_ORE => "COAL_ORE",
        x if x == block_id::IRON_ORE => "IRON_ORE",
        x if x == block_id::GOLD_ORE => "GOLD_ORE",
        x if x == block_id::REDSTONE_ORE => "REDSTONE_ORE",
        x if x == block_id::LAPIS_ORE => "LAPIS_ORE",
        x if x == block_id::DIAMOND_ORE => "DIAMOND_ORE",
        x if x == block_id::EMERALD_ORE => "EMERALD_ORE",
        x if x == block_id::OAK_LOG => "OAK_LOG",
        x if x == block_id::OAK_LEAVES => "OAK_LEAVES",
        x if x == block_id::SPRUCE_LOG => "SPRUCE_LOG",
        x if x == block_id::SPRUCE_LEAVES => "SPRUCE_LEAVES",
        x if x == block_id::BIRCH_LOG => "BIRCH_LOG",
        x if x == block_id::BIRCH_LEAVES => "BIRCH_LEAVES",
        x if x == block_id::SHORT_GRASS => "SHORT_GRASS",
        x if x == block_id::TALL_GRASS => "TALL_GRASS",
        x if x == block_id::SNOW_LAYER => "SNOW_LAYER",
        x if x == block_id::DANDELION => "DANDELION",
        x if x == block_id::POPPY => "POPPY",
        _ => "UNKNOWN",
    }
}

// VarInt zigzag signed read (Bedrock VarInt signed = zigzag LEB128).
fn read_var_i32(buf: &[u8], pos: &mut usize) -> i32 {
    let mut result: u32 = 0;
    let mut shift = 0u32;
    loop {
        let b = buf[*pos];
        *pos += 1;
        result |= ((b & 0x7f) as u32) << shift;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    ((result >> 1) as i32) ^ (-((result & 1) as i32))
}

fn words_count_for_bpb(bpb: u8) -> usize {
    match bpb {
        0 => 0,
        1 => 128,
        2 => 256,
        3 => 410,
        4 => 512,
        5 => 683,
        6 => 820,
        8 => 1024,
        16 => 2048,
        _ => panic!("bpb {bpb} no soportado"),
    }
}

fn main() {
    let gen = NormalGenerator::new(0);
    println!("seed=0 generator spawn_position={:?}", gen.spawn_position());
    let (cx, cz) = (1i32, 0i32);
    let chunk = gen.generate_chunk_internal(cx, cz);
    let dominant = chunk.biomes[7 * 16 + 7];
    println!(
        "chunk ({cx},{cz}) dominant_biome={dominant} biomes_distinct={:?}",
        {
            let mut v = chunk.biomes.to_vec();
            v.sort();
            v.dedup();
            v
        }
    );
    let (sub_chunk_count, payload) = chunk.serialize_network_payload(dominant);
    println!("subChunkCount={sub_chunk_count} payload_len={}", payload.len());
    println!("payload_head_hex={:02x?}", &payload[..payload.len().min(64)]);
    println!("payload_tail_hex={:02x?}", &payload[payload.len().saturating_sub(64)..]);

    // ---- Parse EXACTO como lo haría el cliente (Bedrock 1.26.63) ----
    let mut pos = 0usize;
    let mut distinct: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut total_palette_entries = 0usize;

    for sc in 0..sub_chunk_count {
        let sc_world_y = -4 + sc as i32;
        let ver_off = pos;
        let version = payload[pos];
        pos += 1;
        let layers = payload[pos];
        pos += 1;
        println!("subchunk #{sc} (index Y={sc_world_y}) version={version} layers={layers} @off {ver_off}..{}", pos);
        assert_eq!(version, 8, "versión de subchunk debe ser 8");
        for (li, _) in (0..layers).enumerate() {
            let hdr_off = pos;
            let header = payload[pos];
            pos += 1;
            let bpb = header >> 1;
            let is_runtime = header & 1;
            let nwords = words_count_for_bpb(bpb);
            let words_off = pos;
            pos += nwords * 4;
            println!(
                "  layer {li}: header={header} bpb={bpb} runtime={is_runtime} words={nwords} @hdr {hdr_off} words@{words_off}..{pos}"
            );
            assert_eq!(is_runtime, 1, "paleta debe ser runtime (hash)");
            let psz;
            if bpb != 0 {
                let sz_off = pos;
                psz = read_var_i32(&payload, &mut pos);
                println!("    palette_size={psz} @off {sz_off}..{pos}");
            } else {
                psz = 1;
            }
            for pi in 0..psz {
                let e_off = pos;
                let v = read_var_i32(&payload, &mut pos);
                let u = v as u32;
                println!("    palette[{pi}] off={e_off}..{pos} i32={v} u32={u} {}", name_of(u));
                distinct.entry(u).or_default().push(e_off);
                total_palette_entries += 1;
            }
        }
    }

    // 24 biomes
    println!("biomes a partir de off {pos}");
    for i in 0..24 {
        let hdr_off = pos;
        let header = payload[pos];
        pos += 1;
        let bpb = header >> 1;
        assert_eq!(header & 1, 1, "biome paleta runtime");
        let nwords = words_count_for_bpb(bpb);
        let words_off = pos;
        pos += nwords * 4;
        let psz = if bpb != 0 { read_var_i32(&payload, &mut pos) } else { 1 };
        for pi in 0..psz {
            let e_off = pos;
            let v = read_var_i32(&payload, &mut pos);
            let words_end = words_off + nwords * 4;
            println!("  biome#{i} header={header} bpb={bpb} words@{words_off}..{words_end} palette[{pi}]={v} @off {e_off}..{pos}");
        }
        if i == 0 {
            println!("  (hdr_off={hdr_off})");
        }
    }
    let border_off = pos;
    let border = payload[pos];
    pos += 1;
    println!("border off={border_off} value={border} (esperado 0)");
    let tiles_off = pos;
    println!("tiles off={tiles_off} restantes={} (esperado 0 sin Tiles; PM escribe bytearray vacío=silencio)", payload.len() - pos);
    assert_eq!(pos, payload.len(), "el payload debe consumirse exacto");

    // LevelChunk paquete (encabezado fuera del payload)
    println!("--- LevelChunkPacket (payload es putString: var_u32(len)+payload) ---");
    println!("chunk=({cx},{cz}) dimension=0 sub_chunk_count={sub_chunk_count} cache_enabled=false used_blob_hashes=0 client_biome_update=false");
    println!("wire payload string len={} (var_u32)", sub_chunk_count);
    println!("total palette entries={total_palette_entries} distinct runtime ids={}", distinct.len());
    let mut ids: Vec<(String, u32, i32, usize)> = distinct
        .iter()
        .map(|(u, offs)| (name_of(*u).to_string(), *u, *u as i32, offs[0]))
        .collect();
    ids.sort_by(|a, b| a.3.cmp(&b.3));
    for (n, u, s, off) in &ids {
        println!("  {n} u32={u} i32={s} first_off={off} occ={}", distinct[&*u].len());
    }
    // Palabras clave del task
    for (n, u, s, off) in &ids {
        if matches!(n.as_str(), "AIR" | "GRASS_BLOCK" | "WATER" | "SNOW_LAYER" | "DANDELION" | "POPPY" | "SHORT_GRASS" | "TALL_GRASS") {
            println!("  PIN {n} u32={u} i32={s} @payload_off {off}");
        }
    }
    // Comparación orden / bordes
    println!("sanity: primera paleta no vacía bpb y words alineación, biome_count=24, border=0, tiles_restantes={}", payload.len() - tiles_off);
    let _ = total_palette_entries;
    // Guarda raw para contraste externo
    let path = "C:\\Users\\matic\\AppData\\Local\\Temp\\opencode\\spawn_1_0_seed0.payload.bin";
    std::fs::write(path, &payload).unwrap();
    println!("raw escrito en {path}");

    // ---- Escaneo radio 5 alrededor de (1,0) para cazar bloque suelto ----
    println!("--- SCAN radio 5 (chunks -4..6, -5..5) seed 0 ---");
    let mut global: HashMap<u32, Vec<(i32, i32, usize)>> = HashMap::new();
    for ccx in -4..=6 {
        for ccz in -5..=5 {
            let ch = gen.generate_chunk_internal(ccx, ccz);
            let dom = ch.biomes[7 * 16 + 7];
            let (scn, pl) = ch.serialize_network_payload(dom);
            let mut p = 0usize;
            for _ in 0..scn {
                let _v = pl[p]; p += 1;
                let layers = pl[p]; p += 1;
                for _ in 0..layers {
                    let header = pl[p]; p += 1;
                    let bpb = header >> 1;
                    let nw = words_count_for_bpb(bpb);
                    p += nw * 4;
                    let psz = if bpb != 0 { read_var_i32(&pl, &mut p) } else { 1 };
                    assert!(psz >= 1 && psz <= 4096, "palette_size {psz} en chunk ({ccx},{ccz})");
                    for _ in 0..psz {
                        let eoff = p;
                        let v = read_var_i32(&pl, &mut p);
                        let u = v as u32;
                        global.entry(u).or_default().push((ccx, ccz, eoff));
                    }
                }
            }
            // biomes 24 + border
            for _ in 0..24 {
                let header = pl[p]; p += 1;
                let bpb = header >> 1;
                let nw = words_count_for_bpb(bpb);
                p += nw * 4;
                let psz = if bpb != 0 { read_var_i32(&pl, &mut p) } else { 1 };
                for _ in 0..psz {
                    let _ = read_var_i32(&pl, &mut p);
                }
            }
            let border = pl[p]; p += 1;
            assert_eq!(border, 0, "border debe ser 0 en ({ccx},{ccz})");
            assert_eq!(p, pl.len(), "payload debe consumirse exacto en ({ccx},{ccz}): {p} vs {}", pl.len());
            // biome dominant para logs
            // (silencioso)
            let _ = dom;
        }
    }
    println!("global distinct={} total occ={}", global.len(), global.values().map(|v| v.len()).sum::<usize>());
    let mut all: Vec<(String, u32, i32, usize)> = global.iter().map(|(u, occs)| (name_of(*u).to_string(), *u, *u as i32, occs.len())).collect();
    all.sort_by(|a, b| b.3.cmp(&a.3));
    for (n, u, s, c) in &all {
        println!("  {n} u32={u} i32={s} occ={c} e.g. chunk {:?}", global[u][0]);
    }
    // Hashes fuera de la tabla block_id (UNKNOWN) -> candidatos a Block
    for (n, u, s, c) in &all {
        if n == "UNKNOWN" {
            println!("  !!! UNKNOWN en scan u32={u} i32={s} occ={c}");
        }
    }
}
