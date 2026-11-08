# JavaBridge

Plugin de PocketMine-Rust que abre un listener **TCP** y habla el protocolo de
**Minecraft: Java Edition** para que un cliente Java pueda entrar al mundo
**Bedrock** del servidor.

Todo vive en `plugins/JavaBridge/` (PHP); no toca el código del core.

```
Minecraft Java 1.12.2  --TCP 25565-->  JavaServer (PHP, stream no bloqueante)
                                           |
                                           +--> mundo Bedrock (host calls: block / highest_block)
                                           +--> chat Bedrock (acciones del plugin)
```

## Versiones soportadas

| Protocolo | Versión | Entrada al mundo |
|---|---|---|
| 340 | 1.12.2 | sí |
| 769 | 1.21.4 | sí (heightmaps en NBT) |
| 770 | 1.21.5 | sí |
| 773 | 1.21.9 | sí |
| 774 | 1.21.11 | sí |
| 775 | 26.1 | sí |

Las versiones modernas usan el flujo `handshake -> login -> configuration ->
play`: el puente manda los registros (`dimension_type`, `worldgen/biome` y
`chat_type`), cierra la configuración y entra al mundo con join game, posición,
chunks 1.18+ y el aviso `level_chunks_load_start`. Cualquier otro protocolo se
rechaza con un mensaje que enumera las versiones soportadas.

La versión objetivo es **26.1** (protocolo 775), la misma que corre el servidor
Bedrock actual; 1.21.4 - 1.21.11 se mantienen como compatibilidad.

## Archivos

| Archivo | Contenido |
|---|---|
| `plugin.yml` | Manifiesto (`main: javabridge\Main`, permiso `javabridge.command`). |
| `resources/config.yml` | `host`, `port`, `motd`, `enabled`, `view-distance`, gamemode/difficulty/level-type. |
| `src/javabridge/Main.php` | Ciclo de vida, config, tarea repetitiva que atiende los sockets, reenvío del chat Bedrock -> Java. |
| `src/javabridge/JavaServer.php` | Listener TCP, registro de clientes, status JSON, mundo/chunks y anuncios. |
| `src/javabridge/JavaClient.php` | Máquina de estados por conexión (handshake -> status/login -> play), keep-alive, envío de chunks. |
| `src/javabridge/Protocol.php` | VarInt, cadenas, `position`, frames, UUID offline y JSON de chat. |
| `src/javabridge/Nbt.php` | Escritor NBT de red (registros, textos y heightmaps). |
| `src/javabridge/ModernProfile.php` | Ids de paquete por protocolo (769-775) y registros de la fase `configuration`. |
| `src/javabridge/ModernChunk.php` | Columna 1.18+ (24 secciones, paletas, biomas, heightmaps y luz) construida desde la columna real del servidor. |
| `resources/registry/*.json` | Registros vanilla recortados, uno por protocolo. |
| `src/javabridge/ChunkBuilder.php` | Columna de chunk 1.12.2 desde la columna Bedrock, desplazada +64 en Y. |
| `src/javabridge/JavaBridgeCommand.php` | `/javabridge start\|stop\|status\|reload`. |
| `tests/` | Scripts de verificación; no se cargan en el servidor. |

## Cómo se usa

1. `cargo run -p server-core` (o el binario ya compilado).
2. El plugin debe cargarse desde `plugins/JavaBridge/`. En el log aparece:
   ```
   JavaBridge ... Escuchando en tcp://0.0.0.0:25565 (Java 1.12.2, protocolo 340, view-distance 6).
   ```
3. En el lanzador de Minecraft crea una instalación **1.12.2** y conecta a
   `localhost:25565`.
4. La lista de servidores muestra el MOTD y los jugadores online (Bedrock +
   Java).

## Qué funciona

- **Java 1.12.2 y 1.21.4 / 1.21.5 / 1.21.9 / 1.21.11 / 26.1**: login offline,
  fase de configuración con registros y entrada al mundo con chunks.
- Server list ping con MOTD, jugadores y versión (el nombre y el protocolo se
  adaptan al cliente que pregunta).
- Login offline (sin cifrado) y entrada al mundo.
- Chunks del mundo Bedrock por mapa de alturas; el generador plano por defecto
  se reproduce exactamente (bedrock en Y=0, 2 de tierra, hierba en Y=3).
- Movimiento y envío incremental de chunks (2 por tick) con descarga de los que
  quedan fuera.
- Chat por la misma API en las dos direcciones: el chat de Java dispara
  `PlayerChatEvent` (los plugins lo formatean o lo cancelan igual que el de
  Bedrock) y el mensaje final se difunde con `broadcastMessage`, que llega a
  las sesiones Bedrock y a la bandeja de los clientes Java.
