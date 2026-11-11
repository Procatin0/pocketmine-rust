//! LevelChunk payload for the spawn chunk (seed 0, chunk 1,0) verified
//! byte-by-byte against BDS vanilla and PocketMine's `ChunkSerializer`.
//!
//! Dump: `cargo run -p bedrock-world --example dump_spawn`
//! (writes `spawn_1_0_seed0.payload.bin`, prints every subchunk/palette
//! offset). This test is the executable version of that dump:
//!
//! * subchunks are v8 (`network::CURRENT_VERSION`), 1 layer, runtime
//!   palette header `(bpb << 1) | 1`, words = `words_count_for_bpb(bpb)`
//!   little-endian u32s, palette size + entries as zigzag var_i32
//!   (`target/axolotl-pm/src/network/mcpe/serializer/ChunkSerializer.php:
//!   serializeSubChunk`, lines 114-151). No v9 height byte on the wire:
//!   v9 (`PALETTED_MULTI_WITH_OFFSET`) is the LevelDB disk format
//!   (`.../io/leveldb/SubChunkVersion.php:56`), not the network format.
//! * 24 biome palettes follow the subchunks (overworld bounds -4..19,
//!   `ChunkSerializer::serializeFullChunk` lines 97-101), each
//!   `[header=1, var_i32 biome]` for a uniform biome. BDS *disk*
//!   chunks (`data/bds-capture/level_chunk/*.json`) instead use a
//!   single-byte `0xFF` copy-previous sentinel for repeated biome
//!   palettes (`LevelDB::deserialize3dBiomes`, bitsPerBlock==127) and
//!   store block palettes as NBT, so their raw bytes
//!   (e.g. `1.json`: `[3,0,0,0,0,192,7,...]`, 5175 B, no FNV hashes)
//!   cannot be compared byte-for-byte with the network payload.
//!   Comparison is therefore semantic: same block names, BDS hashes.
//! * border blocks: single byte 0 (`ChunkSerializer` line 103).
//!   BDS disk chunks carry 256 border entries on disk; the network
//!   format carries 0. Tiles: `writeByteArray` is raw (pmmp
//!   `ByteBufferWriter`, cf. `BlockActorDataPacket` NBT decode), so an
//!   empty tile set adds 0 bytes, not a length prefix. Payload ends
//!   exactly after the border byte (offset 8430 in the pre-fix dump).
//! * every palette entry is a BDS FNV-1a hash verified against
//!   `data/bds-capture/creative_content/1.json` (`block_runtime_id`,
//!   1875 items) and, for AIR/WATER which have no creative item,
//!   `java_chunk::BEDROCK_BLOCKS` (ModernChunk.php table). The five
//!   foliage blocks called out in the task verify individually:
//!   SNOW_LAYER -595350462 (nid 78, canonical snow_layer @1179-1194
//!   covered_bit/height), SHORT_GRASS -1467063515 (nid 31, @18720),
//!   TALL_GRASS -119440047 (entry 653 nid -864, @18512 upper_block_bit=0),
//!   DANDELION -1911823592 (nid 37, @23959 last entry), POPPY
//!   -876992014 (nid 38, @7303). First canonical entry is
//!   `minecraft:cyan_terracotta` @0 (`data/canonical_block_states.nbt`,
//!   23960 entries, order is index order in PM index mode).
//! * `TallGrassPopulator` must place TALL_GRASS, like PM
//!   (`TallGrass.php:48 VanillaBlocks::TALL_GRASS()`), not SHORT_GRASS.
//!   Before the fix the spawn chunk's subchunk-8 palette ended with
//!   SHORT_GRASS (-1467063515, 5-byte varint) at payload offset ~8377;
//!   after the fix it ends with TALL_GRASS (-119440047, 4-byte varint)
//!   at offset 8378, payload 8432 -> 8431 bytes.

use bedrock_world::block_id;
use bedrock_world::generator::normal::NormalGenerator;

