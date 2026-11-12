# PocketMine-Rust — Estado del port de axolotl-pm y hoja de ruta

Última actualización: 2026-11-12
Referencia de comportamiento: `target/axolotl-pm` (PocketMine-MP 5 / axolotl) y
`target/axolotl-bedrock-protocol` (protocolo 2225). Ambos clones viven en
`target/` y **no se versionan**: se recrean cuando hacen falta.

Regla del port: cada comando y cada paquete se traduce **uno a uno** desde el
PHP. Mismos nombres, mismos argumentos, mismos límites, mismos mensajes
(claves de traducción de PocketMine) y mismos efectos. Si algo no se puede
replicar todavía, se documenta en la fase correspondiente en vez de inventar un
comportamiento distinto.

---

## 1. Árbol actual del repositorio

```
pocketmine-rust/
├── Cargo.toml                     workspace: 8 crates, resolver 2
├── HANDOFF.md                     estado previo del puente PHP
├── server.properties              config del servidor (allow-cheats, view-distance, ...)
├── pocketmine.yml                 config estilo PM (generadores, etc.)
├── allowlist.json / white-list.txt / ops.txt
├── docs/
│   ├── plugin-api-contract.md     contrato del puente Rust ↔ PHP
│   ├── PORT-ROADMAP.md            ESTE documento
│   └── wiki/                      13 páginas (comandos, eventos, permisos,
│                                  scheduler, inventario/crafteo, scoreboard,
│                                  puente Java, mundo/persistencia, deploy)
├── data/                          datos extraídos de BDS / BedrockData
│   ├── item_ids.txt               834 items clásicos: identifier<TAB>id
│   ├── bds-capture/               38 capturas reales de paquetes
│   ├── canonical_block_states.nbt block_properties_table.json
│   ├── biome_definitions.json, biome_id_map.json, entity_id_map.json
│   ├── required_item_list.json, creative/, recipes/, enums/
│   ├── java/                      tablas del puente Java:
│   │   ├── blocks/                 estados de bloque por protocolo
│   │   ├── items/                  items por protocolo
│   │   ├── connections/            estados de conexión (vallas, muros, ...)
│   │   └── 769.json 770.json 773.json 774.json 775.json
│   └── README.md                  procedencia de cada blob
├── plugins/                       6 plugins PHP de prueba + ScoreHud.phar
├── plugin_data/                   datos generados por los plugins (no versionar)
└── crates/
    ├── bedrock-protocol/          ≈11.400 líneas, protocolo 2225
    │   ├── src/binary.rs          Reader/Writer (varint, zigzag, LE, strings)
    │   ├── src/batch.rs           batch/zlib por frame
    │   ├── src/nbt.rs             NBT de red
    │   ├── src/types.rs           BlockPos, Vector3f, ...
    │   └── src/packets/           52 módulos con 54 paquetes implementados
    ├── bedrock-world/             ≈6.800 líneas
    │   ├── src/chunk.rs, subchunk.rs, paletted_block_array.rs
    │   ├── src/block_id.rs        registro de bloques
    │   ├── src/block_states.rs    tabla canónica, propiedades, `interact`
    │   ├── src/java_chunk.rs      serialización de chunk Java (moderno y legacy)
    │   ├── src/network.rs         serialización de subchunks Bedrock
    │   └── src/generator/         flat, normal, biome, noise/simplex, populators
    ├── server-core/               ≈32.100 líneas, servidor
    │   ├── src/main.rs            arranque, config, consola, tick loop 20 TPS
    │   ├── src/session.rs         PlayerSession + SessionManager (login → juego)
    │   ├── src/world_state.rs     mundo mutable, ediciones, hora, clima, guardado
    │   ├── src/command.rs         49 comandos nativos + framework
    │   ├── src/inventory.rs       PlayerInventory (cuatro ventanas)
    │   ├── src/crafting.rs        CraftingManager, recetas, ItemStackRequest
    │   ├── src/permission.rs      PermissionManager (nodos + attachments)
    │   ├── src/scoreboard.rs      scoreboards por jugador
    │   ├── src/bossbar.rs         boss bars (apiBossBar)
    │   ├── src/plugin_host.rs     PlayerRegistry + ServerPluginHost
    │   ├── src/java_hooks.rs      hooks del mundo hacia el puente Java
    │   ├── src/java_sync.rs       gamemode/teleport/tiempo/tab/entidades Java
    │   ├── src/anvil.rs           lógica del yunque (4 etapas de daño)
    │   ├── src/nbt_json.rs        NBT de `/give [...tags]`
    │   ├── src/access.rs          ban/whitelist/ops
    │   ├── src/query.rs           GS4 Query
    │   ├── src/crash.rs           crash dumps
    │   ├── src/updater.rs         comprobación de actualizaciones
    │   ├── src/metrics.rs         TPS (`/tps`) + sonda de latencia `[lag]`
    │   ├── src/translations.rs    catálogo `eng.ini` embebido
    │   └── src/logging.rs         hilo escritor de logs
    ├── java-bridge/               ≈5.900 líneas, servidor Java Edition
    │   ├── src/server.rs, session.rs, wire.rs
    │   ├── src/hooks.rs           mundo/entidades desde el núcleo Rust
    │   ├── src/display.rs         scoreboard, boss bar, tab
    │   └── src/profile.rs         skins/perfiles
    ├── java-protocol/             ≈3.000 líneas, paquetes Java (775 y anteriores)
    ├── nethernet-rs/              ≈1.700 líneas, transporte NetherNet (WebRTC)
    ├── raklib-rs/                 ≈690 líneas, RakNet (offline ping/pong)
    └── php-plugin-bridge/         ≈5.500 líneas Rust + PHP embebido
        ├── php/*.php              API PM5 en PHP (block, world, player, item,
        │                          command, scheduler, event, plugin, ipc, ...)
        └── src/events/{player,block,server}.rs
```

