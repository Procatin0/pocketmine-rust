# PocketMine-Rust — Contrato de la API PHP de plugins

Estado: milestone 1 (compatibilidad de plugins en proceso)
Rama de trabajo: `master`

Este documento es la fuente de verdad para el puente Rust ↔ PHP. Cualquier
subagente que implemente una parte debe respetar los nombres, formatos y
límites de archivos descritos aquí.

## Objetivo

Permitir que plugins de PocketMine-MP 5 (carpeta `plugins/Foo/` o
`plugins/Foo.phar`) se carguen sin cambios en el servidor Rust:

- `plugin.yml` con `name`, `version`, `main`, `api`, `depend`, `softdepend`,
  `load-before`, `commands` y `permissions`.
- `src/` con autoload PSR-4 relativo al namespace de la clase `main`.
- `PluginBase` con `onLoad`, `onEnable`, `onDisable`, `getDataFolder`,
  `saveResource`, `getConfig`, `saveConfig`, `registerEvents`, `getCommand`.
- Eventos de PocketMine con getters/setters, prioridad, `ignoreCancelled` y
  cancelación.
- Comandos declarados en `plugin.yml` y comandos creados con
  `new SimpleCommand(...)`.
- `Server`, `Player`, `World`, `Position`, `Location`, `Vector3`, `Config`,
  `Scheduler`, `PermissionManager`, `TextFormat`.

No objetivo de este milestone: inventarios reales sincronizados con el cliente,
entidades, generación de mundo desde PHP, tareas asíncronas reales en hilos
separados.

## Arquitectura

```
server-core (Rust)
  └── php_plugin_bridge::PluginManager
        ├── EventDispatcher nativo (Rust)
        └── PhpWorker (hilo dedicado + proceso `php`)
              └── runtime PHP embebido
                    ├── API PocketMine (stubs)
                    ├── registro de plugins/listeners/comandos
                    └── bucle IPC JSON-lines
```

El worker PHP es un proceso hijo persistente. Todas las peticiones están
serializadas: una petición, una respuesta. El worker Rust corre en un hilo
propio y expone una API síncrona (`request_sync`) y otra asíncrona
(`send_command`) que envuelve la síncrona con `spawn_blocking`.

## Archivos y propiedad

### Rust — `crates/php-plugin-bridge`

| Archivo | Responsable | Contenido |
|---|---|---|
| `src/runtime/php.rs` | agente bridge | proceso PHP, hilo worker, IPC, `PhpWorker`, `PhpPlugin` |
| `src/runtime/bootstrap.rs` | agente bridge | `include_str!` de los archivos PHP + escritura del runtime a disco |
| `src/manager.rs` | agente bridge | descubrimiento, orden de carga, dispatch, comandos, tick |
| `src/event.rs` | agente bridge | trait `Event`, `apply_json`, dispatcher |
| `src/context.rs` | agente bridge | `PluginAction` (acciones nuevas), `PlayerContext` |
| `src/events/*.rs` | agente bridge | eventos Rust nuevos |
| `src/command.rs` | agente bridge | `CommandMap` (sin cambios grandes) |
| `src/lib.rs` | director | re-exports e integración final |

### PHP — `crates/php-plugin-bridge/php`

Estos archivos se embeben con `include_str!` y se escriben a un directorio
temporal al iniciar el worker. Deben ser PHP 8.2 válido (`php -l`).

| Archivo | Responsable | Contenido |
|---|---|---|
| `php/math.php` | agente php-core | `Vector3`, `Position`, `Location`, `Vector2` |
| `php/text.php` | agente php-core | `TextFormat`, `Utils`, `ServerException` |
| `php/permission.php` | agente php-core | `Permission`, `PermissionManager` |
| `php/config.php` | agente php-core | `Config` (YAML con `yaml_parse`/`yaml_emit`) |
| `php/scheduler.php` | agente php-core | `Task`, `ClosureTask`, `TaskHandler`, `Scheduler` |
| `php/command.php` | agente php-core | `CommandSender`, `ConsoleCommandSender`, `Command`, `SimpleCommand`, `CommandMap` |
| `php/item.php` | agente php-core | `Item`, `ItemFactory` (stubs funcionales) |
| `php/world.php` | agente php-core | `World`, `WorldManager`, `Block`, `BlockFactory` |
| `php/player.php` | agente php-core | `Player`, `OfflinePlayer`, `PlayerInventory` |
| `php/core.php` | agente php-core | `Server`, `Logger` |
| `php/plugin.php` | agente php-runtime | `Plugin`, `PluginBase`, `PluginDescription`, `PluginManager` |
| `php/event.php` | agente php-runtime | `Event`, `Listener`, `Cancellable`, `EventPriority`, `RegisteredListener`, eventos PM |
| `php/autoload.php` | agente php-runtime | autoloader por plugin (carpeta y phar) |
| `php/ipc.php` | agente php-runtime | runtime IPC, registro, dispatch, comandos, tick |
| `php/bootstrap.php` | agente php-runtime | `require` de todos los archivos y arranque del bucle |

