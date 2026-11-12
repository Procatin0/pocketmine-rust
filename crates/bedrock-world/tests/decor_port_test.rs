//! Decor port regression tests: niduy structures (positional), cave tunnels,
//! and the seed-0 invariants that must not move.
//!
//! The certified anchors (from `java_chunk_test.rs`):
//! - chunk (0, 0) seed 0: biome RIVER, water in section 7 (Y 48..=63) = 812,
//!   non-air blocks in section 7 = 3866, no water outside section 7;
//! - chunk (29, 1) seed 0: biome PLAINS, water in section 7 = 424,
//!   non-air blocks in section 7 = 4021.
//! Caves are capped at Y <= 44 and skipped for water biomes, structures are
//! position-only, and trees/ores keep the original shared RNG stream — so
//! these anchors, and the `World::setBlock` host-edit path, are untouched.

use bedrock_world::block_id;
use bedrock_world::generator::structures::StructureKind;
use bedrock_world::{NormalGenerator, WorldGenerator};

fn section7_counts(chunk: &bedrock_world::Chunk) -> (usize, usize) {
    let mut water = 0usize;
    let mut non_air = 0usize;
    for x in 0..16 {
        for z in 0..16 {
            for y in 48..=63 {
                let b = chunk.get_block(x, y, z);
                if b == block_id::WATER {
                    water += 1;
                }
                if b != block_id::AIR && b != 0 {
                    non_air += 1;
                }
            }
        }
    }
    (water, non_air)
}

fn total_water(chunk: &bedrock_world::Chunk) -> usize {
    let mut n = 0usize;
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..128 {
                if chunk.get_block(x, y, z) == block_id::WATER {
                    n += 1;
                }
            }
        }
    }
    n
}

#[test]
fn seed0_chunk_0_0_fluid_812_and_blocks_3866() {
    let gen = NormalGenerator::new(0);
    let chunk = gen.generate_chunk_internal(0, 0);
    assert_eq!(chunk.biomes[7 * 16 + 7], 7, "chunk (0,0) es RIVER");
    let (water7, blocks7) = section7_counts(&chunk);
    assert_eq!(water7, 812, "fluidCount sección 7");
    assert_eq!(blocks7, 3866, "blockCount sección 7");
    assert_eq!(total_water(&chunk), 812, "toda el agua está en sección 7");
}

#[test]
fn seed0_chunk_29_1_water_424_and_blocks_4021() {
    let gen = NormalGenerator::new(0);
    let chunk = gen.generate_chunk_internal(29, 1);
    assert_eq!(chunk.biomes[7 * 16 + 7], 1, "chunk (29,1) es PLAINS");
    let (water7, blocks7) = section7_counts(&chunk);
    assert_eq!(water7, 424, "agua en sección 7");
    assert_eq!(blocks7, 4021, "bloques no-aire en sección 7");
}

#[test]
fn caves_carve_only_stone_below_cap_on_land_chunk() {
    let gen = NormalGenerator::new(0);
    // (5,5) seed 0 es PLAINS, sin superficie de agua: las cuevas sí actúan.
    let base = gen.generate_base_chunk(5, 5);
    let full = gen.generate_chunk_internal(5, 5);
    assert_eq!(full.biomes[7 * 16 + 7], 1);

    let mut carved = 0usize;
    for x in 0..16 {
        for z in 0..16 {
            let base_h = base.get_highest_block_at(x, z);
            let full_h = full.get_highest_block_at(x, z);
            assert_eq!(base_h.is_some(), full_h.is_some(), "columna ({x},{z})");
            if let (Some(bh), Some(fh)) = (base_h, full_h) {
                // La hierba alta puede coronar (+1); las cuevas nunca tocan
                // el bloque de superficie base.
                assert!(
                    fh == bh || fh == bh + 1,
                    "superficie ({x},{z}) base {bh} full {fh}"
                );
                assert_eq!(
                    full.get_block(x, bh, z),
                    base.get_block(x, bh, z),
                    "bloque de superficie ({x},{z})"
                );
            }
            for y in 0..128 {
                let b0 = base.get_block(x, y, z);
                let b1 = full.get_block(x, y, z);
                if b1 == block_id::WATER {
                    assert_eq!(b0, block_id::WATER, "caves/ores/trees no añaden agua ({x},{y},{z})");
                }
                if b0 == block_id::STONE && b1 == block_id::AIR {
                    assert!(y <= 44, "hueco sobre el cap en ({x},{y},{z})");
                    carved += 1;
                }
                if y > 44 && b0 == block_id::STONE && b1 == block_id::AIR {
                    panic!("hueco sobre cap en ({x},{y},{z})");
                }
            }
        }
    }
    assert!(carved > 0, "las cuevas deberían tallar en la chunk (5,5)");
    println!("seed0 (5,5): cuevas STONE->AIR bajo cap = {carved}");
}