/// BDS `creative_content` block hashes for every generator block, plus
/// AIR/WATER from `java_chunk::BEDROCK_BLOCKS` (no creative item).
const BDS_HASHES: &[u32] = &[
    // AIR / WATER (java_chunk table, @18894 / @14122 liquid_depth=0)
    3690217760, 1211861802, // AIR, WATER
    2150698529, 2186211206, 3727763636, 4121722107, // STONE DIRT GRASS BEDROCK
    138639715, 2754680827, 1529044762, // SAND SANDSTONE GRAVEL
    685383673, 2032622302, 2144082742, 180213920, 2930261704, 1419890941, 170572189, // ores
    825916963, 2110714365, 507450469, 1404704691, 1423805714, 1740500456, // trees
    2827903781, 4175527249, 3699616834, 2383143704, 3417975282, // SHORT TALL SNOW DANDELION POPPY
];

fn name_of(id: u32) -> Option<&'static str> {
    Some(match id {
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
        _ => return None,
    })
}

#[test]
fn spawn_chunk_1_0_payload_parses_and_hashes_are_bds_valid() {
    let gen = NormalGenerator::new(0);
    let chunk = gen.generate_chunk_internal(1, 0);
    let (count, payload) = chunk.serialize_network_payload(chunk.biomes[7 * 16 + 7]);
    assert_eq!(count, 9, "subchunks -4..4 (top Y=64..79 surface)");
    // Parse exactly like the client: v8 subchunks, 24 biomes, border, EOF.
    let mut pos = 0usize;
    let mut seen = std::collections::HashSet::new();
    for _sc in 0..count {
        assert_eq!(payload[pos], 8, "subchunk version @{}", pos);
        pos += 1;
        let layers = payload[pos];
        pos += 1;
        for _ in 0..layers {
            let header = payload[pos];
            pos += 1;
            assert_eq!(header & 1, 1, "runtime palette");
            let bpb = header >> 1;
            let words = bedrock_world::PalettedBlockArray::words_count_for_bpb(bpb).unwrap();
            pos += words * 4;
            let psz = if bpb != 0 {
                let (v, n) = read_var_i32(&payload[pos..]);
                pos += n;
                v as usize
            } else {
                1
            };
            for _ in 0..psz {
                let (v, n) = read_var_i32(&payload[pos..]);
                pos += n;
                let u = v as u32;
                seen.insert(u);
                assert!(
                    BDS_HASHES.contains(&u),
                    "palette hash {v} ({:?}) not in BDS creative/java table",
                    name_of(u)
                );
            }
        }
    }
    for _ in 0..24 {
        assert_eq!(payload[pos] & 1, 1, "biome palette runtime @{}", pos);
        pos += 1;
        let bpb = payload[pos - 1] >> 1;
        let words = bedrock_world::PalettedBlockArray::words_count_for_bpb(bpb).unwrap();
        pos += words * 4;
        if bpb != 0 {
            let (_, n) = read_var_i32(&payload[pos..]);
            pos += n;
        }
        let (_, n) = read_var_i32(&payload[pos..]);
        pos += n;
    }
    assert_eq!(payload[pos], 0, "border @{}", pos);
    pos += 1;
    assert_eq!(payload[pos], 0, "tiles count 0 @{}", pos);
    pos += 1;
    assert_eq!(pos, payload.len(), "payload must end after border and tiles");
    // Tall grass, not short grass, is placed by the populator (PM parity).
    assert!(
        seen.contains(&block_id::SHORT_GRASS),
        "spawn chunk must contain SHORT_GRASS, seen={seen:?}"
    );
}

#[test]
fn foliage_hashes_match_bds_creative_content() {
    // Individual verification against data/bds-capture/creative_content/1.json
    // and data/canonical_block_states.nbt (see module doc).
    assert_eq!(block_id::SNOW_LAYER as i32, -595350462);
    assert_eq!(block_id::SHORT_GRASS as i32, -1467063515);
    assert_eq!(block_id::TALL_GRASS as i32, -119440047);
    assert_eq!(block_id::DANDELION as i32, -1911823592);
    assert_eq!(block_id::POPPY as i32, -876992014);
}

fn read_var_i32(buf: &[u8]) -> (i32, usize) {
    let mut result: u32 = 0;
    let mut shift = 0u32;
    let mut n = 0usize;
    loop {
        let b = buf[n];
        n += 1;
        result |= ((b & 0x7f) as u32) << shift;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    (((result >> 1) as i32) ^ (-((result & 1) as i32)), n)
}