- Los jugadores Java están dados de alta en el servidor (`player.register`):
  aparecen en `getOnlinePlayers()`, reciben `send_message` / `broadcast_message`
  / tips / feedback de comandos y disparan `PlayerJoinEvent` y
  `PlayerQuitEvent`.
- Romper bloques: se aplica al mundo Bedrock real (`world.set_block`) y se
  difunde el bloque a todos los clientes Java.
- `/help`, `/list`, `/me`, `/spawn` y `/tp <x> <y> <z>` locales; los comandos
  de plugins se ejecutan con `CommandMap::dispatch()` y un `Player` real, así
  que su respuesta llega al chat Java por la bandeja.

## Qué no funciona todavía

- Los jugadores Java **no son sesiones Bedrock**: están en el registro y en los
  eventos de jugador y de chat, pero el core no tiene `AddPlayerPacket`, así que
  los clientes Bedrock no los ven como entidades ni reciben su movimiento, y
  eventos de mundo como `BlockBreakEvent` siguen siendo sólo de Bedrock.
- La posición que la API expone de un jugador Java es la del spawn: los
  paquetes de movimiento no se reenvían al servidor todavía.
- Colocar bloques, inventario, salud real, combate, crafteo y contenedores.
- Cifrado y modo online (`online-mode`): todos los clientes entran sin
  autenticación.
- En moderno los registros van recortados a lo imprescindible (overworld,
  plains/the_void y chat) y el chunk se ilumina a cielo completo; no hay
  entidades, inventario ni colocación de bloques.
- Luz real, biomas y bloques no consultados: el chunk se aproxima por altura.
- Los comandos nativos del core (`/tp` de otros jugadores, `/gamemode`, `/say`...)
  todavía no se ejecutan para un jugador Java: el puente sólo llega al
  `CommandMap` de plugins. `/list` y `/help` se resuelven en el puente sobre el
  registro del servidor.

## Problemas encontrados (notas de implementación)

1. **El puente de plugins no puede crear sesiones.** Un jugador Java no puede
   convertirse en `PlayerSession` desde PHP, así que no hay eventos ni
   visibilidad entre protocolos. Es la limitación de diseño más importante:
   para cerrarla el core tendría que exponer una API de "sesión virtual".
2. **No hay lectura de bloques en bloque.** Cada `world.get_block` es una
   llamada JSON al proceso Rust; leer 65.536 bloques por chunk es inviable. Se
   usa `world.get_highest_block` por columna (256 por chunk), se cachea y se
   reparte el envío de chunks entre ticks.
3. **Las acciones devueltas por `onEnable` se descartan.** El host Rust ignora
   las acciones de `enable_plugin`, por eso el arranque del listener no depende
   de acciones y el trabajo periódico va en el scheduler.
4. **El bucle del runtime PHP está bloqueado en `fgets(STDIN)`.** Los sockets
   sólo se atienden en cada `tick` del servidor (20/s). El ping de login y el
   chat van bien; el movimiento del cliente es local y no genera tráfico.
5. **Cada versión moderna tiene sus propios ids.** Los paquetes se desplazan
   entre 1.21.4, 1.21.5, 1.21.9/1.21.11 y 26.1, así que `ModernProfile` lleva
   una tabla por protocolo generada desde `minecraft-data`. Los registros
   vanilla son cientos de KB de NBT: se generan una vez desde el `loginPacket`
   de minecraft-data (`target/gen-registry.js`) y se recortan a las entradas
   que el puente usa. El formato de chunk cambia en 1.21.5 (heightmaps como
   lista tipada en vez de NBT), y la luz viaja dentro de `map_chunk`.
6. **El chunk moderno pesa.** 24 secciones con luz a cielo completo son ~58 KB
   por columna; con `view-distance: 6` el envío inicial ronda los 4 MB, por
   eso hay contrapresión (no se encolan más chunks si el buffer supera 64 KB)
   y se mandan de dos en dos por tick.
7. **El formato del chunk cambió dos veces.** Desde 1.21.5 el array de longs de
   cada paleta ya no lleva prefijo de longitud (se deduce de los bits por
   entrada y del número de entradas) y en 26.1 cada sección empieza con dos
   shorts (bloques y fluidos) en vez de uno. Con el formato viejo el cliente
   muestra "Network Protocol Error" justo al entrar al mundo, porque el primer
   long mal alineado se lee como bits por entrada.
