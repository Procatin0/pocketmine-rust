//! Native (core) commands ported from PocketMine-MP / vanilla Bedrock.
//!
//! Parsing is pure on purpose: the caller supplies a [`CommandSource`] (who ran
//! the command) and a snapshot of [`OnlinePlayer`]s used to resolve targets.
//! Everything that has a side effect is expressed as a [`CommandAction`] and
//! applied by [`crate::session::SessionManager`], which owns the live sessions,
//! the world and the operator list.

/// Position of an online player, used to resolve command targets.
#[derive(Debug, Clone, PartialEq)]
pub struct OnlinePlayer {
    pub name: String,
    pub position: (f64, f64, f64),
    pub gamemode: i32,
}

/// Who executed a command (a player or the server console).
#[derive(Debug, Clone, PartialEq)]
pub struct CommandSource {
    pub name: String,
    pub position: (f64, f64, f64),
    pub is_op: bool,
    /// `allow-cheats` from `server.properties`: when enabled every player can
    /// use administrative commands, like a vanilla world with cheats on.
    pub allow_cheats: bool,
    pub is_console: bool,
    /// `max-players` from `server.properties`, reported by `/list`.
    pub max_players: i32,
    pub world_time: i32,
    /// Seed of the current world, used by `/seed`.
    pub world_seed: u64,
    /// World spawn, used as the origin of relative coordinates when the console
    /// runs a command.
    pub world_spawn: (f64, f64, f64),
    /// Ban and whitelist lists, when the sender runs inside the server.
    pub access: Option<std::sync::Arc<crate::access::AccessList>>,
    /// Loaded plugin names, used by `/plugins`.
    pub plugins: Vec<String>,
    /// TPS averages over 1m/5m/15m, used by `/tps`.
    pub tps: (f64, f64, f64),
    /// Permission registry and attachments, when the sender runs in-server.
    pub permissions: Option<std::sync::Arc<crate::permission::PermissionManager>>,
}

impl CommandSource {
    pub fn console(world_time: i32) -> Self {
        Self {
            name: "CONSOLE".to_string(),
            position: (0.0, 0.0, 0.0),
            is_op: true,
            allow_cheats: true,
            is_console: true,
            max_players: 1000,
            world_time,
            world_seed: 0,
            world_spawn: (0.0, 64.0, 0.0),
            access: None,
            plugins: Vec::new(),
            tps: (20.0, 20.0, 20.0),
            permissions: None,
        }
    }

    /// Operators always may; everybody may while cheats are enabled.
    pub fn can_use_privileged(&self) -> bool {
        self.is_op || self.allow_cheats
    }

    /// `PermissionManager::hasPermission()` for this sender.
    ///
    /// Without a registry (plugin-less or test sources) it falls back to the
    /// historical operator/cheats rule.
    pub fn can(&self, node: &str) -> bool {
        match &self.permissions {
            Some(manager) => manager.has(&self.name, self.is_op, self.allow_cheats, node),
            // Sources built outside a live server still honour PocketMine's
            // default permission values.
            None => default_permissions().has(&self.name, self.is_op, self.allow_cheats, node),
        }
    }
}

/// Registry used when a `CommandSource` has no manager attached (tests, tools).
fn default_permissions() -> &'static crate::permission::PermissionManager {
    static DEFAULT: std::sync::OnceLock<crate::permission::PermissionManager> =
        std::sync::OnceLock::new();
    DEFAULT.get_or_init(crate::permission::PermissionManager::new)
}

/// Weather states shared by `/weather`, the world state and the packets sent
/// to clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherState {
    Clear,
    Rain,
    Thunder,
}

impl WeatherState {
    pub const fn as_i32(self) -> i32 {
        match self {
            Self::Clear => 0,
            Self::Rain => 1,
            Self::Thunder => 2,
        }
    }

    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Clear),
            1 => Some(Self::Rain),
            2 => Some(Self::Thunder),
            _ => None,
        }
    }

    pub fn name_es(self) -> &'static str {
        match self {
            Self::Clear => "despejado",
            Self::Rain => "lluvia",
            Self::Thunder => "tormenta",
        }
    }

    /// LevelEvent ids that put every client in this state.
    pub const fn level_events(self) -> &'static [i32] {
        // 3001 START_RAIN, 3002 STOP_RAIN, 3003 START_THUNDER, 3004 STOP_THUNDER.
        match self {
            Self::Clear => &[3002, 3004],
            Self::Rain => &[3001, 3004],
            Self::Thunder => &[3001, 3003],
        }
    }
}

/// Title operation requested by `/title`, one per `Player::send*()` call.
#[derive(Debug, Clone, PartialEq)]
pub enum TitleAction {
    Clear,
    Reset,
    Title(String),
    Subtitle(String),
    ActionBar(String),
    Times {
        fade_in: i32,
        stay: i32,
        fade_out: i32,
    },
}

/// Side effect requested by a core command.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandAction {
    SetGamemode {
        target: String,
        gamemode: i32,
        message: String,
        notify: String,
    },
    Teleport { target: String, position: (f64, f64, f64), yaw: Option<f32>, pitch: Option<f32> },
    TeleportToSpawn { target: String },
    Broadcast { message: String },
    Message { target: String, message: String },
    Kick { target: String, reason: String },
    SetTime(i32),
    AddTime(i32),
    SetWeather {
        state: WeatherState,
        duration_ticks: i32,
    },
    SetWorldSpawn { x: i32, y: i32, z: i32 },
    SetOp { target: String, op: bool },
    Kill {
        target: String,
        /// `commands.kill.successful` with the victim's name, already filled in.
        message: String,
        /// PocketMine only broadcasts when somebody else was killed.
        broadcast: bool,
        /// Who ran the command: a player name or `CONSOLE`.
        notify: String,
    },
    ClearInventory {
        target: String,
        /// Item filter already resolved to `(network id, meta)`.
        item: Option<(i16, u32)>,
        /// PocketMine's `maxCount`: `-1` removes every matching item, `0` only
        /// counts them and a positive value removes at most that many.
        max_count: i32,
        notify: String,
    },
    Give {
        target: String,
        network_id: i16,
        meta: u32,
        count: u16,
        /// Network NBT from the `[...tags]` argument, empty when absent.
        nbt: Vec<u8>,
        /// `commands.give.success`, already filled in.
        message: String,
        notify: String,
    },
    SetSpawnPoint {
        target: String,
        position: (f64, f64, f64),
        /// `commands.spawnpoint.success`, already filled in.
        message: String,
        notify: String,
    },
    /// `/title`: one `SetTitlePacket` for the target.
    SendTitle {
        target: String,
        action: TitleAction,
    },
    /// `/transferserver`: send `TransferPacket` and close the session.
    Transfer {
        target: String,
        address: String,
        port: u16,
    },
    SetDefaultGamemode(i32),
    SetDifficulty {
        difficulty: i32,
        /// `commands.difficulty.success`, already filled in.
        message: String,
        notify: String,
    },
    BanName {
        name: String,
        reason: String,
        /// `commands.ban.success`, already filled in.
        message: String,
        notify: String,
    },
    BanIp {
        ip: String,
        reason: String,
        /// `commands.banip.success`, already filled in.
        message: String,
        notify: String,
    },
    /// `/ban-ip <player>`: the manager resolves the address from the session.
    BanIpPlayer {
        player: String,
        reason: String,
        notify: String,
    },
    UnbanName {
        name: String,
        /// `commands.unban.success`, already filled in.
        message: String,
        notify: String,
    },
    UnbanIp {
        ip: String,
        /// `commands.unbanip.success`, already filled in.
        message: String,
        notify: String,
    },
    SetWhitelist {
        enabled: bool,
        /// `commands.whitelist.enabled` or `.disabled`.
        message: String,
        notify: String,
    },
    WhitelistReload {
        notify: String,
    },
    AddWhitelist {
        name: String,
        /// `commands.whitelist.add.success`, already filled in.
        message: String,
        notify: String,
    },
    RemoveWhitelist {
        name: String,
        /// `commands.whitelist.remove.success`, already filled in.
        message: String,
        notify: String,
    },
    StopServer {
        /// `commands.stop.start`, already filled in.
        message: String,
        notify: String,
    },
    SaveAll {
        /// `pocketmine.save.start`, already filled in.
        message: String,
        notify: String,
    },
    SetAutoSave {
        enabled: bool,
        /// `commands.save.enabled` or `.disabled`.
        message: String,
        notify: String,
    },
}

/// Result of a recognized core command.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandOutput {
    /// Lines sent back to whoever ran the command.
    pub feedback: Vec<String>,
    pub actions: Vec<CommandAction>,
}

/// Metadata used by `/help`.
pub struct CommandInfo {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub usage: &'static str,
    pub description: &'static str,
}

/// Default weather duration: 10 minutes, in ticks.
pub const DEFAULT_WEATHER_TICKS: i32 = 20 * 600;

pub const COMMANDS: &[CommandInfo] = &[
    CommandInfo {
        name: "help",
        aliases: &["?"],
        usage: "/help [página|comando]",
        description: "Muestra el menú de ayuda",
    },
    CommandInfo {
        name: "tp",
        aliases: &["teleport"],
        usage: "/tp [jugador objetivo] <jugador destino> O /tp [jugador objetivo] <x> <y> <z> [<y-rot> <x-rot>]",
        description: "Teletransporta a un jugador (O a ti mismo) a otro jugador o coordenadas",
    },
    CommandInfo {
        name: "gamemode",
        aliases: &["gm"],
        usage: "/gamemode <modo> [jugador]",
        description: "Cambia el modo de juego del jugador",
    },
    CommandInfo {
        name: "gmc",
        aliases: &[],
        usage: "/gmc [jugador]",
        description: "Atajo de /gamemode creative",
    },
    CommandInfo {
        name: "gms",
        aliases: &[],
        usage: "/gms [jugador]",
        description: "Atajo de /gamemode survival",
    },
    CommandInfo {
        name: "gma",
        aliases: &[],
        usage: "/gma [jugador]",
        description: "Atajo de /gamemode adventure",
    },
    CommandInfo {
        name: "gmsp",
        aliases: &[],
        usage: "/gmsp [jugador]",
        description: "Atajo de /gamemode spectator",
    },
    CommandInfo {
        name: "spawn",
        aliases: &[],
        usage: "/spawn [jugador]",
        description: "Teletransporta al punto de aparición del mundo",
    },
    CommandInfo {
        name: "setworldspawn",
        aliases: &[],
        usage: "/setworldspawn [<x> <y> <z>]",
        description: "Establece el punto de aparición de un mundo. Si no se especifican coordenadas, se utilizarán las coordenadas del jugador.",
    },
    CommandInfo {
        name: "say",
        aliases: &[],
        usage: "/say <mensaje ...>",
        description: "Transmitir el mensaje como servidor",
    },
    CommandInfo {
        name: "me",
        aliases: &[],
        usage: "/me <acción ...>",
        description: "Ejecuta una acción específica en el chat",
    },
    CommandInfo {
        name: "tell",
        aliases: &["msg", "w", "whisper"],
        usage: "/tell <jugador> <mensaje privado>",
        description: "Enviar mensaje privado a un jugador",
    },
    CommandInfo {
        name: "list",
        aliases: &["players"],
        usage: "/list",
        description: "Lista de todos los jugadores en línea",
    },
    CommandInfo {
        name: "kick",
        aliases: &[],
        usage: "/kick <jugador> [razón ...]",
        description: "Expulsa al jugador especificado del servidor",
    },
    CommandInfo {
        name: "give",
        aliases: &[],
        usage: "/give <jugador> <objeto[:extra]> [cantidad] [etiquetas...]",
        description: "Da al jugador especificado una cierta cantidad de objetos",
    },
    CommandInfo {
        name: "clear",
        aliases: &[],
        usage: "/clear [jugador] [objeto] [cantidadMáxima]",
        description: "Borra objetos del inventario del jugador",
    },
    CommandInfo {
        name: "kill",
        aliases: &["suicide"],
        usage: "/kill [jugador]",
        description: "Cometer un suicidio o matar a otros jugadores",
    },
    CommandInfo {
        name: "spawnpoint",
        aliases: &[],
        usage: "/spawnpoint [jugador] [<x> <y> <z>]",
        description: "Define un punto de aparición de los jugadores",
    },
    CommandInfo {
        name: "defaultgamemode",
        aliases: &[],
        usage: "/defaultgamemode <modo>",
        description: "Establece el modo de juego por defecto",
    },
    CommandInfo {
        name: "difficulty",
        aliases: &[],
        usage: "/difficulty <nueva dificultad>",
        description: "Definir dificultad de el juego",
    },
    CommandInfo {
        name: "seed",
        aliases: &[],
        usage: "/seed",
        description: "Muestra la semilla del mundo",
    },
    CommandInfo {
        name: "ban",
        aliases: &[],
        usage: "/ban <nombre> [razón ...]",
        description: "Previene que el jugador especificado utilice este servidor",
    },
    CommandInfo {
        name: "ban-ip",
        aliases: &[],
        usage: "/ban-ip <dirección|nombre> [Razón ...]",
        description: "Previene que la dirección IP especificada utilice este servidor",
    },
    CommandInfo {
        name: "banlist",
        aliases: &[],
        usage: "/banlist [ips|jugadores]",
        description: "Muestra todos los jugadores vetados de este servidor",
    },
    CommandInfo {
        name: "pardon",
        aliases: &["unban"],
        usage: "/pardon <nombre>",
        description: "Permite al jugador especificado utilizar este servidor",
    },
    CommandInfo {
        name: "pardon-ip",
        aliases: &["unban-ip"],
        usage: "/pardon-ip <dirección>",
        description: "Permite que la IP especificada utilice este servidor",
    },
    CommandInfo {
        name: "whitelist",
        aliases: &[],
        usage: "/whitelist <on|off|list|add|remove|reload>",
        description: "Administra la lista de jugadores permitidos en el servidor",
    },
    CommandInfo {
        name: "stop",
        aliases: &[],
        usage: "/stop",
        description: "Detiene el servidor",
    },
    CommandInfo {
        name: "plugins",
        aliases: &["pl"],
        usage: "/plugins",
        description: "Obtén una lista de plugins ejecutándose en el servidor",
    },
    CommandInfo {
        name: "save-all",
        aliases: &[],
        usage: "/save-all",
        description: "Guarda el servidor en el disco",
    },
    CommandInfo {
        name: "save-on",
        aliases: &[],
        usage: "/save-on",
        description: "Habilita el \"Auto-Guardado\" del servidor",
    },
    CommandInfo {
        name: "save-off",
        aliases: &[],
        usage: "/save-off",
        description: "Deshabilita el \"Auto-Guardado\" del servidor",
    },
    CommandInfo {
        name: "tps",
        aliases: &[],
        usage: "/tps",
        description: "Muestra los TPS del servidor en 1m, 5m y 15m",
    },
    CommandInfo {
        name: "time",
        aliases: &[],
        usage: "/time <set|add|query> [valor]",
        description: "Cambia la hora en cada mundo",
    },
    CommandInfo {
        name: "weather",
        aliases: &[],
        usage: "/weather <clear|rain|thunder> [duración]",
        description: "Cambia el clima del mundo",
    },
    CommandInfo {
        name: "op",
        aliases: &[],
        usage: "/op <jugador>",
        description: "Obsequia al jugador especificado el modo \"Operador\"",
    },
    CommandInfo {
        name: "deop",
        aliases: &[],
        usage: "/deop <jugador>",
        description: "Retira el modo \"operador\" del jugador especificado",
    },
    CommandInfo {
        name: "version",
        aliases: &["ver"],
        usage: "/version",
        description: "Obtén la versión del servidor, incluyendo los plugins que utiliza",
    },
    CommandInfo {
        name: "title",
        aliases: &[],
        usage: "/title <jugador> <clear|reset|title|subtitle|actionbar|times> ...",
        description: "Controla títulos en pantalla",
    },
    CommandInfo {
        name: "transferserver",
        aliases: &[],
        usage: "/transferserver <servidor> [puerto]",
        description: "Transferirte a otro servidor",
    },
];

