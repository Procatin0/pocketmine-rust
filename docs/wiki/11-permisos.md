# 11 — Permisos, bans y whitelist

Fuentes: `crates/php-plugin-bridge/php/permission.php` (bridge PHP, espejo de
PM5 `src/permission/*`), `crates/server-core/src/permission.rs` (Rust),
`crates/server-core/src/access.rs` (Rust `BanEntry`/`AccessList`),
`crates/server-core/src/command.rs` (`CommandSource::can`),
`crates/php-plugin-bridge/php/plugin.php` (`plugin.yml` `permissions:` /
`commands:`). Referencia original PM5: `target/axolotl-pm/src/permission/`
(recreable, no versionada).

## 1. Los 61 nodos: `DefaultPermissionNames`

`pocketmine\permission\DefaultPermissionNames` es un `final class` de
constantes puro: cada constante vale el nombre del nodo, con prefijo
`pocketmine.`. Son **61 nodos**: 56 de comando
(`pocketmine.command.*`), 2 de broadcast (`pocketmine.broadcast.admin` /
`pocketmine.broadcast.user`) y 3 raíces de grupo
(`pocketmine.group.console` / `operator` / `user`).

| Grupo | Nodos (ejemplos, todos en el fichero) |
|---|---|
| Broadcast | `broadcast.admin`, `broadcast.user` |
| Ban / whitelist | `command.ban.ip`, `ban.list`, `ban.player`, `unban.ip`, `unban.player`, `whitelist.add|disable|enable|list|reload|remove` |
| Modo, teleport, spawn | `gamemode.other|self`, `teleport.other|self`, `spawnpoint.other|self`, `setworldspawn`, `defaultgamemode`, `difficulty` |
| Inventario / XP / efectos | `give.other|self`, `clear.other|self`, `xp.other|self`, `effect.other|self`, `enchant.other|self`, `kill.other|self`, `title.other|self` |
| Mundo / servidor | `time.add|query|set|start|stop`, `save.disable|enable|perform`, `stop`, `seed`, `status`, `gc`, `timings`, `dumpmemory`, `plugins`, `particle`, `transferserver` |
| Chat / varios | `help`, `me`, `tell`, `version`, `say`, `list`, `kick`, `op.give|take` |
| Raíces | `group.console`, `group.operator`, `group.user` |

El Rust (`permission.rs`) re-declara como `pub const` los nodos que usa
`command.rs` (mismo string) y añade dos nodos que PM no tiene:
`pocketmine.command.scoreboard.self|other` y `pocketmine.command.tps`
(`/scoreboard` y `/tps` son extensiones del port).

## 2. El árbol `console → operator → user`

`DefaultPermissions::registerCorePermissions()` (PHP) y
`PermissionManager::register_defaults()` (Rust) construyen el mismo árbol:

```
pocketmine.group.console        (default op)
 └─ pocketmine.group.operator   (hijo de console, default op)
     └─ pocketmine.group.user   (hijo de operator, default op)
```

- **Console-only**: `command.dumpmemory`, hijo directo de
  `group.console`. En Rust queda registrado como `Op` (no hay flag
  `is_console` en `has()`: el comentario en `register_defaults` lo
  documenta; el arista en el árbol sí es console-only).
- **Operator** (~50 hijos de `group.operator`): todo lo administrativo —
  `ban.*`, `whitelist.*`, `stop`, `save.*`, `gamemode.other`,
  `teleport.other`, `give.other`, `time.*`, `say`, `status`, `timings`,
  `seed`, `op.give|take`, `kick`, `list`, `xp.other`,
  `broadcast.admin`, … Los nodos con par `self/other` aparecen ambos en el
  árbol; el que decide es la regla self/other de cada comando (doc 12).
- **User** (7 hijos de `group.user`): `help`, `me`, `tell`, `version`,
  `kill.self`, `clear.self`, `broadcast.user`.

Además hay tres **alias legacy PM4** (`DefaultPermissions::ROOT` =
`pocketmine`, `OPERATOR` = `pocketmine.operator`, `USER` =
`pocketmine.user`): en PHP `USER` es `DEFAULT_TRUE`; en Rust los tres son
`False` con un único hijo hacia la raíz PM5 correspondiente, para que
plugins viejos que consulten `pocketmine.operator` sigan resolviendo.

### Defaults: `true` / `false` / `op` / `notop`

`Permission::DEFAULT_TRUE/FALSE/OP/NOT_OP` (PHP) ≡
`PermissionDefault::{True,False,Op,NotOp}` (Rust). La resolución en Rust
(`PermissionDefault::resolve`) es:

| Default | Resultado |
|---|---|
| `True` | siempre concede |
| `False` | siempre deniega |
| `Op` | `is_op \|\| allow_cheats` (`server.properties` `allow-cheats`) |
| `NotOp` | `!is_op` |

En `register_defaults()` los leaves llevan su default PM real (la mayoría
`Op`; `True` en `help`, `me`, `tell`, `list`, `clear.self`, `kill.self`,
`version`, `time.query`, `broadcast.user`, `scoreboard.self`, …) mientras
que las **raíces** `group.*` quedan en `False` con los hijos como
introspección: `has()` resuelve el leaf directamente (fast path) y la
cadena `children` solo se recorre vía `children_of()` y en la recursión
de `has()` cuando un nodo tiene hijos (grants por hijo, ver §4).

## 3. `PermissionParser`: defaults desde YAML y `plugin.yml`

- `PermissionParser::defaultFromString(bool|string)` (PHP) y
  `PermissionDefault::from_str` (Rust) aceptan los alias YAML de PM:
  `op|isop|operator|isoperator|admin|isadmin` → `op`,
  `!op|notop|!operator|notoperator|!admin|notadmin` → `notop`,
  `true|false` (y en Rust además `1|0|yes|no|on|off`). Bool YAML real
  (`default: true`) se convierte a `"true"/"false"` antes de parsear.
  Un default desconocido lanza `InvalidArgumentException` (PHP) /
  devuelve `Err("Unknown permission default name ...")` (Rust).
- `PermissionParser::loadPermissions(array $data, $default = 'false')`
  (PHP) valida el subárbol `permissions:` de un `plugin.yml` y devuelve
  `Permission[]`. **Rechaza `children` anidados** («Nested permission
  declarations are no longer supported») — `PluginDescription` lo envuelve
  en `PluginDescriptionParseException('Invalid Plugin "permissions": …')`.
- En Rust, `PermissionManager::load_plugin_permissions_yaml()` /
  `load_plugin_permissions_str()` hacen el mismo trabajo pero **sí aceptan
  `children`** (mapa `hijo: bool` o lista; truthy = grant), con fallback
  de `default` a `op` (igual que `PluginDescription::default` en PM).
  `register_with_children()` aplana a `Vec<String>` (ver §4).

### `permissions:` en `plugin.yml`

```yaml
permissions:
  test.plugin.admin:
    description: "Can manage the plugin"
    default: op
  test.plugin.user:
    description: "Can use the plugin"
    default: true
  test.plugin.child:
    default: notop
    children:
      test.plugin.user: true
```

Este ejemplo es, literalmente, el test `plugin_yml_permissions_register_with_defaults`
de `permission.rs`. Un `commands:` de `plugin.yml` **exige** `permission`
string no vacío (`PluginDescriptionParseException: Command X does not have
a valid permission set`); ver doc 12.

## 4. `PermissionManager` (Rust) y `Permission` / `Permissible` (PHP)

### Rust (`permission.rs`)

```rust
PermissionManager {
  permissions: RwLock<HashMap<String, Permission>>,
  attachments: RwLock<HashMap<String, HashMap<String, bool>>>, // por jugador, minúsculas
}
```

- `register(name, default, children)` / `register_with_children(name,
  default, &[(child, grant)])`. Diferencia con PM: PHP guarda
  `children` como `child => bool` (puede denegar); el Rust aplana a
  `Vec<String>`, así que los hijos denegados se meten en la lista pero
  solo cuentan como grant — documentado en el código.
- `has(player, is_op, allow_cheats, node)`: (1) **attachment** del jugador
  (`set_permission` / `unset_permission` / `clear_permissions`, clave
  lowercased); (2) **hijos recursivos** con límite de **profundidad 8**
  (anti-ciclo); (3) el **default** del nodo registrado. Nodo **no
  registrado** → `is_op || allow_cheats` (igual que
  `PermissionManager::hasNodePermission` en PHP).
- `default_of()` / `children_of()` para introspección y tests.

### PHP (`permission.php`)

- `Permission`: `name`, `description` (string|Translatable), `default`,
  `children: array<string,bool>`; `addChild/removeChild`,
  `recalculatePermissibles()` no-op (sin grafo de subscripciones),
  `getPermissibles()` devuelve `[]`.
