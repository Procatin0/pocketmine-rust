# 17 — Mundo y persistencia

## Modelo del mundo

El mundo vive en `crates/server-core/src/world_state.rs`
(`WorldState`) y `crates/bedrock-world/`:

| Capa | Fichero | Rol |
|---|---|---|
| Terreno determinista | `crates/bedrock-world/src/generator/{normal,flat,biome,noise,caves}.rs` + `populators/{tree,ore,ground_cover,tall_grass}.rs` | Genera la base a partir de la `seed`; `WorldGenerator` expone `block_at(x,y,z)`, `generate_chunk_with_edits`, `highest_block_at`, `seed()` y `spawn_position()` |
| Chunks paletizados | `crates/bedrock-world/src/{chunk,subchunk,paletted_block_array}.rs` + `network.rs` (serialización) | Formato de red Bedrock (`serialize_subchunk`, versión `CURRENT_NETWORK_VERSION`) |
| Estado compartido | `crates/server-core/src/world_state.rs` | Spawn, hora, clima, dificultad, ediciones persistidas, cachés de chunks y colas de cambios de bloques |
| Traducción a Java | `crates/bedrock-world/src/java_chunk.rs` | `ModernChunk` / `ChunkBuilder` (ver `16-java-bridge.md`) |
| Sesiones | `crates/server-core/src/session.rs` (`PlayerSession`, `SessionManager`) | Consumo, guardado y sincronización |

`WorldState::new(name, generator)` recibe el generador de
`ServerConfig::create_world_generator()` (`level-type` +
`level-seed` de `server.properties`). Hay **un solo mundo**,
`"world"`: el multi-world completo (`WorldManager`,
generadores por mundo, `/tp` entre mundos) no existe; un
teletransporte cross-world reinstancia un `WorldState` limpio
con el mismo generador (best-effort, documentado en el header
del módulo).

Fidelidad al bandido `target/niduy-mwg` (referencia externa de
terreno, migas/paridad): el terreno se regenera desde la semilla
con los mismos paisajes (río, llanura, bosque) y los tests de
regresión comparan alturas de piedra/dirt/agua contra el PHP.

## `WorldState`: reloj, clima, spawn, ediciones

- **Reloj**: `TICKS_PER_DAY = 24000`, `DEFAULT_WORLD_TIME = 1000`
  (mañana). `start_time()` / `stop_time()` (`/time start|stop`)
  con `time_running: AtomicBool`; `tick()` avanza solo cuando
  corre y envuelve a 0. Cada tick, `SessionManager::tick`
  → `WorldState::tick` (un solo reloj de 20 TPS en `main.rs`,
  ver `16-java-bridge.md`).
- **Clima**: 0 despejado, 1 lluvia, 2 trueno (`/weather`).
- **Dificultad**: 0 peaceful … 3 hard (`/difficulty`, también
  `SetDifficultyPacket 0x3C` a todos los Bedrock).
- **Spawn**: `spawn()` / `set_spawn(x,y,z)` (`/setworldspawn`) en
  `RwLock`; el spawn por defecto es `generator.spawn_position()`.
- **Ediciones**: mapa `edits: HashMap<(chunk_x, chunk_z),
  HashMap<(x,y,z), block_id>>` sobre el terreno generado.
  `set_block(x,y,z,id)` guarda, invalida la columna y **encola
  el cambio en dos colas independientes** (Bedrock ←→ Java):
  - `block_changes` → `take_changes()` / `drain_changes()` (host
    `world.take_block_changes`, bridge Java, 1×/tick).
  - `broadcast_changes` → `take_broadcast_changes()`
    (`SessionManager::poll_block_changes` → `UpdateBlockPacket
    0x15` a sesiones con la columna en vista).
  - **Vecinos**: tras escribir, `set_block` ejecuta el cascada de
    PocketMine (`World::internalNotifyNeighbourBlockUpdate` →
    `Block::onNearbyBlockChange`): el bloque y sus 6 vecinos
    recalculan conexiones (`bedrock_world::block_states::recalculate_connections`,
    reglas de `Fence`/`Thin`/`Wall`) y, si el estado cambia, se
    reescriben y se vuelve a propagar (máx. 4 pasadas hasta el punto
    fijo, con deduplicado por celda como
    `neighbourBlockUpdateQueueIndex`). Así una valla colocada al lado
    de otra actualiza **las dos** en ambas colas, y romper un bloque
    desconecta las vallas/paredes/paneles vecinos.
  - **Coalescing**: las escrituras del cascada no se encolan una a
    una. Se acumulan en un mapa `(x,y,z) -> (id previo, id final)` y
    se vuelcan al final, **un `BlockChange` por celda con el estado
    final** (equivalente a `changedBlocks` de PM, que se envía una vez
    por tick). Antes cada paso intermedio salía como paquete propio y
    el cliente veía las conexiones parpadear antes de estabilizarse.
  Cada cola está capada a `BLOCK_CHANGE_QUEUE_CAP = 512` y
  descarta lo **más antiguo** (nunca el más nuevo: reejecutar
  los supervivientes converge a la última edición). Un
  `set_block` sin cambio no encola nada. Rango Y
  `WORLD_MIN_Y = -64` … `WORLD_MAX_Y = 320`.
