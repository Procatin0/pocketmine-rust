# HANDOFF — Continuación del proyecto PocketMine-Rust (API de plugins PHP)

Fecha: 2026-11-05
Estado: **compila (`cargo check --workspace` OK)**; `cargo test --workspace` fue interrumpido por falta de saldo, **hay que ejecutarlo y corregir**.
Rama: `master` (sin commitear; todo está en el working tree).

Este documento es el punto de entrada para el siguiente agente. Léelo entero antes de tocar código.
El contrato detallado de la API y del protocolo IPC está en `docs/plugin-api-contract.md` (también en español).
La matriz de compatibilidad de plugins está en `docs/plugin-compatibility.md`.

---

## 1. Qué se ha implementado ya

### 1.1 Mundo (`crates/bedrock-world`)
- `WorldGenerator` ahora tiene:
  - `block_at(x, y, z) -> u32` (coordenadas absolutas, solo terreno).
  - `generate_chunk_with_edits(cx, cz, &[(x, y, z, id)]) -> (u32, Vec<u8>)`.
  - `highest_block_at(x, z) -> i32` (override barato en Flat y Normal).
- `FlatGenerator` y `NormalGenerator` implementan las tres.
- Tests nuevos en `crates/bedrock-world/tests/block_access_test.rs` (7 tests, pasaban antes del corte).
- El test de paridad que dependía de `/tmp/export_chunk.php` se omite si falta el fixture.

### 1.2 API PHP base (`crates/php-plugin-bridge/php/`)
Archivos escritos y validados con `php -l`:

| Archivo | Contenido |
|---|---|
| `math.php` | `Vector3`, `Vector2`, `AxisAlignedBB`, `pocketmine\world\Position`, `Location` + alias `pocketmine\math\Position` |
| `text.php` | `TextFormat` (todas las constantes PM5), `Utils`, `ServerException` |
| `permission.php` | `Permission`, `PermissionManager` (reglas true/false/op/notop), `DefaultPermissions` |
| `config.php` | `Config` YAML/JSON/properties, `save()` atómico, fallback YAML propio |
| `scheduler.php` | `Task`, `ClosureTask`, `TaskHandler`, `Scheduler` con firmas PM5 **y** forma abreviada `(Task, delay/period)` |
| `command.php` | `CommandSender`, `ConsoleCommandSender`, `CommandExecutor`, `Command`, `SimpleCommand`, `CommandMap`, `setPermissionMessage` |
| `item.php` | `Item`, `ItemFactory` |
| `world.php` | `BlockFactory`, `Block`, `World`, `WorldManager`; host calls `world.get_block/set_block/get_highest_block/get_spawn` |
| `player.php` | `Player` (CommandSender), `OfflinePlayer`, `PlayerInventory`; acciones send_message/tip/popup/title/teleport/kick/gamemode |
| `core.php` | `Server` singleton, `Logger`, registro de jugadores, ops, config, comandos, acciones |

### 1.3 Runtime PHP (`crates/php-plugin-bridge/php/`)
| Archivo | Contenido |
|---|---|
| `event.php` | `Event`, `Listener`, `Cancellable`, `EventPriority`, `RegisteredListener`, `EventData` (hidratación), `EventFactory`, todos los eventos del contrato con `fromArray`/`toArray` |
| `plugin.php` | `Plugin` interface, `PluginDescription`, `PluginBase` (init, resources, Config, registerEvents, comandos), `PluginManager` (reflexión de listeners, prioridades, `@ignoreCancelled`, dispatch, unregister), `PluginLoggerFallback`, `FallbackConfig` |
| `autoload.php` | Autoloader PSR-4 por plugin, soporte `phar://`, `vendor/autoload.php` |
| `ipc.php` | `Runtime` (bucle JSON-lines completo: ping, discover_plugins, load/enable/disable, dispatch_event, execute_command, tick, shutdown), `Bridge::hostCall` síncrono |
| `bootstrap.php` | `require` de todos los archivos y `Runtime::main()` |

