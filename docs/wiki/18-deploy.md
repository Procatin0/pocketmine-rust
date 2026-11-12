# 18 — Deploy: configuración, red y operación

## Configuración

Dos ficheros en el directorio de datos (`.`, donde arranca el
binario). Se **materializan en silencio** si no existen, como
`Server.php` (`ServerConfig::load_or_create` en
`crates/server-core/src/config.rs`): `pocketmine.yml` se copia
del recurso embebido `crates/server-core/src/pocketmine_default.yml`
y `server.properties` de los defaults PM. Solo se logean los
fallos de validación genuinos (`network.compression-level` fuera
de 1..9 → `WARN` «Invalid network compression level …» y vuelve a
6), igual que `Server.php`.

### `server.properties`

```ini
motd=Axolotl-PM Server
server-port=19132
server-portv6=19132
server-ip=0.0.0.0
view-distance=10
white-list=0
allow-list=0
whitelist=off
max-players=1000
gamemode=CREATIVE
difficulty=1
level-name=world
level-seed=
level-type=DEFAULT
enable-query=on
enable-ipv6=on
xbox-auth=off
online-mode=off
```

Claves que consume `config.rs`:

- `motd`, `server-ip`, `server-port`, `server-portv6`,
  `enable-ipv6`, `enable-query`, `max-players`, `white-list`
  (tres aliases: `white-list` / `allow-list` / `whitelist`),
  `auto-save`, `allow-cheats`, `language`, `level-type`,
  `level-name`, `level-seed`. Gamemode: `CREATIVE|C|1`,
  `ADVENTURE|A|2`, `SPECTATOR|SP|3|V` (también `gametype`).
  `xbox-auth` solo se aplica si es literal booleano
  (`on/off/true/false/1/0/yes/no`); cualquier otro valor conserva
  el default (igual que `Config::getConfigBool` en PM).
- `view-distance`: radio de chunks enviados. El valor de
  referencia para arrancar seguro es **6**
  (`chunk_cache` FIFO 512 cubre el área de 13×13 = 169 columnas y
  deja hueco para un segundo jugador; `CHUNK_OBJECT_CACHE_CAP
  1024` cubre dos áreas). Por encima el tiempo de generación
  (~18 ms/columna fría) lo absorbe el prefetch de
  `main.rs`, pero en máquinas lentas se nota en el tick Java.
  El default PM (`16`) queda guardado en el código como
  fallback cuando el fichero no trae la clave: sobreescribirlo
  solo con criterio.
- `difficulty`: 0 peaceful, 1 easy, 2 normal, 3 hard — también
  guarda en `WorldState` cuando cambia en juego.

### `pocketmine.yml`

- `settings.enable-dev-builds: true` — see abajo. También
  `force-language`, `shutdown-message`, `query-plugins`,
  `async-workers`, `white-list`.
- `network`: `batch-threshold: 256`, `compression-level: 6`
  (1..9), `async-compression: false`,
  `async-compression-threshold: 10000`, `max-mtu-size: 1492`,
  `enable-encryption: true`, `upnp-forwarding: false`.
- `transport.name: nethernet,raknet` (sección siguiente) con
  `nethernet.key-file: nethernet.key`, `port-range: [19133,
  19140]`, `mtu: 1200`, `builtin-signaling.enabled: true`,
  `builtin-signaling.port: 19132` (`0` = `server-port`).
- `player.verify-xuid: false`, `debug.level: 1`.

El parseo de `pocketmine.yml` es **mínimo e indentación-tolerante**
(secciones `settings` / `network` / `transport` / `nethernet` /
`builtin-signaling` / `tls` / `debug` / `player`): cualquier clave
desconocida se ignora. `port-range` acepta `[19133, 19140]`
(flow YAML, sin llaves) o un solo puerto.

`enable-dev-builds: true` es la divergencia port: PM lo defaultea
a `false` (Axolotl `5.51.2-dev` es dev-build y PM emite 5×
EMERGENCY + `exit(1)` si está a false; ver la compuerta en
`main.rs`). El port Rust lo defaultea a **true** para arrancar
fuera de la caja; ponlo a false si quieres paridad estricta.

Otros ficheros de datos que debas respaldar y versionar
(entorno):

- `world.json` — mundo (v2; ver `17-mundo-persistencia.md`).
- `players/<name>.json` — jugadores (JSON v2 26 campos).
- `ops.txt`, `banned-players.txt`, `banned-ips.txt`,
  `white-list.txt` (formato `entrada|razón`), `allowlist.json`,
  `scoreboard.json`, `nethernet.key`.
- `server.log` + `log_archive/` (ver «Logs»).

## Arranque

```powershell
cargo run -p server-core --release
```