/// Parses and evaluates a core command.
///
/// Returns `None` when the command is not native, so the caller can fall back
/// to plugin commands. The leading `/` is optional (the console omits it).
pub fn handle_command(
    line: &str,
    source: &CommandSource,
    players: &[OnlinePlayer],
) -> Option<CommandOutput> {
    let trimmed = line.trim();
    let without_slash = trimmed.strip_prefix('/').unwrap_or(trimmed);
    let mut parts = without_slash.split_whitespace();
    let name = parts.next()?.to_lowercase();
    let raw_args: Vec<&str> = parts.collect();
    // Selector `[...]` args may contain spaces (`@a[name=Alex, limit=2]`);
    // rejoin them like `CommandMap::dispatch` does.
    let merged = merge_selector_tokens(&raw_args);
    let args: Vec<&str> = merged.iter().map(String::as_str).collect();

    // PocketMine checks the command's permissions before running it. Commands
    // registered with `setPermissions([self, other])` accept *any* of the set
    // (`Command::testPermissionSilent`), and the handler then applies the node
    // that matches the resolved target.
    if let Some(nodes) = permission_nodes(&name) {
        if !nodes.iter().any(|node| source.can(node)) {
            return Some(CommandOutput {
                feedback: vec![red(lang::PERMISSION.replace("{%commandName}", &name))],
                actions: Vec::new(),
            });
        }
    }

    match name.as_str() {
        "help" | "?" => Some(help(source, &args)),
        "tp" | "teleport" => Some(teleport(source, players, &args)),
        "gamemode" | "gm" => {
            let Some(mode) = args.first() else {
                return Some(usage("gamemode"));
            };
            let Some(gamemode) = parse_gamemode(mode) else {
                return Some(feedback(
                    lang::GAMEMODE_UNKNOWN.replace("{%gameModeName}", mode),
                ));
            };
            Some(change_gamemode(source, players, gamemode, args.get(1).copied()))
        }
        "gmc" | "gms" | "gma" | "gmsp" => {
            let gamemode = match name.as_str() {
                "gmc" => 1,
                "gms" => 0,
                "gma" => 2,
                _ => 3,
            };
            Some(change_gamemode(source, players, gamemode, args.first().copied()))
        }
        "spawn" => Some(spawn(source, players, &args)),
        "setworldspawn" => Some(set_world_spawn(source, &args)),
        "say" => Some(say(source, &args)),
        "me" => Some(me(source, &args)),
        "msg" | "tell" | "w" | "whisper" => Some(whisper(source, players, &args)),
        "list" | "players" => Some(list(source, players)),
        "kick" => Some(kick(source, players, &args)),
        "give" => Some(give(source, players, &args)),
        "clear" => Some(clear(source, players, &args)),
        "kill" | "suicide" => Some(kill(source, players, &args)),
        "spawnpoint" => Some(spawnpoint(source, players, &args)),
        "defaultgamemode" => Some(defaultgamemode(source, &args)),
        "difficulty" => Some(difficulty(source, &args)),
        "seed" => Some(seed(source)),
        "ban" => Some(ban(source, players, &args)),
        "ban-ip" => Some(ban_ip(source, players, &args)),
        "banlist" => Some(banlist(source, &args)),
        "pardon" | "unban" => Some(pardon(source, &args)),
        "pardon-ip" | "unban-ip" => Some(pardon_ip(source, &args)),
        "whitelist" => Some(whitelist(source, &args)),
        "stop" => Some(stop(source)),
        "plugins" | "pl" => Some(plugins(source)),
        "save-all" => Some(save_all(source)),
        "save-on" => Some(set_auto_save(source, true)),
        "save-off" => Some(set_auto_save(source, false)),
        "tps" => Some(tps(source)),
        "time" => Some(time(source, &args)),
        "title" => Some(title(source, players, &args)),
        "transferserver" => Some(transfer(source, &args)),
        "weather" => Some(weather(source, &args)),
        "op" => Some(set_op(&args, true)),
        "deop" => Some(set_op(&args, false)),
        "version" | "ver" => Some(version()),
        _ => None,
    }
}

/// Lines a player's chat screen shows per page (`Player::getScreenLineHeight()`).
const PLAYER_SCREEN_LINE_HEIGHT: usize = 7;

/// PHP's `is_numeric()` followed by `(int)`, which `HelpCommand` uses to tell a
/// page number from the beginning of a command name.
fn php_int(value: &str) -> Option<i64> {
    let number: f64 = value.trim().parse().ok()?;
    if !number.is_finite() {
        return None;
    }
    Some(number.trunc() as i64)
}

/// `Command::testPermissionSilent()` while building the help listing.
fn command_visible(source: &CommandSource, name: &str) -> bool {
    if let Some(nodes) = permission_nodes(name) {
        return nodes.iter().any(|node| source.can(node));
    }
    let sub_nodes: &[&str] = match name {
        "time" => &[
            crate::permission::COMMAND_TIME_SET,
            crate::permission::COMMAND_TIME_ADD,
            crate::permission::COMMAND_TIME_QUERY,
        ],
        "whitelist" => &[
            crate::permission::COMMAND_WHITELIST_ADD,
            crate::permission::COMMAND_WHITELIST_REMOVE,
            crate::permission::COMMAND_WHITELIST_ENABLE,
            crate::permission::COMMAND_WHITELIST_DISABLE,
            crate::permission::COMMAND_WHITELIST_LIST,
            crate::permission::COMMAND_WHITELIST_RELOAD,
        ],
        // Extras without a PocketMine node (`/spawn`, `/tps`) stay visible.
        _ => return true,
    };
    sub_nodes.iter().any(|node| source.can(node))
}

/// `/help`, ported from `pocketmine\command\defaults\HelpCommand`.
fn help(source: &CommandSource, args: &[&str]) -> CommandOutput {
    let mut page_number: i64 = 1;
    let mut command_name = String::new();
    if let Some(last) = args.last() {
        match php_int(last) {
            Some(page) => {
                page_number = page.max(1);
                command_name = args[..args.len() - 1].join(" ");
            }
            None => command_name = args.join(" "),
        }
    }

    // The console reports `PHP_INT_MAX` lines, so it never paginates.
    let page_height = if source.is_console {
        usize::MAX
    } else {
        PLAYER_SCREEN_LINE_HEIGHT
    };

    if command_name.is_empty() {
        let mut commands: Vec<&'static CommandInfo> = COMMANDS
            .iter()
            .filter(|info| command_visible(source, info.name))
            .collect();
        // `ksort($commands, SORT_NATURAL | SORT_FLAG_CASE)`.
        commands.sort_by_key(|info| info.name.to_lowercase());

        let page_count = commands.len().div_ceil(page_height) as i64;
        let page = page_number.min(page_count).max(1) as usize;
        let mut lines = vec![fill_template(
            lang::HELP_HEADER,
            &[&page.to_string(), &page_count.to_string()],
        )];
        lines.extend(
            commands
                .iter()
                .skip((page - 1).saturating_mul(page_height))
                .take(page_height)
                .map(|info| format!("§2/{}: §r{}", info.name, info.description)),
        );
        return CommandOutput {
            feedback: lines,
            actions: Vec::new(),
        };
    }

    let lookup = command_name.to_lowercase();
    let Some(info) = find_command(&lookup).filter(|info| command_visible(source, info.name)) else {
        return error(not_found_message(&command_name));
    };

    let mut aliases: Vec<&str> = info.aliases.to_vec();
    aliases.sort_unstable(); // `sort($aliases, SORT_NATURAL)`.
    CommandOutput {
        feedback: vec![
            format!(
                "§e--------- §r{}§e ---------",
                fill_named(lang::HELP_SPECIFIC_HEADER, &[("commandName", info.name)])
            ),
            format!(
                "§6{}",
                fill_named(
                    lang::HELP_SPECIFIC_DESCRIPTION,
                    &[("description", &format!("§r{}", info.description))]
                )
            ),
            format!(
                "§6{}",
                fill_named(
                    lang::HELP_SPECIFIC_USAGE,
                    &[("usage", &format!("§r{}", info.usage.replace('\n', "\n§r")))]
                )
            ),
            format!(
                "§6{}",
                fill_named(
                    lang::HELP_SPECIFIC_ALIASES,
                    &[("aliasList", &format!("§r{}", aliases.join(", ")))]
                )
            ),
        ],
        actions: Vec::new(),
    }
}

fn teleport(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if !source.can_use_privileged() {
        return permission_error();
    }

    match args.len() {
        0 => usage("tp"),
        // /tp <jugador>: go to that player.
        1 => {
            if source.is_console {
                return usage("tp");
            }
            let Some(destination) = resolve_player(source, players, args[0]) else {
                return player_not_found(args[0]);
            };
            teleport_output(
                source,
                &source.name,
                destination.position,
                Some(&destination.name),
                None,
                None,
            )
        }
        // /tp <objetivo> <jugador>: objetivos `@a`/`@p`/`@e`/`@r`
        // teleportan a cada uno a la posición del destino.
        2 => {
            let targets = resolve_players(source, players, args[0]);
            if targets.is_empty() {
                return player_not_found(args[0]);
            }
            let Some(destination) = resolve_player(source, players, args[1]) else {
                return player_not_found(args[1]);
            };
            let target_names: Vec<String> =
                targets.iter().map(|player| player.name.clone()).collect();
            if let Some(denied) = teleport_permission_error(source, &target_names) {
                return denied;
            }
            let mut feedback = Vec::new();
            let mut actions = Vec::new();
            for target in targets {
                feedback.push(teleport_feedback(
                    source,
                    &target.name,
                    destination.position,
                    Some(&destination.name),
                    None,
                    None,
                ));
                actions.push(CommandAction::Teleport {
                    target: target.name.clone(),
                    position: destination.position,
                    yaw: None,
                    pitch: None,
                });
            }
            CommandOutput { feedback, actions }
        }
        // /tp <x> <y> <z> and /tp <objetivo> <x> <y> <z>
        // /tp <x> <y> <z> <yaw> <pitch> and /tp <objetivo> <x> <y> <z> <yaw> <pitch>
        3 | 4 | 5 | 6 => {
            // `len == 3 || len == 5` is the sender teleporting themselves.
            // Otherwise the first arg is the (possibly `@a`/`@e`/...) target.
            let (targets, coord_start): (Vec<(String, (f64, f64, f64))>, usize) =
                if args.len() == 3 || args.len() == 5 {
                    if source.is_console {
                        return usage("tp");
                    }
                    (vec![(source.name.clone(), source.position)], 0)
                } else {
                    let resolved = resolve_players(source, players, args[0]);
                    if resolved.is_empty() {
                        return player_not_found(args[0]);
                    }
                    (
                        resolved
                            .iter()
                            .map(|player| (player.name.clone(), player.position))
                            .collect(),
                        1,
                    )
                };

            let target_names: Vec<String> =
                targets.iter().map(|(name, _)| name.clone()).collect();
            if let Some(denied) = teleport_permission_error(source, &target_names) {
                return denied;
            }

            let (yaw, pitch) = if args.len() == 5 || args.len() == 6 {
                let yaw_value = args[coord_start + 3].parse::<f32>().ok();
                let pitch_value = args[coord_start + 4].parse::<f32>().ok();
                (yaw_value, pitch_value)
            } else {
                (None, None)
            };
            let coordinates = &args[coord_start..coord_start + 3];

            let mut feedback = Vec::new();
            let mut actions = Vec::new();
            for (target, origin) in &targets {
                let Some(position) = parse_position(coordinates, *origin) else {
                    return usage("tp");
                };
                feedback.push(teleport_feedback(source, target, position, None, yaw, pitch));
                actions.push(CommandAction::Teleport {
                    target: target.clone(),
                    position,
                    yaw,
                    pitch,
                });
            }
            CommandOutput { feedback, actions }
        }
        _ => usage("tp"),
    }
}

fn teleport_permission_error(
    source: &CommandSource,
    target_names: &[String],
) -> Option<CommandOutput> {
    let has_self = target_names
        .iter()
        .any(|name| name.eq_ignore_ascii_case(&source.name));
    let has_other = target_names
        .iter()
        .any(|name| !name.eq_ignore_ascii_case(&source.name));
    if (has_self && !source.can(crate::permission::COMMAND_TELEPORT_SELF))
        || (has_other && !source.can(crate::permission::COMMAND_TELEPORT_OTHER))
    {
        Some(permission_for("tp"))
    } else {
        None
    }
}

fn teleport_feedback(
    source: &CommandSource,
    target: &str,
    position: (f64, f64, f64),
    destination_name: Option<&str>,
    yaw: Option<f32>,
    pitch: Option<f32>,
) -> String {
    let (x, y, z) = position;
    if target.eq_ignore_ascii_case(&source.name) {
        match destination_name {
            Some(name) => format!("§aTeletransportado a §f{}.", name),
            None => match (yaw, pitch) {
                (Some(yaw), Some(pitch)) => format!(
                    "§aTeletransportado a §f{:.1}, {:.1}, {:.1} (§f{}° yaw, {}° pitch§a).",
                    x, y, z, yaw, pitch
                ),
                _ => format!("§aTeletransportado a §f{:.1}, {:.1}, {:.1}§a.", x, y, z),
            },
        }
    } else {
        match destination_name {
            Some(name) => format!("§aTeletransportando a §f{} §aa §f{}.", target, name),
            None => match (yaw, pitch) {
                (Some(yaw), Some(pitch)) => format!(
                    "§aTeletransportando a §f{} §aa §f{:.1}, {:.1}, {:.1} (§f{}° yaw, {}° pitch§a).",
                    target, x, y, z, yaw, pitch
                ),
                _ => format!(
                    "§aTeletransportando a §f{} §aa §f{:.1}, {:.1}, {:.1}§a.",
                    target, x, y, z
                ),
            },
        }
    }
}

fn teleport_output(
    source: &CommandSource,
    target: &str,
    position: (f64, f64, f64),
    destination_name: Option<&str>,
    yaw: Option<f32>,
    pitch: Option<f32>,
) -> CommandOutput {
    let target_names = vec![target.to_string()];
    if let Some(denied) = teleport_permission_error(source, &target_names) {
        return denied;
    }
    let feedback = teleport_feedback(source, target, position, destination_name, yaw, pitch);

    action(
        feedback,
        CommandAction::Teleport {
            target: target.to_string(),
            position,
            yaw,
            pitch,
        },
    )
}