Prueba manual ya ejecutada con éxito (los 4 plugins cargan y se habilitan, eventos mutan, comandos responden):

```powershell
(php target\harness3.php) | php crates\php-plugin-bridge\php\bootstrap.php 2>&1
```

Resultados verificados:
- `ExamplePlugin`: `/hello` → `Hello, Steve!`; `PlayerJoinEvent` → mensaje de bienvenida.
- `ChatGuard`: `PlayerChatEvent` muta `hello badword spam` → `hello *** ***`.
- `TestKit`: `/tk`, `/pos`, `/block`, `/tps` responden; listeners join/quit/move.
- `EconomyLite`: `/money`, `/pay`, `/eco` funcionan con dos jugadores; persiste `balances.json`.
- `tick`: scheduler ejecuta heartbeats (`pending_tasks: 3`).

### 1.4 Puente Rust (`crates/php-plugin-bridge/src/`)
- `runtime/bootstrap.rs`: embebe los PHP con `include_str!` y los escribe en `%TEMP%/pocketmine-rust-php` (fallback por PID si Windows bloquea).
- `runtime/php.rs`: proceso PHP persistente + hilo dedicado + canal mpsc; host calls reenviadas al `PluginHost`; `PhpWorker::{start,start_sync,request_sync,send_command,shutdown_sync,set_host}`; `PhpPlugin`; `PluginSource` (dir/phar).
- `host.rs`: trait `PluginHost { fn call(&self, method, params) -> Result<Value, String> }`.
- `manager.rs`: descubrimiento (incl. `.phar` vía PHP), orden topológico, enable/disable en orden, `dispatch_event_sync`, `execute_command_sync`, `tick_sync`, `has_listeners`, `refresh_listener_events`.
- `event.rs` + `events/*.rs`: `apply_json` para todos los eventos, `is_op` en `PlayerContext`, `sender_to_json` aplana el jugador.
- `context.rs`: `PluginAction` completo (mensajes, tips, popups, titles, kick, teleport, set_block, dispatch_command, gamemode).

### 1.5 Servidor (`crates/server-core/src/`)
- `world_state.rs` (nuevo): `WorldState` con ediciones persistentes, `block_at`, `set_block`, `highest_block_at`, `chunk_payload`, `edited_chunk_count`.
- `plugin_host.rs` (nuevo): `PlayerRegistry` y `ServerPluginHost` (implementa `PluginHost`).
- `identity.rs` (nuevo): extracción de `displayName`/XUID desde el JWT de login (base64url + JSON), `unique_id()` FNV-1a. Tiene 3 tests.
- `session.rs`:
  - Sesión con `world`, `plugins`, `registry`, `ops`, `pending_actions`, `pending_outgoing`, `pending_broadcasts`, `joined`, `move_events`.
  - `attach_runtime`, `player_context`, `fire_event`.
  - Login usa identidad real (`username`, `xuid`, `entity_unique_id`).
  - `PlayerJoinEvent` al entrar en mundo; `PlayerQuitEvent` en `remove_session` y `handle_peer_disconnect`.
  - `PlayerMoveEvent` (solo si hay listeners, throttle 1 de cada 2 inputs, cancelable).
  - Chat: `PlayerChatEvent` con recipients, cancelable, muta mensaje/formato; broadcast a todos.
  - Comandos: `PlayerCommandPreprocessEvent` → comandos core → comandos de plugins.
  - `apply_actions` implementa todas las `PluginAction` (incluye `MovePlayerPacket 0x12` para teleport y `SetTitlePacket 0x59`).
  - `refresh_chunk_at` reenvía chunks editados.
  - `dispatch_webrtc_packet` saca la sesión del mapa antes de llamar a PHP (evita deadlocks) y reinserta después.
- `main.rs`: crea `PluginManager` antes del transporte, instala host, tick loop 20 TPS (`spawn_blocking`), consola con `ServerCommandEvent`, carga `ops.txt`, apaga plugins y worker en Ctrl+C.
- `lib.rs`: exporta `WorldState`, `PlayerRegistry`, `ServerPluginHost`, `LoginIdentity`, `parse_login_identity`.
- `Cargo.toml`: añadido `serde_json`.