---

## 2. Comprobaciones actuales (2026-11-12)

```
cargo check --workspace --all-targets        → OK (0 errores)
cargo test -p server-core --lib              → 240 OK
cargo test -p bedrock-protocol --lib         → 109 OK
cargo test -p bedrock-world --lib            →  41 OK
cargo test -p java-bridge                    →  37 + 3 doctests OK
php -l sobre crates/php-plugin-bridge/php/*.php → OK (ver HANDOFF.md)
```

Recuento de `#[test]` por crate (unitarios + integración):
`server-core` ≈300, `bedrock-protocol` ≈120, `bedrock-world` ≈80,
`java-protocol` ≈65, `java-bridge` 37, `php-plugin-bridge` ≈36,
`nethernet-rs` 8, `raklib-rs` (tests de integración aparte).

Nota operativa: los tests de integración de `server-core` necesitan otro
`CARGO_TARGET_DIR` si hay un servidor en marcha, porque Windows bloquea
`target/debug/server-core.exe`.

---

## 3. Cobertura por subsistema

| Subsistema de axolotl-pm | Archivos PHP | Dónde vive en Rust | Estado |
|---|---:|---|---|
| `network/` (sesiones, raknet, compresión) | 109 | `session.rs`, `nethernet-rs`, `raklib-rs` | 🟢 login, chunks, chat, inventario; sin rate limiting |
| `protocol/` (248 paquetes) | 248 | `bedrock-protocol/packets/` | 🟡 54 de 248; faltan `AddPlayer`, `PlayerList`, `AddActor` |
| `world/` (chunks, generadores, luz, formatos) | 278 | `bedrock-world` | 🟡 generación sin luz; persistencia propia (JSON) |
| `block/` (402 bloques) | 402 | `block_states.rs` + `block_id.rs` | 🟢 estados canónicos, propiedades, interacción, conexiones |
| `entity/` + `player/` | 93 | `session.rs`, `java_sync.rs` | 🔴 sin entidades vivas ni visibilidad entre jugadores |
| `inventory/` | 35 | `inventory.rs` | 🟢 cuatro ventanas + transacciones atómicas |
| `item/` | 154 | `inventory.rs`, `nbt_json.rs` | 🟡 items por id + NBT JSON; sin data-driven completo |
| `crafting/` | 25 | `crafting.rs` + `data/recipes/` | 🟢 shaped/shapeless/furnace/potion, rejillas 2x2 y 3x3 |
| `command/` (41 comandos + framework) | 53 | `command.rs` | 🟢 49 registrados, permisos por nodo |
| `permission/` | 14 | `permission.rs` | 🟢 nodos, defaults, attachments |
| `event/` | 152 | `php-plugin-bridge/events/` | 🟡 jugador, bloque, servidor, mundo, inventario |
| `plugin/`, `scheduler/` | 37 | `php-plugin-bridge` | 🟢 directorios y `.phar`, tareas, autoload PSR-4 |
| `scoreboard/` | 3 | `scoreboard.rs` + `java-bridge/display.rs` | 🟢 por jugador, también para clientes Java |
| `form/` | 2 | `php/player.php::sendForm` | 🔴 stub documentado (sin paquete de formularios) |
| `resourcepacks/` | 9 | `resource_packs.rs` | 🟡 handshake básico |
| `lang/` | 3 + .ini | `translations.rs` | 🟡 `eng.ini` embebido; locale configurable pendiente |
| `utils/`, `console/`, `timings/`, `crash/`, `wizard/`, `data/` | 240 | disperso | 🟡 consola ✅, crash ✅, update checker ✅, `/timings` básico |