8. **Los registros con tags no se pueden mandar sin `update_tags`.** El cliente
   26.1 aborta en "Registry Loading" con *Unbound tags* si un registro enviado
9. **El mundo salía vacío y el jugador caía al vacío.** Concurrían tres cosas:
   el radio de chunks usaba el valor crudo del config mientras `login`
   anunciaba otro (el cliente esperaba columnas que no llegaban), las columnas
   iban en orden de fila y de dos en dos por tick (la del jugador llegaba
   tarde) y los heightmaps iban a cero, así que el cliente recalculaba la luz
   de cielo como si el techo estuviera abajo. Ahora se envía primero la columna
   del jugador, hasta agotar 12 ms por tick y con contrapresión, los heightmaps
   llevan la altura real y el spawn se ancla a la superficie que dibujamos.
   referencia `#minecraft:...` y el tag no llega en el paquete de tags. Por eso
   se omiten `enchantment` y `dialog`, y al `dimension_type` se le quita
   `timelines` (campo opcional que apunta al tag `in_overworld`). Los tags de
   bloque (`infiniburn`) sí se toleran sin enviarlos.
   Además, al terminar la configuración el cliente construye los componentes de
   los items y resuelve tags con `getOrThrow`; sin el paquete `tags` aborta con
   *Missing tag* (`minecraft:damage_type / minecraft:is_fire`). Por eso
   `resources/tags/<protocol>.json` lista todos los nombres de tags vanilla de
   la versión (generados con `target/gen-tags.js` desde `misode/mcmeta`); el
   tag existe aunque su lista de entradas vaya vacía. Pero un tag vacío no
   se muestra en F3: por eso `resources/tag_members/<protocol>.json`
   (generado con `tools/gen-tag-members.py` desde minecraft-data) da los
   IDs de registro de los miembros de un subconjunto — fluidos
   (`water`/`lava`, registro `minecraft:fluid` con empty=0,
   flowing_water=1, water=2, flowing_lava=3, lava=4 según `Fluids.<clinit>`
   del jar vanilla — de ahí water=[1,2] y lava=[3,4]), `mineable/*`,
   `climbable`, `fall_damage_resetting` y las
   herramientas de ítem — y el resto se sigue enviando vacío.
   Los tags de bloque (`infiniburn`) se toleran aunque falten.
   La marca del servidor (`Server:` en F3) viaja en el mismo
   configuration como `custom_payload` clientbound (id 1) en el canal
   `minecraft:brand` con el String `PocketMine`; sin ese paquete F3
   muestra `null`.
6. **Sin cifrado.** No se implementa `encryption_begin`, así que solo sirve en
   modo offline; el servidor Bedrock tampoco tiene autenticación Mojang.
7. **Coordenadas y Y distinta.** El mundo Bedrock va de Y=-64 a 319 y Java
   1.12 de 0 a 255; se recorta. `getHighestBlockAt` devuelve Y de Bedrock y
   `ChunkBuilder` la limita a 0..255.
8. **Formato de chunk delicado.** 1.12.2 usa empaquetado no compacto (un valor
   nunca cruza dos longs) y exige luz de cielo/bloque y 256 bytes de bioma; el
   bitmask con `groundUp = true` hace que las secciones omitidas sean aire.
9. **`unpack('J')` y 64 bits en PHP.** Las posiciones se descomponen en dos
   mitades de 32 bits (`Nhigh/Nlow`) para no depender del desbordamiento con
   signo al leer bits altos.
10. **Sin `AddPlayerPacket` no hay jugadores visibles.** Aunque el chat y el
    mundo se comparten, un cliente Java ve un mundo de un solo jugador.
11. **El puente puede tirar los TPS del servidor entero.** El core sirve la
    acción `tick` de forma *síncrona* (`request_sync("tick")`) y espera la
    respuesta de PHP, así que cada milisegundo que el puente pasa trabajando es
    un milisegundo que el bucle del servidor está parado — consola y chat
    incluidos. Todo trabajo pesado tiene que estar acotado por tick: el envío
    de chunks usa 12 ms por tick y el resto se pospone.
12. **Adivinar el terreno era carísimo y encima salía mal.**
    `world.get_highest_block` **regenera el chunk y sus 8 vecinos en cada
    llamada**, sin caché, así que leer 256 columnas por chunk (6.400 consultas
    por pantalla) bloqueaba el servidor decenas de segundos y dejaba el TPS en
    0,1. Ya no se adivina nada: el puente lee la columna real del servidor.