- **Interacción**: `WorldState::interact_block` aplica `Block::onInteract()`
  (botones, palancas, puertas de valla) sobre esta misma ruta de `set_block`,
  así que el cambio se difunde igual y dispara el cascada de vecinos. Ver
  `16-java-bridge.md`.
- **Actualizaciones programadas**: `schedule_delayed_block_update(x, y, z,
  block_id, ticks, sound)` es el `World::scheduleDelayedBlockUpdate` de PM.
  La cola se vacía al principio de `WorldState::tick` con un contador propio
  (sigue corriendo con `/time stop`); cada entrada se aplica con `set_block` y
  su sonido queda en `pending_sounds`, que `SessionManager::tick` difunde a los
  viewers. Hoy el único productor es el botón pulsado, que se suelta solo.
- **Cachés**: `chunk_cache` (payload serializado, FIFO
  `CHUNK_PAYLOAD_CACHE_CAP = 512` — p. ej. el área de
  `view-distance:6`, 169 columnas, con hueco para dos jugadores)
  y `chunk_object_cache` (objeto `Chunk`, FIFO
  `CHUNK_OBJECT_CACHE_CAP = 1024` — dos áreas a radio 10 de 441
  columnas). `set_block` invalida ambas + bump de
  `chunk_versions[(cx,cz)]` (contador por columna que
  `CoreHooks` usa como clave de sus payloads Java convertidos:
  una edición jamás deja un payload stale). El prefetch de
  `main.rs` consulta `has_chunk_cached` para saltar columnas ya
  calientes. Sin caché, generar una columna cuesta ~18 ms; eso
  desplomaba el TPS en la carga inicial, de ahí las cachés.
- **Consultas de bloque sueltas**: `WorldState::block_at` cae al generador
  cuando la celda no está editada, y `propagate_connections` hace decenas de
  consultas por edición. `NormalGenerator::block_at`/`highest_block_at`
  guardan la columna en una caché FIFO de 32 entradas
  (`COLUMN_CACHE_CAP`) porque `generate_chunk_internal` construye un barrio 3x3
  con todos los populators (~12 ms medidos): sin ella, una sola rotura costaba
  125–665 ms de generación repetida (`[lag-slow] actions=4 total=665.4ms
  slowest=665.2ms: SetBlock`). Con caché, 200 consultas en la misma columna
  tardan ~40 µs. La caché es solo para consultas sueltas: el streaming sigue
  generando con ediciones aplicadas (`generate_chunk_with_edits`).
- **Streaming y locks**: `SessionManager::drive_chunk_requests` calienta
  antes las columnas que cada sesión va a enviar (fuera del lock de
  sesiones) y solo después lo toma para montar los paquetes;
  `drain_pending_outgoing_bytes` saca las colas bajo el lock y codifica
  y comprime fuera. Generar una columna (~18 ms) o deflactar sus 62 KB
  con `sessions.write()` tomado bloqueaba a cualquier manejador de
  paquetes: la rotura de un bloque esperaba 60-160 ms (sonido y
  partículas incluidos), el tick derivaba y romper rápido tiraba el
  peer. El test `chunk_streaming_does_not_hold_the_sessions_lock` lo
  fija.
- **Orden por tick**: `main.rs` vacía la cola de salida *antes* del
  tick (que hace el streaming de chunks) y otra vez después, así un ack
  de rotura nunca espera detrás de la generación de terreno.