---

## 4. Estado de los comandos nativos — 49/49 portados

No queda ningún comando de `command/defaults/*.php` sin portar. Los ocho que
faltaban (`status`, `effect`, `enchant`, `xp`, `particle`, `timings`, `gc`,
`dumpmemory`) ya están registrados con sus nodos de permiso y sus mensajes.

| Grupo | Comandos |
|---|---|
| Núcleo | `/help` `?`, `/version` `ver`, `/plugins` `pl`, `/list` `players`, `/stop`, `/tps`, `/timings`, `/status`, `/gc`, `/dumpmemory` |
| Jugador | `/gamemode` `gm` + `/gmc` `/gms` `/gma` `/gmsp`, `/defaultgamemode`, `/tp` `teleport`, `/spawn`, `/spawnpoint`, `/setworldspawn`, `/kill` `suicide`, `/effect`, `/enchant`, `/xp`, `/give`, `/clear` |
| Mundo | `/time`, `/weather`, `/difficulty`, `/seed`, `/particle` |
| Chat | `/say`, `/me`, `/msg` `tell`/`w`/`whisper`, `/title` |
| Administración | `/op`, `/deop`, `/ban`, `/ban-ip`, `/banlist`, `/pardon` `unban`, `/pardon-ip` `unban-ip`, `/whitelist`, `/kick`, `/transferserver`, `/save-all`, `/save-on`, `/save-off`, `/scoreboard` `sb` |

Notas de paridad de los últimos portados:

- `/status` y `/timings` leen `TickMetrics`; `/tps` mantiene las ventanas de
  1/5/15 min estilo PaperMC.
- `/gc` emite `CommandAction::GarbageCollect` y `/dumpmemory` escribe el volcado (con ruta opcional), como `GarbageCollectCommand`/`DumpMemoryCommand`.
- `/xp` emite `CommandAction::AddXp` (niveles con sufijo `L`, como
  `XpCommand::getInteger`), `/effect` y `/enchant` validan y aplican al estado
  de la sesión.
- `/particle` resuelve nombres de `ParticleCommand::getParticle`; la emisión
  depende del `LevelEvent` de partículas (`KNOWN_PARTICLES` documenta el mapeo).
- `/scoreboard` (`sb`) usa `scoreboard.rs`: objetivos, líneas y slots, con
  difusión a clientes Bedrock (`SetDisplayObjective`) y Java.

---

## 5. Fases

Cada fase termina con código compilando, tests en verde y una nota de paridad.

### Fase 0 — Cimientos (✅ completada)

Transporte NetherNet y RakNet, login con identidad real, chunks y generación,
sesión de jugador, chat, comandos base, salud, inventario de 36 slots.

**Criterio de salida:** un cliente Bedrock entra, ve terreno, recibe chat,
ejecuta comandos y muere/resucita con paquetes reales. Cumplido.

### Fase 1 — Inventario y objetos (✅ completada)