fn change_gamemode(
    source: &CommandSource,
    players: &[OnlinePlayer],
    gamemode: i32,
    target_arg: Option<&str>,
) -> CommandOutput {
    // Selector-aware target list (`@a`, `@p`, `@e`, `@r`, ...). Silently
    // resolves through `resolve_players` then applies the same self/other
    // split as the single-target path.
    let targets: Vec<String> = match target_arg {
        Some(query) => {
            let resolved = resolve_players(source, players, query);
            if resolved.is_empty() {
                return player_not_found(query);
            }
            resolved.iter().map(|player| player.name.clone()).collect()
        }
        None => {
            if source.is_console {
                return error("§cDebes especificar un jugador.");
            }
            vec![source.name.clone()]
        }
    };

    // `fetchPermittedPlayerTarget` permission split over the whole set: any
    // `other` target needs `.other`, any self target needs `.self`.
    let has_self = targets
        .iter()
        .any(|name| name.eq_ignore_ascii_case(&source.name));
    let has_other = targets
        .iter()
        .any(|name| !name.eq_ignore_ascii_case(&source.name));
    if (has_self && !source.can(crate::permission::COMMAND_GAMEMODE_SELF))
        || (has_other && !source.can(crate::permission::COMMAND_GAMEMODE_OTHER))
    {
        return permission_for("gamemode");
    }

    let mode_name = gamemode_translatable(gamemode);
    let mut feedback_lines = Vec::new();
    let mut actions = Vec::new();
    for target in targets {
        // PocketMine reports a failure when the target already has that mode.
        let current = players
            .iter()
            .find(|player| player.name.eq_ignore_ascii_case(&target))
            .map(|player| player.gamemode);
        if current == Some(gamemode) {
            feedback_lines.push(lang::GAMEMODE_FAILURE.replace("{%playerName}", &target));
            continue;
        }
        let success = if target.eq_ignore_ascii_case(&source.name) {
            fill_template(lang::GAMEMODE_SUCCESS_SELF, &[mode_name])
        } else {
            fill_template(lang::GAMEMODE_SUCCESS_OTHER, &[mode_name, &target])
        };
        let message = success.clone();
        feedback_lines.push(success);
        actions.push(CommandAction::SetGamemode {
            target,
            gamemode,
            message,
            notify: source.name.clone(),
        });
    }
    if actions.is_empty() && feedback_lines.is_empty() {
        return feedback(lang::GAMEMODE_FAILURE.to_string());
    }
    CommandOutput {
        feedback: feedback_lines,
        actions,
    }
}

fn spawn(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    let targets: Vec<String> = match args.first() {
        Some(query) => {
            if !source.can_use_privileged() {
                return permission_error();
            }
            let resolved = resolve_players(source, players, query);
            if resolved.is_empty() {
                return player_not_found(query);
            }
            resolved
                .iter()
                .map(|player| player.name.clone())
                .collect()
        }
        None => {
            if source.is_console {
                return error("§cDebes especificar un jugador.");
            }
            vec![source.name.clone()]
        }
    };

    let mut feedback = Vec::new();
    let mut actions = Vec::new();
    for target in targets {
        feedback.push(format!(
            "§a{} al punto de aparición del mundo.",
            if target.eq_ignore_ascii_case(&source.name) {
                "Teletransportado"
            } else {
                "Jugador teletransportado"
            }
        ));
        actions.push(CommandAction::TeleportToSpawn { target });
    }
    CommandOutput { feedback, actions }
}

fn set_world_spawn(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if !source.can_use_privileged() {
        return permission_error();
    }

    let position = if args.is_empty() {
        if source.is_console {
            return usage("setworldspawn");
        }
        (
            source.position.0.floor() as i32,
            source.position.1.floor() as i32,
            source.position.2.floor() as i32,
        )
    } else if args.len() >= 3 {
        let Some(position) = parse_position(&args[0..3], source.position) else {
            return usage("setworldspawn");
        };
        (
            position.0.floor() as i32,
            position.1.floor() as i32,
            position.2.floor() as i32,
        )
    } else {
        return usage("setworldspawn");
    };

    action(
        format!(
            "§aPunto de aparición del mundo fijado en §f{}, {}, {}§a.",
            position.0, position.1, position.2
        ),
        CommandAction::SetWorldSpawn {
            x: position.0,
            y: position.1,
            z: position.2,
        },
    )
}

fn say(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if args.is_empty() {
        return usage("say");
    }

    // `SayCommand`: `chat.type.announcement` to everybody, no ack to the sender.
    CommandOutput {
        feedback: Vec::new(),
        actions: vec![CommandAction::Broadcast {
            message: fill_template(lang::CHAT_ANNOUNCEMENT, &[&source.name, &args.join(" ")]),
        }],
    }
}

fn me(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if args.is_empty() {
        return usage("me");
    }

    // `MeCommand`: `chat.type.emote` to everybody, no ack to the sender.
    CommandOutput {
        feedback: Vec::new(),
        actions: vec![CommandAction::Broadcast {
            message: fill_template(lang::CHAT_EMOTE, &[&source.name, &args.join(" ")]),
        }],
    }
}

fn whisper(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() < 2 {
        return usage("msg");
    }

    let Some(target) = resolve_player(source, players, args[0]) else {
        return player_not_found(args[0]);
    };
    if target.name.eq_ignore_ascii_case(&source.name) {
        return feedback(lang::MESSAGE_SAME_TARGET.to_string());
    }
    let text = args[1..].join(" ");

    CommandOutput {
        feedback: vec![format!(
            "§7{}",
            fill_template(lang::MESSAGE_OUTGOING, &[&target.name, &text])
        )],
        actions: vec![CommandAction::Message {
            target: target.name.clone(),
            message: format!(
                "§7{}",
                fill_template(lang::MESSAGE_INCOMING, &[&source.name, &text])
            ),
        }],
    }
}

/// `/list`, ported from `pocketmine\command\defaults\ListCommand`: the count
/// line and then the names sorted with `SORT_STRING`, as two messages.
fn list(source: &CommandSource, players: &[OnlinePlayer]) -> CommandOutput {
    let mut names: Vec<&str> = players.iter().map(|player| player.name.as_str()).collect();
    names.sort_unstable();

    CommandOutput {
        feedback: vec![
            fill_template(
                lang::PLAYERS_LIST,
                &[&names.len().to_string(), &source.max_players.to_string()],
            ),
            names.join(", "),
        ],
        actions: Vec::new(),
    }
}

fn kick(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    let Some(name) = args.first() else {
        return usage("kick");
    };
    let Some(target) = resolve_player(source, players, name) else {
        return player_not_found(name);
    };

    let reason = if args.len() > 1 {
        Some(args[1..].join(" "))
    } else {
        None
    };
    let message = match &reason {
        Some(reason) => fill_template(lang::KICK_SUCCESS_REASON, &[&target.name, reason]),
        None => fill_template(lang::KICK_SUCCESS, &[&target.name]),
    };

    action(
        message,
        CommandAction::Kick {
            target: target.name.clone(),
            reason: reason.unwrap_or_default(),
        },
    )
}

/// Vanilla translation strings used by `/give`, `/clear` and `/kill`, copied
/// verbatim from PocketMine-MP's `spa.ini` so the output matches the PHP server
/// key for key.
pub mod lang {
    pub const USAGE: &str = "Uso: {%0}";
    pub const PERMISSION: &str = "No tienes permiso para ejecutar el comando: {%commandName}";
    pub const PLAYER_NOT_FOUND: &str =
        "No se pudo encontrar un jugador que coincida con \"{%playerName}\"";
    pub const GIVE_ITEM_NOT_FOUND: &str = "No existe tal objeto con el nombre {%0}";
    pub const GIVE_SUCCESS: &str = "Dado {%0} * {%1} a {%2}";
    pub const GIVE_TAG_ERROR: &str = "Error al analizar la etiqueta de datos: {%0}";
    pub const SPAWNPOINT_SUCCESS: &str =
        "Punto de aparición de {%0} establecido en ({%1}, {%2}, {%3})";
    pub const DEFAULTGAMEMODE_SUCCESS: &str =
        "El modo de juego por defecto del mundo es ahora {%0}";
    pub const DIFFICULTY_SUCCESS: &str = "Cambiada la dificultad del juego a {%0}";
    pub const SEED_SUCCESS: &str = "Semilla: {%0}";
    pub const GAMEMODE_UNKNOWN: &str = "Modo de juego desconocido \"{%gameModeName}\"";
    pub const GAMEMODE_FAILURE: &str = "¡Error al cambiar el modo de juego de {%playerName}!";
    pub const GAMEMODE_SUCCESS_SELF: &str = "Modo de juego propio establecido a {%0}";
    pub const GAMEMODE_SUCCESS_OTHER: &str = "Se ha cambiado el modo de juego de {%1} a {%0}";
    pub const GAMEMODE_CHANGED: &str = "Tu modo de juego ha sido actualizado a {%0}";
    pub const BAN_SUCCESS: &str = "Jugador {%0} baneado";
    pub const BAN_USAGE: &str = "/ban <nombre> [razón ...]";
    pub const BANIP_INVALID: &str =
        "Has ingresado una dirección IP inválida o el jugador no está en línea";
    pub const BANIP_SUCCESS: &str = "IP {%0} bloqueada";
    pub const BANIP_SUCCESS_PLAYERS: &str = "Se ha bloqueado la IP {%0} perteneciente a {%1}";
    pub const BANIP_USAGE: &str = "/ban-ip <dirección|nombre> [Razón ...]";
    pub const BANLIST_IPS: &str = "Hay {%0} IPs bloqueadas:";
    pub const BANLIST_PLAYERS: &str = "Hay un total de {%0} jugadores baneados:";
    pub const BANLIST_USAGE: &str = "/banlist [ips|jugadores]";
    pub const UNBAN_SUCCESS: &str = "Levantado castigo de jugador {%0}";
    pub const UNBAN_USAGE: &str = "/pardon <nombre>";
    pub const UNBANIP_SUCCESS: &str = "La IP {%0} ha sido desbaneada";
    pub const UNBANIP_INVALID: &str = "Has ingresado una dirección IP no válida";
    pub const UNBANIP_USAGE: &str = "/pardon-ip <dirección>";
    pub const WHITELIST_ADD_SUCCESS: &str = "Se ha añadido a {%0} a la lista blanca";
    pub const WHITELIST_ADD_USAGE: &str = "/whitelist add <jugador>";
    pub const WHITELIST_DISABLED: &str = "Lista blanca apagada";
    pub const WHITELIST_ENABLED: &str = "Lista blanca encendida";
    pub const WHITELIST_LIST: &str =
        "Hay {%0} (de cada {%1} vistos) jugadores en la lista blanca:";
    pub const WHITELIST_RELOADED: &str = "Lista blanca recargada";
    pub const WHITELIST_REMOVE_SUCCESS: &str = "Se ha removido a {%0} de la lista blanca";
    pub const WHITELIST_REMOVE_USAGE: &str = "/whitelist remove <jugador>";
    pub const WHITELIST_USAGE: &str = "/whitelist <on|off|list|add|remove|reload>";
    pub const STOP_START: &str = "Deteniendo el servidor";
    pub const DISCONNECT_BAN: &str = "Estás baneado de este servidor. Motivo: {%reason}";
    pub const DISCONNECT_BAN_NOREASON: &str = "Estás baneado de este servidor";
    pub const DISCONNECT_BAN_IP: &str = "Baneado de IP";
    pub const DISCONNECT_WHITELISTED: &str = "El servidor está en modo lista blanca";
    pub const DISCONNECT_KICK: &str = "Expulsado por un administrador. Motivo: {%reason}";
    pub const PLUGINS_SUCCESS: &str = "Plugins ({%0}): {%1}";
    pub const SAVE_START: &str = "Guardando datos del servidor...";
    pub const SAVE_SUCCESS: &str = "Se completó el guardado en {%0} segundos";
    pub const SAVE_ENABLED: &str = "El autoguardado del mundo ha sido activado";
    pub const SAVE_DISABLED: &str = "El autoguardado del mundo ha sido desactivado";
    pub const CLEAR_SUCCESS: &str = "Se vació el inventario de {%0}, {%1} objetos eliminados";
    pub const CLEAR_FAILURE_NO_ITEMS: &str =
        "No se ha podido vaciar el inventario de {%0}: no tiene objetos para eliminar";
    pub const CLEAR_TESTING: &str = "{%0} tiene {%1} objetos que coinciden con los criterios";
    pub const KILL_SUCCESSFUL: &str = "{%0} Murió";
    pub const NUM_TOO_BIG: &str =
        "El número que ha introducido ({%0}) es demasiado grande, debe ser como mucho {%1}";
    pub const NUM_TOO_SMALL: &str =
        "El número que ha introducido ({%0}) es demasiado pequeño, debe ser como mínimo {%1}";
    pub const CHAT_ANNOUNCEMENT: &str = "[{%0}] {%1}";
    pub const CHAT_ADMIN: &str = "[{%0}: {%1}]";
    pub const CHAT_EMOTE: &str = "* {%0} {%1}";
    pub const MESSAGE_OUTGOING: &str = "Le susurras a {%0}: {%1}";
    pub const MESSAGE_INCOMING: &str = "{%0} te susurra: {%1}";
    pub const MESSAGE_SAME_TARGET: &str = "¡No puedes enviarte un mensaje privado a ti mismo!";
    pub const KICK_SUCCESS: &str = "{%0} ha sido expulsado del juego";
    pub const KICK_SUCCESS_REASON: &str = "{%0} ha sido expulsado del juego: '{%1}'";
    pub const TIME_SET: &str = "Establecer el tiempo a {%0}";
    pub const TIME_ADDED: &str = "{%0} añadido a la hora";
    pub const TIME_QUERY: &str = "El Tiempo es {%0}";
    pub const TITLE_SUCCESS: &str = "El comando title se ejecutó correctamente";
    pub const PLAYER_USER_ONLY: &str =
        "Esta sintaxis de comando solo puede ser usada como jugador";
    pub const VERSION_SOFTWARE_NAME: &str = "Este servidor está ejecutando {%serverSoftwareName}";
    pub const VERSION_SOFTWARE_VERSION: &str =
        "Versión del servidor: {%serverSoftwareVersion} (git hash: {%serverGitHash})";
    pub const VERSION_MINECRAFT: &str =
        "Versión compatible de Minecraft: {%minecraftVersion} (versión de protocolo: {%minecraftProtocolVersion})";
    pub const PLAYERS_LIST: &str = "Hay {%0}/{%1} jugadores en línea:";
    pub const HELP_HEADER: &str =
        "--- Mostrando página de ayuda {%0} de {%1} (/help [página]) ---";
    pub const HELP_SPECIFIC_HEADER: &str = "Ayuda: /{%commandName}";
    pub const HELP_SPECIFIC_DESCRIPTION: &str = "Descripción: {%description}";
    pub const HELP_SPECIFIC_USAGE: &str = "Uso: {%usage}";
    pub const HELP_SPECIFIC_ALIASES: &str = "Alias: {%aliasList}";
    pub const COMMAND_NOT_FOUND: &str = "Comando desconocido: {%commandName}. Utilice {%helpCommand} para obtener una lista de comandos disponibles.";
}

/// Replaces `{%0}`, `{%1}`, ... in a vanilla translation string.
pub(crate) fn fill_template(template: &str, args: &[&str]) -> String {
    crate::translations::fill_template(template, args)
}

/// Replaces named placeholders such as `{%commandName}` in a translation.
pub(crate) fn fill_named(template: &str, args: &[(&str, &str)]) -> String {
    crate::translations::fill_named(template, args)
}

/// Vanilla prefixes error output with red.
pub(crate) fn red(message: impl AsRef<str>) -> String {
    format!("§c{}", message.as_ref())
}