- **Partículas una sola vez**: el flush de `changedBlocks` solo manda
  el `UpdateBlock`; `PARTICLE_DESTROY_BLOCK` lo emite quien rompe
  (`handle_block_actions` en Bedrock, `CoreHooks::break_block` en
  Java), como `World::useBreakOn`.
- **Edición contada**: `edited_chunk_count()` para diagnóstico.

## `world.json` v2

`WorldState::save_to(path)` escribe JSON v2
(`WORLD_FORMAT_VERSION = 2`), cargado con `load_from`:

```json
{"version":2,"name":"world","time":1000,"weather":0,"difficulty":2,
 "spawn":[26,64,-16],"seed":0,"auto_save":true,"auto_save_interval":6000,
 "time_running":true,"edits":[]}
```

- `edits` es la lista plana `[x, y, z, block_id]` de **todas** las
  ediciones. Los mundos muy editados crecen con el número de
  ediciones, no con el tamaño del mundo.
- v1 (sin `version`) sigue cargando con `auto_save` /
  `time_running = true`; un archivo ausente o corrupto deja el
  mundo intacto (`load_from` devuelve `false`).
- Se guarda en arranque (`main.rs` → `load_from("world.json")`
  en silencio, PM no tiene `world.json`) y con `/save-all` +
  autoguardado cada 6000 ticks (5 min = `ticks-per.autosave`,
  `WorldManager`). `auto_save` / `auto_save_interval` son
  configurables (`/save-on` `/save-off`, `WorldManager::
  setAutoSaveInterval()`).

### Divergencia documentada con PocketMine

PM persiste en **LevelDB/Anvil** (`level.dat` + `db/` de chunks).
Este port **no reimplementa LevelDB a propósito**: `world.json`
guarda el mismo snapshot lógico (tiempo, clima, dificultad,
spawn, semilla y ediciones) sin un serializador NBT/chunk.
Consecuencias (también en el header de `world_state.rs` y en
`docs/PORT-ROADMAP.md`, Fase 2):

- Mundos PM existentes (`level.dat`) **no son legibles**: el
  terreno se recrea desde la semilla y las ediciones se
  reejecutan.
- Sin NBT, los chunks no se serializan a disco: todo lo que no
  sea edición + semilla se regenera al arrancar.
- El inventario **aún no se persiste** (ver «Jugadores»).

## Jugadores: `PlayerContext` JSON (26 campos)

Bedrock y virtual (Java) guardan en **el mismo sitio**,
`players/<nombre-en-minúsculas>.json`, sin staging intermedio:

- Bedrock: `PlayerSession::save_player_data` /
  `load_player_data` (`crates/server-core/src/session.rs`).
- Java virtual: `PlayerContext::to_save_json` /
  `apply_save_json` (`crates/php-plugin-bridge/src/context.rs`)
  vía `player.save` — el mismo objeto byte a byte.

Formato canónico v2 (26 campos), producido
`PlayerContext::to_save_json`:

`name`, `xuid`, `display_name`, `x`, `y`, `z`, `yaw`, `pitch`,
`gamemode`, `health`, `max_health`, `food`, `saturation`, `xp`,
`xp_level`, `air`, `flying`, `sneaking`, `sprinting`,
`swimming`, `gliding`, `world`, `is_op`, `spawn_point`,
`first_played`, `last_played`.

Detalles:

- `y` es **Y canónico Bedrock** (nunca el `y+64` legacy Java; el
  `+64` vive solo en paquetes Java legacy).
- `first_played`/`last_played` son microtiempo (`microtime(true)`
  en PHP). Si el autosave llega antes de que la sesión cargara,
  se preservan los del fichero existente. `has_played_before()`
  (umbral axolotl: `last > first + 0.001s`).
- `food`/`saturation` por defecto 20, `air` 300, el resto 0/false.
  `spawn_point: [x,y,z] | null`.
- Al cargar, se aceptan alias PHP (`flying`/`is_flying`,
  `xp_level`/`xpLevel`, `sneaking`/`is_sneaking`, …) y ficheros
  v1 de 8 campos con defaults para el resto.
- Se cargan al hacer login (`PlayerSession::load_player_data`
  tras `PlayerJoinEvent` previo) y se escriben al salir y en
  `/save-all`/autoguardado (6000 ticks, `save_all()` en
  `spawn_blocking`).

PM usa `players/<name>.dat` (NBT); aquí JSON a propósito para
evitar el serializador NBT. Inventario pendiente (ver siguiente).

