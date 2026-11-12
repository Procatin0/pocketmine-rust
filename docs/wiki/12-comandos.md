# 12 — Comandos

Fuentes: `crates/server-core/src/command.rs` (comandos nativos Rust),
`crates/php-plugin-bridge/php/command.php` (jerarquía PHP `Command`),
`crates/php-plugin-bridge/php/plugin.php` (`plugin.yml` `commands:`),
`crates/php-plugin-bridge/php/core.php` (`Server::dispatchCommand`),
`crates/php-plugin-bridge/php/ipc.php` (acción `dispatch_command`).

Hay **dos capas** que conviven: (1) el **parser puro Rust** (`handle_command`
→ `CommandOutput { feedback, actions }`, sin efectos secundarios: el
`SessionManager` aplica los `CommandAction`); (2) la **jerarquía PHP
PM5** (`Command` / `SimpleCommand` / `PluginCommand` / …) para plugins.
Un comando desconocido en Rust devuelve `None` y cae a plugins; en PHP,
`Server::dispatchCommand` prueba primero el `CommandMap` y si no acierta
emite la acción IPC `dispatch_command` hacia Rust.

## 1. `CommandSource` (Rust): quién ejecuta

```rust
CommandSource { name, position, is_op, allow_cheats, is_console,
  max_players, world_time, world_seed, world_spawn,
  access: Option<Arc<AccessList>>, plugins: Vec<String>,
  tps: (f64,f64,f64), permissions: Option<Arc<PermissionManager>> }
```

`CommandSource::console()` = `CONSOLE`, op, en `0,0,0`,
`world_spawn = (0,64,0)`, `max_players = 1000`, TPS `(20,20,20)`.
`can(&node)` delega en `PermissionManager::has()` (doc 11);
`can_use_privileged()` = `is_op || allow_cheats`. Los jugadores online se
pasan como snapshot `&[OnlinePlayer] { name, position, gamemode }`.

## 2. Los ~50 comandos nativos (`COMMANDS` + `handle_command`)

`COMMANDS: &[CommandInfo { name, aliases, usage, description }]` alimenta a
`/help`; `handle_command(line, source, players)` valida permisos **antes**
del `match` (ver §5) y despacha. Extras del port sin nodo PM:
`/spawn`, `/tps`, `/weather`, `/scoreboard`, `/gmc|gms|gma|gmsp` (atajos de
`/gamemode`). Traducciones en el submódulo `lang` copiadas clave a clave de
`eng.ini` de PM (`{%0}`, `{%commandName}`, `{%playerName}`).

Efectos como enum `CommandAction` (un `Vec` por salida): `SetGamemode`,
`Teleport`, `TeleportToSpawn`, `Broadcast`, `Message`, `Kick`, `SetTime`,
`AddTime`, `SetWeather`, `SetWorldSpawn`, `SetOp`, `Kill`,
`ClearInventory`, `Give`, `SetSpawnPoint`, `SendTitle`, `Scoreboard`,
`Transfer`, `SetDefaultGamemode`, `SetDifficulty`, `BanName|BanIp|BanIpPlayer`,
`UnbanName|UnbanIp`, `SetWhitelist`, `WhitelistReload`, `AddWhitelist`,
`RemoveWhitelist`, `StopServer`, `SaveAll`, `SetAutoSave`, `SetTimeRunning`,
`ApplyEffect`, `Enchant`, `AddXp`, `SpawnParticle`, `Timings`,
`GarbageCollect`, `DumpMemory`.

## 3. Jerarquía PHP: `Command`, `CommandExecutor`, `SimpleCommand`, …

`Command` (`command.php`, abstracta): `name`, `description`,
`usageMessage`, `aliases`/`activeAliases`, `permissions: string[]`,
`permissionMessage`, `commandMap`, `executor`. Flujo:

- **Permisos**: `setPermissions(array)` exige que cada nodo exista en
  `PermissionManager`; `setPermission(string)` parte por `;`.
  `testPermissionSilent()` = **alguno** (OR, no AND);
  `testPermission()` muestra `permissionMessage` (defecto:
  `§cYou do not have permission…`). `getPermission()` devuelve el primero.
- **Registro**: `register/unregister(CommandMap)`, `isRegistered()`,
  `setLabel()` diferido (`nextLabel`) si ya está registrado,
  `getOwningPlugin()` por barrido de `Server::getPlugins()`.