Lo que imprime al arrancar (paridad con `Server::__construct`):
`Loading server configuration`, `Selected English (eng) as the
base language`, el banner dev-build si `enable-dev-builds` está a
true (3× WARN devBuild + versión), `pocketmine.server.start`
(AQUA, `1.26.60`), auth Xbox on/off (+warning si off),
`pocketmine.server.info` + license, plugins (`loadPlugins` +
`enablePlugins` STARTUP/POSTWORLD), `Preparing world "world"`,
interfaces de red, `pocketmine.server.startFinished`.

Dos sockets nativos en el arranque:

1. **`0.0.0.0:25565`** — Java (TCP, Tokio) — ver
   `16-java-bridge.md`.
2. Transporte Bedrock — ver siguiente sección.

Para producción, compila en release:

```powershell
cargo build --release -p server-core
```

y despliega el binario con su directorio de datos junto a él.

### Disco lleno: `cargo clean`

El `target/` puede ser enorme (binarios de dev + deps). Si el
disco se llena (ENOSPC, Windows `ERROR_DISK_FULL`), limpia:

```powershell
cargo clean
```

Eso borra `target/` entero; la siguiente compilación lo
reconstruye (≈ varios minutos). Solo `target/` está en
`.gitignore` (junto a `target/axolotl-pm` y
`target/axolotl-bedrock-protocol`, clones externos de referencia
que se recrean cuando faltan). Nunca versiones `target/`,
`server.log`, `players/`, `plugin_data/`, `world.json` editado a
mano o `nethernet.key`.

## Red: NetherNet vs RakNet (puerto 19132)

`transport.name` acepta hasta 8 transports, `nethernet,raknet` por
default. `config.rs` capándalo; `main.rs::start_network_interfaces`
valida en este orden PM: transport desconocido → WARNING
`pocketmine.server.transport.unknown` y se saltan; ninguno
válido → EMERGENCY `noneConfigured` y arranca apagado.

| Transporte | Puerto | Protocolo | Estado |
|---|---|---|---|
| **NetherNet** (WebRTC) | TCP signaling **19132** (+UDP gameplay en `port-range` o signaling+1) | HTTP signaling (`/v1/join`) + DTLS/SRTP DataChannel | Activo; el path preferido |
| **RakNet** | UDP **19132** (wild card `0.0.0.0`) | RakNet/STUN + ping offline | **Deprecado** (`pocketmine.server.raknet.deprecated` al arrancar) |

Detalles operativos:

- **TCP signaling y UDP gameplay comparten número de puerto**
  (19132) sin colisión: son protocolos distintos, como el
  default de PM (ambos 19132). Lo que no puede haber son **dos
  sockets UDP en el mismo puerto**: si el UDP de RakNet
  (`server-port` = 19132) colisiona con el UDP de WebRTC, el
  guard `raknet_collides_webrtc` deja el puerto a NetherNet y
  salva el bind de RakNet con un warning. No toques
  `server-port` a 19133 sin ajustar `port-range`: era el bug que
  causaba que RakNet y WebRTC se pisaran.
- **Bind en wildcard**: `0.0.0.0` y `[::]` dual-stack (con
  `enable-ipv6`). Nunca ates a una IP concreta (Hyper-V
  `172.19.16.1`, etc.): no tendrá ruta a LAN/loopback y cada
  ICE write devuelve Windows `10049`/`10051`.
- **Identidad NetherNet**: `nethernet.key` se crea si no existe
  (`ensure_identity_key_file`, `nethernet.key` en la raíz del
  repo). Los clientes fijan la pubkey al primer join; si lo
  rotas, los jugadores tendrán que re-verificar. TLS de
  signaling sin certificado = plain HTTP (aviso
  `pocketmine.server.nethernet.tls.disabled`); con certificado
  emite `tls.enabled`. `identity-domain` vacío = MOTD sin códigos.
- **UPnP**: `network.upnp-forwarding: false` (deprecado: solo
  reenvía el puerto RakNet; NetherNet ya hace NAT vía
  STUN/TURN — claves `transport.nethernet.ice-servers`, no
  implementadas en Rust todavía).
- **IPv6**: `enable-ipv6: on` abre `[::]:19132` para signaling y
  anuncia `networkStart` v6; `server-portv6 = 19132`.

Motd de RakNet (para la lista de servidores):
`MCPE;<motd>;<protocolo 2225>;<versión 1.26.60>;0;<max>;<server_id>;
<gamemode>;<puerto>;<puerto>;`.

## Query (GS4)

`crates/server-core/src/query.rs` — `QueryHandler` / `QueryInfo` /
`DedicatedQueryNetworkInterface` (protocolo `0xFE 0xFD`, UT3).

- `enable-query: on` en `server.properties`
  (también `query-plugins: true` en `pocketmine.yml` para
  listar plugins).
- `HANDSHAKE (9)` → token challenge = `BE32(substr(SHA512(salt
  + ":" + token), 7, 4))` donde `salt` es la IP del solicitante.
