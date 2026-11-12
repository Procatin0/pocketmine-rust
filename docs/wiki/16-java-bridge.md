# 16 — JavaBridge: Java Edition nativo en Rust

## Qué es

`java-bridge` es el listener TCP para Minecraft: Java Edition, en Rust
nativo (Tokio). Sustituye al plugin PHP `plugins/JavaBridge`
(`JavaServer.php` + `JavaClient.php` + `Protocol.php` +
`ModernProfile.php`), que queda retirado: `main.rs` ya no lo carga
como plugin. Lee:

- `crates/java-bridge/README.md` — auto-documentación del crate
  (mapeo PHP → Rust).
- `crates/java-bridge/src/{lib,server,session,hooks,profile,wire,bossbar}.rs`
- `crates/java-protocol/src/{protocol,modern,nbt,scoreboard,bossbar}.rs`
  — primitivas de cable big-endian byte a byte idénticas a
  `plugins/JavaBridge/src/javabridge/Protocol.php` (1.12.2 / 340
  más helpers compartidos de 769–775).
- `crates/server-core/src/{java_hooks,java_sync}.rs` — `CoreHooks`,
  implementación en producción de `ServerHooks`.
- `crates/bedrock-world/src/java_chunk.rs` — `ModernChunk` /
  `ChunkBuilder`: traducción de columna Bedrock → bytes Java.

El bridge no decide lógica de juego: solo convierte y enmarca lo que
`ServerHooks` le entrega. Chat y comandos se despachan en
`server-core` (`ServerHooks::handle_chat` devuelve `ChatOutcome` con
líneas y teletransporte/gamemode del remitente; la sesión solo las
envía/aplica).

## Puerto y protocolos

Escucha en **TCP 25565** (`java-bridge::server::DEFAULT_PORT`,
`main.rs` → `JavaServer::run_arc("0.0.0.0:25565")`). La
**única excepción de alcance vigente**: `run_arc` solo acepta
conexiones y vuelca sockets (incluida el drenaje cada 20 ms);
**no programa ningún intervalo propio**.

Protocolos soportados:

| Protocolo | Versión | Dónde vive |
|---|---|---|
| 340 | 1.12.2 (legacy) | `java-protocol::protocol` + `java-bridge::wire` |
| 769 | 1.21.4 | `java-bridge::profile::TABLE_769` |
| 770 | 1.21.5 | `java-bridge::profile::TABLE_770` |
| 773 | 1.21.9 | `java-bridge::profile::TABLE_773` |
| 774 | 1.21.11 | `java-bridge::profile::TABLE_774` |
| 775 | 26.1 | `java-bridge::profile::TABLE_775` |

> [!IMPORTANT]
> **Prioridad de desarrollo**: La implementación de Java Edition está **priorizada para la versión 26.1 (protocolo 775)** como versión principal de compatibilidad. Posteriormente se irá añadiendo y completando el soporte para las versiones más antiguas.

La tabla moderna (`ModernProfile.php` en PHP) vive en
`crates/java-bridge/src/profile.rs`: `packet_id(protocol, key)` con
alias (`position_look` → `sb_position_look`, `player_info_update` →
`player_info`, `brand` → `config_brand`, …) y flags por versión:

- `spawn_velocity_is_lp_vec3`: desde 773 la velocidad de
  `spawn_entity` va como `lpVec3` en medio; en 769/770 como
  `vec3i16` final.
- `player_entity_type`: 147 (769), 148 (770), 151 (773),
  155 (774/775).
- `heightmaps_as_nbt` y `palette_data_has_length`: solo 769.
- `sections_have_fluid_count`: solo 775 (ver «Agua y `fluidCount`»).

La marca que ve el cliente en F3 (`Server:`) es
`profile::BRAND = "PocketMine"` con canal `minecraft:brand`. No es
el nombre del despliegue: el nombre público del servicio es el
configurado en `motd`, y la marca JAVA es solo informativa.