`InventoryTransaction` (0x1E) con los cinco tipos y límite de 50 acciones,
`InventorySlotPacket`, `ItemStackRequest` (0x93) con respuestas, `PlayerInventory`
con cuatro ventanas (0/36, 120/4, 119/1, 124/1), aplicación atómica con resync
de las cuatro ventanas, `/give` con NBT JSON (`nbt_json.rs`), `/clear` sobre
todas las ventanas.

Pendiente dentro de la fase: drops como entidades de objeto (depende de la
fase 4) y persistencia del inventario en `players/*.json`.

### Fase 2 — Comandos (✅ completada)

49/49 registrados, con alias, nodos `pocketmine.command.*.self/.other`,
mensajes de `eng.ini` y tests de parseo. Ver sección 4.

### Fase 3 — Permisos, traducciones y consola (🟡 3/4)

✅ `permission.rs`: nodos con default (`true`/`false`/`op`/`notop`), hijos y
`PermissionAttachment` por jugador. ✅ consola de comandos por stdin, con las
mismas rutas de permisos que el chat. ✅ `translations.rs` con el catálogo
`eng.ini` embebido.
🟡 Falta el selector de locale (`pocketmine.yml` → `spa.ini`).

### Fase 4 — Entidades, combate y experiencia (🟡 en curso)

Ya modelados: `RemoveActorPacket` (0x0E), `MoveActorAbsolutePacket` (0x12),
`ActorEventPacket` (animación de muerte), salud, muerte y respawn por sesión,
`/xp`, `/effect` y `/enchant` sobre el estado del jugador.

Falta:

1. `AddPlayerPacket` (0x0C), `PlayerListPacket` (0x3F) con skins y
   `AddActorPacket` → **sin esto dos clientes no se ven entre sí**.
2. Daño real con causas (`EntityDamageEvent`), armadura, invulnerabilidad y
   regeneración.
3. Muerte completa: drops, pantalla de muerte, respawn en el spawn real.
4. Experiencia visible (`UpdateAbilities` + `xp` en `AddPlayer`).
5. Mobs: no hay IA ni spawner.

**Criterio de salida:** dos clientes se ven, se hacen daño y mueren con drops y
respawn; `/kill` y `/xp` coherentes con el estado.

### Fase 5 — Crafteo, contenedores y uso de objetos (🟡 2/3)

✅ `crafting.rs` con `CraftingManager`, ingredientes `Exact`/`MetaWildcard`/
`TagWildcard`, rejillas 2x2 y 3x3 y el motor de `ItemStackRequest`
(Take/Place/Swap/...). ✅ `/give` con `block_runtime_id` correcto al colocar.
🟡 Falta: ventanas de contenedor (cofre, horno: `ContainerOpen/Close`,
`ContainerSetData`, `InventoryContent` por ventana) y durabilidad/comida.

### Fase 6 — Mundo completo (🔴 la más grande que queda)

1. **Luz**: motor de luz de cielo y de bloque. Hoy los chunks Java declaran
   sky light 15 y block light vacío (`java_chunk.rs::light()`) y los subchunks
   Bedrock no llevan capa de luz → cuevas iluminadas y antorchas sin efecto.
2. Física barata: arena/grava, agua y lava, crecimiento.
3. Persistencia real: hoy `world.json` v2 (hora, clima, dificultad, spawn,
   semilla, ediciones) y `players/*.json`; **no** se guardan chunks generados
   ni existe `level.dat`, y no se leen mundos PM existentes (decisión
   consciente, ver `world_state.rs`).
4. Biomas reales y populators que los respeten.
5. Multimundo: `SessionManager` tiene **un** `WorldState`; el `Teleport` con
   otro nombre crea un mundo nuevo con el mismo generador.

### Fase 7 — API de plugins a la par de PM (🟡)

Cubierto: eventos de jugador/bloque/servidor/mundo, comandos con permisos,
scheduler, `World`/`Player`/`Item`/`Block`, scoreboard, boss bar, tareas,
carga de directorios y `.phar` con autoload PSR-4.

Falta: inventario desde PHP (`PlayerInventory` completo), entidades, formularios
(`ModalForm`/`SimpleForm`/`CustomForm` + paquete de respuesta; hoy
`sendForm()` es un stub), `AsyncPool` real.