- `broadcastCommandMessage(source, msg, sendToSource=true)`: el sender
  recibe la línea plana y los admins `[sender: mensaje]` gris itálica
  (aquí vía `Server::addAction('broadcast_admin')`).
- `execute(sender, label, args)` es abstracta.

**`CommandExecutor`**: interfaz con
`onCommand(CommandSender $sender, Command $command, string $label, array $args): bool`.

**`SimpleCommand extends Command`** (la forma moderna de plugin): recibe un
`?callable $executor`. `execute()`: (1) `testPermission()`; (2) callable si
existe; (3) `getExecutor()` callable o con `onCommand()`; (4) si nada,
`§cUsage: {usage}` y `false`.

**`ClosureCommand extends Command`** (legacy, deprecated en PM5): closure +
`string[] $permissions` obligatorio; `execute()` = la closure tal cual (sin
`testPermission` previo: la closure decide).

**`PluginCommand extends Command`**: comando propiedad de un `Plugin`;
`execute()` = `testPermission()` + `$plugin->onCommand(...)` con catch (stderr
`[PHP/COMMAND] {name} failed: …`, `false`).

**Extras**: `FormattedCommandAlias` (alias de servidor: re-despacha cada
`target` por el `CommandMap`), `InvalidCommandSyntaxException` →
`InvalidArgumentException`, `PermissionCommandException` → `Exception`.

**`CommandSender`** / **`ConsoleCommandSender`** (singleton): `CONSOLE`,
`isOp()=true`, `hasPermission()=true` siempre, `isPlayer()=false`,
`getLanguageCode()='eng'`, mensajes por `addAction('console_message')` con
fallback a stderr.

## 4. `CommandMap` y `fallbackPrefix`

`CommandMap::register(string $fallbackPrefix, Command $command, ?string $label)`:

1. Guarda **siempre** `$prefix . ':' . $label` (label lowercased).
2. Registra la etiqueta pelada solo si está **libre**; los comandos vanilla
   (registrados primero) ganan el nombre pelado. Si el pelado está ocupado,
   el label visible del comando pasa a `$prefix:$label`.
3. Cada **alias** se intenta igual; si no cabe, se descarta en silencio
   (comportamiento PM).
4. `command->register($this)` y hook opcional `Server::syncCommands()`.

`getCommand(name)` (lowercase, con resolución por la tabla
`alias => canonical`), `registerAll(prefix, cmds)`, `registerAlias()`,
`unregister()`, `clearCommands()`.

**`dispatch(sender, commandLine)`**: quita un `/` inicial, parte por
espacios (`preg_split('/\s+/')`), label = primer token lowercased, resto =
args tras `mergeSelectorTokens()` (§6); si el comando no existe devuelve
`false`; si existe, `execute()` dentro de try/catch (error →
`§cAn internal error occurred while executing this command.` + stderr) y
devuelve `true` (aunque `execute` devuelva `false`, p. ej. falta de permiso).

**`plugin.yml` → comandos**: `PluginDescription` parsea `commands:` como
`name → PluginDescriptionCommandEntry { description, usage, aliases,
permission, permission-message }` y **exige `permission` string no vacío**.
`PluginManager::createCommandsFromDescription()` construye un
`SimpleCommand(name, description, usage, aliases, permission)` por entrada
(sin executor: mostrará Usage salvo que el plugin lo fije después;
`getCommand(name)` / `setCommands()` en `PluginBase` gestionan el mapa
local del plugin). `Server::dispatchCommand()` (core.php) completa el
ciclo: map → acción IPC `dispatch_command` → Rust.

## 5. Chequeo de permisos en Rust y regla self/other

`permission_nodes(command) -> Option<&[&str]>` mapea comando+alias a su(s)
nodo(s) — p. ej. `"tp"|"teleport" → [TELEPORT_SELF, TELEPORT_OTHER]`,
`"give" → [GIVE_SELF, GIVE_OTHER]`, `"op" → [OP_GIVE]`, `"deop" → [OP_TAKE]`,
`"save-all|save-on|save-off → [SAVE_PERFORM|SAVE_ENABLE|SAVE_DISABLE]`,
`"time" → [TIME_SET, TIME_ADD]` (subcomandos chequean su nodo propio),
`"whitelist" → 6 nodos` (add/remove/enable/disable/list/reload). En
`handle_command`, si **ningún** nodo pasa, devuelve feedback
`You don't have permission to use the command: {name}` sin acciones.
`/help` oculta los comandos que el sender no puede ver
(`command_visible`, con casos especiales `time`/`whitelist`).