/// `pocketmine.command.notFound` for an unknown command, prefixed red like
/// `SimpleCommandMap::dispatch()` and `HelpCommand`.
pub fn not_found_message(command_name: &str) -> String {
    red(fill_named(
        lang::COMMAND_NOT_FOUND,
        &[("commandName", command_name), ("helpCommand", "/help")],
    ))
}

enum BoundedIntError {
    Syntax,
    TooBig,
    TooSmall,
}

/// `VanillaCommand::getBoundedInt`: non-numeric input is a syntax error, while
/// out-of-range values are reported with the vanilla translation keys.
fn bounded_int(input: &str, min: i64, max: i64) -> Result<i64, BoundedIntError> {
    let Ok(value) = input.trim().parse::<f64>() else {
        return Err(BoundedIntError::Syntax);
    };
    if !value.is_finite() {
        return Err(BoundedIntError::Syntax);
    }
    let value = value as i64;
    if value > max {
        return Err(BoundedIntError::TooBig);
    }
    if value < min {
        return Err(BoundedIntError::TooSmall);
    }
    Ok(value)
}

/// PHP's `(int)` cast: optional sign followed by digits, anything else is
/// ignored (`"3abc"` is `3`, `"abc"` is `0`).
fn php_int_cast(input: &str) -> i64 {
    let trimmed = input.trim_start();
    let (sign, rest) = match trimmed.strip_prefix('-') {
        Some(rest) => (-1i64, rest),
        None => (1i64, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return 0;
    }
    digits.parse::<i64>().unwrap_or(0).saturating_mul(sign)
}

/// `Server::getPlayerByPrefix`: an exact name wins, otherwise the first
/// case-insensitive prefix match.
#[allow(dead_code)]
fn player_by_prefix<'a>(players: &'a [OnlinePlayer], name: &str) -> Option<&'a OnlinePlayer> {
    let needle = name.to_lowercase();
    let mut prefixed: Option<&OnlinePlayer> = None;
    for player in players {
        let candidate = player.name.to_lowercase();
        if candidate == needle {
            return Some(player);
        }
        if prefixed.is_none() && candidate.starts_with(&needle) {
            prefixed = Some(player);
        }
    }
    prefixed
}

/// `VanillaCommand::fetchPermittedPlayerTarget`: `@s` (or no argument) resolves
/// to the sender, and the self/other permission split is done by the caller.
/// `@` selectors (other than `@s`) resolve through
/// [`resolve_selector_targets`] and return the first match; for every match
/// use [`resolve_players`] directly.
fn fetch_permitted_player_target<'a>(
    source: &CommandSource,
    players: &'a [OnlinePlayer],
    target: Option<&str>,
    command: &str,
    self_allowed: bool,
    other_allowed: bool,
) -> Result<Option<&'a OnlinePlayer>, CommandOutput> {
    let player = match target {
        Some(name) if !name.eq_ignore_ascii_case("@s") => match resolve_player(source, players, name) {
            Some(player) => player,
            None => {
                return Err(error(red(fill_template(lang::PLAYER_NOT_FOUND, &[name]))));
            }
        },
        _ => {
            if source.is_console {
                // PocketMine throws InvalidCommandSyntaxException here.
                return Err(usage(command));
            }
            match players
                .iter()
                .find(|player| player.name.eq_ignore_ascii_case(&source.name))
            {
                Some(player) => player,
                None => {
                    return Err(error(red(fill_template(
                        lang::PLAYER_NOT_FOUND,
                        &[&source.name],
                    ))));
                }
            }
        }
    };

    let is_self = player.name.eq_ignore_ascii_case(&source.name);
    let allowed = if is_self { self_allowed } else { other_allowed };
    if !allowed {
        let message = lang::PERMISSION.replace("{%commandName}", command);
        return Err(error(red(message)));
    }
    Ok(Some(player))
}

/// `/give <player> <item[:damage]> [amount] [tags...]`, ported one to one from
/// `pocketmine\command\defaults\GiveCommand`.
fn give(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() < 2 {
        return usage("give");
    }

    let self_allowed = source.can(crate::permission::COMMAND_GIVE_SELF);
    let other_allowed = source.can(crate::permission::COMMAND_GIVE_OTHER);
    let target = match fetch_permitted_player_target(
        source,
        players,
        Some(args[0]),
        "give",
        self_allowed,
        other_allowed,
    ) {
        Ok(Some(player)) => player.name.clone(),
        Ok(None) => source.name.clone(),
        Err(output) => return output,
    };

    let Some((network_id, meta, _)) = crate::inventory::resolve_item(args[1]) else {
        return error(red(fill_template(lang::GIVE_ITEM_NOT_FOUND, &[args[1]])));
    };

    let count = match args.get(2) {
        None => crate::inventory::max_stack_size(network_id),
        Some(value) => match bounded_int(value, 1, 32_767) {
            Ok(count) => count as u16,
            Err(BoundedIntError::Syntax) => return usage("give"),
            Err(BoundedIntError::TooBig) => {
                return error(red(fill_template(lang::NUM_TOO_BIG, &[value, "32767"])));
            }
            Err(BoundedIntError::TooSmall) => {
                return error(red(fill_template(lang::NUM_TOO_SMALL, &[value, "1"])));
            }
        },
    };

    let mut nbt = Vec::new();
    if args.len() > 3 {
        // `JsonNbtParser::parseJson(implode(" ", array_slice($args, 3)))`.
        let data = args[3..].join(" ");
        match crate::nbt_json::parse(&data) {
            Ok(tag) => nbt = bedrock_protocol::nbt::write_network_nbt(&tag),
            // `commands.give.tagError` carries the parser message and is sent
            // without the red prefix.
            Err(reason) => return error(fill_template(lang::GIVE_TAG_ERROR, &[&reason])),
        }
    }

    let display = crate::inventory::item_display_name(network_id).unwrap_or_else(|| args[1].into());
    let label = format!("{} ({})", display, args[1]);
    let message = fill_template(lang::GIVE_SUCCESS, &[&label, &count.to_string(), &target]);

    CommandOutput {
        feedback: vec![message.clone()],
        actions: vec![CommandAction::Give {
            target,
            network_id,
            meta,
            count,
            nbt,
            message,
            notify: source.name.clone(),
        }],
    }
}

/// `/clear [player] [item] [maxCount]`, ported one to one from
/// `pocketmine\command\defaults\ClearCommand`.
fn clear(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() > 3 {
        return usage("clear");
    }

    // `clear.self` is granted to everyone, `clear.other` to operators.
    let target = match fetch_permitted_player_target(
        source,
        players,
        args.first().copied(),
        "clear",
        source.can(crate::permission::COMMAND_CLEAR_SELF),
        source.can(crate::permission::COMMAND_CLEAR_OTHER),
    ) {
        Ok(Some(player)) => player.name.clone(),
        Ok(None) => source.name.clone(),
        Err(output) => return output,
    };

    let mut item = None;
    let mut max_count = -1i64;
    if let Some(raw) = args.get(1) {
        let Some((network_id, meta, _)) = crate::inventory::resolve_item(raw) else {
            return error(red(fill_template(lang::GIVE_ITEM_NOT_FOUND, &[raw])));
        };
        item = Some((network_id, meta));
        if let Some(value) = args.get(2) {
            // `getInteger($sender, $value, -1)` clamps instead of erroring.
            max_count = php_int_cast(value).clamp(-1, 30_000_000);
        }
    }

    // The amount removed is only known once the inventory is touched, so the
    // result line is produced by the session manager.
    CommandOutput {
        feedback: Vec::new(),
        actions: vec![CommandAction::ClearInventory {
            target,
            item,
            max_count: max_count as i32,
            notify: source.name.clone(),
        }],
    }
}

/// `/kill [player]`, ported one to one from
/// `pocketmine\command\defaults\KillCommand`.
fn kill(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() >= 2 {
        return usage("kill");
    }

    // `kill.self` is granted to everyone, `kill.other` to operators.
    let target = match fetch_permitted_player_target(
        source,
        players,
        args.first().copied(),
        "kill",
        source.can(crate::permission::COMMAND_KILL_SELF),
        source.can(crate::permission::COMMAND_KILL_OTHER),
    ) {
        Ok(Some(player)) => player.name.clone(),
        Ok(None) => source.name.clone(),
        Err(output) => return output,
    };

    let message = fill_template(lang::KILL_SUCCESSFUL, &[&target]);
    let broadcast = !target.eq_ignore_ascii_case(&source.name);
    CommandOutput {
        feedback: vec![message.clone()],
        actions: vec![CommandAction::Kill {
            target,
            message,
            broadcast,
            notify: source.name.clone(),
        }],
    }
}

fn time(source: &CommandSource, args: &[&str]) -> CommandOutput {
    let sub = args.first().map(|arg| arg.to_lowercase());

    match sub.as_deref() {
        // `TimeCommand` gates every subcommand with its own node.
        Some("set") | Some("add") => {
            let node = if sub.as_deref() == Some("set") {
                crate::permission::COMMAND_TIME_SET
            } else {
                crate::permission::COMMAND_TIME_ADD
            };
            if !source.can(node) {
                return permission_for("time");
            }
            let Some(value) = args.get(1) else {
                return usage("time");
            };
            let Some(ticks) = parse_time_value(value) else {
                return usage("time");
            };
            if sub.as_deref() == Some("set") {
                action(
                    fill_template(lang::TIME_SET, &[&ticks.to_string()]),
                    CommandAction::SetTime(ticks),
                )
            } else {
                action(
                    fill_template(lang::TIME_ADDED, &[&ticks.to_string()]),
                    CommandAction::AddTime(ticks),
                )
            }
        }
        Some("query") | None => {
            if !source.can(crate::permission::COMMAND_TIME_QUERY) {
                return permission_for("time");
            }
            feedback(fill_template(
                lang::TIME_QUERY,
                &[&source.world_time.to_string()],
            ))
        }
        Some(_) => usage("time"),
    }
}

fn weather(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if !source.can_use_privileged() {
        return permission_error();
    }
    let Some(kind) = args.first() else {
        return usage("weather");
    };

    let state = match kind.to_lowercase().as_str() {
        "clear" | "despejado" | "sun" => WeatherState::Clear,
        "rain" | "lluvia" => WeatherState::Rain,
        "thunder" | "tormenta" => WeatherState::Thunder,
        _ => return usage("weather"),
    };

    // Optional duration in seconds, converted to ticks.
    let duration_ticks = match args.get(1) {
        Some(value) => match value.parse::<f64>() {
            Ok(seconds) if seconds.is_finite() && seconds >= 0.0 => (seconds * 20.0) as i32,
            _ => return usage("weather"),
        },
        None => DEFAULT_WEATHER_TICKS,
    };

    action(
        format!("§aClima cambiado a §f{}§a.", state.name_es()),
        CommandAction::SetWeather {
            state,
            duration_ticks,
        },
    )
}

fn set_op(args: &[&str], op: bool) -> CommandOutput {
    let Some(name) = args.first() else {
        return usage(if op { "op" } else { "deop" });
    };
    let name = name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("CONSOLE") {
        return error("§cNombre de jugador no válido.");
    }

    action(
        if op {
            format!("§a{} ahora es operador.", name)
        } else {
            format!("§a{} ya no es operador.", name)
        },
        CommandAction::SetOp {
            target: name.to_string(),
            op,
        },
    )
}

fn version() -> CommandOutput {
    // `VersionCommand` prints the software, its version and the MCPE version.
    CommandOutput {
        feedback: vec![
            lang::VERSION_SOFTWARE_NAME.replace("{%serverSoftwareName}", "PocketMine-Rust"),
            lang::VERSION_SOFTWARE_VERSION
                .replace("{%serverSoftwareVersion}", concat!("v", env!("CARGO_PKG_VERSION")))
                .replace("{%serverGitHash}", "unknown"),
            lang::VERSION_MINECRAFT
                .replace("{%minecraftVersion}", "1.26.60")
                .replace("{%minecraftProtocolVersion}", "2225"),
        ],
        actions: Vec::new(),
    }
}

fn find_command(name: &str) -> Option<&'static CommandInfo> {
    COMMANDS
        .iter()
        .find(|info| info.name == name || info.aliases.contains(&name))
}

fn feedback(message: impl Into<String>) -> CommandOutput {
    CommandOutput {
        feedback: vec![message.into()],
        actions: Vec::new(),
    }
}

fn error(message: impl Into<String>) -> CommandOutput {
    feedback(message)
}

fn action(message: impl Into<String>, action: CommandAction) -> CommandOutput {
    let message = message.into();
    CommandOutput {
        feedback: if message.is_empty() {
            Vec::new()
        } else {
            vec![message]
        },
        actions: vec![action],
    }
}

fn usage(name: &str) -> CommandOutput {
    match find_command(name) {
        Some(info) => error(format!("§cUso: {}", info.usage)),
        None => error("§cUso incorrecto."),
    }
}

fn permission_error() -> CommandOutput {
    error("§cNo tienes permiso para usar este comando.")
}

fn player_not_found(name: &str) -> CommandOutput {
    error(format!("§cJugador no encontrado: §f{}§c.", name))
}

fn parse_gamemode(value: &str) -> Option<i32> {
    match value.to_lowercase().as_str() {
        "0" | "s" | "survival" | "supervivencia" => Some(0),
        "1" | "c" | "creative" | "creativo" => Some(1),
        "2" | "a" | "adventure" | "aventura" => Some(2),
        "3" | "sp" | "spectator" | "espectador" => Some(3),
        _ => None,
    }
}

/// Parses `day`, `noon`, ... or a raw tick count.
fn parse_time_value(value: &str) -> Option<i32> {
    let named = match value.to_lowercase().as_str() {
        "day" | "dia" | "día" => Some(1_000),
        "noon" | "mediodia" | "mediodía" => Some(6_000),
        "sunset" | "atardecer" => Some(12_000),
        "night" | "noche" => Some(13_000),
        "midnight" | "medianoche" => Some(18_000),
        "sunrise" | "amanecer" => Some(23_000),
        _ => None,
    };
    match named {
        Some(ticks) => Some(ticks),
        None => value.parse::<i32>().ok().map(|ticks| ticks.rem_euclid(24_000)),
    }
}

/// Parses a coordinate, honoring Bedrock's `~` (relative) syntax.
/// Resolves a coordinate token. Supports absolute values, relative `~offset`
/// (added to origin) and local `^offset` (added to origin, same as `~` for
/// single-axis resolution; PocketMine distinguishes ^ for local-relative but
/// in our flat command context both behave as origin + offset).
fn parse_coordinate(value: &str, origin: f64) -> Option<f64> {
    let parsed = match value.strip_prefix('~') {
        Some("") => origin,
        Some(offset) => origin + offset.parse::<f64>().ok()?,
        None => match value.strip_prefix('^') {
            Some("") => origin,
            Some(offset) => origin + offset.parse::<f64>().ok()?,
            None => value.parse::<f64>().ok()?,
        },
    };
    parsed.is_finite().then_some(parsed)
}

fn parse_position(tokens: &[&str], origin: (f64, f64, f64)) -> Option<(f64, f64, f64)> {
    if tokens.len() != 3 {
        return None;
    }
    Some((
        parse_coordinate(tokens[0], origin.0)?,
        parse_coordinate(tokens[1], origin.1)?,
        parse_coordinate(tokens[2], origin.2)?,
    ))
}