13. **La API del puente no daba chunks.** PocketMine expone
    `World::getChunk()` con los bloques de la columna; el puente sólo tenía
    `world.get_block` (una host call por bloque) y `world.get_highest_block`.
    Se añadió `world.get_chunk` en `ServerPluginHost` (devuelve la columna en
    formato de red de Bedrock, hex) y `pocketmine\world\Chunk` en
    `crates/php-plugin-bridge/php/world.php` con `getBlockStateId()`,
    `getHighestBlockAt()`, `getBiomeId()` y `getSectionPayload()`, al estilo de
    `pocketmine\world\format\Chunk` de axolotl-pm. El plugin traduce entonces
    bloque a bloque (los ids de red de Bedrock son hashes FNV-1a de
    `crates/bedrock-world/src/block_id.rs`) y el mundo Java coincide con el
    Bedrock, incluidas las roturas de los jugadores.
14. **El mundo salía revirado: X e Y intercambiadas.** Bedrock indexa la
    subsección como `x << 8 | z << 4 | y` y Java como `y << 8 | z << 4 | x`.
    La traducción recorría las 4096 entradas en orden de Java pero leía el
    mismo índice del chunk Bedrock, así que se llevaba el bloque de `x=1` a
    `y=1`: el terreno aparecía plano y gris, las columnas de piedra salían
    tumbadas y los árboles se convertían en torres. Había que reordenar los
    cuatro bits de cada eje al leer (`plugins/JavaBridge/src/javabridge/
    ModernChunk.php`). Los tests no lo veían porque el chunk de prueba era
    uniforme en X y Z: ahora lleva un bloque suelto y se comprueba que sigue en
    su coordenada (`tests/modern_test.php`, 5 comprobaciones).
15. **La hierba salía nevada, los árboles de lado y las plantas no existían.**
    Los estados de Java se habían copiado de `minStateId`, que es el *primer*
    estado del bloque, no el normal: 8 es `grass_block[snowy=true]` (toda la
    hierba con nieve) y 136 es `oak_log[axis=x]` (los troncos tumbados).
    Además faltaban en la tabla la capa de nieve, la hierba corta/alta y las
    flores, que se dibujaban como aire, y las menas de diamante, redstone y
    esmeralda tienen ids distintos en cada versión. Ahora
    `ModernProfile::blockState()` traduce por *nombre* con los estados por
    defecto de `target/javamc-<versión>/blocks.json`, así que hierba, troncos
    verticales, hojas, nieve y plantas salen en su estado correcto en 1.21.4,
    1.21.5, 1.21.9, 1.21.11 y 26.1.
16. **El mundo salía descolocado aunque los ids fueran los correctos.** El
    contenedor de biomas de cada sección se enviaba con `bits=1`, una paleta de
    dos entradas y un solo long, pero con 1 bit por entrada el cliente espera
    64 longs: se comía 504 bytes de la sección siguiente y todo el chunk se leía
    corrido (de ahí el agua plana sin forma). Ahora los biomas viajan como
    paleta de valor único (`bits=0`, sin array de datos), que es lo que espera
    el cliente (en 1.21.4 con su longitud a 0). Se añadió
    `cargo run -p bedrock-world --example dump_column -- <cx> <cz> <prefijo>` y
    `tests/real_column_test.php`, que comparan una columna generada por el
    servidor Rust con la columna que recibe Java bloque a bloque: 16.156
    bloques con la misma coordenada y la misma superficie, en 1.21.4, 1.21.5 y
    26.1.
17. **Java y Bedrock aparecían en puntos distintos.** Java usaba el *bloque*
    del spawn (y encima `max(spawn, superficie + 1)`), mientras que el cliente
    Bedrock aparece en `player_position()` del generador: centro del bloque
    (`+0,5` en X y Z) y altura de los ojos (`+1,62` en Y). Era medio bloque de
    diferencia en horizontal y 1,62 en vertical, además de que la altura se
    recalculaba con la superficie del chunk. Ahora `world.get_spawn` devuelve
    también `player` (el punto exacto de `player_position()`) y
    `World::getPlayerSpawnLocation()` lo expone al plugin: `JavaServer::spawn()`
    devuelve ese punto y el cliente Java se coloca ahí tal cual (1.12.2 sigue
    sumando 64 a la Y por el desplazamiento de su mundo). `chunk_api_test`,
    `wire_test` y `modern_test` comprueban ese punto exacto.