Fallback: el cliente legacy 340 envía su protocolo en el
handshake; cualquier protocolo desconocido se rechaza limpio, por
número, sin la tabla moderna (el test `packet_id(340, "join_game")
== None` cubre el corte).

## Máquina de estados de la sesión

`crates/java-bridge/src/session.rs` (`JavaSession`, sin socket,
orientada a bytes: `poll_bytes` entra, `take_out` sale) reproduce
`JavaClient.php`:

```
handshake → status | login → configuration → play
```

- *status*: ping/cantidad de jugadores vía `server::status_json`
  (versión, `players.max/online/sample`, `description.text` = motd).
- *login*: `login_success`, nombre offline (UUID offline del
  nombre), `player.load` vía `ServerHooks::load_player_data`.
- *configuration* (solo moderno): `registry_data` (uno por
  registro, vía `ServerHooks::registry_payloads`), `tags` y `brand`
  antes de `finish_configuration`, o el cliente se queda bloqueado.
- *play*: `join_game`, `position`, chunks, entidades, TAB, tiempo,
  keep-alive, chat y comandos.

Keep-alive: el servidor pregunta a los 10 s y corta a los 30 s
sin respuesta (`JavaSession::keep_alive` / `tick`). Los frames
entrantes respetan `MAX_FRAME_LENGTH = 2 MiB` y cadenas de
`32767 * 4` como en PHP.

Todo el juego llega a través del crate `hooks::ServerHooks`
(trato en la sección siguiente): la sesión nunca toca
`SessionManager`, `PlayerRegistry` ni `WorldState` directamente.

## TAB sync (`player_info` / `sync_tab`)

La lista de jugadores que ve un observador Java se mantiene con
`JavaSession::sync_tab` (diff entre `targets` y `tab_listed`) y
`server::tick_tab`:

- **Modernos (769–775)**: `player_info_update` con flags. Añadir
  usa `0x01 | 0x04 | 0x08 | 0x10` (add player + gamemode + listed +
  latency), cambio de gamemode usa solo **`0x04`** (el bitflag
  destacado en tests: `player_info_update_gamemode_bitflag_is_0x04`
  verifica primer byte `== 0x04`), y borrar usa `player_remove`.
- **Legacy 340**: `S2C_PLAYER_INFO (0x2E)` con acciones 0 (add),
  3 (gamemode) y 4 (remove).

Objetivos: `server::tab_targets` devuelve **todos** los jugadores
en línea (incluido el observador), con nombre en minúsculas como
clave. Bedrock y virtual (Java) pueden coexistir con el mismo
nombre en minúsculas; la sesión normaliza por `lower()` y el
diff añade/borra/cambia sin duplicar. El tick de TAB corre **cada
tick** (diff solo en memoria, sin host call).

## Tags, brand, configuración

En `begin_configuration` (moderno), el orden importa:

1. `registry_data` (payload por registro desde
   `ServerHooks::registry_payloads`).
2. **Tags `0x0d`** (`update_tags`, `tags_payload` del hook): debe
   existir antes de `finish_configuration` o el cliente moderno
   abandona la configuration.
3. **Brand `0x01`** (`config_brand` = `custom_payload` clientbound,
   canal `minecraft:brand` + string `"PocketMine"` vía
   `profile::brand_payload()`).
4. `finish_configuration` y `login_acknowledged`.

Los ids `tags = 0x0d` y `brand = 0x01` son los mismos en
769/770/773/774/775 (test `modern_ids_spot_checks`).

## Agua y `fluidCount` (775)

26.1 añadió un contador de fluido por sección (`fluidCount`, u16
big-endian antes de cada sección). El cliente usa `fluidCount > 0`
(`LevelChunkSection.hasFluid()`) para decidir si una sección
tiene fluido: con 0 **nunca examina los bloques** y el jugador
cae a través del agua como si fuera aire.