### Fase 8 — Red, escalado y compatibilidad (🟡)

✅ RakNet + NetherNet en el mismo puerto, cinco versiones Java con tablas
propias, batch con zlib, `[lag]` para diagnosticar.
🟡 Falta: rate limiting y `PacketViolationWarning` efectivo, multiworld,
`transfer-server` completo, benchmark de 20 bots.

### Fase 9 — Operación y cierre (🟡)

✅ crash dumps (`crash.rs`), GS4 Query (`query.rs`), update checker
(`updater.rs`), logs con hilo escritor propio, `/save-all` + autoguardado.
🟡 Falta: CI (`fmt`, `clippy`, `cargo test --workspace`, `php -l`), rotación de
logs y documentación de paridad por paquete.

---

## 6. Arquitectura y limitantes

Lo que sigue no es deuda de port, es el diseño actual y dónde se rompe.
Ordenado por impacto.

### 6.1 Modelo de ejecución

- **Un proceso Rust** (servidor) + **un proceso PHP** (plugins) unidos por IPC
  de canal (`php-plugin-bridge`).
- **Un bucle de tick de 50 ms** en `main.rs`: vaciado temprano de salida →
  `SessionManager::tick()` → vaciado + `send_to_peer` → tick del conector Java
  (en el blocking pool, pero *awaited*) → tick PHP → autoguardado.
- **Un lock global de sesiones** (`RwLock<HashMap<u64, PlayerSession>>`).
- **Un `WorldState`** por servidor, con cachés de payload Bedrock y de objetos
  Java (512 y 1024 entradas) e invalidación por `chunk_versions`.
- Un hilo escritor de logs con cola acotada (`logging.rs`), separado del juego.

### 6.2 Limitantes

| # | Limitante | Dónde | Consecuencia | Arreglo |
|---|---|---|---|---|
| 1 | Eventos PHP **síncronos** en el hilo del paquete. `BlockBreakEvent` se dispara **siempre** por IPC (a diferencia de `BlockPlaceEvent`/`PlayerInteractEvent`, que saltan si `has_listeners` es falso) | `session.rs:2727`, `plugin_host.rs` | Un plugin pesado mete su latencia en cada rotura | Gatear por `has_listeners` y despachar asíncrono lo no cancelable |
| 2 | Lock global de sesiones | `session.rs` | Un `write()` frena a todos los peers | Slots por peer o actor por sesión |
| 3 | Generación de terreno en el tick (rayon, `CHUNK_SENDING_PER_TICK = 4` por sesión, ~18 ms/columna) | `world_state.rs`, `drive_chunk_requests` | El tick se llena al cruzar chunks | Pool de generación con cola por distancia |
| 4 | Cada edición invalida la columna entera y se re-serializa en **dos** formatos | `WorldState::set_block` | ~18 ms por columna editada bajo demanda | Parchear subchunk en memoria, un solo árbol + dos serializadores |
| 5 | Sin motor de luz | `java_chunk.rs`, subchunks Bedrock | Cuevas iluminadas, antorchas sin efecto | Fase 6 |
| 6 | Sin entidades vivas ni visibilidad entre jugadores | `packets/` incompleto | Dos jugadores no se ven | Fase 4 |
| 7 | Ancho de banda por peer: `DataChannel::send` con control de flujo SCTP (~62 KB por chunk) | `nethernet-rs/signaling.rs` | Un canal saturado para el tick | Prioridad para paquetes pequeños, límite por tick configurable |
| 8 | Sin anticheat: la posición del cliente se acepta | `handle_auth_input` | Movimiento libre salvo `force_move_sync` | Validación de velocidad |
| 9 | Persistencia propia (JSON, sin chunks ni `level.dat`) | `world_state.rs` | El mundo se regenera y solo se aplican deltas | Fase 6 |
| 10 | Un solo mundo y un solo protocolo Bedrock (2225) | `SessionManager` | Multiworld y versiones nuevas requieren tablas | Fase 8 |
| 11 | Cobertura end-to-end escasa | `tests/` | El lag se diagnosticó con logs manuales, no con un test | Cliente/robot automatizado |

### 6.3 Instrumentación de latencia

Cada segundo, `metrics::LagProbe` escribe en el log:

```
[lag] tick=50.1/52.0ms(n=20) body=6.2/44.0ms(n=20) session=1.4/12.0ms(n=20)
      flush=0.5/9.0ms(n=20) send=0.2/6.0ms(n=48) java=3.9/31.0ms(n=20)
      dispatch=1.9/60.0ms(n=20) packets=1.6/58.0ms(n=20) actions=0.2/3.0ms(n=20)
      chunk=0.3/20.0ms(n=4) drain=0.4/2.0ms(n=20) poll=0.1/2.0ms(n=20)
```

`tick` es el periodo real, `body` la iteración completa, `dispatch` un lote
entrante (con `packets` = parseo + manejadores y `actions` = aplicar lo
encolado), `lock` la espera por el lock de sesiones, `ipc` los eventos de
plugin (roundtrip al proceso PHP), `flush`/`send` el transporte y
`chunk`/`drain`/`poll` las tres rutas calientes del mundo.

Un lote de más de 20 ms deja además un `[lag-slow]` con `npk`, el paquete
más caro, y otro con la acción más cara si `apply_actions` pasa de 20 ms;
los envíos SCTP de más de 20 ms dejan `[:WebRTC] slow data channel send`.
Referencia completa en `docs/wiki/18-deploy.md`.

**Siguiente paso recomendado:** la limitante 1 (gatear `BlockBreakEvent` por
`has_listeners`), porque es la única que hoy se paga en cada rotura y es un
cambio localizado; después la 3, que es la que más crece con jugadores.

---

## 7. Riesgos y decisiones abiertas

| Tema | Riesgo | Decisión actual |
|---|---|---|
| Eventos PHP | Un plugin lento degrada el tick | IPC síncrono, con `has_listeners` en la mayoría de eventos; pendiente en `BlockBreakEvent` |
| Luz | Chunks oscuros o planos | Fase 6; hoy sky 15 / block 0 |
| Entidades | Sin visibilidad ni combate real | Fase 4 |
| Persistencia | El mundo se regenera al reiniciar; solo se guardan deltas | JSON deliberado hasta tener NBT/Anvil |
| Mundos PM existentes | No se pueden abrir | Documentado: no se implementa LevelDB |
| Formularios | Un plugin con UI no funciona | Stub en `sendForm()`; falta el paquete |
| Compatibilidad Java | Coexistencia multiversión | 775 principal; 769/770/773/774 con tablas propias; 340 solo en ids legacy |
| Escalado | Lock global + generación en el tick | Fase 8 |
| Tests E2E | Regresiones de latencia invisibles | `[lag]` + `/tps`; robot pendiente |

---

## 8. Convenciones de trabajo

- Un cambio = una referencia a `target/axolotl-pm`; si el PHP y el Rust
  discrepan, gana el PHP o se documenta la desviación.
- Rutas calientes en `debug!` (login, acciones de bloque, sonidos, poll de
  bloques, `set_block`); `info!` solo para eventos de operador.
- Nada de lógica de juego en `main.rs`: el bucle solo orquesta.
- Cada subsistema con su doc de wiki; `PORT-ROADMAP.md` es el índice de estado,
  no la referencia de comportamiento.

---

## 9. Compatibilidad Minecraft: Java Edition

El listener Java (`crates/java-protocol`, `crates/java-bridge`) está priorizado
para **1.21.11 / protocolo 775**, con tablas de bloques, items y conexiones
propias para 774, 773, 770 y 769. El 340 (1.12.2) solo tiene de momento el
mapeo de ids legacy de algunos paquetes (p. ej. `boss_bar`), no tablas de mundo.

Cubierto hoy: handshake y login (perfiles, skins), chunks modernos y legacy con
conversión de bloques y estados de conexión, movimiento y rotación
bidireccionales, romper y colocar bloques (colocación en creativo), chat,
comandos con permisos, gamemode, teleport, sincronización de hora, tab list,
scoreboard, boss bar y entidades básicas (`remove_actor`, movimiento).

Pendiente: inventario de supervivencia y ventanas de contenedor, formularios,
y la versión mínima de compatibilidad hacia atrás (el resto de protocolos se
añade sobre las tablas ya presentes).
