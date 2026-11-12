---
title: "01 — Introducción a la Plugin API"
description: "Qué es la Plugin API de Axolotl-PM: PHP bridge + crates Rust, filosofía Bedrock + Java, requisitos y ciclo de vida."
nav_order: 1
---

# 01 — Introducción a la Plugin API

[← Hub de la wiki](index.md) · [Siguiente: 02 — Quickstart →](02-quickstart.md)

## 1. ¿Qué es la Plugin API?

La **Plugin API** es el contrato público que permite extender
**Axolotl-PM / PocketMine-Rust** con plugins escritos en **PHP**, sin
recompilar el servidor. Es la misma API que PocketMine-MP 5:
`PluginBase`, `plugin.yml`, `Listener`, eventos, comandos, permisos y
scheduler. Si un plugin PMMP 5 funciona en Axolotl-PM, funciona aquí.

Por dentro, la API tiene dos mitades:

```
                    ┌─────────────────────────────────────────────┐
                    │           PLUGIN (PHP 8.2)                  │
                    │  plugins/MiPlugin/plugin.yml                │
                    │  plugins/MiPlugin/src/MiPlugin/Main.php     │
                    │  class Main extends PluginBase { ... }      │
                    └──────────────────┬──────────────────────────┘
                                       │  IPC JSON-lines
                                       │  (stdin/stdout, 1 línea = 1 JSON)
                    ┌──────────────────▼──────────────────────────┐
                    │      PHP WORKER (proceso hijo `php`)        │
                    │  runtime PHP embebido: stubs de PocketMine, │
                    │  autoloader PSR-4, ciclo IPC, PluginManager │
                    └──────────────────▲──────────────────────────┘
                                       │  host calls / PluginAction
                    ┌──────────────────┴──────────────────────────┐
                    │         SERVIDOR RUST (server-core)         │
                    │  PluginManager, EventDispatcher, CommandMap │
                    │  Bedrock 19132 + Java 25565, mundos, red    │
                    └─────────────────────────────────────────────┘
```

- **Lado PHP** (`crates/php-plugin-bridge/php/*.php`): stubs de
  `Server`, `Player`, `World`, `PluginBase`, `TextFormat`, `Config`,
  `Scheduler`, `Command`, eventos, etc. Se embeben con `include_str!`
  y se escriben a un directorio temporal al arrancar.
- **Lado Rust** (`crates/php-plugin-bridge/src/`): el `PluginManager`
  descubre `plugins/`, ordena dependencias, carga/habilita/deshabilita
  y despacha eventos. El worker PHP corre en su propio hilo y el Rust
  le habla con `request_sync()` (serializado: una petición, una
  respuesta) o `send_command()` sobre `spawn_blocking`.

El contrato completo —mensajes IPC, acciones `PluginAction`, eventos y
límites— vive en `docs/plugin-api-contract.md` del repo.

## 2. Filosofía: Bedrock + Java, un solo núcleo

El proyecto resuelve una tensión clásica: **Bedrock manda en el
protocolo, Java manda en el ecosistema**. La respuesta es un diseño en
crates Rust por borde y un núcleo compartido:

| Crate | Rol |
|-------|-----|
| `bedrock-protocol` | Paquetes Bedrock (protocolo 2225, MC 1.26.60). |
| `bedrock-world` | Chunks, bloques, mundo. |
| `raklib-rs` | RakNet (UDP de Bedrock). |
| `nethernet-rs` | WebRTC / señalización para Bedrock. |
| `java-protocol` + `java-bridge` | Listener Java nativo en `0.0.0.0:25565`. |
| `server-core` | Sesiones, ticks, comandos, logging, `ServerPluginHost`. |
| `php-plugin-bridge` | El puente Rust ↔ PHP descrito arriba. |

Principios que se derivan de ahí:

1. **Paridad con Axolotl-PM / PocketMine-MP**: mismos nombres, mensajes
   (claves de traducción), límites y efectos que el PHP de referencia.
   No se inventa comportamiento nuevo donde PM ya lo define.
2. **Un tick para todos**: Bedrock y Java comparten el reloj central
   (50 ms). No hay un segundo bucle para Java.
3. **Los plugins no ven la diferencia**: un evento como
   `PlayerJoinEvent` o un comando `/hello` funciona igual venga el
   jugador de Bedrock o de Java. El plugin habla con la API; el crate
   de protocolo traduce.

## 3. Requisitos para un plugin

### 3.1 Dónde vive

Todos los plugins se descubren automáticamente en la carpeta
**`plugins/`** de la raíz del servidor (se crea si no existe). Dos
formas válidas:

```text
plugins/
├── MiPlugin/                 # plugin en carpeta
│   ├── plugin.yml
│   ├── src/MiPlugin/Main.php
│   └── resources/config.yml  # opcional
└── Otro.phar                 # plugin empaquetado (phar://)
```

Referencias de ejemplo ya en el repo: `plugins/ExamplePlugin/`,
`plugins/TestKit/`, `plugins/ChatGuard/`, `plugins/EconomyLite/`.

### 3.2 `plugin.yml` (obligatorio)

| Campo | Requerido | Ejemplo |
|-------|-----------|---------|
| `name` | ✅ | `ExamplePlugin` |
| `version` | ✅ | `"1.1.0"` |
| `main` | ✅ (FQCN de `PluginBase`) | `pocketmine\example\Main` |
| `api` | recomendado | `["5.0.0"]` |
| `author` / `authors` | opcional | `PocketMine-Rust Team` |
| `description` / `website` | opcional | — |
| `depend` | opcional (duro) | `[CoreLib]` |
| `softdepend` | opcional (si está, va antes) | — |
| `load-before` | opcional | `[ChatGuard]` |
| `load` | opcional | `startup` o `postworld` (ver §4) |
| `src-namespace-prefix` | opcional | — |
| `commands` | opcional | ver §5 del quickstart |
| `permissions` | opcional | — |