## `ChunkManager`, `Light`, `ChunkTicker`

`crates/bedrock-world/src/generator/manager.rs` define el abstracción
`ChunkManager` para los populators (árboles, menas, tall grass,
cuevas). Hay dos implementaciones:

- `ChunkManager3x3`: ventana 3×3 de chunks vecino cargada en
  memoria. `NormalGenerator::populate_chunk` lo usa para que
  decoración y menas crucen bordes (una cueva que cruza el borde
  sangra un chunk vecino).
- `ChunkManager for Chunk`: el chunk ya cargado/generado.

`ChunkTicker` **no** existe como sistema en este port. Lo que hay
es la semántica equivalente donde aplica: `WorldState::tick()`
(clock) + `SessionManager::tick` (sync a clientes). Podar
`chunk-ticking` (`tick-radius: 3`, `blocks-per-subchunk-per-tick:
3`, `disable-block-ticking` de `pocketmine.yml`) y `chunk-sending`
(`per-tick: 4`, `spawn-radius: 4`) son entradas de configuración
reales de PM que el mapa de plantillas conserva pero el tick
rustico aún no ejecuta efectos (crecimiento, random ticks). Ver
`docs/PORT-ROADMAP.md`, Fases 5–6.

`Light` (propagación de luz de cielo/bloque) **no** existe como
sistema. El chunk de red Bedrock y la luz Java son estáticos:
`ModernChunk::light()` manda sky light 15 pleno y block light
vacío (ver `16-java-bridge.md`); el cliente recalcula la luz de
cielo desde los heightmaps, y el capture real
`data/bds-capture/level_chunk` ya tiene la estructura para la
Fase 6. Resultado visible: chunks con luz de Java correctos de
día; sin luz propagada real de Bedrock ni antorchas por ahora.

## Pendientes P0 (persistencia real)

| Pendiente | Estado actual | Qué falta |
|---|---|---|
| **LevelDB / Anvil (`level.dat` + `db/`)** | JSON `world.json` (divergencia documentada) | Serializer chunk ↔ LevelDB, loader de `level.dat`, migración de mundos PM |
| **NBT completo** | Sin serializador; solo NBT mínimo en la fase 1 («NBT mínimo para `/give [... tags]`») | `crates/bedrock-protocol/src/nbt.rs` (tags Compound/String/Int/List), items con NBT, `canonical_block_states.nbt`, `block_properties_table.json` |
| **Inventario persistido** | En memoria (`inventory.rs`, 4 ventanas: 0–36 inventario, 120 armadura, 119 offhand, 124 cursor) | Guardado en el JSON del jugador o `.dat`, restauración al login |
| **Luz** | Estática (`light()` capa cielo llena) | Propagación cielo/bloque (Fase 6 PORT-ROADMAP) |
| **ChunkTicker / random ticks** | Sin sistema | `tick-radius`, `blocks-per-subchunk-per-tick`, crecimiento/física barata (Fase 6) |
| **Multi-world** | Un único `WorldState` | `WorldManager` con generadores por mundo |

Orden recomendado (Fases del `PORT-ROADMAP`): **NivelDB/NBT antes
que lo demás**, porque sin persistencia real el mundo se
regenera al reiniciar y no puedes portar físicas ni crafteo
persistente. El inventario persistido va después (Fase 1:
`ItemStackRequestPacket 0x93`, NBT mínimo, items data-driven).
El resto (luz, ticker, multi-world) es Fase 6–8.

## Ficheros relacionados

- `crates/server-core/src/world_state.rs` — `WorldState` entero
  (save/load v2, colas, cachés, tests `snapshot_roundtrips…`,
  `v1_files_without_version_still_load`).
- `crates/server-core/src/session.rs` — `PlayerSession`,
  `save_player_data` / `load_player_data`.
- `crates/php-plugin-bridge/src/context.rs` — `PlayerContext`,
  `to_save_json` (JSON v2 canónico, 26 campos).
- `crates/bedrock-world/src/{chunk,subchunk,paletted_block_array,network,block_id}.rs` — formato de chunk.
- `crates/bedrock-world/src/generator/manager.rs` — `ChunkManager`.
- `crates/bedrock-world/src/java_chunk.rs` — luz y traducción Java.
- `docs/PORT-ROADMAP.md` — Fases 2 (persistencia JSON), 6 (mundo completo).
