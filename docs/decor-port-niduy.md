# Port niduy `minecraft-world-generator` → Rust `bedrock-world` (decoración)

Upstream: <https://github.com/niduy/minecraft-world-generator> (TypeScript, MIT).
Su README acredita **Cubiome**, **MCP-Reborn** y *"Minecraft terrain
generation in a nutshell"*.

## Hallazgo clave

El repositorio de niduy **no contiene árboles, cuevas ni ores a nivel de
bloque**. Es un renderizador 2D (mapa de biomas sobre canvas) de la
generación Java moderna (1.18+): ruido de biomas tipo Cubiome + **posiciones
de 12 estructuras** (`STRUCTURE` en `const.ts`). No hay `setBlock`, ni
paletas, ni voxels. Portar "árboles/cuevas/ores desde niduy" es, en rigor,
imposible sin inventar: lo que se porta es su única capa decorativa real
(**posiciones de estructuras**) y se rellenan cuevas según el patrón
PocketMine ya existente, manteniendo árboles/ores actuales.

## Comparativa por capas

| Capa | niduy (`minecraft.ts` / `const.ts`) | `NormalGenerator` Rust (antes) | Este port | Test que lo cubre |
|---|---|---|---|---|
| Terreno base | Ruido Java 1.18+ (`BiomeNoise`, splines, `Xoroshiro`) | Simplex 3D PocketMine + elevación gaussiana por bioma | Sin cambio (la semilla 0 manda) | `java_chunk_test` (fluid 812) |
| GroundCover | No existe (biomas continuos) | `GroundCoverPopulator` en `generate_base_chunk` | Sin cambio | `generator_test` |
| Árboles | **No existen** | `TreePopulator` Oak/Spruce/Birch por bioma (`populators/tree.rs`) | Sin cambio (ya es PocketMine) | `decor_port_test::cave_generation_is_deterministic` (vóxeles estables) |
| Hierba alta | No existe | `TallGrassPopulator` por bioma | Sin cambio | `generator_test` |
| Ores | **No existen** | `OrePopulator` (carbón→diamante, tierra, grava, esmeralda) | Sin cambio; las cuevas **no** borran ores (solo tallan `STONE`) | `caves_carve_only_stone_below_cap_on_land_chunk` |
| Cuevas | **No existen** | No existían | **Nuevo** `generator/caves.rs`: túneles spaghetti, 3 por chunk, solo `STONE→AIR`, `6 <= Y <= 44`, RNG dedicado | `caves::carves_air_below_max_y…`, `caves_carve_only_stone_below_cap…` |
| Estructuras | 12 `STRUCTURE` con `(salt, region_size, chunk_range)` + offset triangular + LCG Java 48-bit | No existían | **Nuevo** `generator/structures.rs` + `NormalGenerator::structures_at`: posiciones puras, sin vóxeles | `structures::*`, `structures_are_deterministic_and_sane`, `structures_do_not_mutate_voxels` |
| RNG | `Random` Java (LCG `0x5DEECE66D`) por chunk (`chunkGenerateRnd`) | `PocketMine Random` XorShift128, semilla por chunk `0xdeadbeef ^ cx<<8 ^ cz ^ seed` | Sin intercambio: estructuras usan su propio LCG u64, cuevas usan un XorShift **dedicado**. El stream compartido ores/árboles no se consume | `seed0_chunk_0_0_fluid_812_and_blocks_3866` (byte-identidad) |
| Streaming | Dibujo en canvas sin entorno de chunk manager | `ChunkManager3x3` (árboles/ores sangran 1 chunk) | Cuevas y árboles usan el mismo `ChunkManager::set_block_at`; fuera del 3×3 se recorta | (mismo que árboles) |
| `World::setBlock` (PHP) | No existe equivalente reedición API | Host `world.set_block` → `world.json` → `generate_chunk_with_edits` | Sin cambio; es **edición persistida**, no scatter RNG | `world_setblock_host_edit_is_not_a_populator` |