### Rust — `crates/server-core`

| Archivo | Responsable | Contenido |
|---|---|---|
| `src/session.rs` | agente server | hooks de eventos, identidad de login, cola de salida, acciones |
| `src/main.rs` | agente server | tick loop, consola, wiring del `PluginHost` |
| `src/plugin_host.rs` | agente server | implementación de `PluginHost` (mundo/jugadores) |

### Plugins de prueba — `plugins/`

| Ruta | Responsable |
|---|---|
| `plugins/ExamplePlugin/` | agente tests |
| `plugins/TestKit/` | agente tests |
| `plugins/ChatGuard/` | agente tests |
| `plugins/EconomyLite/` | agente tests |

## Protocolo IPC (JSON lines)

Cada línea es un objeto JSON UTF-8 sin saltos internos. Rust escribe peticiones
en stdin del proceso PHP; PHP responde en stdout. stderr se hereda para logs.

### Peticiones Rust → PHP

```json
{"action":"ping"}
{"action":"discover_plugins","dir":"plugins"}
{"action":"load_plugin","name":"Foo","main":"Acme\\Foo\\Main","file":"...","data_folder":"...","plugin_dir":"..."}
{"action":"enable_plugin","name":"Foo"}
{"action":"disable_plugin","name":"Foo"}
{"action":"dispatch_event","event":"PlayerChatEvent","data":{...}}
{"action":"execute_command","sender":{"type":"player","name":"Steve"},"command":"hello","args":["a"]}
{"action":"tick","tick":123}
{"action":"shutdown"}
{"action":"call_result","id":7,"ok":true,"result":{...}}
```

`load_plugin.file` es una ruta absoluta o una URI `phar://...`.
`plugin_dir` es el directorio del plugin o `phar://...` para phar.
`data_folder` es `plugin_data/<Name>/` ya creado por Rust.

### Respuestas PHP → Rust

Todas llevan `status` (`ok`/`error`), `message` opcional y `actions` (array,
puede faltar). Campos adicionales según la acción:

```json
{"status":"ok","actions":[]}
{"status":"ok","event":{...},"cancelled":false,"actions":[...]}
{"status":"ok","handled":true,"actions":[...],"message":null}
{"status":"ok","plugins":[{"kind":"phar","path":"...","manifest":"name: Foo\n..."}]}
{"status":"ok","commands":["hello"],"listeners":3}
{"status":"ok","tick":123,"actions":[],"pending_tasks":1}
```

`dispatch_event` debe devolver SIEMPRE `event` con el estado final del evento
(después de que los plugins lo modifiquen) y `cancelled`.

### Host calls PHP → Rust (síncronas)

Durante el manejo de una petición, PHP puede pedir datos al servidor. Escribe
una línea en stdout y espera una línea en stdin:

```json
{"type":"host_call","id":1,"method":"world.get_block","params":{"x":0,"y":64,"z":0}}
```

Rust responde:

```json
{"type":"host_result","id":1,"ok":true,"result":{"id":1}}
```

Métodos mínimos del host:

- `world.get_block` → `{"id":<u32>}`
- `world.set_block` → `{"ok":true}`
- `world.get_highest_block` → `{"y":<i32>}`
- `world.get_spawn` → `{"x":..,"y":..,"z":..}`
- `player.get` → datos del jugador por nombre
- `server.get_players` → lista de nombres
- `server.get_max_players` → `{"max":<i32>}`
- `server.get_motd` → `{"motd":"..."}`
- `server.is_op` → `{"op":true}`
- `server.has_permission` → `{"allowed":true}`

Si el host no está disponible, Rust responde `ok:false` y PHP usa un fallback.

## Acciones de plugin (`PluginAction`)

Formato JSON (compatible con `#[serde(tag="action", content="data")]`):

```json
{"action":"send_message","data":{"target_player":"Steve","message":"hi"}}
```

Acciones obligatorias del milestone:

| action | data |
|---|---|
| `send_message` | `target_player`, `message` |
| `broadcast_message` | `message` |
| `console_message` | `message` |
| `kick_player` | `target_player`, `reason` |
| `teleport` | `target_player`, `x`, `y`, `z`, `world` opcional |
| `set_block` | `world`, `x`, `y`, `z`, `block_id` |
| `dispatch_command` | `command`, `sender` opcional |
| `send_tip` | `target_player`, `message` |
| `send_popup` | `target_player`, `message` |
| `send_title` | `target_player`, `title`, `subtitle`, `fade_in`, `stay`, `fade_out` |
| `broadcast_tip` | `message` |
| `broadcast_popup` | `message` |
| `broadcast_title` | `title`, `subtitle`, `fade_in`, `stay`, `fade_out` |
| `set_gamemode` | `target_player`, `gamemode` |
| `set_scoreboard` | `target_player`, `scoreboard` (`title`, `objectives[{name, display_name, slot, sort_order, background[r,g,b], lines[{score, text}]}]`) |
| `remove_scoreboard` | `target_player` |
| `set_bossbar` | `bossbar` (`boss_id`, `diverse`, `title`, `subtitle`, `percentage`, `color`, `overlay`, `flags`, `entity`, `players[]`, `hidden[]`, `overrides[{player, title?, subtitle?, percentage?, color?}]`) — apiBossBar `BossBar`/`DiverseBossBar` (Bedrock `BossEventPacket` 0x4A + Java `boss_bar`) |
| `remove_bossbar` | `boss_id` |

`target_player` acepta el nombre exacto o `@all`/`@a` para todos.

## Eventos

`PlayerContext` serializado:

```json
{
  "name":"Steve","display_name":"Steve","address":"127.0.0.1:19132",
  "runtime_id":1,"unique_id":1,"ping":0,"gamemode":0,
  "x":0.0,"y":64.0,"z":0.0,"yaw":0.0,"pitch":0.0,"world":"world",
  "health":20.0,"max_health":20.0
}
```

Eventos del milestone y campos mutables:

| Evento | JSON | Mutables |
|---|---|---|
| `PlayerJoinEvent` | `player`, `join_message` | `join_message` |
| `PlayerQuitEvent` | `player`, `quit_message`, `reason` | `quit_message` |
| `PlayerChatEvent` | `player`, `message`, `format`, `recipients`, `cancelled` | `message`, `format`, `recipients` |
| `PlayerCommandPreprocessEvent` | `player`, `message`, `cancelled` | `message` |
| `PlayerMoveEvent` | `player`, `from`, `to`, `cancelled` (`from`/`to` son `Location`: `x`, `y`, `z`, `yaw`, `pitch`, `world`) | `to` |
| `BlockBreakEvent` | `player`, `world`, `x`, `y`, `z`, `block_id`, `cancelled` | cancelación |
| `BlockPlaceEvent` | `player`, `world`, `x`, `y`, `z`, `block_id`, `cancelled` | cancelación |
| `PlayerInteractEvent` | `player`, `world`, `x`, `y`, `z`, `action`, `cancelled` | cancelación |
| `ServerCommandEvent` | `sender`, `command`, `cancelled` | `command` |
| `PlayerDeathEvent` | `player`, `death_message` | `death_message` |
| `PlayerRespawnEvent` | `player`, `position`, `cancelled` | `position` |

La clase PHP correspondiente debe exponer los getters/setters de PocketMine
(`getPlayer`, `getMessage`, `setMessage`, `isCancelled`, `cancel`, etc.) y
`fromArray`/`toArray` estáticos para la hidratación.

## Registro de listeners

`PluginBase::registerEvents(Listener $listener, ?Plugin $plugin = null)`:

1. Recorre los métodos públicos del listener.
2. Toma el primer parámetro cuyo tipo sea una subclase de `Event`.
3. Lee el docblock del método:
   - `@priority MONITOR|HIGHEST|HIGH|NORMAL|LOW|LOWEST`
   - `@ignoreCancelled true|false`
4. Registra `[$listener, $method]` en el `PluginManager` con el plugin dueño.
5. Si el plugin implementa `Listener` y no llamó a `registerEvents`, el runtime
   puede autoregistrar sus métodos al habilitar (compatibilidad hacia atrás
   con `ExamplePlugin`).

Al deshabilitar un plugin se eliminan sus listeners y comandos.

## Diferencias conocidas / límites

- `World::getBlock`/`setBlock` usan host calls; si el host no responde,
  devuelven aire/`false`.
- `broadcastMessage` encola paquetes para cada sesión; se entregan en el
  siguiente paquete entrante de cada cliente o en el tick de salida.
- Inventarios, entidades, efectos y encantamientos son stubs en memoria.
- Las tareas del scheduler se ejecutan en el hilo PHP al recibir `tick`.

## Comandos de verificación

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo test --workspace
php -l crates/php-plugin-bridge/php/bootstrap.php
```

Prueba manual:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo run -p server-core
```

Los logs deben mostrar el arranque del worker, la carga de `plugins/` y la
ejecución de `/hello` desde `ExamplePlugin`.