Más allá del gate: la **regla self/other** se aplica tras resolver los
objetivos — si algún target es el propio sender necesita `.self`, si hay
algún otro necesita `.other` (`fetch_permitted_player_targets` en Rust,
`fetchPermittedPlayerTargets` en PHP, que lanza
`PermissionCommandException`). Ej.: `/clear @s` con nodo `clear.self`
suficiente; `/clear @a` exige también `clear.other`.

## 6. Selectores `@a @s @p @e @r @n @c`

| Selector | Significado | Sort / limit por defecto |
|---|---|---|
| `@s` | el propio sender | limit 1 |
| `@p`, `@n` | jugador más cercano (alias `@n`) | `nearest`, limit 1 |
| `@a`, `@c` | todos los jugadores (legacy `@c`) | limit ∞ |
| `@e` | «entidades»: **solo jugadores online** (sin lista no-jugador en el runtime Bedrock) | limit ∞ |
| `@r` | uno al azar | `random`, limit 1 |

Argumentos dentro de `[...]`: `x|y|z` (con `~`/`^` relativo al origen),
`distance=min..max` (`5`, `..5`, `5..`, `1..5`), `name=`, `type=`
(`player(s)` no-op; otro tipo **no casa nada**), `gamemode=` (nombres ES/EN),
`limit=`, `sort=nearest|furthest|arbitrary|random`. Claves desconocidas se
ignoran; selector mal formado o vacío → `commands.generic.player.notFound`.

Implementación espejo en ambas capas (`resolve_selector_targets` /
`VanillaCommand::resolveSelectorTargets`, mismo orden filtros→sort→limit):

- **`merge_selector_tokens`**: el split por espacios trocearía
  `@a[distance=1..5, name=Steve]`; fusiona tokens hasta que los corchetes
  balanceen (de paso mantiene juntos los `[...]` de NBT/JSON de `/give` y
  `/title`).
- Origen: posición del sender (consola: `0,0,0`, salvo override `x/y/z`;
  en PHP `senderOrigin()` da la posición del `Player` o `0,0,0` si consola).
- `@r`/`sort=random`: Rust un xorshift con contador+`SystemTime` (sin crate
  rand); PHP `shuffle()`.
- `@e` avisa en logs (`tracing::warn!` en Rust,
  `trigger_error(E_USER_NOTICE)` en PHP) de que devuelve jugadores como
  entidades.
- Resolución de nombres: exacto case-insensitive primero, luego prefijo
  único (`player_by_prefix`); `resolve_players` = plural de `resolve_player`.
  Comandos que lo usan: `tp`, `gamemode`, `spawn`, `kill`, `kick`, `give`,
  `clear`, `effect`, `enchant`, `xp`, `title`, `spawnpoint`, `scoreboard`,
  `ban` (nombre), …

## 7. Helpers `VanillaCommand` (PHP)

`getInteger(sender, value, min, max)` = clamp silencioso; `getBoundedInt()`
= **lanza** `InvalidCommandSyntaxException` si no es numérico o sale de
`[min,max]` (Rust: `bounded_int` devuelve las claves
`num.tooBig|tootSmall`); `getDouble()`; `getBoolean()` acepta
`on|true|t|1`. `fetchPermittedPlayerTarget/Targets(sender, ?target,
selfPerm, otherPerm)`: sin target → el sender (consola sin target → syntax
error); con `@` → selector sobre `Server::getOnlinePlayers()`; con nombre
→ `getPlayerByPrefix`/`getPlayer`; permiso por target (self/other) y error
con la primera negación.

## 8. Limitaciones

- Sin parser de sintaxis de Bedrock real (sin `/execute …`,
  sin JSON de comandos): cada comando nativo es un `match` a mano con su
  `usage()`.
- `SimpleCommand` creados desde `plugin.yml` nacen **sin executor**: el
  plugin debe asignarlo (`setExecutor` / `setExecutorCallable`) o serán
  solo Usage; `PluginCommand` existe pero `Plugin` no declara `onCommand`
  en la interfaz (solo `PluginCommand::execute` lo invoca).
- `@e` no incluye mobs/items; selectores no-jugador quedan para cuando el
  runtime tenga lista de entidades no-jugador.