/// Merges whitespace-split tokens that belong to a single selector `[...]`.
/// `handle_command` splits on whitespace for the command name/args, so a
/// selector written `@a[distance=1..5, name=Steve]` would split at the space
/// after the comma. This joins tokens until brackets balance again.
/// It also keeps JSON/NBT bracket pairs together; both callers rejoin with
/// spaces anyway (`/give` tags, `/title` text), so merging is safe there.
fn merge_selector_tokens(raw: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut buffer: Option<String> = None;
    let mut depth: i32 = 0;
    for token in raw {
        let delta = token.matches('[').count() as i32 - token.matches(']').count() as i32;
        match &mut buffer {
            None => {
                depth = delta;
                if depth <= 0 {
                    out.push((*token).to_string());
                } else {
                    buffer = Some((*token).to_string());
                }
            }
            Some(current) => {
                current.push(' ');
                current.push_str(token);
                depth += delta;
                if depth <= 0 {
                    out.push(buffer.take().expect("buffer present"));
                    depth = 0;
                }
            }
        }
    }
    if let Some(current) = buffer {
        out.push(current);
    }
    out
}

/// Sort order for selector results (`sort=nearest|furthest|arbitrary|random`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorSort {
    Nearest,
    Furthest,
    Arbitrary,
    Random,
}

/// Parsed `[...]` arguments shared by every `@` selector.
#[derive(Debug, Clone, Default)]
struct SelectorArgs {
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    distance_min: Option<f64>,
    distance_max: Option<f64>,
    limit: Option<usize>,
    sort: Option<SelectorSort>,
    name: Option<String>,
    type_: Option<String>,
    gamemode: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorKind {
    Self_,
    Nearest,
    AllPlayers,
    AllEntities,
    Random,
}

/// `@s`, `@p`, `@a`, `@e`, `@r` plus the `@n`/`@c` aliases. `@n` is nearest
/// (alias of `@p`); `@c` is a legacy alias for all players (`@a`). Neither
/// is vanilla Bedrock nor PocketMine vanilla, but both appear in forks and
/// are kept as aliases rather than silently failing.
fn is_selector(query: &str) -> bool {
    query.starts_with('@')
}

/// Parses `distance=` values: `5`, `..5`, `5..`, `1..5`.
fn parse_distance_range(value: &str) -> Option<(Option<f64>, Option<f64>)> {
    let value = value.trim();
    if let Some((min, max)) = value.split_once("..") {
        let min = min.trim();
        let max = max.trim();
        let min_value = if min.is_empty() {
            None
        } else {
            Some(min.parse::<f64>().ok()?)
        };
        let max_value = if max.is_empty() {
            None
        } else {
            Some(max.parse::<f64>().ok()?)
        };
        if min_value.is_none() && max_value.is_none() {
            return None;
        }
        Some((min_value, max_value))
    } else {
        let single = value.parse::<f64>().ok()?;
        Some((None, Some(single)))
    }
}

/// Parses a `@` selector. Returns `None` for an unknown selector letter or
/// a malformed `[...]` block, which callers report as
/// `commands.generic.player.notFound`.
fn parse_selector(
    query: &str,
    source_position: (f64, f64, f64),
) -> Option<(SelectorKind, SelectorArgs)> {
    if !query.starts_with('@') || query.len() < 2 {
        return None;
    }
    let letter = (query.as_bytes()[1] as char).to_ascii_lowercase();
    let kind = match letter {
        's' => SelectorKind::Self_,
        'p' | 'n' => SelectorKind::Nearest,
        'a' | 'c' => SelectorKind::AllPlayers,
        'e' => SelectorKind::AllEntities,
        'r' => SelectorKind::Random,
        _ => return None,
    };
    let rest = query[2..].trim();
    if rest.is_empty() {
        return Some((kind, SelectorArgs::default()));
    }
    if !rest.starts_with('[') || !rest.ends_with(']') {
        return None;
    }
    let inner = &rest[1..rest.len() - 1];
    let mut args = SelectorArgs::default();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim().to_lowercase();
        let value = value.trim().trim_matches('"').trim_matches('\'').trim();
        match key.as_str() {
            "x" => args.x = parse_coordinate(value, source_position.0),
            "y" => args.y = parse_coordinate(value, source_position.1),
            "z" => args.z = parse_coordinate(value, source_position.2),
            "distance" => {
                if let Some((min, max)) = parse_distance_range(value) {
                    args.distance_min = min;
                    args.distance_max = max;
                }
            }
            "limit" => {
                if let Ok(value) = value.parse::<i64>() {
                    args.limit = Some(if value < 0 { 0 } else { value as usize });
                }
            }
            "sort" => {
                args.sort = match value.to_lowercase().as_str() {
                    "nearest" => Some(SelectorSort::Nearest),
                    "furthest" => Some(SelectorSort::Furthest),
                    "arbitrary" => Some(SelectorSort::Arbitrary),
                    "random" => Some(SelectorSort::Random),
                    _ => None,
                };
            }
            "name" => args.name = Some(value.to_string()),
            "type" => args.type_ = Some(value.to_string()),
            "gamemode" => args.gamemode = parse_gamemode(value),
            _ => {}
        }
    }
    Some((kind, args))
}

fn selector_distance(position: (f64, f64, f64), origin: (f64, f64, f64)) -> f64 {
    let dx = position.0 - origin.0;
    let dy = position.1 - origin.1;
    let dz = position.2 - origin.2;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Small xorshift generator so `@r` / `sort=random` do not need a rand
/// dependency. Mixed with wall time and a counter, so consecutive calls do
/// not repeat the same pick within a tick.
fn next_pseudo_random() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0x853c49e6748fea9b);
    let count = COUNTER.fetch_add(0x9e3779b97f4a7c15, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let mut state = count ^ nanos ^ count.wrapping_shl(6) ^ count.wrapping_shr(2);
    if state == 0 {
        state = 0x9e3779b97f4a7c15;
    }
    state ^= state >> 12;
    state ^= state << 25;
    state ^= state >> 27;
    state.wrapping_mul(0x2545F4914F6CDD1D)
}

fn shuffle_vec<T>(values: &mut Vec<T>) {
    if values.len() <= 1 {
        return;
    }
    for index in (1..values.len()).rev() {
        let swap_with = (next_pseudo_random() % (index as u64 + 1)) as usize;
        values.swap(index, swap_with);
    }
}

/// Resolves a parsed `@` selector against the online snapshot.
///
/// `@e` returns the online players as entities: the Bedrock runtime has no
/// non-player entity list yet, so mobs/items are not included (documented
/// in the module docs). `type=player` is a no-op filter; any other `type`
/// matches nothing.
fn resolve_selector_targets<'a>(
    source: &CommandSource,
    players: &'a [OnlinePlayer],
    kind: SelectorKind,
    args: &SelectorArgs,
) -> Vec<&'a OnlinePlayer> {
    let origin = (
        args.x.unwrap_or(source.position.0),
        args.y.unwrap_or(source.position.1),
        args.z.unwrap_or(source.position.2),
    );
    let mut pool: Vec<&OnlinePlayer> = match kind {
        SelectorKind::Self_ => players
            .iter()
            .filter(|player| player.name.eq_ignore_ascii_case(&source.name))
            .collect(),
        _ => players.iter().collect(),
    };
    if let Some(name_filter) = &args.name {
        pool.retain(|player| player.name.eq_ignore_ascii_case(name_filter));
    }
    if let Some(type_filter) = &args.type_ {
        let lowered = type_filter.to_lowercase();
        if lowered != "player" && lowered != "players" {
            return Vec::new();
        }
    }
    if let Some(gamemode) = args.gamemode {
        pool.retain(|player| player.gamemode == gamemode);
    }
    if args.distance_min.is_some() || args.distance_max.is_some() {
        pool.retain(|player| {
            let distance = selector_distance(player.position, origin);
            let min_ok = args.distance_min.map(|min| distance >= min).unwrap_or(true);
            let max_ok = args.distance_max.map(|max| distance <= max).unwrap_or(true);
            min_ok && max_ok
        });
    }
    let effective_sort = args.sort.unwrap_or(match kind {
        SelectorKind::Nearest => SelectorSort::Nearest,
        SelectorKind::Random => SelectorSort::Random,
        SelectorKind::Self_ | SelectorKind::AllPlayers | SelectorKind::AllEntities => {
            SelectorSort::Arbitrary
        }
    });
    match effective_sort {
        SelectorSort::Nearest => pool.sort_by(|first, second| {
            selector_distance(first.position, origin)
                .partial_cmp(&selector_distance(second.position, origin))
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        SelectorSort::Furthest => pool.sort_by(|first, second| {
            selector_distance(second.position, origin)
                .partial_cmp(&selector_distance(first.position, origin))
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        SelectorSort::Arbitrary => {}
        SelectorSort::Random => shuffle_vec(&mut pool),
    }
    let limit = args.limit.unwrap_or(match kind {
        SelectorKind::Nearest | SelectorKind::Random | SelectorKind::Self_ => 1,
        SelectorKind::AllPlayers | SelectorKind::AllEntities => usize::MAX,
    });
    if limit == 0 {
        return Vec::new();
    }
    pool.truncate(limit);
    pool
}

/// Resolves a player by exact name, then by unique prefix. Selectors (`@s`,
/// `@p`, `@a`, `@e`, `@r`, `@n`, `@c` with optional `[...]`) resolve through
/// [`resolve_selector_targets`] and return the first match (already sorted
/// and limited). For multiple targets see [`resolve_players`].
fn resolve_player<'a>(
    source: &CommandSource,
    players: &'a [OnlinePlayer],
    query: &str,
) -> Option<&'a OnlinePlayer> {
    if is_selector(query) {
        let (kind, args) = parse_selector(query, source.position)?;
        return resolve_selector_targets(source, players, kind, &args)
            .into_iter()
            .next();
    }
    let query_lower = query.to_lowercase();
    if let Some(exact) = players
        .iter()
        .find(|player| player.name.to_lowercase() == query_lower)
    {
        return Some(exact);
    }
    let matches: Vec<&OnlinePlayer> = players
        .iter()
        .filter(|player| player.name.to_lowercase().starts_with(&query_lower))
        .collect();
    if matches.len() == 1 {
        matches.first().copied()
    } else {
        None
    }
}

/// Resolves a query to every matching player (selector-aware). Non-selector
/// names follow [`resolve_player`]'s unique-prefix rule: ambiguous prefixes
/// resolve to no players.
fn resolve_players<'a>(
    source: &CommandSource,
    players: &'a [OnlinePlayer],
    query: &str,
) -> Vec<&'a OnlinePlayer> {
    if is_selector(query) {
        let Some((kind, args)) = parse_selector(query, source.position) else {
            return Vec::new();
        };
        return resolve_selector_targets(source, players, kind, &args);
    }
    match resolve_player(source, players, query) {
        Some(player) => vec![player],
        None => Vec::new(),
    }
}

fn permission_for(command: &str) -> CommandOutput {
    error(red(lang::PERMISSION.replace("{%commandName}", command)))
}

/// `GameMode::getTranslatableName()` values from PocketMine's Spanish locale.
pub(crate) fn gamemode_translatable(gamemode: i32) -> &'static str {
    match gamemode {
        1 => "Modo Creativo",
        2 => "Modo Aventura",
        3 => "Modo Espectador",
        _ => "Modo Supervivencia",
    }
}

/// `World::getDifficultyFromString`.
fn parse_difficulty(value: &str) -> Option<i32> {
    match value.to_lowercase().as_str() {
        "0" | "peaceful" | "pacifico" | "pacífico" => Some(0),
        "1" | "easy" | "facil" | "fácil" => Some(1),
        "2" | "normal" => Some(2),
        "3" | "hard" | "dificil" | "difícil" => Some(3),
        _ => None,
    }
}

/// PHP's `(string) round($value, 2)`: no trailing zeros and no `.0` suffix.
fn format_coord(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.fract() == 0.0 {
        return format!("{}", rounded as i64);
    }
    let mut text = format!("{:.2}", rounded);
    while text.ends_with('0') {
        text.pop();
    }
    text
}

/// `/spawnpoint [player] [<x> <y> <z>]`, ported from
/// `pocketmine\command\defaults\SpawnPointCommand`.
fn spawnpoint(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    let target = match fetch_permitted_player_target(
        source,
        players,
        args.first().copied(),
        "spawnpoint",
        source.can(crate::permission::COMMAND_SPAWNPOINT_SELF),
        source.can(crate::permission::COMMAND_SPAWNPOINT_OTHER),
    ) {
        Ok(Some(player)) => player.name.clone(),
        Ok(None) => source.name.clone(),
        Err(output) => return output,
    };

    let (x, y, z) = if args.len() == 4 {
        // The console has no position, so PocketMine falls back to the world
        // spawn as the origin for relative coordinates.
        let base = if source.is_console {
            source.world_spawn
        } else {
            source.position
        };
        let (Some(x), Some(y), Some(z)) = (
            parse_coordinate(args[1], base.0),
            parse_coordinate(args[2], base.1),
            parse_coordinate(args[3], base.2),
        ) else {
            return usage("spawnpoint");
        };
        let y = y.clamp(
            crate::world_state::WORLD_MIN_Y as f64,
            crate::world_state::WORLD_MAX_Y as f64,
        );
        (x, y, z)
    } else if args.len() <= 1 && !source.is_console {
        (
            source.position.0.floor(),
            source.position.1.floor(),
            source.position.2.floor(),
        )
    } else {
        return usage("spawnpoint");
    };

    let message = fill_template(
        lang::SPAWNPOINT_SUCCESS,
        &[&target, &format_coord(x), &format_coord(y), &format_coord(z)],
    );
    CommandOutput {
        feedback: vec![message.clone()],
        actions: vec![CommandAction::SetSpawnPoint {
            target,
            position: (x, y, z),
            message,
            notify: source.name.clone(),
        }],
    }
}

/// `/title <player> <clear|reset|title|subtitle|actionbar|times> ...`, ported
/// one to one from `pocketmine\command\defaults\TitleCommand`.
fn title(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() < 2 {
        return usage("title");
    }

    let target = match fetch_permitted_player_target(
        source,
        players,
        Some(args[0]),
        "title",
        source.can(crate::permission::COMMAND_TITLE_SELF),
        source.can(crate::permission::COMMAND_TITLE_OTHER),
    ) {
        Ok(Some(player)) => player.name.clone(),
        Ok(None) => source.name.clone(),
        Err(output) => return output,
    };

    let action = match args[1] {
        "clear" => TitleAction::Clear,
        "reset" => TitleAction::Reset,
        "title" | "subtitle" | "actionbar" => {
            if args.len() < 3 {
                return usage("title");
            }
            let text = args[2..].join(" ");
            match args[1] {
                "title" => TitleAction::Title(text),
                "subtitle" => TitleAction::Subtitle(text),
                _ => TitleAction::ActionBar(text),
            }
        }
        "times" => {
            if args.len() < 5 {
                return usage("title");
            }
            let mut times = [0i32; 3];
            for (index, slot) in times.iter_mut().enumerate() {
                let value = args[2 + index];
                match bounded_int(value, i32::MIN as i64, i32::MAX as i64) {
                    Ok(parsed) => *slot = parsed as i32,
                    Err(BoundedIntError::Syntax) => return usage("title"),
                    Err(BoundedIntError::TooBig) => {
                        return error(red(fill_template(
                            lang::NUM_TOO_BIG,
                            &[value, &i32::MAX.to_string()],
                        )));
                    }
                    Err(BoundedIntError::TooSmall) => {
                        return error(red(fill_template(
                            lang::NUM_TOO_SMALL,
                            &[value, &i32::MIN.to_string()],
                        )));
                    }
                }
            }
            // `Player::setTitleDuration()` ignores negative durations, but the
            // command still reports success.
            if times.iter().any(|value| *value < 0) {
                return feedback(lang::TITLE_SUCCESS.to_string());
            }
            TitleAction::Times {
                fade_in: times[0],
                stay: times[1],
                fade_out: times[2],
            }
        }
        _ => return usage("title"),
    };

    CommandOutput {
        feedback: vec![lang::TITLE_SUCCESS.to_string()],
        actions: vec![CommandAction::SendTitle { target, action }],
    }
}