18. **"No es el mismo mundo": los dos clientes estaban en lugares distintos.**
    El servidor guarda la posición de cada jugador en `players/<nombre>.json` y
    el cliente Bedrock vuelve ahí al reconectar: el estado de pruebas estaba en
    `x=10022, y=76.6, z=6997`, a 10 000 bloques del spawn, mientras Java aparecía
    en el spawn. Los dos clientes miraban trozos distintos del mismo mundo y el
    terreno "no se parecía en nada". Se comprobó con el generador en las dos
    zonas (`real_column_test.php` en el spawn y en el chunk (626, 437) del punto
    guardado: columna real, columna 26.1 y columna 1.12.2 idénticas bloque a
    bloque en ambos sitios). Para comparar de verdad hay que estar en las mismas
    coordenadas: `/tp <x> <y> <z>` desde Java usa coordenadas del mundo Bedrock
    (el cliente 1.12.2 suma el desplazamiento de 64 él solo), y en Bedrock sirve
    `/tp` o borrar ese jugador de `players/` para volver a aparecer en el spawn.
    También se corrigió que `/spawn` y `/tp` en 1.12.2 no aplicaban el
    desplazamiento de +64: dejaban al jugador 64 bloques por debajo del terreno.
19. **El chat y las acciones de los plugins no pasaban por la misma API.** El
    puente escribía sus propios mensajes (`<nombre> mensaje`), reenviaba el chat
    Bedrock a Java con un listener y no disparaba `PlayerChatEvent` con el chat
    Java; los plugins tampoco podían hablarle a un jugador Java. Se cerró con
    jugadores virtuales: `player.register` da de alta al jugador Java en el
    `PlayerRegistry` compartido, el chat Java dispara `PlayerChatEvent` desde
    PHP y el resultado se difunde con `broadcastMessage`, y
    `SessionManager::deliver_text` entrega las acciones (`send_message`,
    `broadcast_message`, tips, feedback de comandos) a la sesión Bedrock o, si
    el nombre es de un jugador del puente, a su bandeja, que el puente vacía
    cada tick con `player.take_messages`. Se eliminaron los listeners
    `onChat`/`onJoin`/`onQuit` del plugin y `/list` ya no mezcla dos listas.

## Verificación sin servidor

Los scripts de `tests/` comprueban el traductor sin arrancar el servidor Rust:

```bash
cd plugins/JavaBridge
php -l src/javabridge/*.php        # sintaxis de todos los archivos
php tests/chunk_api_test.php       # decodificador Chunk del runtime y punto de aparición (12 comprobaciones)
php tests/protocol_selftest.php    # VarInt, frames, posición, UUID, chat y chunk 1.12.2 (29 comprobaciones)
php tests/nbt_test.php             # NBT contra prismarine-nbt, incluidos los registros reales (1.471 casos)
php tests/wire_test.php            # 1.12.2 por sockets reales, incluida la API de plugins (45 comprobaciones)
php tests/modern_test.php          # 1.21.4 a 26.1: login, configuration, join game, chunks, chat y API (133 comprobaciones)
php tests/real_column_test.php /tmp/col_spawn 775   # columna real del generador vs columna de Java, bloque a bloque
php tests/real_column_legacy_test.php /tmp/col_s    # columna real vs columna 1.12.2 (con el desplazamiento de +64)
```

`wire_test.php` abre un puerto efímero en loopback con stubs de `pocketmine\*`,
hace el status ping, entra como `Steve`, recibe los 9 chunks de
`view-distance: 1`, prueba `/list`, `/spawn`, `/tp` y romper un bloque, y
verifica el rechazo de nombres duplicados, de protocolos distintos y el anuncio
de salida. `tests/parity_stubs.php` modela el registro, los eventos y la
bandeja: las dos pruebas de red comprueban que el jugador Java queda registrado,
que su chat pasa por `PlayerChatEvent`, que su mensaje se difunde con
`broadcastMessage`, que una línea dejada por un plugin llega a su socket y que
la salida pasa por `PlayerQuitEvent` y lo saca del registro.

El lado Rust tiene su propia prueba de la host call, sobre un mundo generado de
verdad y con ediciones de bloques:

```bash
cargo test -p server-core --lib plugin_host
```

## Ideas para la siguiente iteración

1. Añadir una acción `broadcast_raw`/`send_raw` para que el plugin pueda
   enviar paquetes Bedrock arbitrarios y aparecer como entidad.
2. Soportar 1.16.5 y 1.20.4 con tablas de ids por versión (ya hay capturas en
   `data/bds-capture/` para el lado Bedrock).
3. Comandos Bedrock desde Java: necesita `dispatch_command` con feedback hacia
   el cliente.
4. Terminar la tabla Bedrock → Java: diamante, redstone, esmeralda y arcilla
   cambian de id entre versiones y hoy se dibujan como piedra.