Si falta `name`, `version` o `main`, o `load` no es `startup`/`postworld`,
el plugin no carga: el log muestra
`Could not load plugin '…': Invalid plugin manifest: …` y el servidor
deja de arrancar (`EMERGENCY … someLoadErrors`).

El código vive en `src/` con autoload **PSR-4** relativo al namespace
de `main`: `main: pocketmine\example\Main` carga
`src/pocketmine/example/Main.php`.

### 3.3 Requisitos de ejecución

- **PHP 8.2** en el `PATH` (el runtime necesita `yaml_parse`/
  `yaml_emit`; el worker es un proceso `php` hijo).
- **Rust** compilado: `cargo run -p server-core` (el binario del
  servidor) o `cargo test --workspace` para los tests de crates.
- Nada más: el servidor crea `plugin_data/<Nombre>/` por plugin al
  cargarse.

## 4. Ciclo de vida del plugin

Todo plugin pasa por tres fases, en este orden, respetando las
dependencias (`depend` / `softdepend` / `load-before` ordenan
topológicamente; un ciclo o una dependencia desconocida aborta la
carga):

### 4.1 Load — `Loading {Nombre} v{versión}`

1. Se descubre el directorio (o el `.phar`), se lee `plugin.yml` y se
   valida el manifiesto.
2. Se calcula el orden de carga (`depend`, `softdepend`, `load-before`).
3. Se instancia la clase `main` y se llama a **`onLoad()`** (por defecto
   no hace nada; es para leer configuración temprana).
4. Se crea `plugin_data/<Nombre>/`.

### 4.2 Enable — `Enabling {Nombre} v{versión}`

Depende de `load:` en `plugin.yml` y de `PluginEnableOrder`
(`crates/php-plugin-bridge/src/manager.rs`):

| `load:` | Fase | Cuándo |
|---------|------|--------|
| `startup` (`PluginEnableOrder::STARTUP`) | Antes de los mundos | Tras cargar plugins, antes de `Preparing world "world"`. |
| `postworld` (`PluginEnableOrder::POSTWORLD`, defecto) | Después de los mundos | Tras `Preparing world "world"`. |

En `enable` se llama a **`onEnable()`**: es donde registras listeners
(`registerEvents()`), comandos (`getCommand()`), scheduler, etc. Si
`onEnable` lanza una excepción, o el plugin se deshabilita a sí mismo
(*suicide*), el log muestra
`CRITICAL Could not enable plugin '…'`, el servidor emite
`EMERGENCY … someEnableErrors` y **deja de arrancar**. Por eso
`onEnable` nunca debe chocar con la consola.

### 4.3 Disable — `Disabling {Nombre} v{versión}`

Al apagar el servidor, los plugins se deshabilitan en **orden inverso**
al de enable. Se llama a **`onDisable()`** (guarda datos, cancela
tareas). Los errores de `onDisable` se registran como `CRITICAL` pero
**no detienen el apagado**.

```text
Arranque del servidor (crates/server-core/src/main.rs)
  │
  ├─ load_plugins("plugins")          → "Loading {Nombre} v{versión}"   (onLoad)
  ├─ enablePlugins(STARTUP)           → "Enabling {Nombre} v{versión}"  (onEnable, load: startup)
  ├─ Preparing world "world"
  ├─ enablePlugins(POSTWORLD)         → "Enabling {Nombre} v{versión}"  (onEnable, load: postworld)
  ├─ … servidor en marcha (eventos, comandos, tick) …
  └─ Apagado                          → "Disabling {Nombre} v{versión}" (onDisable, orden inverso)
```

## 5. Logs y colores de Axolotl-PM

El log imita a `MainLogger` de Axolotl-PM (ver
`crates/server-core/src/logging.rs`). Formato en consola:

```text
[18:26:43.210] [Server thread/INFO]: Loading ExamplePlugin v1.1.0
```

- Hora en aqua (`§b`, ANSI `\x1b[36m`), `Server thread` para el hilo
  principal; otros hilos como `{Nombre} thread`.
- Ocho niveles con su color PM: `EMERGENCY`/`ALERT`/`CRITICAL` en
  rojo `§c`, `ERROR` en rojo oscuro `§4`, `WARNING` en amarillo `§e`,
  `NOTICE` en aqua `§b`, `INFO` en blanco `§f`, `DEBUG` en gris `§8`
  (solo si `debug.level > 1` en `pocketmine.yml` o
  `POCKETMINE_LOG_LEVEL=debug`).
- Los códigos `§0–§9, §a–§f` de los plugins (p. ej. `TextFormat::GREEN`)
  se convierten a ANSI en terminal y se limpian sin terminal.
- Todo lo que ves en consola se guarda **sin color** en `server.log`
  con prefijo `YYYY-MM-DD`; al superar 32 MiB se archiva en
  `log_archive/`. Para desactivarlo: `POCKETMINE_NO_LOG_FILE=1`.

Los logs de plugin salen por `$this->getLogger()->info(...)` /
`warning(...)` / etc. y heredan el mismo formato y la misma hora que el
core, así que podéis seguir un plugin en `server.log` sin grep raro.

→ [02 — Quickstart: tu primer plugin](02-quickstart.md)