/// `/transferserver <host> [port]`, ported from
/// `pocketmine\command\defaults\TransferServerCommand`.
///
/// There is no success feedback: `Player::transfer()` sends the packet and
/// `NetworkSession::transfer()` closes the session without a disconnect screen.
/// `PlayerTransferEvent` is still missing from the plugin bridge.
fn transfer(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if args.is_empty() {
        return usage("transferserver");
    }
    if source.is_console {
        return error(red(lang::PLAYER_USER_ONLY));
    }

    // `(int) ($args[1] ?? 19132)` then `writeUnsignedShort()` truncates.
    let port = php_int_cast(args.get(1).copied().unwrap_or("19132")) as u16;
    CommandOutput {
        feedback: Vec::new(),
        actions: vec![CommandAction::Transfer {
            target: source.name.clone(),
            address: args[0].to_string(),
            port,
        }],
    }
}

/// `/defaultgamemode <mode>`, ported from
/// `pocketmine\command\defaults\DefaultGamemodeCommand`.
fn defaultgamemode(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_DEFAULTGAMEMODE) {
        return permission_for("defaultgamemode");
    }
    let Some(mode) = args.first() else {
        return usage("defaultgamemode");
    };
    let Some(gamemode) = parse_gamemode(mode) else {
        return error(lang::GAMEMODE_UNKNOWN.replace("{%gameModeName}", mode));
    };

    CommandOutput {
        feedback: vec![fill_template(
            lang::DEFAULTGAMEMODE_SUCCESS,
            &[gamemode_translatable(gamemode)],
        )],
        actions: vec![CommandAction::SetDefaultGamemode(gamemode)],
    }
}

/// `/difficulty <difficulty>`, ported from
/// `pocketmine\command\defaults\DifficultyCommand`.
fn difficulty(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_DIFFICULTY) {
        return permission_for("difficulty");
    }
    if args.len() != 1 {
        return usage("difficulty");
    }
    let Some(difficulty) = parse_difficulty(args[0]) else {
        return usage("difficulty");
    };

    let message = fill_template(lang::DIFFICULTY_SUCCESS, &[&difficulty.to_string()]);
    CommandOutput {
        feedback: vec![message.clone()],
        actions: vec![CommandAction::SetDifficulty {
            difficulty,
            message,
            notify: source.name.clone(),
        }],
    }
}

/// `/seed`, ported from `pocketmine\command\defaults\SeedCommand`.
fn seed(source: &CommandSource) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_SEED) {
        return permission_for("seed");
    }
    feedback(fill_template(lang::SEED_SUCCESS, &[&source.world_seed.to_string()]))
}

/// `DefaultPermissionNames` nodes gating a command.
///
/// PocketMine registers commands with one node or with a `[self, other]` pair;
/// `Command::testPermissionSilent()` accepts any node of the set and the handler
/// then checks the node of the resolved target.
///
/// Commands whose permission depends on the subcommand (`/time`, `/whitelist`)
/// keep their in-command checks and return `None` here.
fn permission_nodes(command: &str) -> Option<&'static [&'static str]> {
    use crate::permission::*;
    match command {
        "help" | "?" => Some(&[COMMAND_HELP]),
        "tp" | "teleport" => Some(&[COMMAND_TELEPORT_SELF, COMMAND_TELEPORT_OTHER]),
        "gamemode" | "gm" | "gmc" | "gms" | "gma" | "gmsp" => {
            Some(&[COMMAND_GAMEMODE_SELF, COMMAND_GAMEMODE_OTHER])
        }
        "setworldspawn" => Some(&[COMMAND_SETWORLDSPAWN]),
        "say" => Some(&[COMMAND_SAY]),
        "me" => Some(&[COMMAND_ME]),
        "msg" | "tell" | "w" | "whisper" => Some(&[COMMAND_TELL]),
        "list" | "players" => Some(&[COMMAND_LIST]),
        "kick" => Some(&[COMMAND_KICK]),
        "give" => Some(&[COMMAND_GIVE_SELF, COMMAND_GIVE_OTHER]),
        "clear" => Some(&[COMMAND_CLEAR_SELF, COMMAND_CLEAR_OTHER]),
        "kill" | "suicide" => Some(&[COMMAND_KILL_SELF, COMMAND_KILL_OTHER]),
        "spawnpoint" => Some(&[COMMAND_SPAWNPOINT_SELF, COMMAND_SPAWNPOINT_OTHER]),
        "title" => Some(&[COMMAND_TITLE_SELF, COMMAND_TITLE_OTHER]),
        "transferserver" => Some(&[COMMAND_TRANSFERSERVER]),
        "defaultgamemode" => Some(&[COMMAND_DEFAULTGAMEMODE]),
        "difficulty" => Some(&[COMMAND_DIFFICULTY]),
        "seed" => Some(&[COMMAND_SEED]),
        "op" => Some(&[COMMAND_OP_GIVE]),
        "deop" => Some(&[COMMAND_OP_TAKE]),
        "version" | "ver" => Some(&[COMMAND_VERSION]),
        "ban" => Some(&[COMMAND_BAN_PLAYER]),
        "ban-ip" => Some(&[COMMAND_BAN_IP]),
        "banlist" => Some(&[COMMAND_BAN_LIST]),
        "pardon" | "unban" => Some(&[COMMAND_UNBAN_PLAYER]),
        "pardon-ip" | "unban-ip" => Some(&[COMMAND_UNBAN_IP]),
        "stop" => Some(&[COMMAND_STOP]),
        "plugins" | "pl" => Some(&[COMMAND_PLUGINS]),
        "save-all" => Some(&[COMMAND_SAVE_PERFORM]),
        "save-on" => Some(&[COMMAND_SAVE_ENABLE]),
        "save-off" => Some(&[COMMAND_SAVE_DISABLE]),
        "tps" => Some(&[COMMAND_TPS]),
        _ => None,
    }
}

/// PocketMine's `Player::isValidUserName()`.
fn is_valid_username(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 16
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ' ')
}

/// `Server::getPlayerByPrefix()` for the online snapshot: exact match first.
fn resolve_player_name(players: &[OnlinePlayer], value: &str) -> String {
    let lower = value.to_lowercase();
    if let Some(exact) = players
        .iter()
        .find(|player| player.name.to_lowercase() == lower)
    {
        return exact.name.clone();
    }
    players
        .iter()
        .find(|player| player.name.to_lowercase().starts_with(&lower))
        .map(|player| player.name.clone())
        .unwrap_or_else(|| value.to_string())
}

/// `/ban <name> [reason ...]`, ported from `pocketmine\command\defaults\BanCommand`.
fn ban(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_BAN_PLAYER) {
        return permission_for("ban");
    }
    if args.is_empty() {
        return usage("ban");
    }

    let name = args[0];
    let reason = args[1..].join(" ");
    let target = resolve_player_name(players, name);
    let message = fill_template(lang::BAN_SUCCESS, &[&target]);
    CommandOutput {
        feedback: vec![message.clone()],
        actions: vec![CommandAction::BanName {
            name: name.to_string(),
            reason,
            message,
            notify: source.name.clone(),
        }],
    }
}

/// `/ban-ip <address|name> [reason ...]`, ported from
/// `pocketmine\command\defaults\BanIpCommand`.
fn ban_ip(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_BAN_IP) {
        return permission_for("ban-ip");
    }
    if args.is_empty() {
        return usage("ban-ip");
    }

    let value = args[0];
    let reason = args[1..].join(" ");
    if crate::access::is_ip_address(value) {
        let message = fill_template(lang::BANIP_SUCCESS, &[value]);
        return CommandOutput {
            feedback: vec![message.clone()],
            actions: vec![CommandAction::BanIp {
                ip: value.to_string(),
                reason,
                message,
                notify: source.name.clone(),
            }],
        };
    }

    let lower = value.to_lowercase();
    let Some(player) = players
        .iter()
        .find(|player| player.name.to_lowercase().starts_with(&lower))
    else {
        return feedback(lang::BANIP_INVALID.to_string());
    };

    CommandOutput {
        feedback: Vec::new(),
        actions: vec![CommandAction::BanIpPlayer {
            player: player.name.clone(),
            reason,
            notify: source.name.clone(),
        }],
    }
}

/// `/banlist [ips|players]`, ported from
/// `pocketmine\command\defaults\BanListCommand`.
fn banlist(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_BAN_LIST) {
        return permission_for("banlist");
    }

    let mut mode = "players";
    if let Some(value) = args.first() {
        mode = match value.to_lowercase().as_str() {
            "ips" => "ips",
            "players" => "players",
            _ => return usage("banlist"),
        };
    }

    let mut entries = match (source.access.as_ref(), mode) {
        (Some(access), "ips") => access.banned_ips(),
        (Some(access), _) => access.banned_names(),
        (None, _) => Vec::new(),
    };
    entries.sort();
    let header = if mode == "ips" {
        fill_template(lang::BANLIST_IPS, &[&entries.len().to_string()])
    } else {
        fill_template(lang::BANLIST_PLAYERS, &[&entries.len().to_string()])
    };

    CommandOutput {
        feedback: vec![header, entries.join(", ")],
        actions: Vec::new(),
    }
}

/// `/pardon <name>`, ported from `pocketmine\command\defaults\PardonCommand`.
fn pardon(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_UNBAN_PLAYER) {
        return permission_for("pardon");
    }
    if args.len() != 1 {
        return usage("pardon");
    }

    let message = fill_template(lang::UNBAN_SUCCESS, &[args[0]]);
    CommandOutput {
        feedback: vec![message.clone()],
        actions: vec![CommandAction::UnbanName {
            name: args[0].to_string(),
            message,
            notify: source.name.clone(),
        }],
    }
}

/// `/pardon-ip <address>`, ported from
/// `pocketmine\command\defaults\PardonIpCommand`.
fn pardon_ip(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_UNBAN_IP) {
        return permission_for("pardon-ip");
    }
    if args.len() != 1 {
        return usage("pardon-ip");
    }
    if !crate::access::is_ip_address(args[0]) {
        return feedback(lang::UNBANIP_INVALID.to_string());
    }

    let message = fill_template(lang::UNBANIP_SUCCESS, &[args[0]]);
    CommandOutput {
        feedback: vec![message.clone()],
        actions: vec![CommandAction::UnbanIp {
            ip: args[0].to_string(),
            message,
            notify: source.name.clone(),
        }],
    }
}

/// `/whitelist <on|off|list|add|remove|reload>`, ported from
/// `pocketmine\command\defaults\WhitelistCommand`.
fn whitelist(source: &CommandSource, args: &[&str]) -> CommandOutput {
    match args.len() {
        1 => match args[0].to_lowercase().as_str() {
            "on" => {
                if !source.can(crate::permission::COMMAND_WHITELIST_ENABLE) {
                    return permission_for("whitelist");
                }
                let message = lang::WHITELIST_ENABLED.to_string();
                CommandOutput {
                    feedback: vec![message.clone()],
                    actions: vec![CommandAction::SetWhitelist {
                        enabled: true,
                        message,
                        notify: source.name.clone(),
                    }],
                }
            }
            "off" => {
                if !source.can(crate::permission::COMMAND_WHITELIST_DISABLE) {
                    return permission_for("whitelist");
                }
                let message = lang::WHITELIST_DISABLED.to_string();
                CommandOutput {
                    feedback: vec![message.clone()],
                    actions: vec![CommandAction::SetWhitelist {
                        enabled: false,
                        message,
                        notify: source.name.clone(),
                    }],
                }
            }
            "list" => {
                if !source.can(crate::permission::COMMAND_WHITELIST_LIST) {
                    return permission_for("whitelist");
                }
                let mut entries = source
                    .access
                    .as_ref()
                    .map(|access| access.whitelist_entries())
                    .unwrap_or_default();
                entries.sort();
                let count = entries.len().to_string();
                let header = lang::WHITELIST_LIST
                    .replace("{%0}", &count)
                    .replace("{%1}", &count);
                CommandOutput {
                    feedback: vec![header, entries.join(", ")],
                    actions: Vec::new(),
                }
            }
            "reload" => {
                if !source.can(crate::permission::COMMAND_WHITELIST_RELOAD) {
                    return permission_for("whitelist");
                }
                CommandOutput {
                    feedback: vec![lang::WHITELIST_RELOADED.to_string()],
                    actions: vec![CommandAction::WhitelistReload {
                        notify: source.name.clone(),
                    }],
                }
            }
            "add" => feedback(lang::USAGE.replace("{%0}", lang::WHITELIST_ADD_USAGE)),
            "remove" => feedback(lang::USAGE.replace("{%0}", lang::WHITELIST_REMOVE_USAGE)),
            _ => usage("whitelist"),
        },
        2 => {
            let subcommand = args[0].to_lowercase();
            let name = args[1];
            if !is_valid_username(name) {
                return usage("whitelist");
            }
            match subcommand.as_str() {
                "add" => {
                    if !source.can(crate::permission::COMMAND_WHITELIST_ADD) {
                        return permission_for("whitelist");
                    }
                    let message = fill_template(lang::WHITELIST_ADD_SUCCESS, &[name]);
                    CommandOutput {
                        feedback: vec![message.clone()],
                        actions: vec![CommandAction::AddWhitelist {
                            name: name.to_string(),
                            message,
                            notify: source.name.clone(),
                        }],
                    }
                }
                "remove" => {
                    if !source.can(crate::permission::COMMAND_WHITELIST_REMOVE) {
                        return permission_for("whitelist");
                    }
                    let message = fill_template(lang::WHITELIST_REMOVE_SUCCESS, &[name]);
                    CommandOutput {
                        feedback: vec![message.clone()],
                        actions: vec![CommandAction::RemoveWhitelist {
                            name: name.to_string(),
                            message,
                            notify: source.name.clone(),
                        }],
                    }
                }
                _ => usage("whitelist"),
            }
        }
        _ => usage("whitelist"),
    }
}

/// `/stop`, ported from `pocketmine\command\defaults\StopCommand`.
fn stop(source: &CommandSource) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_STOP) {
        return permission_for("stop");
    }
    let message = lang::STOP_START.to_string();
    CommandOutput {
        feedback: vec![message.clone()],
        actions: vec![CommandAction::StopServer {
            message,
            notify: source.name.clone(),
        }],
    }
}

/// `/plugins [pl]`, ported from `pocketmine\command\defaults\PluginsCommand`.
fn plugins(source: &CommandSource) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_PLUGINS) {
        return permission_for("plugins");
    }
    // PocketMine prefixes enabled plugins with green, disabled ones with red.
    // Registered plugins are enabled in this runtime.
    let mut list: Vec<String> = source
        .plugins
        .iter()
        .map(|name| format!("§a{name}"))
        .collect();
    list.sort();
    feedback(fill_template(
        lang::PLUGINS_SUCCESS,
        &[&list.len().to_string(), &list.join("§r, ")],
    ))
}