- `STATISTICS (0)` → info corta o larga (`\x00\x01player_\x00\x00`
  + lista de jugadores). `GAME_ID = "MINECRAFTPE"`,
  `server_engine = "Axolotl-PM 5.51.2-dev"`, `version = "1.26.60"`.
- Cableado (`main.rs`): si RakLib escucha en el puerto de
  gameplay, el handler **viaja en el socket RakNet**
  (`set_raw_handler`, como `Network::registerRawPacketHandler`
  de PM); si no, `QueryListener` dedicado en su propio UDP.
  Arranque loguea `pocketmine.server.query.running` (+ línea v6
  con `enable-ipv6`).
- `QueryHandler` es `Arc`-compartido y puro (`handle_packet`
  sin socket) → testeable. Funciona para MOTD/status en
  servidores externos (p. ej. `serverlist` sites que hacen GS4).

## Firewall / exposición

- **Bedrock (NetherNet)**: abrir **TCP 19132** (signaling).
  Si hay clientes fuera de LAN, también el rango UDP
  `19133–19140` (o el `port-range` que configures) para el
  DataChannel WebRTC.
- **Bedrock (RakNet legacy)**: abrir **UDP 19132** en el
  firewall si sigues usando RakNet o el Query sobre RakNet.
- **Java**: abrir **TCP 25565** si quieres clientes Java users.
- **LAN**: el discovery pasa por broadcast UDP (señalización
  built-in). Un firewall con `0.0.0.0` en LAN, no wildcard
  externo, evita spam de scanners.
- El `identity-domain` vacío es ok solo en localhost/LAN.
  Expuesto a Internet sin TLS, los paquetes HTTP de señalización
  van en claro (solo metadatos de peer, sin claves): usa
  `builtin-signaling.tls.certificate/key` o un reverse proxy
  (`reverse-proxy.enabled: false` por defecto, header
  `X-Forwarded-For`) si expones públicamente.

Comprobación rápida post-deploy:

```powershell
Test-NetConnection -ComputerName localhost -Port 25565
# Bedrock RakNet: lista del servidor o cliente Bedrock
# Query GS4: query UDP 19132 con un cliente GS4 (Handshake 9 → Statistic 0)
```

## Logs

`crates/server-core/src/logging.rs` (trazado en `tracing`):

- Consola (formato PM, `§` con colores por canal:
  `§7§o[fuente: mensaje]`, `chat.type.admin`) y fichero
  **`server.log`** en el directorio de datos.
- `debug.level: 1` en `pocketmine.yml` (`> 1` activa debug, p. ej.
  `Server unique id` / `Machine unique id`).
- Archivado: ficheros ≥ **32 MiB** se renombran a
  `log_archive/` con la cabecera
  `--- Starting new log file - old log file archived as {name} ---`,
  espejo de `MainLoggerThread::archiveLogFile`. `log_archive/`
  existe ya en la raíz del repo.
- Ruido habitual: NetherNet `builtinSignalingDisabled`, `tls.*`,
  `udpMuxWithoutPortRange`; el warning de deprecación de RakNet;
  arranques de transport en `start_network_interfaces`.
- Persistencia silenciosa: si `world.json` o
  `players/<name>.json` falta o es corrupto, arranque sin log
  (PM no tiene `world.json`); `set_server_property` avisará en
  otro nivel si no puede escribir `server.properties`.

Apagado ordenado: `/stop` o `Ctrl+C` (`SignalHandler` en PM) →
`server.forcingShutdown` → `disablePlugins` (15 s timeout) →
`worker.shutdown` (10 s) → cierre de sesiones
(`NetworkSessionManager::close("Server closed")`) →
`Unloading all worlds` → `Stopping other threads` → `exit(0)`
(retornar del Tokio se colgaría en el pool de bloqueo).

## Variable de entorno relevante al deploy

| Var | Para qué |
|---|---|
| `RUST_LOG` | Filtro `tracing` (parece respectado por `logging::init`) |
| `CARGO_BUILD_JOBS` | Limita los jobs en release de máquina lenta con pocos cores; evita agotar RAM/swap |

## Ficheros relacionados

- `server.properties`, `pocketmine.yml` (repo, raíz).
- `crates/server-core/src/config.rs` — `ServerConfig`, defaults, `load_or_create`, validación.
- `crates/server-core/src/pocketmine_default.yml` — plantilla que se copia cuando falta `pocketmine.yml`.
- `crates/server-core/src/main.rs` — `start_network_interfaces` (puertos, Query, RakNet vs NetherNet, logs PM).
- `crates/server-core/src/query.rs` — GS4.
- `crates/nethernet-rs/`, `crates/raklib-rs/` — transports.
- `crates/server-core/src/logging.rs` — `server.log` + archivado.
- `docs/PORT-ROADMAP.md` — fases 8 (red/escala) y 9 (operación).