La implementación está en `bedrock-world/src/java_chunk.rs`
(`ModernChunk::from_chunk` / `from_chunk_with_fluid_report`):

- `is_fluid(state)`: solo `86, 101, 102, 117` (agua y lava en
  estados fluido, ids del registro `minecraft:fluid`).
- `has_fluid(count)`: `count > 0`.
- Columna de referencia, chunk `(0, 0)` seed 0: **fluidCount de la
  sección 7 = 812** (812 bloques de agua en Y 48..=63, ninguna
  fuera de esa sección; 3866 no-aire contados en la sección 7).
  Chunk `(29, 1)`: 424 de agua en la sección 7. Cubierto por
  `bedrock-world/tests/decor_port_test.rs` y
  `java_chunk_test.rs` (`fluid_range_is_water_and_lava_only`
  aserta incluso `has_fluid(812)`).

Columna: 24 secciones de 16×16×16, Y = −64..319. Bedrock indexa
`x<<8 | z<<4 | y` y Java `y<<8 | z<<4 | x`; el encoder recorre en
orden Java leyendo el índice Bedrock reordenado — leerlo en el
mismo orden es el bug clásico de ejes X/Y cruzados.

Luz: `ModernChunk::light()` manda sky light 15 en las 24 (+2
secciones de guarda = 26) y block light vacío (~55 KB
idénticos en cada chunk). No hay propagación real de luz (ver
`17-mundo-persistencia.md`): el cliente recalcula la luz de cielo
desde los heightmaps.

## `ServerHooks` y `CoreHooks`

`crates/java-bridge/src/hooks.rs` (trait `ServerHooks`) es la
frontera entre el listener y el core. Existía para evitar ciclo
de dependencias (`java-bridge` no depende de `server-core`); en
producción lo implementa `server-core/src/java_hooks.rs`
(`CoreHooks`), que delega en:

| Hook (PHP) | Implementación en producción |
|---|---|
| `player.register`, `player.load`, `player.get`, `player.update_position`, `player.take_messages` | `SessionManager` / `PlayerRegistry` (`registry.all()`, inbox `take_messages`) |
| `server.get_players_full` | `ServerHooks::snapshots` (Bedrock + Java), `entity_id_for` central desde `1_000_000`, nunca reutilizado |
| `world.get_chunk/get_block/set_block`, `world.take_block_changes` | `WorldState` |
| `Server::addPlayer` + anuncio de join | `join_api` / `leave_api` |
| chat / `PlayerChatEvent` | `handle_chat` → `ChatOutcome` (líneas + teleporte/gamemode del remitente) |
| `placeBlock` | encola en `drain_block_changes` (`broadcastBlockChange` en PHP iba inline); la sesión solo reenvía el estado real ante fallo para deshacer predicción del cliente |
| chunks legados/modernos | `legacy_chunk` / `modern_chunk` (de `Protocol`/`ModernChunk`); en el tick solo `try_modern_chunk` / `try_legacy_chunk` (sin generar: una columna fría cuesta ~18 ms y rompe el presupuesto) |
| `registry_data`, `tags` | `registry_payloads` / `tags_payload` |
| bossbar / scoreboard Java | `take_bossbar_frames` / `take_scoreboard_frames` (frames ya enmarcados por protocolo —340 string, 769+ NBT—, drenados cada tick como mensajes de plugin) |

`PlayerSnapshot` es Bedrock-canónico (yaw/pitch Bedrock); la
conversión a yaw/pitch Java se hace en la sesión. `is_virtual`
espeja `PlayerContext::is_virtual` y el snapshot Bedrock gana al
virtual ante colisión de nombres.

Entidades para observadores (`server::entity_targets_for` +
`JavaSession::sync_entities`): espejo inverso de
`sync_virtuals_locked` (Bedrock ve los virtuales). Gate por chunk
(`has_sent_chunk`): no se spawnea fuera de columnas cargadas.
Vistas: paquetes `spawn_entity` (0x01 en modernos, `lpVec3` en
773+), `rel_entity_move`, `entity_move_look`, `entity_look`,
`entity_head_rotation`, `entity_destroy`, `entity_metadata`
(93 en 769 … 99 en 775, componente Optional tipo 6 + presencia
`0x01` antes del NBT anónimo) y `entity_teleport` (119/118/123/125).