### `World::setBlock` vs populators

- **Populator** (`crates/bedrock-world/src/generator/populators/*`,
  `manager.rs`): scatter determinista por chunk, vía `ChunkManager::set_block_at`,
  limitado al 3×3. No persiste; el bloque vive solo en el `Chunk` generado.
- **`World::setBlock`** (`crates/php-plugin-bridge/php/world.php:493,924` →
  `Bridge::hostCall('world.set_block', …)`): edición **host** persistida en
  `world.json`, aplicada en `generate_chunk_with_edits` filtrando por chunk.
  No consume RNG ni toca el terreno base.

## Invariantes preservadas (seed 0)

| Chunk | Bioma | Agua sec. 7 | Bloques sec. 7 | Antes | Después |
|---|---|---|---|---|---|
| (0, 0) | RIVER (7) | 812 | 3866 | ✔ | ✔ (sin cuevas: bioma de agua; cap Y ≤ 44) |
| (29, 1) | PLAINS (1) | 424 | 4021 | ✔ | ✔ (cuevas ≤ Y 44; sec. 7 intacta) |
| (5, 5) | PLAINS (1) | 0 | 4096 | — | cuevas: 281 celdas `STONE→AIR` bajo cap |

Tres decisiones lo garantizan: (1) RNG de cuevas **dedicado** post-ore/tree,
(2) cap absoluto `Y <= 44` (sección 7 = Y 48..=63, inalcanzable), (3) gate
de bioma de agua (`OCEAN | RIVER | ICE_PLAINS | SWAMPLAND`) + tallado solo
`STONE`.

## Estructuras semilla 0 (32×32 chunks, 13 hallazgos)

`JunglePyramid (0,10)`, `SwampHut (0,18)`, `OceanRuins (5,23)`, `OceanRuins (9,2)`,
`Shipwreck (14,8)`, `Igloo (14,10)`, `AncientCity (15,1)`, `Village (15,2)`,
`DesertPyramid (19,2)`, `RuinedPortal (20,9)`, `OceanMonument (21,18)`,
`OceanRuins (28,21)`, `OceanRuins (29,7)`. Ninguna cae en (0,0) ni (29,1);
aun así son **posición-only** (ningún bloque) y no alteran terreno.

Divergencia menor documentada: la rama `chunkRange == 1` de la TS escribe
`pos.x` dos veces y nunca `pos.z`; el port toma la rama sensata (ninguna de
las 12 configs usa rango 1, sin pérdida de paridad).

## Tests

- Nuevos: `crates/bedrock-world/tests/decor_port_test.rs` (7 tests, todos ✔
  en copia aislada — ver abajo).
- Existentes intactos: `java_chunk_test` (8 ✔, incluido
  `real_column_chunk_0_0_ocean_water_y62_fluidcount_812`),
  `generator_test` (6 ✔), lib unittests (17 ✔).

> Nota de entorno: durante el trabajo el workspace compartido contenía en
> paralelo el WIP ajeno `generator/java_terrain.rs` (no compilaba en ese
> momento); tras su corrección, `cargo test -p bedrock-world` pasa completo
> en el árbol compartido: lib 24 + block_access 7 + decor_port 7 +
> generator 6 + java_chunk 8 + subchunk 9. Ningún fichero ajeno fue
> modificado por este port.

## Diff (resumen)

```diff
+ crates/bedrock-world/src/generator/structures.rs  # LCG Java + 12 configs + lookup por chunk
+ crates/bedrock-world/src/generator/caves.rs        # CavePopulator (Y 6..44, STONE-only)
  crates/bedrock-world/src/generator/mod.rs          # pub mod caves/structures
  crates/bedrock-world/src/generator/normal.rs       # campo cave_populator + paso final con RNG dedicado + structures_at()
+ crates/bedrock-world/tests/decor_port_test.rs      # 7 tests de regresión decor + semilla 0
+ docs/decor-port-niduy.md                           # este documento
```