### 1.6 Transporte (`crates/nethernet-rs/src/signaling.rs`)
- `NetherNetPacketHandler` tiene `on_disconnect(peer)` con default vacío.
- Se llama al cerrar el DataChannel; `main.rs` lo usa para disparar `PlayerQuitEvent`.

### 1.7 Plugins de prueba (`plugins/`)
- `ExamplePlugin` (actualizado): config, permiso, comando `/hello`, evento join.
- `ChatGuard`: filtra palabras (`@priority HIGH`, `@ignoreCancelled true`), `/chatguard`, tarea repetitiva.
- `TestKit`: `/tk`, `/pos`, `/block`, `/tps`; join/quit/move; softdepend de ExamplePlugin.
- `EconomyLite`: `/money`, `/pay`, `/eco`; JSON persistente; scheduler; permisos.
- `plugin_data/` se genera al probar (no versionar; ver más abajo).

---

## 2. Lo primero que debe hacer el siguiente agente

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo test --workspace
```

El comando quedó interrumpido por falta de saldo. Hay que:
1. Ejecutarlo y corregir cualquier test roto (especialmente `server-core/tests/session_lifecycle_test.rs`, que usa `PlayerSession::new` y ahora arrastra el estado nuevo).
2. Ejecutar `php -l` sobre todos los PHP:
   ```powershell
   Get-ChildItem crates\php-plugin-bridge\php\*.php | ForEach-Object { php -l $_.FullName }
   ```
3. Arrancar el servidor y verificar en logs que carga los 4 plugins:
   ```powershell
   cargo run -p server-core
   ```
   Debe aparecer `PluginManager loaded 4 plugin(s)` y los `[Server/INFO] ... habilitado` de cada plugin.

---

## 3. Problemas conocidos / deuda técnica (prioridad alta → baja)

### P1 — Eventos de bloque ✅ RESUELTO (2026-11-05)
`BlockBreakEvent` y `PlayerInteractEvent` ya se disparan desde `PlayerAuthInputPacket` (0x90).

**Correcciones al análisis previo**: la especificación original de esta sección era incorrecta.
Verificado contra el fuente de pmmp/BedrockProtocol (`PlayerAuthInputPacket.php`,
`PlayerBlockActionWithBlockInfo.php`, `CommonTypes::getBlockPosition`, `BitSet.php`,
`PlayerAction.php`, `PlayerAuthInputFlags.php`):
- Las flags NO son `var_u64`: son un **bitset varlen de 7 bits por byte** (máx. 10 bytes
  para 65 bits). `PERFORM_BLOCK_ACTIONS` = **bit 35** (no "flag de BlockAction").
- Tipos de acción reales: `START_BREAK=0`, `ABORT_BREAK=1`, `STOP_BREAK=2` (sin
  payload), `CRACK_BLOCK=18`, `PREDICT_DESTROY_BLOCK=26`, `CONTINUE_DESTROY_BLOCK=27`.
  No existen `CrackBreak=2 / PredictDestroy=3 / Destroy=4 / ContinueDestroy=5`.
- `BlockPosition` = **3× varint con signo** (x, y, z) — la y NO es `var_u32` — más
  `face` varint con signo.

**Implementación**:
- `crates/bedrock-protocol/src/packets/player_auth_input.rs`: `parse_block_actions()`
  best-effort con doble intento de layout (moderno con `interactionMode`+`interactRotation`,
  y legacy sin ellos), validación estricta (tipos conocidos, face 0-5, coords sanas,
  regla de cola) y vía de escape segura (vacío si hay payloads de item o algo no cuadra).
  8 tests unitarios.
- `crates/server-core/src/session.rs` (`handle_block_actions`, al final de
  `handle_player_auth_input`, solo si hay listeners): `START_BREAK` → `PlayerInteractEvent`
  (left-click); `PREDICT_DESTROY_BLOCK` → `BlockBreakEvent` con el id actual; si no se
  cancela se aplica la rotura (aire vía `SetBlock` a nivel de manager + refresh a todos);
  si se cancela se reenvía el chunk a la sesión para deshacer la predicción del cliente.
- `plugins/TestKit`: listeners MONITOR `onBlockBreak`/`onInteract` que solo loguean
  (verificación en vivo + ejemplo para plugins reales).
- Test de regresión: cola `input_data` inparseable nunca rompe el movimiento.
- Verificado: `cargo test --workspace` verde, harness PHP dispara ambos eventos a TestKit.

**Pendiente (más complejo, sin cambios)**: `BlockPlaceEvent` requiere decodificar
`InventoryTransactionPacket`, que el servidor aún no maneja. `ABORT_BREAK`/`CRACK_BLOCK`/
`CONTINUE_DESTROY_BLOCK` se ignoran a propósito (sin evento en el contrato / progreso de
crack demasiado ruidoso).

### P1 — `cargo test` no verificado
No se llegó a ejecutar la suite completa tras la integración. Es posible que falle algún test por:
- `PlayerSession` con campos nuevos (los constructores públicos siguen funcionando).
- `OutgoingPacket` sin cambios.
- `session_lifecycle_test.rs` compara número de respuestas; los eventos no añaden paquetes salvo que haya plugins (en tests no hay `PluginManager`, así que debería seguir igual).

### P2 — `tick_sync` bloquea el hilo de Tokio si el worker PHP se cuelga
`request_sync` tiene timeout de 30 s y `spawn_blocking` evita bloquear el runtime, pero una petición colgada puede acumular ticks. Considerar timeout de 1–2 s para `tick` y bajar el intervalo si hay backlog.

### P2 — Teleport entre mundos crea un `WorldState` nuevo
En `apply_actions` (`Teleport` con `world` distinto) se hace `Arc::new(WorldState::new(...))`, perdiendo ediciones previas de ese mundo. Lo correcto es que `SessionManager` tenga un `HashMap<String, Arc<WorldState>>` y `WorldState` comparta ediciones. Ahora mismo solo hay un mundo (`world`), así que no afecta al caso normal.

### P2 — `Block::getWorld()` y `World` de PHP usan host calls con `Bridge::isActive()`
Fuera de una petición devuelven valores por defecto (aire/0). Es intencional para no bloquear el arranque. Si un plugin llama a `getBlock` en `onEnable`, obtendrá aire. Si se quiere soportar, el worker Rust tendría que atender host calls también en `load/enable` (ya lo hace: `Bridge::setActive(true)` se activa en `handleLine`; el problema es solo el `WorldManager` construido antes de la primera petición). Revisar `WorldManager::getOrCreate` si se detecta.

### P2 — `plugin_data/` está en el working tree sin `.gitignore`
Añadir a `.gitignore`:
```
/plugin_data/
target/harness*.php
```
Los harness de prueba (`target/harness.php`, `harness2.php`, `harness3.php`) están en `target/`, que ya está ignorado.

### P3 — `PhpWorker::request_sync` y eventos concurrentes
Las peticiones al worker están serializadas por un `std::sync::mpsc` y un hilo; varias sesiones que disparen eventos a la vez se encolan. Es correcto, pero el rendimiento con muchos jugadores puede sufrir. Alternativa futura: pool de workers PHP (uno por N plugins) o eventos por lotes.

### P3 — Inventario/entidades/efectos son stubs en memoria
Documentado en `docs/plugin-compatibility.md`. No hay sincronización con el cliente.

### P3 — `Player::getGamemode()` devuelve int; algunos plugins esperan `GameMode` object
PM5 usa `GameMode::SURVIVAL` etc. Se puede añadir una clase `GameMode` con constantes y hacer que `getGamemode()` devuelva el objeto (rompería el contrato actual de `PlayerContext`). Evaluar compatibilidad real antes de cambiar.

### P3 — `World::getBlock` devuelve bloque con id legacy mapeado
`BlockFactory::legacyIdOf` mapea los IDs de red FNV-1a conocidos a IDs legacy. Los bloques no mapeados devuelven el hash como entero positivo. Ampliar el mapa si un plugin concreto lo necesita.

---

## 4. Comandos y verificaciones útiles

```powershell
# Compilar todo
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo check --workspace