Colas de bloques Bedrock → Java y Java → Bedrock son
independientes en `WorldState` (cap `512`, descarta lo más
antiguo): drenar Java no roba cambios a Bedrock ni al revés.

## Tick único (núcleo en `main.rs`)

Hay **un solo reloj** de 50 ms (20 TPS) en
`crates/server-core/src/main.rs`, con `MissedTickBehavior::Skip`
(sin catch-up en ráfaga). En cada pulso, en orden:

```
SessionManager::tick() → reloj del mundo, SetTime 1 Hz o salto inmediato,
                          reenvío Fase 2 UpdateBlock, sync de virtuales,
                          PreSpawn forzado 1 Hz — solo encola envelopes
JavaServer::tick_at(N)  → mismo número: keep-alive, TAB cada tick,
                          sync_time cada 20 ticks o salto, entidades
                          cada 4 ticks (5 Hz), drenaje de bloques
                          cada 2 ticks (10 Hz), streaming con
                          presupuesto compartido (12 ms, 64/batch,
                          backpressure 256 KiB, rotación 20 intentos,
                          techo chunk_batch_received). Solo encola
                          en out_buf; el socket vuelca por su path.
tick_tx.send(N)         → sidecar PHP (tick_sync en spawn_blocking,
                          serializado; un plugin atascado solo
                          encola, nunca para el reloj)
autosave cada 6000 ticks → spawn_blocking(save_all)
```

Los antiguos segundos relojes (flusheo de 20 Hz para Bedrock,
intervalo `Skip` de 50 ms para `run_arc` en Java) **desaparecieron**:
los polls viven en `SessionManager::tick` y
`handle_stream` alimenta bytes + drena 20 ms. El trabajo pesado
(generación de chunks fríos, `tick_sync` PHP, guardado) corre en
`spawn_blocking` o en sidecars: el prefetch de chunks
(`main.rs`, cada 500 ms, `CoreHooks::prefetch_blocking`) calienta
la caché de objetos + bytes convertidos por protocolos conectados
para que el tick Java consuma bytes ya generados.

**Fases del tick Java** (`java-bridge/src/server.rs`, presupuesto
`TICK_BUDGET = 10 ms`): `tick()` por sesión, `tick_tab()` cada
tick, `tick_time_sync()` cada 20 ticks o salto,
`tick_entities()` cada 4 ticks, `tick_block_changes()` cada 2
ticks. El payload de bloques se lee una sola vez en
`tick_block_changes` y se envía en broadcast a los clientes con
la columna ya enviada.

## Tests

```powershell
cargo test -p java-bridge
cargo test -p java-protocol
```

- `crates/java-bridge/tests/wire_test.rs` — puerto de
  `wire_test.php` (340 real sobre TCP: status, login, 9 chunks,
  `/list`, bandeja de plugins, chat, `/spawn`, picar/colocar,
  `/tp` +64, nombre duplicado, protocolo erróneo, desconexión).
- `crates/java-bridge/tests/modern_test.rs` — puerto de
  `modern_test.php` en alcance listener: status/login/
  configuration/join_game/chunks en 769–775, `player_info_update`
  `0x04`, `spawn_entity` `lpVec3`, `entity_metadata` Optional,
  `update_time` por versión. El **contenido** de chunks no se
  aserta a propósito (ámbito `ModernChunk`).

Limitaciones conocidas: NBT completo (`Nbt.php`) solo está el
componente de texto anónimo (`system_chat`, nametags, kick);
traducción de estados de bloque/chunks la pone `bedrock-world`;
dispacho de comandos queda en `server-core`.