- `PermissionManager` singleton: `register/add/removePermission`,
  `getDefaultPermissions(op)`, `subscribe/unsubscribe`, `clearPermissions`,
  `hasPermission(Permission, isOp, explicit)` con `match` sobre el
  default, y `hasNodePermission(name, isOp, explicitMap)`.
- `Permissible` / `PermissibleInternal`: `setBasePermission` /
  `unsetBasePermission` guardan **roots** y `recalculatePermissions()`
  expande hijos con XOR de negación (`calculateChildPermissions`), con
  diff devuelto. Esto es lo que `Player::hasPermission` usa cuando no hay
  mapa explícito.

### Integración con comandos

`CommandSource::can(node)` (`command.rs`): si el source trae
`permissions: Option<Arc<PermissionManager>>` usa ese manager; si no
(tests, herramientas), un `OnceLock` estático con
`PermissionManager::new()` — mismo árbol. `Command::setPermissions()`
(PHP) exige además que **todos** los nodos existan en el manager
(`Cannot use non-existing permission "…"`).

## 5. `BanEntry` y `BanList` (bans + whitelist)

Dos implementaciones paralelas, mismo formato de fichero PM:

| Aspecto | PHP (`permission.php`) | Rust (`access.rs` → `AccessList`) |
|---|---|---|
| `BanEntry` | `name|created|source|expires|reason` (`BanEntry::$format = 'Y-m-d H:i:s'`) | idéntico, `BAN_DATE_FORMAT` |
| Defaults | name lowercased, `created = time()`, source `(Unknown)`, `expires = null`, reason `Banned by an operator.` | idéntico |
| `getString()` / `get_string()` | `Forever` si no expira, fechas GMT | `Forever` si no expira, fechas UTC (round-trip estable) |
| `fromString` / `from_string` | Acepta el formato PM5 y el legacy `name\|reason` (2º campo no-fecha = razón) e incluso `name` pelado | idéntico (`parse_unix` acepta `Y-m-d H:i:s [±zzzz]` o entero unix) |
| `BanList` | clase por fichero, `enabled=true`, `isBanned/getEntry/getEntries` purgan expirados en cada lectura, `load()` ignora `#` y parseos malos (stderr), `save()` con cabecera `# victim name \| …` | `AccessList` unifica nombres + IPs + whitelist: `ban_name_with/ban_ip_with(source, expires)`, `pardon_name/pardon_ip`, `name_ban_entry/ip_ban_entry`, `denial()` |

Detalles que merecen saber:

- **Ficheros PM**: `banned-players.txt`, `banned-ips.txt`, `white-list.txt`
  (y el flag `white-list|whitelist|allow-list` de `server.properties`).
  `AccessList::load_from_dir(dir)` los carga y desde ahí persiste cada
  cambio; sin `dir` es 100% en memoria (tests/servidor embebido).
- **`removeExpired()` corre antes de cada lectura** (`isBanned`,
  `getEntry`, `getEntries`, `denial`): un ban expirado desaparece sin
  `/pardon` explícito — paridad con PM `BanList::isBanned`.
- **`denial(name, ip, is_op)`** (Rust, equivalente a lo que el login
  muestra): ban de nombre → `pocketmine.disconnect.ban: {%reason}` (o
  `…ban.noreason` si reason vacío); ban de IP →
  `…ban` con `IP banned` si no hay razón; whitelist activada y jugador no
  operador fuera de lista → `disconnect.whitelisted`. **Los operadores
  bypassean la whitelist** (paridad `Server::isWhitelisted()`).
- Las IPs se validan con `inet_pton()` semántico (`is_ip_address`:
  IPv4 y IPv6, `999.1.1.1` no es IP).
- El bridge PHP no tiene un `Server::getBanList()` unificado: cada
  `BanList` vive en el fichero que le pases al constructor; el Rust es el
  que centraliza los tres ficheros en `AccessList`, accesible desde
  `CommandSource.access: Option<Arc<AccessList>>` para los comandos
  `/ban`, `/ban-ip`, `/banlist`, `/pardon`, `/pardon-ip`, `/whitelist`
  (doc 12).

## 6. Limitaciones conocidas

- Sin objetos `PermissionAttachment` ni eventos de permisos: attachments
  son el mapa `HashMap<String, HashMap<String, bool>>` de `PermissionManager`.
- Sin árbol de herencia efectivo para defaults `True/Op` de hojas: la
  recursión por hijos existe, pero el fast path (`has()` sobre el default
  del leaf) es lo que decide en la práctica.
- `dumpmemory` queda `Op` en Rust pese a ser console-only en el árbol
  (falta flag `is_console` en `has()`).