# Tests
cargo test --workspace

# Lint PHP
Get-ChildItem crates\php-plugin-bridge\php\*.php | ForEach-Object { php -l $_.FullName }

# Runtime PHP aislado (los harness están en target/, ignorado por git)
(php target\harness3.php) | php crates\php-plugin-bridge\php\bootstrap.php 2>&1

# Servidor completo
cargo run -p server-core
```

Para probar `/hello` desde consola, el servidor lee stdin (tarea de consola en `main.rs`): escribe `hello` y Enter.

---

## 5. Mapa de archivos clave

```
crates/php-plugin-bridge/
  php/                      API + runtime embebido (15 archivos)
  src/runtime/php.rs        worker PHP, IPC, host calls
  src/runtime/bootstrap.rs  include_str! + escritura a temp
  src/manager.rs            descubrimiento, orden, dispatch, comandos, tick
  src/host.rs               trait PluginHost
  src/context.rs            PluginAction, PlayerContext
  src/events/*.rs           eventos Rust

crates/server-core/
  src/world_state.rs        terreno + ediciones + chunks
  src/plugin_host.rs        PlayerRegistry + ServerPluginHost
  src/identity.rs           JWT login → displayName/XUID
  src/session.rs            sesiones, eventos, acciones, chat, comandos
  src/main.rs               wiring, tick loop, consola, RakNet/NetherNet

plugins/                    ExamplePlugin, ChatGuard, TestKit, EconomyLite
docs/plugin-api-contract.md contrato IPC/API (fuente de verdad)
docs/plugin-compatibility.md matriz de compatibilidad
```

---

## 6. Decisiones de diseño que NO se deben revertir sin motivo

1. **El worker PHP es un proceso hijo persistente** (`php bootstrap.php`), no `php -r` por petición. Permite estado (plugins, scheduler, config) y arranque rápido.
2. **`PhpWorker` usa un hilo dedicado + mpsc**. No usar directamente `tokio::process` con `Mutex<Child>`: las host calls síncronas necesitan leer/escribir stdin/stdout en el mismo hilo.
3. **`dispatch_webrtc_packet` saca la sesión del mapa** antes de llamar a PHP. Si se vuelve a meter la llamada dentro del lock, hay deadlock con `PluginHost` (que lee `PlayerRegistry`/`WorldState`, no `sessions`, pero el riesgo es real si se amplía).
4. **`PlayerContext` lleva `is_op`**; PHP lo usa para `Player::isOp()`. Los ops se leen de `ops.txt` al arrancar.
5. **Las ediciones de bloque viven en `WorldState`**, no en el generador. Así `block_at`/`chunk_payload` son consistentes y el generador sigue siendo puro/cacheable.
6. **Los eventos PHP se hidratan de vuelta** con `apply_json`; no se reemplaza el evento entero.

---

## 7. Siguiente milestone sugerido

1. Cerrar P1 (eventos de bloque) y verificar `cargo test`.
2. Añadir `GameMode` y `Enchantment`/`Effect` si se quiere subir compatibilidad con plugins reales.
3. Soporte de `PlayerJoinEvent` cancelable no aplica (no lo es), pero `PlayerQuitEvent` sí permite cambiar el mensaje; ya funciona.
4. Prueba con un plugin real de Poggit (p. ej. `EconomyAPI` o `SimpleEconomy`) para encontrar APIs faltantes. El contrato actual cubre lo básico; un plugin real revelará getters/setters adicionales.
5. Empaquetar `.phar` de prueba (el descubrimiento ya lo soporta) y validar `phar://` en Windows.