/// `/save-all`, ported from `pocketmine\command\defaults\SaveCommand`.
fn save_all(source: &CommandSource) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_SAVE_PERFORM) {
        return permission_for("save-all");
    }
    let message = lang::SAVE_START.to_string();
    CommandOutput {
        feedback: vec![message.clone()],
        actions: vec![CommandAction::SaveAll {
            message,
            notify: source.name.clone(),
        }],
    }
}

/// Colors one Paper `/tps` value: green ≥ 18, yellow ≥ 15, red ≥ 10, dark red
/// below, with `*` when the server is running at the 20 TPS cap.
fn format_tps(value: f64) -> String {
    let capped = value.min(20.0);
    let color = if capped >= 18.0 {
        "§a"
    } else if capped >= 15.0 {
        "§e"
    } else if capped >= 10.0 {
        "§c"
    } else {
        "§4"
    };
    let star = if capped >= 20.0 { "*" } else { "" };
    format!("{color}{star}{capped:.1}")
}

/// `/tps`: PaperMC's TPS readout, not part of PocketMine's command set.
fn tps(source: &CommandSource) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_TPS) {
        return permission_for("tps");
    }
    let values = [source.tps.0, source.tps.1, source.tps.2];
    let joined = values
        .iter()
        .map(|value| format_tps(*value))
        .collect::<Vec<_>>()
        .join("§r, ");
    feedback(format!("§6TPS from last 1m, 5m, 15m: {joined}"))
}