#[test]
fn cave_generation_is_deterministic() {
    let gen = NormalGenerator::new(0);
    let a = gen.generate_chunk_internal(5, 5);
    let b = gen.generate_chunk_internal(5, 5);
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..128 {
                assert_eq!(a.get_block(x, y, z), b.get_block(x, y, z), "({x},{y},{z})");
            }
        }
    }
}

#[test]
fn structures_are_deterministic_and_sane() {
    let gen = NormalGenerator::new(0);
    let mut found = 0usize;
    for cx in 0..32 {
        for cz in 0..32 {
            let a = gen.structures_at(cx, cz);
            let b = gen.structures_at(cx, cz);
            assert_eq!(a, b, "chunk ({cx},{cz}) determinista");
            for pos in &a {
                assert_eq!(pos.chunk_x, cx);
                assert_eq!(pos.chunk_z, cz);
                assert_eq!(pos.block_x, cx * 16);
                assert_eq!(pos.block_z, cz * 16);
                found += 1;
            }
        }
    }
    assert!(found > 0, "seed 0 debería hallar estructuras en 32x32 chunks");
    println!("seed0 estructuras en 32x32 chunks: {found}");
    for kind in [
        StructureKind::DesertPyramid,
        StructureKind::Village,
        StructureKind::OceanMonument,
        StructureKind::Mansion,
        StructureKind::AncientCity,
    ] {
        assert!(bedrock_world::generator::structures::structure_config(kind).is_some());
    }
}

#[test]
fn structures_do_not_mutate_voxels() {
    // Las posiciones son solo lectura: regenerar <<con>> structures_at
    // devuelve el mismo chunk bloque a bloque.
    let gen = NormalGenerator::new(12345);
    let _before = gen.structures_at(3, -7);
    let a = gen.generate_chunk_internal(3, -7);
    let _after = gen.structures_at(3, -7);
    let b = gen.generate_chunk_internal(3, -7);
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..128 {
                assert_eq!(a.get_block(x, y, z), b.get_block(x, y, z), "({x},{y},{z})");
            }
        }
    }
}

#[test]
fn world_setblock_host_edit_is_not_a_populator() {
    // `World::setBlock` (PHP -> host `world.set_block` -> `world.json`)
    // se materializa vía `generate_chunk_with_edits`: un bloque donde ningún
    // populator (árboles/ores/cuevas) pone piedra en el aire a Y=90.
    let gen = NormalGenerator::new(0);
    let (cx, _payload) = (5, gen.generate_chunk(5, 5));
    let _ = cx;
    let plain = gen.generate_chunk_internal(5, 5);
    let wx = 5 * 16 + 3;
    let wz = 5 * 16 + 9;
    assert_ne!(
        plain.get_block(3, 90, 9),
        block_id::STONE,
        "sin edición no hay piedra flotante a Y=90"
    );

    let (_, payload) = gen.generate_chunk_with_edits(5, 5, &[(wx, 90, wz, block_id::STONE)]);
    assert!(!payload.is_empty());
    // El camino editado contiene la piedra; el chunk puro no.
    let (_, payload_plain) = gen.generate_chunk(5, 5);
    assert_ne!(payload, payload_plain, "la edición altera el payload");
}