/// `/save-on` and `/save-off`, ported from `SaveOnCommand`/`SaveOffCommand`.
fn set_auto_save(source: &CommandSource, enabled: bool) -> CommandOutput {
    let command = if enabled { "save-on" } else { "save-off" };
    let node = if enabled {
        crate::permission::COMMAND_SAVE_ENABLE
    } else {
        crate::permission::COMMAND_SAVE_DISABLE
    };
    if !source.can(node) {
        return permission_for(command);
    }
    let message = if enabled {
        lang::SAVE_ENABLED
    } else {
        lang::SAVE_DISABLED
    }
    .to_string();
    CommandOutput {
        feedback: vec![message.clone()],
        actions: vec![CommandAction::SetAutoSave {
            enabled,
            message,
            notify: source.name.clone(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player_source(name: &str, position: (f64, f64, f64)) -> CommandSource {
        CommandSource {
            name: name.to_string(),
            position,
            is_op: true,
            allow_cheats: true,
            is_console: false,
            max_players: 1000,
            world_time: 1_000,
            world_seed: 12_345_678,
            world_spawn: (0.0, 64.0, 0.0),
            access: None,
            plugins: Vec::new(),
            tps: (20.0, 20.0, 20.0),
            permissions: None,
        }
    }

    fn players() -> Vec<OnlinePlayer> {
        vec![
            OnlinePlayer {
                name: "Steve".to_string(),
                position: (10.0, 64.0, -5.0),
                gamemode: 0,
            },
            OnlinePlayer {
                name: "Alex".to_string(),
                position: (-20.0, 70.0, 8.5),
                gamemode: 1,
            },
        ]
    }

    #[test]
    fn tp_accepts_absolute_and_relative_coordinates() {
        let source = player_source("Steve", (10.0, 64.0, -5.0));

        let output = handle_command("/tp 100 70 -300", &source, &players()).expect("tp is native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Teleport {
                target: "Steve".to_string(),
                position: (100.0, 70.0, -300.0),
                yaw: None,
                pitch: None,
            }]
        );

        let output = handle_command("tp ~5 ~ ~-2.5", &source, &players()).expect("tp is native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Teleport {
                target: "Steve".to_string(),
                position: (15.0, 64.0, -7.5),
                yaw: None,
                pitch: None,
            }]
        );
    }

    #[test]
    fn tp_handles_player_targets() {
        let source = player_source("Steve", (10.0, 64.0, -5.0));

        let output = handle_command("/tp Alex", &source, &players()).expect("tp is native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Teleport {
                target: "Steve".to_string(),
                position: (-20.0, 70.0, 8.5),
                yaw: None,
                pitch: None,
            }]
        );

        let output = handle_command("/tp alex steve", &source, &players()).expect("tp is native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Teleport {
                target: "Alex".to_string(),
                position: (10.0, 64.0, -5.0),
                yaw: None,
                pitch: None,
            }]
        );

        let output = handle_command("/tp Alex ~ ~10 ~", &source, &players()).expect("tp is native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Teleport {
                target: "Alex".to_string(),
                position: (-20.0, 80.0, 8.5),
                yaw: None,
                pitch: None,
            }]
        );
    }

    #[test]
    fn tp_reports_unknown_players_and_usage() {
        let source = player_source("Steve", (10.0, 64.0, -5.0));
        let output = handle_command("/tp Nobody", &source, &players()).expect("tp is native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("Jugador no encontrado"));

        let output = handle_command("/tp", &source, &players()).expect("tp is native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("Uso:"));
    }

    #[test]
    fn tp_requires_privileges_when_cheats_are_off() {
        let mut source = player_source("Steve", (0.0, 64.0, 0.0));
        source.is_op = false;
        source.allow_cheats = false;
        let output = handle_command("/tp 1 2 3", &source, &players()).expect("tp is native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("permiso"));
    }

    fn gamemode_success_self(mode: i32) -> String {
        fill_template(lang::GAMEMODE_SUCCESS_SELF, &[gamemode_translatable(mode)])
    }

    fn gamemode_success_other(mode: i32, target: &str) -> String {
        fill_template(
            lang::GAMEMODE_SUCCESS_OTHER,
            &[gamemode_translatable(mode), target],
        )
    }

    #[test]
    fn gamemode_aliases_change_the_right_target() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        let output = handle_command("/gamemode c", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::SetGamemode {
                target: "Steve".to_string(),
                gamemode: 1,
                message: gamemode_success_self(1),
                notify: "Steve".to_string(),
            }]
        );

        let output = handle_command("/gms Alex", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::SetGamemode {
                target: "Alex".to_string(),
                gamemode: 0,
                message: gamemode_success_other(0, "Alex"),
                notify: "Steve".to_string(),
            }]
        );

        let output = handle_command("/gamemode 3 Alex", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::SetGamemode {
                target: "Alex".to_string(),
                gamemode: 3,
                message: gamemode_success_other(3, "Alex"),
                notify: "Steve".to_string(),
            }]
        );
    }

    #[test]
    fn gamemode_keeps_the_mode_when_it_is_already_set() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));
        // Alex already plays in creative.
        let output = handle_command("/gamemode creative Alex", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(
            output.feedback,
            vec!["¡Error al cambiar el modo de juego de Alex!".to_string()]
        );
    }

    #[test]
    fn world_commands_produce_actions() {
        let source = player_source("Steve", (4.6, 65.2, -3.5));

        let output = handle_command("/time set night", &source, &players()).expect("native");
        assert_eq!(output.actions, vec![CommandAction::SetTime(13_000)]);

        let output = handle_command("/time query", &source, &players()).expect("native");
        assert_eq!(output.actions, Vec::new());
        assert!(output.feedback[0].contains("1000"));

        let output = handle_command("/weather thunder", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::SetWeather {
                state: WeatherState::Thunder,
                duration_ticks: DEFAULT_WEATHER_TICKS,
            }]
        );

        let output = handle_command("/weather rain 30", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::SetWeather {
                state: WeatherState::Rain,
                duration_ticks: 600,
            }]
        );

        let output = handle_command("/setworldspawn", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::SetWorldSpawn { x: 4, y: 65, z: -4 }]
        );
    }

    #[test]
    fn broadcast_and_private_messages() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        // `chat.type.announcement`, no ack to the sender.
        let output = handle_command("/say hola a todos", &source, &players()).expect("native");
        assert!(output.feedback.is_empty());
        assert_eq!(
            output.actions,
            vec![CommandAction::Broadcast {
                message: "[Steve] hola a todos".to_string(),
            }]
        );

        // `chat.type.emote`, no ack to the sender.
        let output = handle_command("/me saluda", &source, &players()).expect("native");
        assert!(output.feedback.is_empty());
        assert_eq!(
            output.actions,
            vec![CommandAction::Broadcast {
                message: "* Steve saluda".to_string(),
            }]
        );

        // `commands.message.display.outgoing` and `.incoming`.
        let output = handle_command("/msg Alex ven aqui", &source, &players()).expect("native");
        assert_eq!(
            output.feedback,
            vec!["§7Le susurras a Alex: ven aqui".to_string()]
        );
        match &output.actions[0] {
            CommandAction::Message { target, message } => {
                assert_eq!(target, "Alex");
                assert_eq!(message, "§7Steve te susurra: ven aqui");
            }
            other => panic!("expected a private message, got {:?}", other),
        }

        // `commands.message.sameTarget`.
        let output = handle_command("/msg Steve hola", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(output.feedback, vec![lang::MESSAGE_SAME_TARGET.to_string()]);
    }

    #[test]
    fn version_reports_software_and_protocol() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));
        let output = handle_command("/version", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "Este servidor está ejecutando PocketMine-Rust"
        );
        assert!(output.feedback[1].contains("git hash"));
        assert!(output.feedback[2].contains("1.26.60"));
    }

    #[test]
    fn give_accepts_json_nbt_tags() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        let output = handle_command(
            "/give Alex diamond_sword 1 {\"display\":{\"Name\":\"Excalibur\"}}",
            &source,
            &players(),
        )
        .expect("native");
        let expected = bedrock_protocol::nbt::write_network_nbt(&bedrock_protocol::nbt::Tag::Compound(
            vec![(
                "display".to_string(),
                bedrock_protocol::nbt::Tag::Compound(vec![(
                    "Name".to_string(),
                    bedrock_protocol::nbt::Tag::String("Excalibur".to_string()),
                )]),
            )],
        ));
        match &output.actions[0] {
            CommandAction::Give { count, nbt, .. } => {
                assert_eq!(*count, 1);
                assert_eq!(nbt, &expected);
            }
            other => panic!("expected /give, got {:?}", other),
        }

        // `commands.give.tagError` reports the parser message, uncoloured.
        let output = handle_command("/give Alex stone 1 {\"oops\":", &source, &players())
            .expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].starts_with("Error al analizar la etiqueta de datos: "));
    }

    #[test]
    fn give_resolves_items_and_counts() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        let output = handle_command("/give Alex stone", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Give {
                target: "Alex".to_string(),
                network_id: 1,
                meta: 0,
                count: 64,
                nbt: Vec::new(),
                message: "Dado Stone (stone) * 64 a Alex".to_string(),
                notify: "Steve".to_string(),
            }]
        );
        assert_eq!(
            output.feedback,
            vec!["Dado Stone (stone) * 64 a Alex".to_string()]
        );

        // Legacy numeric ids and damage values are accepted.
        let output = handle_command("/give Alex 1:2 3", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Give {
                target: "Alex".to_string(),
                network_id: 1,
                meta: 2,
                count: 3,
                nbt: Vec::new(),
                message: "Dado Stone (1:2) * 3 a Alex".to_string(),
                notify: "Steve".to_string(),
            }]
        );

        let output = handle_command("/give Alex not_an_item 1", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(
            output.feedback[0],
            "§cNo existe tal objeto con el nombre not_an_item"
        );

        let output = handle_command("/give Alex stone 0", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(
            output.feedback[0],
            "§cEl número que ha introducido (0) es demasiado pequeño, debe ser como mínimo 1"
        );

        let output = handle_command("/give Alex stone 40000", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "§cEl número que ha introducido (40000) es demasiado grande, debe ser como mucho 32767"
        );

        let output = handle_command("/give Alex stone abc", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(
            output.feedback[0],
            "§cUso: /give <jugador> <objeto[:extra]> [cantidad] [etiquetas...]"
        );
    }

    #[test]
    fn clear_and_kill_target_the_right_players() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        let output = handle_command("/clear Alex stone", &source, &players()).expect("native");
        assert!(output.feedback.is_empty(), "the count comes from the manager");
        assert_eq!(
            output.actions,
            vec![CommandAction::ClearInventory {
                target: "Alex".to_string(),
                item: Some((1, 0)),
                max_count: -1,
                notify: "Steve".to_string(),
            }]
        );

        let output =
            handle_command("/clear Alex minecraft:stone 0", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::ClearInventory {
                target: "Alex".to_string(),
                item: Some((1, 0)),
                max_count: 0,
                notify: "Steve".to_string(),
            }]
        );

        let output = handle_command("/clear", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::ClearInventory {
                target: "Steve".to_string(),
                item: None,
                max_count: -1,
                notify: "Steve".to_string(),
            }]
        );

        let output = handle_command("/kill", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Kill {
                target: "Steve".to_string(),
                message: "Steve Murió".to_string(),
                broadcast: false,
                notify: "Steve".to_string(),
            }]
        );
        assert_eq!(output.feedback, vec!["Steve Murió".to_string()]);

        let output = handle_command("/kill Alex", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Kill {
                target: "Alex".to_string(),
                message: "Alex Murió".to_string(),
                broadcast: true,
                notify: "Steve".to_string(),
            }]
        );
    }

    #[test]
    fn item_commands_follow_the_vanilla_permissions() {
        let mut source = player_source("Steve", (0.0, 64.0, 0.0));
        source.is_op = false;
        source.allow_cheats = false;

        // `/give` is operator-only, even when giving to yourself.
        let output = handle_command("/give Steve stone", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(
            output.feedback[0],
            "§cNo tienes permiso para ejecutar el comando: give"
        );

        // Clearing yourself is granted to everyone, clearing others is not.
        let output = handle_command("/clear", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 1);
        let output = handle_command("/clear Alex", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("permiso"));

        // Killing yourself is granted to everyone, killing others is not.
        let output = handle_command("/kill", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 1);
        let output = handle_command("/kill Alex", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("permiso"));
    }

    #[test]
    fn world_administration_commands_follow_pocketmine() {
        let source = player_source("Steve", (10.4, 64.0, -5.6));

        let output = handle_command("/spawnpoint", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::SetSpawnPoint {
                target: "Steve".to_string(),
                position: (10.0, 64.0, -6.0),
                message: "Punto de aparición de Steve establecido en (10, 64, -6)".to_string(),
                notify: "Steve".to_string(),
            }]
        );

        let output =
            handle_command("/spawnpoint Alex 100 70.5 -50", &source, &players()).expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::SetSpawnPoint { position, .. } if *position == (100.0, 70.5, -50.0)
        ));

        let output =
            handle_command("/spawnpoint Alex ~2 ~ ~-1", &source, &players()).expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::SetSpawnPoint { position, .. }
                if (position.0 - 12.4).abs() < 1e-6
                    && (position.1 - 64.0).abs() < 1e-6
                    && (position.2 + 6.6).abs() < 1e-6
        ));

        let output = handle_command("/spawnpoint Alex 0 70", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("Uso:"));

        let output =
            handle_command("/defaultgamemode creative", &source, &players()).expect("native");
        assert_eq!(output.actions, vec![CommandAction::SetDefaultGamemode(1)]);
        assert_eq!(
            output.feedback[0],
            "El modo de juego por defecto del mundo es ahora Modo Creativo"
        );

        let output =
            handle_command("/defaultgamemode nope", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(output.feedback[0], "Modo de juego desconocido \"nope\"");

        let output = handle_command("/difficulty hard", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 1);
        assert_eq!(output.feedback[0], "Cambiada la dificultad del juego a 3");

        let output = handle_command("/difficulty 7", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("Uso:"));

        let output = handle_command("/seed", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Semilla: 12345678");
    }

    #[test]
    fn ban_and_whitelist_commands_follow_pocketmine() {
        let mut source = player_source("Steve", (0.0, 64.0, 0.0));
        let access = std::sync::Arc::new(crate::access::AccessList::new());
        access.ban_name("Alex", "grief");
        access.ban_ip("10.0.0.9", "spam");
        access.add_whitelist("Alex");
        source.access = Some(access);

        let output = handle_command("/banlist", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Hay un total de 1 jugadores baneados:");
        assert_eq!(output.feedback[1], "alex");

        let output = handle_command("/banlist ips", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Hay 1 IPs bloqueadas:");
        assert_eq!(output.feedback[1], "10.0.0.9");

        let output = handle_command("/banlist nope", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("Uso:"));

        let output = handle_command("/pardon-ip nope", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Has ingresado una dirección IP no válida");
        assert!(output.actions.is_empty());

        let output = handle_command("/whitelist list", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "Hay 1 (de cada 1 vistos) jugadores en la lista blanca:"
        );

        let output =
            handle_command("/whitelist add Herobrine", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Se ha añadido a Herobrine a la lista blanca");
        assert!(matches!(
            &output.actions[0],
            CommandAction::AddWhitelist { name, .. } if name == "Herobrine"
        ));

        let output = handle_command("/whitelist add", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Uso: /whitelist add <jugador>");

        let output = handle_command("/ban Alex", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Jugador Alex baneado");
        assert!(matches!(
            &output.actions[0],
            CommandAction::BanName { reason, .. } if reason.is_empty()
        ));

        let output = handle_command("/ban-ip 10.1.1.1 spam", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "IP 10.1.1.1 bloqueada");

        let output = handle_command("/ban-ip nobody", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "Has ingresado una dirección IP inválida o el jugador no está en línea"
        );

        let output = handle_command("/stop", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Deteniendo el servidor");
        assert_eq!(
            output.actions,
            vec![CommandAction::StopServer {
                message: "Deteniendo el servidor".to_string(),
                notify: "Steve".to_string(),
            }]
        );
    }

    #[test]
    fn plugins_and_save_commands_follow_pocketmine() {
        let mut source = player_source("Steve", (0.0, 64.0, 0.0));
        source.plugins = vec!["TestKit".to_string(), "ChatGuard".to_string()];

        let output = handle_command("/plugins", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "Plugins (2): §aChatGuard§r, §aTestKit"
        );

        let output = handle_command("/pl", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "Plugins (2): §aChatGuard§r, §aTestKit"
        );

        let output = handle_command("/save-all", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Guardando datos del servidor...");
        assert!(matches!(
            &output.actions[0],
            CommandAction::SaveAll { .. }
        ));

        let output = handle_command("/save-off", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "El autoguardado del mundo ha sido desactivado"
        );
        assert!(matches!(
            &output.actions[0],
            CommandAction::SetAutoSave { enabled: false, .. }
        ));

        let output = handle_command("/save-on", &source, &players()).expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::SetAutoSave { enabled: true, .. }
        ));
    }

    #[test]
    fn tps_command_reports_paper_style_colors() {
        let mut source = player_source("Steve", (0.0, 64.0, 0.0));
        source.tps = (20.0, 19.4, 12.3);

        let output = handle_command("/tps", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "§6TPS from last 1m, 5m, 15m: §a*20.0§r, §a19.4§r, §c12.3"
        );

        source.tps = (9.5, 17.0, 20.0);
        let output = handle_command("/tps", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "§6TPS from last 1m, 5m, 15m: §49.5§r, §e17.0§r, §a*20.0"
        );
    }

    #[test]
    fn permission_nodes_gate_commands_and_attachments_override_them() {
        let mut source = player_source("Guest", (0.0, 64.0, 0.0));
        source.is_op = false;
        source.allow_cheats = false;
        let permissions = std::sync::Arc::new(crate::permission::PermissionManager::new());
        source.permissions = Some(std::sync::Arc::clone(&permissions));

        // Operator-only command.
        let output = handle_command("/seed", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "§cNo tienes permiso para ejecutar el comando: seed"
        );
        assert!(output.actions.is_empty());

        // Everyone may check the online players.
        let output = handle_command("/list", &source, &players()).expect("native");
        assert!(!output.feedback[0].contains("permiso"));

        // Granting the node through an attachment allows the command.
        permissions.set_permission("Guest", crate::permission::COMMAND_SEED, true);
        let output = handle_command("/seed", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Semilla: 12345678");

        // ...and denying it on an operator blocks it too.
        let mut admin = player_source("Admin", (0.0, 64.0, 0.0));
        admin.permissions = Some(std::sync::Arc::clone(&permissions));
        permissions.set_permission("Admin", crate::permission::COMMAND_SEED, false);
        let output = handle_command("/seed", &admin, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "§cNo tienes permiso para ejecutar el comando: seed"
        );
    }

    #[test]
    fn permission_nodes_gate_self_and_other_variants() {
        let mut source = player_source("Guest", (0.0, 64.0, 0.0));
        source.is_op = false;
        source.allow_cheats = false;
        let permissions = std::sync::Arc::new(crate::permission::PermissionManager::new());
        source.permissions = Some(std::sync::Arc::clone(&permissions));

        fn denied(output: &CommandOutput) -> bool {
            output.feedback.iter().any(|line| line.contains("permiso"))
        }

        // `clear.self` is granted to everyone, `clear.other` only to operators.
        let output = handle_command("/clear", &source, &players()).expect("native");
        assert!(!denied(&output));
        let output = handle_command("/clear Alex", &source, &players()).expect("native");
        assert!(denied(&output));
        permissions.set_permission("Guest", crate::permission::COMMAND_CLEAR_OTHER, true);
        let output = handle_command("/clear Alex", &source, &players()).expect("native");
        assert!(!denied(&output));

        // Every whitelist subcommand has its own node.
        let subcommands = [
            ("list", crate::permission::COMMAND_WHITELIST_LIST),
            ("on", crate::permission::COMMAND_WHITELIST_ENABLE),
            ("off", crate::permission::COMMAND_WHITELIST_DISABLE),
            ("reload", crate::permission::COMMAND_WHITELIST_RELOAD),
            ("add Steve", crate::permission::COMMAND_WHITELIST_ADD),
            ("remove Steve", crate::permission::COMMAND_WHITELIST_REMOVE),
        ];
        for (subcommand, node) in subcommands {
            let command = format!("/whitelist {subcommand}");
            let output = handle_command(&command, &source, &players()).expect("native");
            assert!(denied(&output), "{command} should require {node}");
            permissions.set_permission("Guest", node, true);
            let output = handle_command(&command, &source, &players()).expect("native");
            assert!(!denied(&output), "{command} should be allowed by {node}");
        }
    }

    #[test]
    fn permission_gate_accepts_either_self_or_other() {
        let mut source = player_source("Guest", (0.0, 64.0, 0.0));
        source.is_op = false;
        source.allow_cheats = false;
        let permissions = std::sync::Arc::new(crate::permission::PermissionManager::new());
        source.permissions = Some(std::sync::Arc::clone(&permissions));

        // `setPermissions([self, other])`: only `.other` already runs the command.
        permissions.set_permission("Guest", crate::permission::COMMAND_GAMEMODE_OTHER, true);
        let output = handle_command("/gms Alex", &source, &players()).expect("native");
        assert!(!output.feedback.iter().any(|line| line.contains("permiso")));
        assert_eq!(output.actions.len(), 1);

        // ...but changing the own mode still needs `.self`.
        let output = handle_command("/gms", &source, &players()).expect("native");
        assert!(output.feedback.iter().any(|line| line.contains("permiso")));
        assert!(output.actions.is_empty());

        // And the opposite direction: `.self` alone cannot touch another player.
        let mut other = player_source("Guest2", (0.0, 64.0, 0.0));
        other.is_op = false;
        other.allow_cheats = false;
        other.permissions = Some(std::sync::Arc::clone(&permissions));
        permissions.set_permission("Guest2", crate::permission::COMMAND_GAMEMODE_SELF, true);
        let output = handle_command("/gms", &other, &players()).expect("native");
        assert!(!output.feedback.iter().any(|line| line.contains("permiso")));
        assert_eq!(output.actions.len(), 1);
        let output = handle_command("/gms Alex", &other, &players()).expect("native");
        assert!(output.feedback.iter().any(|line| line.contains("permiso")));
        assert!(output.actions.is_empty());
    }

    #[test]
    fn help_paginates_like_pocketmine() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        let output = handle_command("/help", &source, &players()).expect("native");
        let pages = COMMANDS.len().div_ceil(PLAYER_SCREEN_LINE_HEIGHT);
        assert_eq!(
            output.feedback[0],
            format!("--- Mostrando página de ayuda 1 de {pages} (/help [página]) ---")
        );
        assert_eq!(output.feedback.len(), 1 + PLAYER_SCREEN_LINE_HEIGHT);
        assert!(output.feedback[1].starts_with("§2/ban: §r"));

        // A page past the end falls back to the last one.
        let output = handle_command("/help 99", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            format!("--- Mostrando página de ayuda {pages} de {pages} (/help [página]) ---")
        );

        // The console reports PHP_INT_MAX lines, so it lists everything.
        let console = CommandSource::console(1_000);
        let output = handle_command("/help", &console, &players()).expect("native");
        assert_eq!(output.feedback.len(), 1 + COMMANDS.len());
    }

    #[test]
    fn help_describes_a_single_command_with_pocketmine_keys() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));
        let info = find_command("tp").expect("tp is native");

        let output = handle_command("/help tp", &source, &players()).expect("native");
        assert_eq!(
            output.feedback,
            vec![
                "§e--------- §rAyuda: /tp§e ---------".to_string(),
                format!("§6Descripción: §r{}", info.description),
                format!("§6Uso: §r{}", info.usage),
                "§6Alias: §rteleport".to_string(),
            ]
        );

        let output = handle_command("/help nope", &source, &players()).expect("native");
        assert_eq!(
            output.feedback,
            vec![concat!(
                "§cComando desconocido: nope. Utilice /help para obtener una lista de ",
                "comandos disponibles."
            )]
        );
    }

    #[test]
    fn help_hides_commands_the_sender_cannot_use() {
        let mut guest = player_source("Guest", (0.0, 64.0, 0.0));
        guest.is_op = false;
        guest.allow_cheats = false;
        guest.permissions = Some(std::sync::Arc::new(crate::permission::PermissionManager::new()));

        let output = handle_command("/help op", &guest, &players()).expect("native");
        assert_eq!(
            output.feedback,
            vec![concat!(
                "§cComando desconocido: op. Utilice /help para obtener una lista de ",
                "comandos disponibles."
            )]
        );

        let output = handle_command("/help help", &guest, &players()).expect("native");
        assert_eq!(output.feedback[0], "§e--------- §rAyuda: /help§e ---------");
        assert_eq!(output.feedback[3], "§6Alias: §r?");
    }

    #[test]
    fn list_reports_count_max_and_sorted_names() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        let output = handle_command("/list", &source, &players()).expect("native");
        assert_eq!(
            output.feedback,
            vec![
                "Hay 2/1000 jugadores en línea:".to_string(),
                "Alex, Steve".to_string()
            ]
        );

        let output = handle_command("/list", &source, &[]).expect("native");
        assert_eq!(
            output.feedback,
            vec!["Hay 0/1000 jugadores en línea:".to_string(), String::new()]
        );
    }

    #[test]
    fn title_builds_the_same_packets_as_pocketmine() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        let output =
            handle_command("/title Alex title Hola mundo", &source, &players()).expect("native");
        assert_eq!(output.feedback, vec![lang::TITLE_SUCCESS.to_string()]);
        assert_eq!(
            output.actions,
            vec![CommandAction::SendTitle {
                target: "Alex".to_string(),
                action: TitleAction::Title("Hola mundo".to_string()),
            }]
        );

        for (input, expected) in [
            ("/title Alex clear", TitleAction::Clear),
            ("/title Alex reset", TitleAction::Reset),
            (
                "/title Alex subtitle Hola",
                TitleAction::Subtitle("Hola".to_string()),
            ),
            (
                "/title Alex actionbar Eco",
                TitleAction::ActionBar("Eco".to_string()),
            ),
            (
                "/title Alex times 10 20 30",
                TitleAction::Times {
                    fade_in: 10,
                    stay: 20,
                    fade_out: 30,
                },
            ),
        ] {
            let output = handle_command(input, &source, &players()).expect("native");
            assert_eq!(
                output.actions,
                vec![CommandAction::SendTitle {
                    target: "Alex".to_string(),
                    action: expected,
                }],
                "{input}"
            );
        }

        // Negative durations are ignored by `setTitleDuration()`.
        let output =
            handle_command("/title Alex times -1 20 30", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(output.feedback, vec![lang::TITLE_SUCCESS.to_string()]);

        let usage_line =
            "§cUso: /title <jugador> <clear|reset|title|subtitle|actionbar|times> ...";
        for input in [
            "/title Alex",
            "/title Alex actionbar",
            "/title Alex nope",
            "/title Alex times 1 2",
            "/title Alex times 1 2 muchas",
        ] {
            let output = handle_command(input, &source, &players()).expect("native");
            assert_eq!(output.feedback, vec![usage_line.to_string()], "{input}");
            assert!(output.actions.is_empty(), "{input}");
        }
    }

    #[test]
    fn transferserver_matches_the_pocketmine_command() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        let output = handle_command("/transferserver play.example.net", &source, &players())
            .expect("native");
        assert!(output.feedback.is_empty());
        assert_eq!(
            output.actions,
            vec![CommandAction::Transfer {
                target: "Steve".to_string(),
                address: "play.example.net".to_string(),
                port: 19132,
            }]
        );

        // `(int)` cast: "abc" is 0 and the port is truncated to 16 bits.
        let output = handle_command("/transferserver host 25565", &source, &players())
            .expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Transfer {
                target: "Steve".to_string(),
                address: "host".to_string(),
                port: 25565,
            }]
        );
        let output =
            handle_command("/transferserver host abc", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Transfer {
                target: "Steve".to_string(),
                address: "host".to_string(),
                port: 0,
            }]
        );

        let output = handle_command("/transferserver", &source, &players()).expect("native");
        assert_eq!(
            output.feedback,
            vec!["§cUso: /transferserver <servidor> [puerto]".to_string()]
        );
        assert!(output.actions.is_empty());

        let console = CommandSource::console(1_000);
        let output = handle_command("/transferserver host", &console, &players()).expect("native");
        assert_eq!(
            output.feedback,
            vec!["§cEsta sintaxis de comando solo puede ser usada como jugador".to_string()]
        );
        assert!(output.actions.is_empty());
    }

    #[test]
    fn unknown_commands_fall_through_to_plugins() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));
        assert!(handle_command("/hello", &source, &players()).is_none());
        assert!(handle_command("hello", &source, &players()).is_none());
    }

    #[test]
    fn console_can_run_commands_without_the_players_position() {
        let console = CommandSource::console(1_000);

        let output = handle_command("tp Steve 0 80 0", &console, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Teleport {
                target: "Steve".to_string(),
                position: (0.0, 80.0, 0.0),
                yaw: None,
                pitch: None,
            }]
        );

        let output = handle_command("say hola", &console, &players()).expect("native");
        assert_eq!(output.actions.len(), 1);

        let output = handle_command("tp", &console, &players()).expect("native");
        assert!(output.feedback[0].contains("Uso:"));
    }
}
