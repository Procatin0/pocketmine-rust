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
    /// Canonical identity: what command actions carry and what the registry
    /// is keyed by (the raw wire username of a Java player).
    pub name: String,
    /// Name shown to players: a Java player already carries its `.` prefix
    /// here (`java_bridge::display::java_player_name`), so every surface
    /// (chat, TAB, nametags, `/list`, feedback) shows the same name.
    pub display_name: String,
    pub position: (f64, f64, f64),
    pub gamemode: i32,
}

/// Strips the Java display prefix (`JAVA_PREFIX_IN_BEDROCK`), so `.Steve`
/// and `Steve` name the same player in command input.
fn java_name_without_prefix(name: &str) -> &str {
    match name.strip_prefix(java_bridge::display::JAVA_PREFIX_IN_BEDROCK) {
        Some(rest) if !rest.is_empty() => rest,
        _ => name,
    }
}

/// Name comparison that ignores the Java `.` display prefix: a Java player is
/// named `.Steve`, so both `/gamemode creative Steve` and
/// `/gamemode creative .Steve` must resolve to that same target.
fn same_player_name(left: &str, right: &str) -> bool {
    java_name_without_prefix(left).eq_ignore_ascii_case(java_name_without_prefix(right))
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

    /// English display name for command feedback.
    pub fn name_en(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Rain => "rain",
            Self::Thunder => "thunder",
        }
    }

    /// Historical Spanish accessor, kept for API compatibility. New code
    /// should use [`WeatherState::name_en`]; this returns the English name.
    pub fn name_es(self) -> &'static str {
        self.name_en()
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

/// Scoreboard mutation requested by `/scoreboard` (Rust extension over
/// the PM `Player::setScoreboard()` API; Bedrock's own `/scoreboard`
/// command manages server objectives instead, which this port does not
/// expose as a command).
#[derive(Debug, Clone, PartialEq)]
pub enum ScoreboardOperation {
    /// Remove every objective (`Player::removeScoreboard()`).
    Clear,
    /// Create/replace an objective (empty lines).
    AddObjective(crate::scoreboard::ScoreboardObjective),
    /// Remove one objective by name.
    RemoveObjective(String),
    /// Add/replace one line.
    AddScore {
        objective_name: String,
        score: u32,
        text: String,
    },
    /// Remove one line by score.
    RemoveScore { objective_name: String, score: u32 },
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
    /// `/scoreboard`: one mutation on the target's scoreboard (`global`
    /// targets the persisted default scoreboard instead).
    Scoreboard {
        target: String,
        operation: ScoreboardOperation,
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
    /// `/time start|stop`: `World::startTime()/stopTime()` on every world.
    SetTimeRunning {
        running: bool,
        /// `Restarted the time` / `Stopped the time`, already filled in.
        message: String,
        notify: String,
    },
    /// `/effect`: apply or remove a mob effect. `duration_secs: None` with
    /// `remove=false` means infinite. Session applies the MobEffect packet
    /// (currently a logged no-op until the protocol grows it).
    ApplyEffect {
        target: String,
        effect: String,
        duration_secs: Option<i32>,
        amplifier: i32,
        particles: bool,
        remove: bool,
        message: String,
        notify: String,
    },
    /// `/enchant`: enchant the held item. `enchantment` is the raw id;
    /// validation (max level, item compatibility) happens in the session.
    Enchant {
        target: String,
        enchantment: String,
        level: i32,
        message: String,
        notify: String,
    },
    /// `/xp`: `amount` signed; `is_levels` selects levels (`L` suffix) vs xp.
    AddXp {
        target: String,
        amount: i32,
        is_levels: bool,
        message: String,
        notify: String,
    },
    /// `/particle`: visual-only; the session logs it until a
    /// `LevelEvent`/`Particle` packet path exists.
    SpawnParticle {
        name: String,
        position: (f64, f64, f64),
        delta: (f64, f64, f64),
        count: i32,
        data: Option<String>,
        message: String,
    },
    /// `/timings <on|off|reset|report|paste>`: `mode` is the literal arg.
    /// `message` is the `broadcastCommandMessage` line (already filled).
    Timings {
        mode: String,
        message: String,
        notify: String,
    },
    /// `/gc`: fire garbage collection. The session counts collected chunks
    /// and entities; the command itself only reports the header here.
    GarbageCollect {
        notify: String,
    },
    /// `/dumpmemory [path]`: dump a memory snapshot. `path` is the requested
    /// output (the session fills the default when `None`).
    DumpMemory {
        path: Option<String>,
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
        usage: "/help [page|command name]",
        description: "Shows the help menu",
    },
    CommandInfo {
        name: "tp",
        aliases: &["teleport"],
        usage: "/tp [target player] <destination player> OR /tp [target player] <x> <y> <z> [<y-rot> <x-rot>]",
        description: "Teleports the given player (or yourself) to another player or coordinates",
    },
    CommandInfo {
        name: "gamemode",
        aliases: &["gm"],
        usage: "/gamemode <mode> [player]",
        description: "Changes the player to a specific game mode",
    },
    CommandInfo {
        name: "gmc",
        aliases: &[],
        usage: "/gmc [player]",
        description: "Shortcut for /gamemode creative",
    },
    CommandInfo {
        name: "gms",
        aliases: &[],
        usage: "/gms [player]",
        description: "Shortcut for /gamemode survival",
    },
    CommandInfo {
        name: "gma",
        aliases: &[],
        usage: "/gma [player]",
        description: "Shortcut for /gamemode adventure",
    },
    CommandInfo {
        name: "gmsp",
        aliases: &[],
        usage: "/gmsp [player]",
        description: "Shortcut for /gamemode spectator",
    },
    CommandInfo {
        name: "spawn",
        aliases: &[],
        usage: "/spawn [player]",
        description: "Teleports to the world spawn point",
    },
    CommandInfo {
        name: "setworldspawn",
        aliases: &[],
        usage: "/setworldspawn [<x> <y> <z>]",
        description: "Sets a world's spawn point. If no coordinates are specified, the player's coordinates will be used.",
    },
    CommandInfo {
        name: "say",
        aliases: &[],
        usage: "/say <message ...>",
        description: "Broadcasts the given message as the sender",
    },
    CommandInfo {
        name: "me",
        aliases: &[],
        usage: "/me <action ...>",
        description: "Performs the specified action in chat",
    },
    CommandInfo {
        name: "tell",
        aliases: &["msg", "w", "whisper"],
        usage: "/tell <player> <private message ...>",
        description: "Sends a private message to the given player",
    },
    CommandInfo {
        name: "list",
        aliases: &["players"],
        usage: "/list",
        description: "Lists all online players",
    },
    CommandInfo {
        name: "kick",
        aliases: &[],
        usage: "/kick <player> [reason ...]",
        description: "Removes the specified player from the server",
    },
    CommandInfo {
        name: "give",
        aliases: &[],
        usage: "/give <player> <item[:damage]> [amount] [tags...]",
        description: "Gives the specified player a certain amount of items",
    },
    CommandInfo {
        name: "clear",
        aliases: &[],
        usage: "/clear [player] [item] [maxCount]",
        description: "Clears items from player inventory",
    },
    CommandInfo {
        name: "kill",
        aliases: &["suicide"],
        usage: "/kill [player]",
        description: "Commit suicide or kill other players",
    },
    CommandInfo {
        name: "spawnpoint",
        aliases: &[],
        usage: "/spawnpoint [player] [<x> <y> <z>]",
        description: "Sets a player's spawn point",
    },
    CommandInfo {
        name: "defaultgamemode",
        aliases: &[],
        usage: "/defaultgamemode <mode>",
        description: "Set the default gamemode",
    },
    CommandInfo {
        name: "difficulty",
        aliases: &[],
        usage: "/difficulty <new difficulty>",
        description: "Sets the game difficulty",
    },
    CommandInfo {
        name: "seed",
        aliases: &[],
        usage: "/seed",
        description: "Shows the world seed",
    },
    CommandInfo {
        name: "ban",
        aliases: &[],
        usage: "/ban <name> [reason ...]",
        description: "Prevents the specified player from using this server",
    },
    CommandInfo {
        name: "ban-ip",
        aliases: &[],
        usage: "/ban-ip <address|name> [reason ...]",
        description: "Prevents the specified IP address from using this server",
    },
    CommandInfo {
        name: "banlist",
        aliases: &[],
        usage: "/banlist [ips|players]",
        description: "View all players banned from this server",
    },
    CommandInfo {
        name: "pardon",
        aliases: &["unban"],
        usage: "/pardon <name>",
        description: "Allows the specified player to use this server",
    },
    CommandInfo {
        name: "pardon-ip",
        aliases: &["unban-ip"],
        usage: "/pardon-ip <address>",
        description: "Allows the specified IP address to use this server",
    },
    CommandInfo {
        name: "whitelist",
        aliases: &[],
        usage: "/whitelist <on|off|list|add|remove|reload>",
        description: "Manages the list of players allowed to use this server",
    },
    CommandInfo {
        name: "stop",
        aliases: &[],
        usage: "/stop",
        description: "Stops the server",
    },
    CommandInfo {
        name: "plugins",
        aliases: &["pl"],
        usage: "/plugins",
        description: "Gets a list of plugins running on the server",
    },
    CommandInfo {
        name: "save-all",
        aliases: &[],
        usage: "/save-all",
        description: "Saves the server to disk",
    },
    CommandInfo {
        name: "save-on",
        aliases: &[],
        usage: "/save-on",
        description: "Enables server autosaving",
    },
    CommandInfo {
        name: "save-off",
        aliases: &[],
        usage: "/save-off",
        description: "Disables server autosaving",
    },
    CommandInfo {
        name: "tps",
        aliases: &[],
        usage: "/tps",
        description: "Shows server TPS in 1m, 5m and 15m (Paper extra, use /status for PocketMine format)",
    },
    CommandInfo {
        name: "status",
        aliases: &[],
        usage: "/status",
        description: "Reads back the server's performance.",
    },
    CommandInfo {
        name: "effect",
        aliases: &[],
        usage: "/effect <player> <effect> [seconds] [amplifier] [hideParticles] OR /effect <player> clear OR /effect <player> <effect> infinite [amplifier] [hideParticles]",
        description: "Adds/Removes effects on players",
    },
    CommandInfo {
        name: "enchant",
        aliases: &[],
        usage: "/enchant <player> <enchantment ID> [level]",
        description: "Adds enchantments on items",
    },
    CommandInfo {
        name: "xp",
        aliases: &[],
        usage: "/xp <experience[L]> [player]",
        description: "Adds or removes player experience",
    },
    CommandInfo {
        name: "particle",
        aliases: &[],
        usage: "/particle <name> <x> <y> <z> <xd> <yd> <zd> [count] [data]",
        description: "Adds particles to a world",
    },
    CommandInfo {
        name: "timings",
        aliases: &[],
        usage: "/timings <reset|report|on|off|paste>",
        description: "Records timings to see performance of the server.",
    },
    CommandInfo {
        name: "gc",
        aliases: &[],
        usage: "/gc",
        description: "Fires garbage collection tasks",
    },
    CommandInfo {
        name: "dumpmemory",
        aliases: &[],
        usage: "/dumpmemory [path]",
        description: "Dumps diagnostic information about memory usage to disk",
    },
    CommandInfo {
        name: "time",
        aliases: &[],
        usage: "/time <set|add> <value> OR /time <start|stop|query>",
        description: "Changes the time on each world",
    },
    CommandInfo {
        name: "weather",
        aliases: &[],
        usage: "/weather <clear|rain|thunder> [duration]",
        description: "Changes world weather",
    },
    CommandInfo {
        name: "op",
        aliases: &[],
        usage: "/op <player>",
        description: "Gives the specified player operator status",
    },
    CommandInfo {
        name: "deop",
        aliases: &[],
        usage: "/deop <player>",
        description: "Takes the specified player's operator status",
    },
    CommandInfo {
        name: "version",
        aliases: &["ver", "about"],
        usage: "/version [plugin name]",
        description: "Gets the version of this server including any plugins in use",
    },
    CommandInfo {
        name: "title",
        aliases: &[],
        usage: "/title <player> <clear|reset|title|subtitle|actionbar|times> ...",
        description: "Controls screen titles",
    },
    CommandInfo {
        name: "scoreboard",
        aliases: &["sb"],
        usage: "/scoreboard <player|global> <clear|add|remove|set|list> ...",
        description: "Manages scoreboard objectives and lines",
    },
    CommandInfo {
        name: "transferserver",
        aliases: &[],
        usage: "/transferserver <server> [port]",
        description: "Transfer yourself to another server",
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
        "status" => Some(status(source, players)),
        "effect" => Some(effect(source, players, &args)),
        "enchant" => Some(enchant(source, players, &args)),
        "xp" => Some(xp(source, players, &args)),
        "particle" => Some(particle(source, players, &args)),
        "timings" => Some(timings(source, &args)),
        "gc" => Some(gc(source)),
        "dumpmemory" => Some(dumpmemory(source, &args)),
        "time" => Some(time(source, &args)),
        "title" => Some(title(source, players, &args)),
        "scoreboard" | "sb" => Some(scoreboard(source, players, &args)),
        "transferserver" => Some(transfer(source, &args)),
        "weather" => Some(weather(source, &args)),
        "op" => Some(set_op(&args, true)),
        "deop" => Some(set_op(&args, false)),
        "version" | "ver" | "about" => Some(version(source, &args)),
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
            crate::permission::COMMAND_TIME_START,
            crate::permission::COMMAND_TIME_STOP,
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
        .any(|name| same_player_name(name, &source.name));
    let has_other = target_names
        .iter()
        .any(|name| !same_player_name(name, &source.name));
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
    if same_player_name(target, &source.name) {
        match destination_name {
            Some(name) => format!("§aTeleported to §f{}.", name),
            None => match (yaw, pitch) {
                (Some(yaw), Some(pitch)) => format!(
                    "§aTeleported to §f{:.1}, {:.1}, {:.1} (§f{}° yaw, {}° pitch§a).",
                    x, y, z, yaw, pitch
                ),
                _ => format!("§aTeleported to §f{:.1}, {:.1}, {:.1}§a.", x, y, z),
            },
        }
    } else {
        match destination_name {
            Some(name) => format!("§aTeleporting §f{} §ato §f{}.", target, name),
            None => match (yaw, pitch) {
                (Some(yaw), Some(pitch)) => format!(
                    "§aTeleporting §f{} §ato §f{:.1}, {:.1}, {:.1} (§f{}° yaw, {}° pitch§a).",
                    target, x, y, z, yaw, pitch
                ),
                _ => format!(
                    "§aTeleporting §f{} §ato §f{:.1}, {:.1}, {:.1}§a.",
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
    // (identity, display name): actions carry the identity the registry is
    // keyed by, feedback shows the name the player actually has (a Java
    // player already carries its `.` prefix in `display_name`).
    let targets: Vec<(String, String)> = match target_arg {
        Some(query) => {
            let resolved = resolve_players(source, players, query);
            if resolved.is_empty() {
                return player_not_found(query);
            }
            resolved
                .iter()
                .map(|player| (player.name.clone(), player.display_name.clone()))
                .collect()
        }
        None => {
            if source.is_console {
                return error("§cYou must specify a player.");
            }
            vec![(source.name.clone(), source.name.clone())]
        }
    };

    // `fetchPermittedPlayerTarget` permission split over the whole set: any
    // `other` target needs `.other`, any self target needs `.self`.
    let has_self = targets
        .iter()
        .any(|(name, _)| same_player_name(name, &source.name));
    let has_other = targets
        .iter()
        .any(|(name, _)| !same_player_name(name, &source.name));
    if (has_self && !source.can(crate::permission::COMMAND_GAMEMODE_SELF))
        || (has_other && !source.can(crate::permission::COMMAND_GAMEMODE_OTHER))
    {
        return permission_for("gamemode");
    }

    let mode_name = gamemode_translatable(gamemode);
    let mut feedback_lines = Vec::new();
    let mut actions = Vec::new();
    for (target, display) in targets {
        // PocketMine reports a failure when the target already has that mode.
        let current = players
            .iter()
            .find(|player| same_player_name(&player.name, &target))
            .map(|player| player.gamemode);
        if current == Some(gamemode) {
            feedback_lines.push(lang::GAMEMODE_FAILURE.replace("{%playerName}", &display));
            continue;
        }
        let success = if same_player_name(&target, &source.name) {
            fill_template(lang::GAMEMODE_SUCCESS_SELF, &[mode_name])
        } else {
            fill_template(lang::GAMEMODE_SUCCESS_OTHER, &[mode_name, &display])
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
                return error("§cYou must specify a player.");
            }
            vec![source.name.clone()]
        }
    };

    let mut feedback = Vec::new();
    let mut actions = Vec::new();
    for target in targets {
        feedback.push(format!(
            "§a{} to the world spawn point.",
            if same_player_name(&target, &source.name) {
                "Teleported"
            } else {
                "Player teleported"
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
            "§aWorld spawn point set to §f{}, {}, {}§a.",
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
    if same_player_name(&target.name, &source.name) {
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
    // Java players are listed with the same `.`-prefixed name they carry in
    // chat, TAB and nametags (`display_name`).
    let mut names: Vec<&str> = players
        .iter()
        .map(|player| player.display_name.as_str())
        .collect();
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
    let Some(query) = args.first() else {
        return usage("kick");
    };
    let targets = resolve_players(source, players, query);
    if targets.is_empty() {
        return player_not_found(query);
    }

    let reason = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        String::new()
    };
    let mut feedback = Vec::new();
    let mut actions = Vec::new();
    for target in targets {
        let message = if reason.is_empty() {
            fill_template(lang::KICK_SUCCESS, &[&target.name])
        } else {
            fill_template(lang::KICK_SUCCESS_REASON, &[&target.name, &reason])
        };
        feedback.push(message);
        actions.push(CommandAction::Kick {
            target: target.name.clone(),
            reason: reason.clone(),
        });
    }
    CommandOutput { feedback, actions }
}

/// Vanilla translation strings, copied verbatim from PocketMine-MP's
/// `eng.ini` (the runtime `eng` locale) so the output matches the PHP server
/// key for key.
pub mod lang {
    // Values mirror `resources/eng.ini` (the runtime `eng` locale).
    pub const USAGE: &str = "Usage: {%0}";
    pub const PERMISSION: &str = "You don't have permission to use the command: {%commandName}";
    pub const PLAYER_NOT_FOUND: &str = "Cannot find a player matching \"{%playerName}\"";
    pub const GIVE_ITEM_NOT_FOUND: &str = "There is no such item with name {%0}";
    pub const GIVE_SUCCESS: &str = "Given {%0} * {%1} to {%2}";
    pub const GIVE_TAG_ERROR: &str = "Data tag parsing failed: {%0}";
    pub const SPAWNPOINT_SUCCESS: &str = "Set {%0}'s spawn point to ({%1}, {%2}, {%3})";
    pub const DEFAULTGAMEMODE_SUCCESS: &str = "The world's default game mode is now {%0}";
    pub const DIFFICULTY_SUCCESS: &str = "Set game difficulty to {%0}";
    pub const SEED_SUCCESS: &str = "Seed: {%0}";
    pub const GAMEMODE_UNKNOWN: &str = "Unknown game mode \"{%gameModeName}\"";
    pub const GAMEMODE_FAILURE: &str = "Game mode change for {%playerName} failed!";
    pub const GAMEMODE_SUCCESS_SELF: &str = "Set own game mode to {%0}";
    pub const GAMEMODE_SUCCESS_OTHER: &str = "Set {%1}'s game mode to {%0}";
    pub const GAMEMODE_CHANGED: &str = "Your game mode has been updated to {%0}";
    pub const BAN_SUCCESS: &str = "Banned player {%0}";
    pub const BAN_USAGE: &str = "/ban <name> [reason ...]";
    pub const BANIP_INVALID: &str = "You have entered an invalid IP address or a player that is not online";
    pub const BANIP_SUCCESS: &str = "Banned IP address {%0}";
    pub const BANIP_SUCCESS_PLAYERS: &str = "Banned IP address {%0} belonging to {%1}";
    pub const BANIP_USAGE: &str = "/ban-ip <address|name> [reason ...]";
    pub const BANLIST_IPS: &str = "There are {%0} total banned IP addresses:";
    pub const BANLIST_PLAYERS: &str = "There are {%0} total banned players:";
    pub const BANLIST_USAGE: &str = "/banlist [ips|players]";
    pub const UNBAN_SUCCESS: &str = "Unbanned player {%0}";
    pub const UNBAN_USAGE: &str = "/pardon <name>";
    pub const UNBANIP_SUCCESS: &str = "Unbanned IP address {%0}";
    pub const UNBANIP_INVALID: &str = "You have entered an invalid IP address";
    pub const UNBANIP_USAGE: &str = "/pardon-ip <address>";
    pub const WHITELIST_ADD_SUCCESS: &str = "Added {%0} to the whitelist";
    pub const WHITELIST_ADD_USAGE: &str = "/whitelist add <player>";
    pub const WHITELIST_DISABLED: &str = "Turned off the whitelist";
    pub const WHITELIST_ENABLED: &str = "Turned on the whitelist";
    pub const WHITELIST_LIST: &str = "There are {%0} (out of {%1} seen) whitelisted players:";
    pub const WHITELIST_RELOADED: &str = "Reloaded the whitelist";
    pub const WHITELIST_REMOVE_SUCCESS: &str = "Removed {%0} from the whitelist";
    pub const WHITELIST_REMOVE_USAGE: &str = "/whitelist remove <player>";
    pub const WHITELIST_USAGE: &str = "/whitelist <on|off|list|add|remove|reload>";
    pub const STOP_START: &str = "Stopping the server";
    pub const DISCONNECT_BAN: &str = "You are banned from this server. Reason: {%reason}";
    pub const DISCONNECT_BAN_NOREASON: &str = "You are banned from this server";
    pub const DISCONNECT_BAN_IP: &str = "IP banned";
    pub const DISCONNECT_WHITELISTED: &str = "Server is whitelisted";
    pub const DISCONNECT_KICK: &str = "Kicked by admin. Reason: {%reason}";
    pub const PLUGINS_SUCCESS: &str = "Plugins ({%0}): {%1}";
    pub const SAVE_START: &str = "Saving server data...";
    pub const SAVE_SUCCESS: &str = "Save completed in {%0} seconds";
    pub const SAVE_ENABLED: &str = "Turned on world auto-saving";
    pub const SAVE_DISABLED: &str = "Turned off world auto-saving";
    pub const CLEAR_SUCCESS: &str = "Cleared the inventory of {%0}, removing {%1} items";
    pub const CLEAR_FAILURE_NO_ITEMS: &str = "Could not clear inventory of {%0}, no items to remove";
    pub const CLEAR_TESTING: &str = "{%0} has {%1} items that match the criteria";
    pub const KILL_SUCCESSFUL: &str = "Killed {%0}";
    pub const NUM_TOO_BIG: &str = "The number you have entered ({%0}) is too big, it must be at most {%1}";
    pub const NUM_TOO_SMALL: &str = "The number you have entered ({%0}) is too small, it must be at least {%1}";
    pub const CHAT_ANNOUNCEMENT: &str = "[{%0}] {%1}";
    pub const CHAT_ADMIN: &str = "[{%0}: {%1}]";
    pub const CHAT_EMOTE: &str = "* {%0} {%1}";
    pub const MESSAGE_OUTGOING: &str = "You whisper to {%0}: {%1}";
    pub const MESSAGE_INCOMING: &str = "{%0} whispers to you: {%1}";
    pub const MESSAGE_SAME_TARGET: &str = "You can't send a private message to yourself!";
    pub const KICK_SUCCESS: &str = "Kicked {%0} from the game";
    pub const KICK_SUCCESS_REASON: &str = "Kicked {%0} from the game: '{%1}'";
    pub const TIME_SET: &str = "Set the time to {%0}";
    pub const TIME_ADDED: &str = "Added {%0} to the time";
    pub const TIME_QUERY: &str = "Time is {%0}";
    pub const TITLE_SUCCESS: &str = "Title command successfully executed";
    pub const SCOREBOARD_SUCCESS: &str = "Scoreboard command successfully executed";
    pub const PLAYER_USER_ONLY: &str = "This command syntax can only be used as a player";
    pub const VERSION_SOFTWARE_NAME: &str = "This server is running {%serverSoftwareName}";
    pub const VERSION_SOFTWARE_VERSION: &str = "Server version: {%serverSoftwareVersion} (git hash: {%serverGitHash})";
    pub const VERSION_MINECRAFT: &str = "Compatible Minecraft version: {%minecraftVersion} (protocol version: {%minecraftProtocolVersion})";
    pub const PLAYERS_LIST: &str = "There are {%0}/{%1} players online:";
    pub const HELP_HEADER: &str = "--- Showing help page {%0} of {%1} (/help [page]) ---";
    pub const HELP_SPECIFIC_HEADER: &str = "Help: /{%commandName}";
    pub const HELP_SPECIFIC_DESCRIPTION: &str = "Description: {%description}";
    pub const HELP_SPECIFIC_USAGE: &str = "Usage: {%usage}";
    pub const HELP_SPECIFIC_ALIASES: &str = "Aliases: {%aliasList}";
    pub const COMMAND_NOT_FOUND: &str = "Unknown command: {%commandName}. Use {%helpCommand} for a list of available commands.";
    // /effect (eng.ini: commands.effect.*)
    pub const EFFECT_NOT_FOUND: &str = "There is no such mob effect with ID {%0}";
    pub const EFFECT_SUCCESS_REMOVED_ALL: &str = "Took all effects from {%0}";
    pub const EFFECT_SUCCESS_REMOVED: &str = "Took {%0} from {%1}";
    pub const EFFECT_SUCCESS: &str = "Given {%0} * {%1} to {%2} for {%3} seconds";
    pub const EFFECT_SUCCESS_INFINITE: &str = "Given {%0} * {%1} to {%2} for an infinite duration";
    pub const EFFECT_FAILURE_NOT_ACTIVE_ALL: &str = "Couldn't take any effects from {%0} as they do not have any";
    pub const EFFECT_FAILURE_NOT_ACTIVE: &str = "Couldn't take {%0} from {%1} as they do not have the effect";
    // /enchant (eng.ini: commands.enchant.*)
    pub const ENCHANT_NO_ITEM: &str = "The target does not hold an item";
    pub const ENCHANT_NOT_FOUND: &str = "There is no such enchantment with ID {%0}";
    pub const ENCHANT_SUCCESS: &str = "Enchanting succeeded for {%0}";
    // /xp (eng.ini: commands.xp.*)
    pub const XP_FAILURE_WIDTHDRAW: &str = "Cannot give player negative experience points";
    pub const XP_SUCCESS: &str = "Gave {%0} experience to {%1}";
    pub const XP_SUCCESS_LEVELS: &str = "Gave {%0} experience levels to {%1}";
    pub const XP_SUCCESS_NEGATIVE_LEVELS: &str = "Taken {%0} levels from {%1}";
    // /particle (eng.ini: commands.particle.*)
    pub const PARTICLE_NOT_FOUND: &str = "Unknown effect name {%0}";
    pub const PARTICLE_SUCCESS: &str = "Playing effect {%0} for {%1} times";
    // /status (eng.ini: pocketmine.command.status.*)
    pub const STATUS_HEADER: &str = "Server status";
    pub const STATUS_UPTIME: &str = "Uptime: {%uptime}";
    pub const STATUS_UPTIME_SECONDS: &str = "{%seconds} seconds";
    pub const STATUS_UPTIME_MINUTES: &str = "{%minutes} minutes {%seconds} seconds";
    pub const STATUS_UPTIME_HOURS: &str = "{%hours} hours {%minutes} minutes {%seconds} seconds";
    pub const STATUS_UPTIME_DAYS: &str = "{%days} days {%hours} hours {%minutes} minutes {%seconds} seconds";
    pub const STATUS_TPS_CURRENT: &str = "Current TPS: {%info}";
    pub const STATUS_TPS_AVERAGE: &str = "Average TPS: {%info}";
    pub const STATUS_TPS_STAT: &str = "{%ticksPerSecond} ({%loadPercent}%)";
    pub const STATUS_NETWORK_UPLOAD: &str = "Network upload: {%info}";
    pub const STATUS_NETWORK_DOWNLOAD: &str = "Network download: {%info}";
    pub const STATUS_NETWORK_STAT: &str = "{%kbps} kB/s";
    pub const STATUS_THREADS: &str = "Thread count: {%count}";
    pub const STATUS_MEMORY_MAIN: &str = "Main thread memory: {%info}";
    pub const STATUS_MEMORY_TOTAL: &str = "Total memory: {%info}";
    pub const STATUS_MEMORY_VIRTUAL: &str = "Total virtual memory: {%info}";
    pub const STATUS_MEMORY_MANAGER: &str = "Maximum memory (manager): {%info}";
    pub const STATUS_MEMORY_STAT: &str = "{%megabytes} MB.";
    pub const STATUS_WORLD: &str = "World {%worldName}: {%loadedChunks} loaded chunks, {%tickingChunks} ticking chunks, {%entities} entities. Time {%timeStat}";
    pub const STATUS_WORLD_TIME_STAT: &str = "{%milliseconds}ms";
    // /timings (eng.ini: pocketmine.command.timings.*)
    pub const TIMINGS_ALREADY_ENABLED: &str = "Timings are already enabled";
    pub const TIMINGS_ENABLE: &str = "Enabled Timings & Reset";
    pub const TIMINGS_DISABLE: &str = "Disabled Timings";
    pub const TIMINGS_DISABLED: &str = "Please enable timings by typing /timings on";
    pub const TIMINGS_RESET: &str = "Timings reset";
    pub const TIMINGS_COLLECT: &str = "Collecting timing results...";
    pub const TIMINGS_WRITE: &str = "Timings written to {%0}";
    pub const TIMINGS_READ: &str = "You can read the results at {%0}";
    pub const TIMINGS_PASTE_ERROR: &str = "An error happened while pasting the report";
    pub const TIMINGS_USAGE: &str = "/timings <reset|report|on|off|paste>";
    // /gc (eng.ini: pocketmine.command.gc.*)
    pub const GC_HEADER: &str = "Garbage collection result";
    pub const GC_CHUNKS: &str = "Chunks: {%chunksCollected}";
    pub const GC_ENTITIES: &str = "Entities: {%entitiesCollected}";
    pub const GC_CYCLES: &str = "Cycles: {%cyclesCollected}";
    pub const GC_MEMORY_FREED: &str = "Memory freed: {%memoryFreed} MB";
    // /time start|stop (PM broadcasts these literally, untranslated)
    pub const TIME_RESTARTED: &str = "Restarted the time";
    pub const TIME_STOPPED: &str = "Stopped the time";
    // /version <plugin>
    pub const VERSION_NO_SUCH_PLUGIN: &str = "This server is not running any plugin by that name. Use /plugins to get a list of plugins.";
    pub const VERSION_PLUGIN_HEADER: &str = "{%pluginName} version {%version}";
    pub const VERSION_PLUGIN_AUTHOR: &str = "Author: {%author}";
    pub const VERSION_PLUGIN_AUTHORS: &str = "Authors: {%authors}";
    pub const VERSION_PLUGIN_WEBSITE: &str = "Website: {%website}";
    pub const TIME_USAGE: &str = "/time <set|add> <value> OR /time <start|stop|query>";
    pub const EFFECT_USAGE: &str = "/effect <player> <effect> [seconds] [amplifier] [hideParticles] OR /effect <player> clear OR /effect <player> <effect> infinite [amplifier] [hideParticles]";
    pub const ENCHANT_USAGE: &str = "/enchant <player> <enchantment ID> [level]";
    pub const XP_USAGE: &str = "/xp <experience[L]> [player]";
    pub const PARTICLE_USAGE: &str = "/particle <name> <x> <y> <z> <xd> <yd> <zd> [count] [data]";
    pub const VERSION_USAGE: &str = "/version [plugin name]";
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
fn player_by_prefix<'a>(players: &'a [OnlinePlayer], name: &str) -> Option<&'a OnlinePlayer> {
    let needle = java_name_without_prefix(name).to_lowercase();
    let mut prefixed: Option<&OnlinePlayer> = None;
    for player in players {
        let candidate = java_name_without_prefix(&player.name).to_lowercase();
        if candidate == needle {
            return Some(player);
        }
        if prefixed.is_none() && !needle.is_empty() && candidate.starts_with(&needle) {
            prefixed = Some(player);
        }
    }
    prefixed
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
            .filter(|player| same_player_name(&player.name, &source.name))
            .collect(),
        _ => players.iter().collect(),
    };
    if let Some(name_filter) = &args.name {
        pool.retain(|player| same_player_name(&player.name, name_filter));
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
    if matches!(kind, SelectorKind::AllEntities) {
        tracing::warn!(
            "Selector @e: no non-player entities in the Bedrock runtime, returning {} player(s) as entities (type filters other than player match nothing)",
            pool.len()
        );
    }
    pool
}

/// `VanillaCommand::fetchPermittedPlayerTarget` extended with `@` selectors.
///
/// - `@s` (or no argument) resolves to the sender.
/// - `@p`/`@n` nearest player to the sender (0,0,0 for the console unless
///   `x`/`y`/`z` override it), `@a`/`@c` all players, `@e` all entities
///   (players only for now, see [`resolve_selector_targets`]), `@r` random.
/// - `[...]` arguments: `distance=min..max`, `name=`, `type=`, `x`/`y`/`z`,
///   `limit=`, `sort=nearest|furthest|arbitrary|random`, `gamemode=`.
///   Unknown keys are ignored.
///
/// Permission rule for multiple targets: everyone who is the sender needs
/// `self_allowed`, anyone else needs `other_allowed`.
fn fetch_permitted_player_targets<'a>(
    source: &CommandSource,
    players: &'a [OnlinePlayer],
    target: Option<&str>,
    command: &str,
    self_allowed: bool,
    other_allowed: bool,
) -> Result<Vec<&'a OnlinePlayer>, CommandOutput> {
    let targets: Vec<&OnlinePlayer> = match target {
        None => {
            if source.is_console {
                // PocketMine throws InvalidCommandSyntaxException here.
                return Err(usage(command));
            }
            match players
                .iter()
                .find(|player| same_player_name(&player.name, &source.name))
            {
                Some(player) => vec![player],
                None => {
                    return Err(error(red(fill_named(
                        lang::PLAYER_NOT_FOUND,
                        &[("playerName", &source.name)],
                    ))));
                }
            }
        }
        Some(query) if is_selector(query) => {
            let Some((kind, args)) = parse_selector(query, source.position) else {
                return Err(error(red(fill_named(
                    lang::PLAYER_NOT_FOUND,
                    &[("playerName", query)],
                ))));
            };
            let resolved = resolve_selector_targets(source, players, kind, &args);
            if resolved.is_empty() {
                return Err(error(red(fill_named(
                    lang::PLAYER_NOT_FOUND,
                    &[("playerName", query)],
                ))));
            }
            resolved
        }
        Some(name) => match player_by_prefix(players, name) {
            Some(player) => vec![player],
            None => {
                return Err(error(red(fill_named(
                    lang::PLAYER_NOT_FOUND,
                    &[("playerName", name)],
                ))));
            }
        },
    };

    let has_self = targets
        .iter()
        .any(|player| same_player_name(&player.name, &source.name));
    let has_other = targets
        .iter()
        .any(|player| !same_player_name(&player.name, &source.name));
    if has_self && !self_allowed {
        let message = lang::PERMISSION.replace("{%commandName}", command);
        return Err(error(red(message)));
    }
    if has_other && !other_allowed {
        let message = lang::PERMISSION.replace("{%commandName}", command);
        return Err(error(red(message)));
    }
    Ok(targets)
}

/// `/give <player> <item[:damage]> [amount] [tags...]`, ported one to one from
/// `pocketmine\command\defaults\GiveCommand`.
fn give(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() < 2 {
        return usage("give");
    }

    let self_allowed = source.can(crate::permission::COMMAND_GIVE_SELF);
    let other_allowed = source.can(crate::permission::COMMAND_GIVE_OTHER);
    let targets = match fetch_permitted_player_targets(
        source,
        players,
        Some(args[0]),
        "give",
        self_allowed,
        other_allowed,
    ) {
        Ok(targets) => targets,
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
    let mut feedback = Vec::new();
    let mut actions = Vec::new();
    for player in targets {
        let target = player.name.clone();
        let message = fill_template(lang::GIVE_SUCCESS, &[&label, &count.to_string(), &target]);
        feedback.push(message.clone());
        actions.push(CommandAction::Give {
            target,
            network_id,
            meta,
            count,
            nbt: nbt.clone(),
            message,
            notify: source.name.clone(),
        });
    }

    CommandOutput { feedback, actions }
}

/// `/clear [player] [item] [maxCount]`, ported one to one from
/// `pocketmine\command\defaults\ClearCommand`.
fn clear(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() > 3 {
        return usage("clear");
    }

    // `clear.self` is granted to everyone, `clear.other` to operators.
    let targets = match fetch_permitted_player_targets(
        source,
        players,
        args.first().copied(),
        "clear",
        source.can(crate::permission::COMMAND_CLEAR_SELF),
        source.can(crate::permission::COMMAND_CLEAR_OTHER),
    ) {
        Ok(targets) => targets,
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
    let actions = targets
        .iter()
        .map(|player| CommandAction::ClearInventory {
            target: player.name.clone(),
            item,
            max_count: max_count as i32,
            notify: source.name.clone(),
        })
        .collect();
    CommandOutput {
        feedback: Vec::new(),
        actions,
    }
}

/// `/kill [player]`, ported one to one from
/// `pocketmine\command\defaults\KillCommand`.
fn kill(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() >= 2 {
        return usage("kill");
    }

    // `kill.self` is granted to everyone, `kill.other` to operators.
    let targets = match fetch_permitted_player_targets(
        source,
        players,
        args.first().copied(),
        "kill",
        source.can(crate::permission::COMMAND_KILL_SELF),
        source.can(crate::permission::COMMAND_KILL_OTHER),
    ) {
        Ok(targets) => targets,
        Err(output) => return output,
    };

    let mut feedback = Vec::new();
    let mut actions = Vec::new();
    for player in targets {
        let target = player.name.clone();
        let message = fill_template(lang::KILL_SUCCESSFUL, &[&player.display_name]);
        let broadcast = !same_player_name(&target, &source.name);
        feedback.push(message.clone());
        actions.push(CommandAction::Kill {
            target,
            message,
            broadcast,
            notify: source.name.clone(),
        });
    }
    CommandOutput { feedback, actions }
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
        Some("start") => {
            if !source.can(crate::permission::COMMAND_TIME_START) {
                return permission_for("time");
            }
            CommandOutput {
                feedback: vec![lang::TIME_RESTARTED.to_string()],
                actions: vec![CommandAction::SetTimeRunning {
                    running: true,
                    message: lang::TIME_RESTARTED.to_string(),
                    notify: source.name.clone(),
                }],
            }
        }
        Some("stop") => {
            if !source.can(crate::permission::COMMAND_TIME_STOP) {
                return permission_for("time");
            }
            CommandOutput {
                feedback: vec![lang::TIME_STOPPED.to_string()],
                actions: vec![CommandAction::SetTimeRunning {
                    running: false,
                    message: lang::TIME_STOPPED.to_string(),
                    notify: source.name.clone(),
                }],
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
        format!("§aWeather changed to §f{}§a.", state.name_en()),
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
        return error("§cInvalid player name.");
    }

    action(
        if op {
            format!("§a{} is now an operator.", name)
        } else {
            format!("§a{} is no longer an operator.", name)
        },
        CommandAction::SetOp {
            target: name.to_string(),
            op,
        },
    )
}

/// `/version [plugin]`, ported from `pocketmine\command\defaults\VersionCommand`.
/// `about` is its PM alias. With an argument it describes matching plugins;
/// without metadata (only names are in `CommandSource`) it reports the name
/// and unknown version — full description/website/authors need a richer
/// `PluginDescription` surface (see `plugins` source).
fn version(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if args.is_empty() {
        // `VersionCommand` prints the software, its version and the MCPE version.
        return CommandOutput {
            feedback: vec![
                lang::VERSION_SOFTWARE_NAME.replace("{%serverSoftwareName}", "Axolotl-PM"),
                lang::VERSION_SOFTWARE_VERSION
                    .replace("{%serverSoftwareVersion}", "5.51.2-dev")
                    .replace("{%serverGitHash}", "unknown"),
                lang::VERSION_MINECRAFT
                    .replace("{%minecraftVersion}", "1.26.60")
                    .replace("{%minecraftProtocolVersion}", "2225"),
            ],
            actions: Vec::new(),
        };
    }
    let query = args.join(" ");
    let needle = query.to_lowercase();
    let mut lines = Vec::new();
    let mut found = false;
    // Exact match wins; otherwise substring (PM `stripos`).
    let exact = source
        .plugins
        .iter()
        .find(|name| name.eq_ignore_ascii_case(&query));
    if let Some(name) = exact {
        lines.push(
            lang::VERSION_PLUGIN_HEADER
                .replace("{%pluginName}", name)
                .replace("{%version}", "unknown"),
        );
        return CommandOutput { feedback: lines, actions: Vec::new() };
    }
    for name in &source.plugins {
        if name.to_lowercase().contains(&needle) {
            lines.push(
                lang::VERSION_PLUGIN_HEADER
                    .replace("{%pluginName}", name)
                    .replace("{%version}", "unknown"),
            );
            found = true;
        }
    }
    if !found {
        return feedback(lang::VERSION_NO_SUCH_PLUGIN.to_string());
    }
    CommandOutput { feedback: lines, actions: Vec::new() }
}

/// `/effect`, ported from `pocketmine\command\defaults\EffectCommand`.
/// Effect ids are treated as opaque strings (the Rust port has no
/// `StringToEffectParser` registry yet — see the divergence note in
/// `player_context`/session). Any non-empty id is accepted; removal paths
/// (`clear`, duration `0`) mirror PM keys but the session currently always
/// reports success because it cannot verify the active set.
fn effect(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() < 2 {
        return usage("effect");
    }
    let targets = match fetch_permitted_player_targets(
        source,
        players,
        Some(args[0]),
        "effect",
        source.can(crate::permission::COMMAND_EFFECT_SELF),
        source.can(crate::permission::COMMAND_EFFECT_OTHER),
    ) {
        Ok(targets) => targets,
        Err(output) => return output,
    };

    if args[1].eq_ignore_ascii_case("clear") {
        let mut feedback = Vec::new();
        let mut actions = Vec::new();
        for player in &targets {
            let target = player.name.clone();
            let message = fill_template(lang::EFFECT_SUCCESS_REMOVED_ALL, &[&target]);
            feedback.push(message.clone());
            actions.push(CommandAction::ApplyEffect {
                target,
                effect: String::new(),
                duration_secs: None,
                amplifier: 0,
                particles: true,
                remove: true,
                message,
                notify: source.name.clone(),
            });
        }
        return CommandOutput { feedback, actions };
    }

    let effect_name = args[1].to_string();
    let mut duration_secs: Option<i32> = None;
    let mut infinite = false;
    if args.len() >= 3 {
        if args[2].eq_ignore_ascii_case("infinite") {
            infinite = true;
        } else {
            match bounded_int(args[2], 0, (i32::MAX / 20) as i64) {
                Ok(seconds) => {
                    duration_secs = Some(seconds as i32);
                    if seconds == 0 {
                        let mut feedback = Vec::new();
                        let mut actions = Vec::new();
                        for player in &targets {
                            let target = player.name.clone();
                            let message = fill_template(
                                lang::EFFECT_SUCCESS_REMOVED,
                                &[&effect_name, &target],
                            );
                            feedback.push(message.clone());
                            actions.push(CommandAction::ApplyEffect {
                                target,
                                effect: effect_name.clone(),
                                duration_secs: Some(0),
                                amplifier: 0,
                                particles: true,
                                remove: true,
                                message,
                                notify: source.name.clone(),
                            });
                        }
                        return CommandOutput { feedback, actions };
                    }
                }
                Err(BoundedIntError::Syntax) => return usage("effect"),
                Err(BoundedIntError::TooBig) => {
                    return error(red(fill_template(
                        lang::NUM_TOO_BIG,
                        &[args[2], &(i32::MAX / 20).to_string()],
                    )));
                }
                Err(BoundedIntError::TooSmall) => {
                    return error(red(fill_template(
                        lang::NUM_TOO_SMALL,
                        &[args[2], "0"],
                    )));
                }
            }
        }
    }
    if infinite {
        duration_secs = None;
    }

    let amplifier = match args.get(3) {
        Some(value) => match bounded_int(value, 0, 255) {
            Ok(amp) => amp as i32,
            Err(BoundedIntError::Syntax) => return usage("effect"),
            Err(BoundedIntError::TooBig) => {
                return error(red(fill_template(lang::NUM_TOO_BIG, &[value, "255"])));
            }
            Err(BoundedIntError::TooSmall) => {
                return error(red(fill_template(lang::NUM_TOO_SMALL, &[value, "0"])));
            }
        },
        None => 0,
    };

    let particles = match args.get(4) {
        Some(v) => {
            let lower = v.to_lowercase();
            !(lower == "on" || lower == "true" || lower == "t" || lower == "1")
        }
        None => true,
    };

    let mut feedback = Vec::new();
    let mut actions = Vec::new();
    for player in &targets {
        let target = player.name.clone();
        let message = if infinite {
            fill_template(
                lang::EFFECT_SUCCESS_INFINITE,
                &[&effect_name, &amplifier.to_string(), &target],
            )
        } else {
            let secs = duration_secs.unwrap_or(0).to_string();
            fill_template(
                lang::EFFECT_SUCCESS,
                &[&effect_name, &amplifier.to_string(), &target, &secs],
            )
        };
        feedback.push(message.clone());
        actions.push(CommandAction::ApplyEffect {
            target,
            effect: effect_name.clone(),
            duration_secs,
            amplifier,
            particles,
            remove: false,
            message,
            notify: source.name.clone(),
        });
    }
    CommandOutput { feedback, actions }
}

/// `/enchant`, ported from `pocketmine\command\defaults\EnchantCommand`.
/// The held-item check and enchantment registry live in the session (which
/// owns the inventory); here we only resolve the target and bounds.
fn enchant(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() < 2 {
        return usage("enchant");
    }
    let targets = match fetch_permitted_player_targets(
        source,
        players,
        Some(args[0]),
        "enchant",
        source.can(crate::permission::COMMAND_ENCHANT_SELF),
        source.can(crate::permission::COMMAND_ENCHANT_OTHER),
    ) {
        Ok(targets) => targets,
        Err(output) => return output,
    };
    let enchantment = args[1].to_string();
    let level = match args.get(2) {
        Some(value) => match bounded_int(value, 1, 255) {
            Ok(level) => level as i32,
            Err(BoundedIntError::Syntax) => return usage("enchant"),
            Err(BoundedIntError::TooBig) => {
                return error(red(fill_template(lang::NUM_TOO_BIG, &[value, "255"])));
            }
            Err(BoundedIntError::TooSmall) => {
                return error(red(fill_template(lang::NUM_TOO_SMALL, &[value, "1"])));
            }
        },
        None => 1,
    };
    let mut feedback = Vec::new();
    let mut actions = Vec::new();
    for player in &targets {
        let target = player.name.clone();
        let message = fill_template(lang::ENCHANT_SUCCESS, &[&target]);
        feedback.push(message.clone());
        actions.push(CommandAction::Enchant {
            target,
            enchantment: enchantment.clone(),
            level,
            message,
            notify: source.name.clone(),
        });
    }
    CommandOutput { feedback, actions }
}

/// `/xp <xp[L]> [player]`, ported from `pocketmine\command\defaults\XpCommand`.
/// `getInteger` clamps instead of erroring, so out-of-range input is clamped
/// to the int32 / level bounds; negative xp fails with the vanilla key.
fn xp(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.is_empty() {
        return usage("xp");
    }
    let targets = match fetch_permitted_player_targets(
        source,
        players,
        args.get(1).copied(),
        "xp",
        source.can(crate::permission::COMMAND_XP_SELF),
        source.can(crate::permission::COMMAND_XP_OTHER),
    ) {
        Ok(targets) => targets,
        Err(output) => return output,
    };
    let raw = args[0];
    let is_levels = raw.ends_with('L') || raw.ends_with('l');
    if is_levels {
        let number_part = &raw[..raw.len() - 1];
        let amount = php_int_cast(number_part).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        let mut feedback = Vec::new();
        let mut actions = Vec::new();
        for player in &targets {
            let target = player.name.clone();
            let message = if amount >= 0 {
                fill_template(lang::XP_SUCCESS_LEVELS, &[&amount.to_string(), &target])
            } else {
                fill_template(
                    lang::XP_SUCCESS_NEGATIVE_LEVELS,
                    &[&amount.unsigned_abs().to_string(), &target],
                )
            };
            feedback.push(message.clone());
            actions.push(CommandAction::AddXp {
                target,
                amount,
                is_levels: true,
                message,
                notify: source.name.clone(),
            });
        }
        CommandOutput { feedback, actions }
    } else {
        let amount = php_int_cast(raw).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        if amount < 0 {
            return feedback(lang::XP_FAILURE_WIDTHDRAW.to_string());
        }
        let mut feedback = Vec::new();
        let mut actions = Vec::new();
        for player in &targets {
            let target = player.name.clone();
            let message = fill_template(lang::XP_SUCCESS, &[&amount.to_string(), &target]);
            feedback.push(message.clone());
            actions.push(CommandAction::AddXp {
                target,
                amount,
                is_levels: false,
                message,
                notify: source.name.clone(),
            });
        }
        CommandOutput { feedback, actions }
    }
}

/// Particles recognised by `ParticleCommand::getParticle`. Unknown names hit
/// `commands.particle.notFound` (red). `Type` ids are informational only
/// until the session can emit `LevelEvent` packets.
pub const KNOWN_PARTICLES: &[&str] = &[
    "explode", "hugeexplosion", "hugeexplosionseed", "bubble", "splash", "wake", "water",
    "crit", "smoke", "spell", "instantspell", "dripwater", "driplava", "townaura", "spore",
    "portal", "flame", "lava", "reddust", "snowballpoof", "slime", "itembreak", "terrain",
    "heart", "ink", "droplet", "enchantmenttable", "happyvillager", "angryvillager",
    "forcefield", "mobflame", "iconcrack", "blockcrack", "blockdust", "sonicexplosion",
];

/// `/particle`, ported from `pocketmine\command\defaults\ParticleCommand`.
fn particle(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() < 7 {
        return usage("particle");
    }
    let name_lower = args[0].to_lowercase();
    if !KNOWN_PARTICLES.contains(&name_lower.as_str()) {
        return error(red(fill_template(lang::PARTICLE_NOT_FOUND, &[&name_lower])));
    }
    // Player coordinates are relative (`~`); console uses absolute values and
    // the default world.
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
        return usage("particle");
    };
    let delta = |value: &str| -> Option<f64> {
        value.parse::<f64>().ok().filter(|v| v.is_finite())
    };
    let (Some(dx), Some(dy), Some(dz)) = (delta(args[4]), delta(args[5]), delta(args[6])) else {
        return usage("particle");
    };
    let count = match args.get(7) {
        Some(value) => php_int_cast(value).max(1) as i32,
        None => 1,
    };
    let data = args.get(8).map(|s| s.to_string());
    let message = fill_template(lang::PARTICLE_SUCCESS, &[&name_lower, &count.to_string()]);
    let _ = players;
    CommandOutput {
        feedback: vec![message.clone()],
        actions: vec![CommandAction::SpawnParticle {
            name: name_lower,
            position: (x, y, z),
            delta: (dx, dy, dz),
            count,
            data,
            message,
        }],
    }
}

/// `/status`, ported from `pocketmine\command\defaults\StatusCommand`.
///
/// PM reads uptime, TPS, bandwidth, threads and memory plus per-world
/// chunk/entity counts. This port has only the TPS sampler (`tps`: 1m/5m/15m)
/// in `CommandSource`, so uptime, memory, threads and networking are reported
/// as `0` until `SessionManager` wires them in (see the divergence note in
/// `status()`). The layout, keys and gold prefix still match PM so new
/// wiring drops in without changing output shape.
fn status(source: &CommandSource, players: &[OnlinePlayer]) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_STATUS) {
        return permission_for("status");
    }
    // PM: `current TPS` = instantaneous, `average TPS` = tick average.
    // Ours: 1m window as current, 15m window as average (both capped at 20
    // by `TickMetrics`).
    let current = source.tps.0;
    let average = source.tps.2;
    let color_for = |tps: f64| -> &'static str {
        if tps < 12.0 {
            "§c"
        } else if tps < 17.0 {
            "§6"
        } else {
            "§a"
        }
    };
    // PM shows the tick load percent next to the TPS; the sampler does not
    // track it yet, so derive `load = 100 - tps*5` bounded to 0..=100.
    let stat = |tps: f64| -> String {
        let load = (100.0 - tps * 5.0).clamp(0.0, 100.0);
        fill_named(
            lang::STATUS_TPS_STAT,
            &[
                ("ticksPerSecond", &format!("{:.2}", tps)),
                ("loadPercent", &format!("{:.0}", load)),
            ],
        )
    };

    let mut lines = Vec::new();
    lines.push(format!("§e--------- §r{}§e ---------", lang::STATUS_HEADER));
    // Uptime: no start time in `CommandSource` yet — report 0s (see doc).
    let uptime = fill_named(lang::STATUS_UPTIME_SECONDS, &[("seconds", "0")]);
    lines.push(format!("§6{}", fill_named(lang::STATUS_UPTIME, &[("uptime", &uptime)])));
    lines.push(format!(
        "§6{}",
        fill_named(
            lang::STATUS_TPS_CURRENT,
            &[("info", &format!("{}{}", color_for(current), stat(current)))]
        )
    ));
    lines.push(format!(
        "§6{}",
        fill_named(
            lang::STATUS_TPS_AVERAGE,
            &[("info", &format!("{}{}", color_for(average), stat(average)))]
        )
    ));
    let bandwidth = |_: f64| -> String {
        fill_named(lang::STATUS_NETWORK_STAT, &[("kbps", "0")])
    };
    lines.push(format!(
        "§6{}",
        fill_named(lang::STATUS_NETWORK_UPLOAD, &[("info", &format!("§c{}", bandwidth(0.0)))])
    ));
    lines.push(format!(
        "§6{}",
        fill_named(lang::STATUS_NETWORK_DOWNLOAD, &[("info", &format!("§c{}", bandwidth(0.0)))])
    ));
    lines.push(format!(
        "§6{}",
        fill_named(lang::STATUS_THREADS, &[("count", "0")])
    ));
    let memory = |_: u64| -> String {
        fill_named(lang::STATUS_MEMORY_STAT, &[("megabytes", "0.00")])
    };
    lines.push(format!(
        "§6{}",
        fill_named(lang::STATUS_MEMORY_MAIN, &[("info", &format!("§c{}", memory(0)))])
    ));
    lines.push(format!(
        "§6{}",
        fill_named(lang::STATUS_MEMORY_TOTAL, &[("info", &format!("§c{}", memory(0)))])
    ));
    lines.push(format!(
        "§6{}",
        fill_named(lang::STATUS_MEMORY_VIRTUAL, &[("info", &format!("§c{}", memory(0)))])
    ));
    // Single-world stub: chunk/entity counts need world-manager wiring.
    let time_stat = fill_named(lang::STATUS_WORLD_TIME_STAT, &[("milliseconds", "0.00")]);
    lines.push(format!(
        "§6{}",
        fill_named(
            lang::STATUS_WORLD,
            &[
                ("worldName", "\"world\""),
                ("loadedChunks", "0"),
                ("tickingChunks", "0"),
                ("entities", &players.len().to_string()),
                ("timeStat", &time_stat),
            ]
        )
    ));
    CommandOutput { feedback: lines, actions: Vec::new() }
}

/// `/timings`, ported from `pocketmine\command\defaults\TimingsCommand`.
/// Modes: `on|off|reset|report|merged|paste`. Report/paste upload needs the
/// timings HTTP service (unavailable); `report`/`merged` report the local
/// stub path instead.
fn timings(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_TIMINGS) {
        return permission_for("timings");
    }
    let Some(mode) = args.first().map(|arg| arg.to_lowercase()) else {
        return usage("timings");
    };
    if args.len() != 1 {
        return usage("timings");
    }
    // No timings handler exists in Rust yet — every mode is a logged no-op
    // with the PM message, so operators get honest feedback instead of a
    // fake report.
    let (feedback_line, action_message) = match mode.as_str() {
        "on" => (lang::TIMINGS_ENABLE.to_string(), lang::TIMINGS_ENABLE.to_string()),
        "off" => (lang::TIMINGS_DISABLE.to_string(), lang::TIMINGS_DISABLE.to_string()),
        "reset" => (lang::TIMINGS_RESET.to_string(), lang::TIMINGS_RESET.to_string()),
        "report" | "merged" => {
            (fill_template(lang::TIMINGS_WRITE, &["timings-timings.txt"]), fill_template(lang::TIMINGS_WRITE, &["timings-timings.txt"]))
        }
        "paste" => (lang::TIMINGS_PASTE_ERROR.to_string(), lang::TIMINGS_PASTE_ERROR.to_string()),
        _ => return usage("timings"),
    };
    CommandOutput {
        feedback: vec![feedback_line],
        actions: vec![CommandAction::Timings {
            mode,
            message: action_message,
            notify: source.name.clone(),
        }],
    }
}

/// `/gc`, ported from `pocketmine\command\defaults\GarbageCollectorCommand`.
/// The session reports counts after unloading; the command only fires the
/// request (counts are session-produced, like `/clear`).
fn gc(source: &CommandSource) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_GC) {
        return permission_for("gc");
    }
    CommandOutput {
        feedback: vec![lang::GC_HEADER.to_string()],
        actions: vec![CommandAction::GarbageCollect {
            notify: source.name.clone(),
        }],
    }
}

/// `/dumpmemory [path]`, ported from
/// `pocketmine\command\defaults\DumpMemoryCommand`. PM dumps with
/// `memory_get_usage` + `MemoryManager::dumpServerMemory`; here the session
/// writes a text snapshot (TODO: native heap dump).
fn dumpmemory(source: &CommandSource, args: &[&str]) -> CommandOutput {
    if !source.can(crate::permission::COMMAND_DUMPMEMORY) {
        return permission_for("dumpmemory");
    }
    if args.len() > 1 {
        return usage("dumpmemory");
    }
    CommandOutput {
        feedback: Vec::new(),
        actions: vec![CommandAction::DumpMemory {
            path: args.first().map(|s| s.to_string()),
        }],
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
        Some(info) => error(format!("§cUsage: {}", info.usage)),
        None => error("§cIncorrect usage."),
    }
}

fn permission_error() -> CommandOutput {
    error("§cYou don't have permission to use this command.")
}

fn player_not_found(name: &str) -> CommandOutput {
    error(red(fill_named(
        lang::PLAYER_NOT_FOUND,
        &[("playerName", name)],
    )))
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
        return resolve_selector_targets(source, players, kind, &args).into_iter().next();
    }
    let query_lower = java_name_without_prefix(query).to_lowercase();
    if let Some(exact) = players.iter().find(|player| {
        java_name_without_prefix(&player.name).to_lowercase() == query_lower
    }) {
        return Some(exact);
    }
    let matches: Vec<&OnlinePlayer> = players
        .iter()
        .filter(|player| {
            java_name_without_prefix(&player.name)
                .to_lowercase()
                .starts_with(&query_lower)
        })
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

/// `GameMode::getTranslatableName()` values from PocketMine's English locale.
pub(crate) fn gamemode_translatable(gamemode: i32) -> &'static str {
    match gamemode {
        1 => "Creative Mode",
        2 => "Adventure Mode",
        3 => "Spectator Mode",
        _ => "Survival Mode",
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
    let targets = match fetch_permitted_player_targets(
        source,
        players,
        args.first().copied(),
        "spawnpoint",
        source.can(crate::permission::COMMAND_SPAWNPOINT_SELF),
        source.can(crate::permission::COMMAND_SPAWNPOINT_OTHER),
    ) {
        Ok(targets) => targets,
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

    let mut feedback = Vec::new();
    let mut actions = Vec::new();
    for player in targets {
        let target = player.name.clone();
        let message = fill_template(
            lang::SPAWNPOINT_SUCCESS,
            &[&target, &format_coord(x), &format_coord(y), &format_coord(z)],
        );
        feedback.push(message.clone());
        actions.push(CommandAction::SetSpawnPoint {
            target,
            position: (x, y, z),
            message,
            notify: source.name.clone(),
        });
    }
    CommandOutput { feedback, actions }
}

/// `/title <player> <clear|reset|title|subtitle|actionbar|times> ...`, ported
/// one to one from `pocketmine\command\defaults\TitleCommand`.
fn title(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() < 2 {
        return usage("title");
    }

    let targets = match fetch_permitted_player_targets(
        source,
        players,
        Some(args[0]),
        "title",
        source.can(crate::permission::COMMAND_TITLE_SELF),
        source.can(crate::permission::COMMAND_TITLE_OTHER),
    ) {
        Ok(targets) => targets,
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

    let actions = targets
        .iter()
        .map(|player| CommandAction::SendTitle {
            target: player.name.clone(),
            action: action.clone(),
        })
        .collect();
    CommandOutput {
        feedback: vec![lang::TITLE_SUCCESS.to_string()],
        actions,
    }
}

/// `/scoreboard <player|global> <clear|add|remove|set> ...` (Rust
/// extension over `Player::setScoreboard()`):
/// - `clear`: remove every objective.
/// - `add <objective> <score> <text...>`: add/replace one line
///   (creates the objective with display name `<objective>` when
///   missing).
/// - `remove <objective> <score>`: remove one line.
/// - `set <objective> <display...>`: create the objective or update
///   its display name (existing lines are kept).
///
/// `global` targets the persisted default scoreboard (`scoreboard.json`)
/// instead of one player's board.
fn scoreboard(source: &CommandSource, players: &[OnlinePlayer], args: &[&str]) -> CommandOutput {
    if args.len() < 2 {
        return usage("scoreboard");
    }

    let targets: Vec<String> = if args[0].eq_ignore_ascii_case("global") {
        if !source.can(crate::permission::COMMAND_SCOREBOARD_OTHER) {
            return error(red(
                lang::PERMISSION.replace("{%commandName}", "scoreboard"),
            ));
        }
        vec!["global".to_string()]
    } else {
        match fetch_permitted_player_targets(
            source,
            players,
            Some(args[0]),
            "scoreboard",
            source.can(crate::permission::COMMAND_SCOREBOARD_SELF),
            source.can(crate::permission::COMMAND_SCOREBOARD_OTHER),
        ) {
            Ok(found) => found.iter().map(|player| player.name.clone()).collect(),
            Err(output) => return output,
        }
    };

    let operation = match args[1] {
        "clear" => ScoreboardOperation::Clear,
        "add" => {
            if args.len() < 5 {
                return usage("scoreboard");
            }
            let score = match bounded_int(args[3], 0, 1_000_000_000) {
                Ok(parsed) => parsed as u32,
                Err(_) => return usage("scoreboard"),
            };
            ScoreboardOperation::AddScore {
                objective_name: args[2].to_string(),
                score,
                text: args[4..].join(" "),
            }
        }
        "remove" => {
            if args.len() < 4 {
                return usage("scoreboard");
            }
            let score = match bounded_int(args[3], 0, 1_000_000_000) {
                Ok(parsed) => parsed as u32,
                Err(_) => return usage("scoreboard"),
            };
            ScoreboardOperation::RemoveScore {
                objective_name: args[2].to_string(),
                score,
            }
        }
        "set" => {
            if args.len() < 4 {
                return usage("scoreboard");
            }
            ScoreboardOperation::AddObjective(crate::scoreboard::ScoreboardObjective::sidebar(
                args[2],
                args[3..].join(" "),
            ))
        }
        _ => return usage("scoreboard"),
    };

    let actions = targets
        .into_iter()
        .map(|target| CommandAction::Scoreboard {
            target,
            operation: operation.clone(),
        })
        .collect();
    CommandOutput {
        feedback: vec![lang::SCOREBOARD_SUCCESS.to_string()],
        actions,
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
        "scoreboard" | "sb" => Some(&[COMMAND_SCOREBOARD_SELF, COMMAND_SCOREBOARD_OTHER]),
        "transferserver" => Some(&[COMMAND_TRANSFERSERVER]),
        "defaultgamemode" => Some(&[COMMAND_DEFAULTGAMEMODE]),
        "difficulty" => Some(&[COMMAND_DIFFICULTY]),
        "seed" => Some(&[COMMAND_SEED]),
        "op" => Some(&[COMMAND_OP_GIVE]),
        "deop" => Some(&[COMMAND_OP_TAKE]),
        "version" | "ver" | "about" => Some(&[COMMAND_VERSION]),
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
        "status" => Some(&[COMMAND_STATUS]),
        "effect" => Some(&[COMMAND_EFFECT_SELF, COMMAND_EFFECT_OTHER]),
        "enchant" => Some(&[COMMAND_ENCHANT_SELF, COMMAND_ENCHANT_OTHER]),
        "xp" => Some(&[COMMAND_XP_SELF, COMMAND_XP_OTHER]),
        "particle" => Some(&[COMMAND_PARTICLE]),
        "timings" => Some(&[COMMAND_TIMINGS]),
        "gc" => Some(&[COMMAND_GC]),
        "dumpmemory" => Some(&[COMMAND_DUMPMEMORY]),
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
                display_name: "Steve".to_string(),
                position: (10.0, 64.0, -5.0),
                gamemode: 0,
            },
            OnlinePlayer {
                name: "Alex".to_string(),
                display_name: "Alex".to_string(),
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
        assert!(output.feedback[0].contains("Cannot find a player"));
        // The named placeholder must be filled with the real name, not left
        // literal in the output.
        assert!(output.feedback[0].contains("Nobody"));
        assert!(!output.feedback[0].contains("{%playerName}"));

        let output = handle_command("/tp", &source, &players()).expect("tp is native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("Usage:"));
    }

    #[test]
    fn tp_requires_privileges_when_cheats_are_off() {
        let mut source = player_source("Steve", (0.0, 64.0, 0.0));
        source.is_op = false;
        source.allow_cheats = false;
        let output = handle_command("/tp 1 2 3", &source, &players()).expect("tp is native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("permission"));
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
            vec!["Game mode change for Alex failed!".to_string()]
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
            vec!["§7You whisper to Alex: ven aqui".to_string()]
        );
        match &output.actions[0] {
            CommandAction::Message { target, message } => {
                assert_eq!(target, "Alex");
                assert_eq!(message, "§7Steve whispers to you: ven aqui");
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
            "This server is running Axolotl-PM"
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
        assert!(output.feedback[0].starts_with("Data tag parsing failed: "));
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
                message: "Given Stone (stone) * 64 to Alex".to_string(),
                notify: "Steve".to_string(),
            }]
        );
        assert_eq!(
            output.feedback,
            vec!["Given Stone (stone) * 64 to Alex".to_string()]
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
                message: "Given Stone (1:2) * 3 to Alex".to_string(),
                notify: "Steve".to_string(),
            }]
        );

        let output = handle_command("/give Alex not_an_item 1", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(
            output.feedback[0],
            "§cThere is no such item with name not_an_item"
        );

        let output = handle_command("/give Alex stone 0", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(
            output.feedback[0],
            "§cThe number you have entered (0) is too small, it must be at least 1"
        );

        let output = handle_command("/give Alex stone 40000", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "§cThe number you have entered (40000) is too big, it must be at most 32767"
        );

        let output = handle_command("/give Alex stone abc", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(
            output.feedback[0],
            "§cUsage: /give <player> <item[:damage]> [amount] [tags...]"
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
                message: "Killed Steve".to_string(),
                broadcast: false,
                notify: "Steve".to_string(),
            }]
        );
        assert_eq!(output.feedback, vec!["Killed Steve".to_string()]);

        let output = handle_command("/kill Alex", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Kill {
                target: "Alex".to_string(),
                message: "Killed Alex".to_string(),
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
            "§cYou don't have permission to use the command: give"
        );

        // Clearing yourself is granted to everyone, clearing others is not.
        let output = handle_command("/clear", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 1);
        let output = handle_command("/clear Alex", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("permission"));

        // Killing yourself is granted to everyone, killing others is not.
        let output = handle_command("/kill", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 1);
        let output = handle_command("/kill Alex", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("permission"));
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
                message: "Set Steve's spawn point to (10, 64, -6)".to_string(),
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
        assert!(output.feedback[0].contains("Usage:"));

        let output =
            handle_command("/defaultgamemode creative", &source, &players()).expect("native");
        assert_eq!(output.actions, vec![CommandAction::SetDefaultGamemode(1)]);
        assert_eq!(
            output.feedback[0],
            "The world's default game mode is now Creative Mode"
        );

        let output =
            handle_command("/defaultgamemode nope", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(output.feedback[0], "Unknown game mode \"nope\"");

        let output = handle_command("/difficulty hard", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 1);
        assert_eq!(output.feedback[0], "Set game difficulty to 3");

        let output = handle_command("/difficulty 7", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("Usage:"));

        let output = handle_command("/seed", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Seed: 12345678");
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
        assert_eq!(output.feedback[0], "There are 1 total banned players:");
        assert_eq!(output.feedback[1], "alex");

        let output = handle_command("/banlist ips", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "There are 1 total banned IP addresses:");
        assert_eq!(output.feedback[1], "10.0.0.9");

        let output = handle_command("/banlist nope", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("Usage:"));

        let output = handle_command("/pardon-ip nope", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "You have entered an invalid IP address");
        assert!(output.actions.is_empty());

        let output = handle_command("/whitelist list", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "There are 1 (out of 1 seen) whitelisted players:"
        );

        let output =
            handle_command("/whitelist add Herobrine", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Added Herobrine to the whitelist");
        assert!(matches!(
            &output.actions[0],
            CommandAction::AddWhitelist { name, .. } if name == "Herobrine"
        ));

        let output = handle_command("/whitelist add", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Usage: /whitelist add <player>");

        let output = handle_command("/ban Alex", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Banned player Alex");
        assert!(matches!(
            &output.actions[0],
            CommandAction::BanName { reason, .. } if reason.is_empty()
        ));

        let output = handle_command("/ban-ip 10.1.1.1 spam", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Banned IP address 10.1.1.1");

        let output = handle_command("/ban-ip nobody", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "You have entered an invalid IP address or a player that is not online"
        );

        let output = handle_command("/stop", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Stopping the server");
        assert_eq!(
            output.actions,
            vec![CommandAction::StopServer {
                message: "Stopping the server".to_string(),
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
        assert_eq!(output.feedback[0], "Saving server data...");
        assert!(matches!(
            &output.actions[0],
            CommandAction::SaveAll { .. }
        ));

        let output = handle_command("/save-off", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "Turned off world auto-saving"
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
            "§cYou don't have permission to use the command: seed"
        );
        assert!(output.actions.is_empty());

        // Everyone may check the online players.
        let output = handle_command("/list", &source, &players()).expect("native");
        assert!(!output.feedback[0].contains("permission"));

        // Granting the node through an attachment allows the command.
        permissions.set_permission("Guest", crate::permission::COMMAND_SEED, true);
        let output = handle_command("/seed", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], "Seed: 12345678");

        // ...and denying it on an operator blocks it too.
        let mut admin = player_source("Admin", (0.0, 64.0, 0.0));
        admin.permissions = Some(std::sync::Arc::clone(&permissions));
        permissions.set_permission("Admin", crate::permission::COMMAND_SEED, false);
        let output = handle_command("/seed", &admin, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            "§cYou don't have permission to use the command: seed"
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
            output.feedback.iter().any(|line| line.contains("permission"))
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
        assert!(!output.feedback.iter().any(|line| line.contains("permission")));
        assert_eq!(output.actions.len(), 1);

        // ...but changing the own mode still needs `.self`.
        let output = handle_command("/gms", &source, &players()).expect("native");
        assert!(output.feedback.iter().any(|line| line.contains("permission")));
        assert!(output.actions.is_empty());

        // And the opposite direction: `.self` alone cannot touch another player.
        let mut other = player_source("Guest2", (0.0, 64.0, 0.0));
        other.is_op = false;
        other.allow_cheats = false;
        other.permissions = Some(std::sync::Arc::clone(&permissions));
        permissions.set_permission("Guest2", crate::permission::COMMAND_GAMEMODE_SELF, true);
        let output = handle_command("/gms", &other, &players()).expect("native");
        assert!(!output.feedback.iter().any(|line| line.contains("permission")));
        assert_eq!(output.actions.len(), 1);
        let output = handle_command("/gms Alex", &other, &players()).expect("native");
        assert!(output.feedback.iter().any(|line| line.contains("permission")));
        assert!(output.actions.is_empty());
    }

    #[test]
    fn help_paginates_like_pocketmine() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        let output = handle_command("/help", &source, &players()).expect("native");
        let pages = COMMANDS.len().div_ceil(PLAYER_SCREEN_LINE_HEIGHT);
        assert_eq!(
            output.feedback[0],
            format!("--- Showing help page 1 of {pages} (/help [page]) ---")
        );
        assert_eq!(output.feedback.len(), 1 + PLAYER_SCREEN_LINE_HEIGHT);
        assert!(output.feedback[1].starts_with("§2/ban: §r"));

        // A page past the end falls back to the last one.
        let output = handle_command("/help 99", &source, &players()).expect("native");
        assert_eq!(
            output.feedback[0],
            format!("--- Showing help page {pages} of {pages} (/help [page]) ---")
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
                "§e--------- §rHelp: /tp§e ---------".to_string(),
                format!("§6Description: §r{}", info.description),
                format!("§6Usage: §r{}", info.usage),
                "§6Aliases: §rteleport".to_string(),
            ]
        );

        let output = handle_command("/help nope", &source, &players()).expect("native");
        assert_eq!(
            output.feedback,
            vec![concat!(
                "§cUnknown command: nope. Use /help for a list of available commands."
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
                "§cUnknown command: op. Use /help for a list of available commands."
            )]
        );

        let output = handle_command("/help help", &guest, &players()).expect("native");
        assert_eq!(output.feedback[0], "§e--------- §rHelp: /help§e ---------");
        assert_eq!(output.feedback[3], "§6Aliases: §r?");
    }

    #[test]
    fn list_reports_count_max_and_sorted_names() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        let output = handle_command("/list", &source, &players()).expect("native");
        assert_eq!(
            output.feedback,
            vec![
                "There are 2/1000 players online:".to_string(),
                "Alex, Steve".to_string()
            ]
        );

        let output = handle_command("/list", &source, &[]).expect("native");
        assert_eq!(
            output.feedback,
            vec!["There are 0/1000 players online:".to_string(), String::new()]
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
            "§cUsage: /title <player> <clear|reset|title|subtitle|actionbar|times> ...";
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
    fn scoreboard_parses_clear_add_remove_set_and_global() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        let output = handle_command("/scoreboard Alex clear", &source, &players()).expect("native");
        assert_eq!(output.feedback, vec![lang::SCOREBOARD_SUCCESS.to_string()]);
        assert_eq!(
            output.actions,
            vec![CommandAction::Scoreboard {
                target: "Alex".to_string(),
                operation: ScoreboardOperation::Clear,
            }]
        );

        let output = handle_command(
            "/scoreboard Alex add main 100 Oro extra",
            &source,
            &players(),
        )
        .expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Scoreboard {
                target: "Alex".to_string(),
                operation: ScoreboardOperation::AddScore {
                    objective_name: "main".to_string(),
                    score: 100,
                    text: "Oro extra".to_string(),
                },
            }]
        );

        let output =
            handle_command("/scoreboard Alex remove main 42", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Scoreboard {
                target: "Alex".to_string(),
                operation: ScoreboardOperation::RemoveScore {
                    objective_name: "main".to_string(),
                    score: 42,
                },
            }]
        );

        let output = handle_command(
            "/scoreboard Alex set main Números top",
            &source,
            &players(),
        )
        .expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Scoreboard {
                target: "Alex".to_string(),
                operation: ScoreboardOperation::AddObjective(
                    crate::scoreboard::ScoreboardObjective::sidebar("main", "Números top")
                ),
            }]
        );

        let output =
            handle_command("/scoreboard global clear", &source, &players()).expect("native");
        assert_eq!(
            output.actions,
            vec![CommandAction::Scoreboard {
                target: "global".to_string(),
                operation: ScoreboardOperation::Clear,
            }]
        );

        let usage_line = "§cUsage: /scoreboard <player|global> <clear|add|remove|set|list> ...";
        for input in [
            "/scoreboard",
            "/scoreboard Alex",
            "/scoreboard Alex nope",
            "/scoreboard Alex add main",
            "/scoreboard Alex add main muchas Oro",
            "/scoreboard Alex remove main",
            "/scoreboard Alex set main",
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
            vec!["§cUsage: /transferserver <server> [port]".to_string()]
        );
        assert!(output.actions.is_empty());

        let console = CommandSource::console(1_000);
        let output = handle_command("/transferserver host", &console, &players()).expect("native");
        assert_eq!(
            output.feedback,
            vec!["§cThis command syntax can only be used as a player".to_string()]
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
        assert!(output.feedback[0].contains("Usage:"));
    }

    #[test]
    fn new_pocketmine_commands_produce_actions() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));

        // /effect <player> <effect> [secs] [amp] [hide]
        let output = handle_command("/effect Alex speed 30 1", &source, &players()).expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::ApplyEffect { duration_secs: Some(30), amplifier: 1, .. }
        ));
        let output = handle_command("/effect Alex clear", &source, &players()).expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::ApplyEffect { remove: true, .. }
        ));
        let output = handle_command("/effect Alex speed infinite 2", &source, &players()).expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::ApplyEffect { duration_secs: None, .. }
        ));
        let output = handle_command("/effect Alex speed", &source, &players()).expect("native");
        assert!(!output.actions.is_empty());

        // /enchant
        let output = handle_command("/enchant Alex sharpness 5", &source, &players()).expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::Enchant { level: 5, .. }
        ));

        // /xp <amount[L]> [player]
        let output = handle_command("/xp 100 Alex", &source, &players()).expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::AddXp { amount: 100, is_levels: false, .. }
        ));
        let output = handle_command("/xp 5L Alex", &source, &players()).expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::AddXp { amount: 5, is_levels: true, .. }
        ));
        let output = handle_command("/xp -5 Alex", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert_eq!(output.feedback[0], lang::XP_FAILURE_WIDTHDRAW);

        // /particle <name> <x> <y> <z> <xd> <yd> <zd> [count] [data]
        let output = handle_command("/particle flame 0 64 0 1 1 1 3", &source, &players()).expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::SpawnParticle { count: 3, .. }
        ));
        let output = handle_command("/particle nope 0 64 0 1 1 1", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains(lang::PARTICLE_NOT_FOUND.split('{').next().unwrap()));

        // /status (PM format) and /tps (Paper format) both work
        let output = handle_command("/status", &source, &players()).expect("native");
        assert!(output.feedback[0].contains(lang::STATUS_HEADER));
        assert!(output.feedback.iter().any(|line| line.contains(lang::STATUS_TPS_CURRENT.split(':').next().unwrap())));
        let output = handle_command("/tps", &source, &players()).expect("native");
        assert!(output.feedback[0].contains("TPS from last 1m"));

        // /timings <on|off|reset|report|paste>
        for mode in ["on", "off", "reset", "report", "paste"] {
            let output = handle_command(&format!("/timings {mode}"), &source, &players()).expect("native");
            assert_eq!(output.actions.len(), 1, "{mode}");
        }
        let output = handle_command("/timings nope", &source, &players()).expect("native");
        assert!(output.actions.is_empty());

        // /gc and /dumpmemory
        let output = handle_command("/gc", &source, &players()).expect("native");
        assert_eq!(output.feedback, vec![lang::GC_HEADER.to_string()]);
        assert!(matches!(&output.actions[0], CommandAction::GarbageCollect { .. }));
        let output = handle_command("/dumpmemory dumps/x.txt", &source, &players()).expect("native");
        assert!(matches!(&output.actions[0], CommandAction::DumpMemory { path: Some(_) }));

        // /time start|stop
        let output = handle_command("/time start", &source, &players()).expect("native");
        assert!(matches!(&output.actions[0], CommandAction::SetTimeRunning { running: true, .. }));
        let output = handle_command("/time stop", &source, &players()).expect("native");
        assert!(matches!(&output.actions[0], CommandAction::SetTimeRunning { running: false, .. }));

        // /version <plugin> and /about alias
        let mut with_plugins = player_source("Steve", (0.0, 64.0, 0.0));
        with_plugins.plugins = vec!["TestKit".to_string()];
        let output = handle_command("/version TestKit", &with_plugins, &players()).expect("native");
        assert!(output.feedback[0].contains("TestKit"));
        let output = handle_command("/about", &source, &players()).expect("native");
        assert!(output.feedback[0].contains("Axolotl-PM"));
        let output = handle_command("/version NoSuchPlugin", &source, &players()).expect("native");
        assert_eq!(output.feedback[0], lang::VERSION_NO_SUCH_PLUGIN);
    }

    fn three_players() -> Vec<OnlinePlayer> {
        let mut list = players();
        list.push(OnlinePlayer {
            name: "Zombie".to_string(),
            display_name: "Zombie".to_string(),
            position: (2.0, 64.0, 2.0),
            gamemode: 0,
        });
        list
    }

    #[test]
    fn selector_s_targets_the_sender() {
        let source = player_source("Steve", (10.0, 64.0, -5.0));
        let output = handle_command("/give @s stone", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 1);
        assert!(matches!(
            &output.actions[0],
            CommandAction::Give { target, .. } if target == "Steve"
        ));
    }

    #[test]
    fn selector_p_picks_the_nearest_player() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));
        // Zombie at (2,64,2) is ~2.8 away, Steve at (10,64,-5) ~11 away,
        // Alex at (-20,70,8.5) far. Nearest to (0,64,0) is Zombie.
        let output = handle_command("/give @p stone", &source, &three_players()).expect("native");
        assert_eq!(output.actions.len(), 1);
        assert!(matches!(
            &output.actions[0],
            CommandAction::Give { target, .. } if target == "Zombie"
        ));
        // @n is a documented alias of @p.
        let output = handle_command("/give @n stone", &source, &three_players()).expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::Give { target, .. } if target == "Zombie"
        ));
    }

    #[test]
    fn selector_p_honours_an_origin_override() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));
        // Nearest to Alex (-20,70,8.5) is Alex itself.
        let output = handle_command("/give @p[x=-20,y=70,z=8.5] stone", &source, &players())
            .expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::Give { target, .. } if target == "Alex"
        ));
    }

    #[test]
    fn selector_a_and_e_expand_to_every_player() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));
        let output = handle_command("/give @a stone", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 2);
        assert_eq!(output.feedback.len(), 2);
        let targets: Vec<String> = output
            .actions
            .iter()
            .filter_map(|action| match action {
                CommandAction::Give { target, .. } => Some(target.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(targets, vec!["Steve".to_string(), "Alex".to_string()]);

        // @e currently returns players as entities (no non-player entities).
        let output = handle_command("/kill @e", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 2);
        assert_eq!(output.feedback.len(), 2);
    }

    #[test]
    fn selector_r_picks_one_online_player() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));
        let output = handle_command("/give @r stone", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 1);
        match &output.actions[0] {
            CommandAction::Give { target, .. } => {
                assert!(target == "Steve" || target == "Alex", "got {target}");
            }
            other => panic!("expected /give, got {other:?}"),
        }
    }

    #[test]
    fn selector_arguments_filter_sort_and_limit() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));
        // distance=..6 keeps Zombie (~2.8) only.
        let output = handle_command("/give @a[distance=..6] stone", &source, &three_players())
            .expect("native");
        assert_eq!(output.actions.len(), 1);
        assert!(matches!(
            &output.actions[0],
            CommandAction::Give { target, .. } if target == "Zombie"
        ));
        // distance range with spaces after commas still merges.
        let output =
            handle_command("/give @a[distance=1..50, limit=2] stone", &source, &three_players())
                .expect("native");
        assert_eq!(output.actions.len(), 2);
        // name= filters to a single player.
        let output = handle_command("/give @a[name=Alex] stone", &source, &players())
            .expect("native");
        assert_eq!(output.actions.len(), 1);
        assert!(matches!(
            &output.actions[0],
            CommandAction::Give { target, .. } if target == "Alex"
        ));
        // sort=furthest from (0,64,0) puts Alex (-20,70,8.5) first.
        let output =
            handle_command("/give @a[sort=furthest,limit=1] stone", &source, &players())
                .expect("native");
        assert!(matches!(
            &output.actions[0],
            CommandAction::Give { target, .. } if target == "Alex"
        ));
        // @e[type=player] is a no-op; other types match nothing.
        let output = handle_command("/give @e[type=player] stone", &source, &players())
            .expect("native");
        assert_eq!(output.actions.len(), 2);
        let output = handle_command("/give @e[type=zombie] stone", &source, &players())
            .expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("Cannot find a player"));
    }

    #[test]
    fn selector_permission_needs_other_for_any_other_target() {
        let mut guest = player_source("Guest", (0.0, 64.0, 0.0));
        guest.is_op = false;
        guest.allow_cheats = false;
        guest.permissions = Some(std::sync::Arc::new(crate::permission::PermissionManager::new()));
        let players = vec![
            OnlinePlayer {
                name: "Guest".to_string(),
                display_name: "Guest".to_string(),
                position: (0.0, 64.0, 0.0),
                gamemode: 0,
            },
            OnlinePlayer {
                name: "Alex".to_string(),
                display_name: "Alex".to_string(),
                position: (5.0, 64.0, 5.0),
                gamemode: 0,
            },
        ];
        // @s only touches self, so the `.self` grant is enough.
        let output = handle_command("/clear @s", &guest, &players).expect("native");
        assert_eq!(output.actions.len(), 1);
        // @a includes Alex, which needs `.other` and is denied.
        let output = handle_command("/clear @a", &guest, &players).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("permission"));
    }

    #[test]
    fn selector_tp_teleports_every_match() {
        let source = player_source("Steve", (10.0, 64.0, -5.0));
        let output = handle_command("/tp @a Alex", &source, &players()).expect("native");
        // Both players teleport to Alex's position.
        assert_eq!(output.actions.len(), 2);
        assert_eq!(output.feedback.len(), 2);
        for action in &output.actions {
            match action {
                CommandAction::Teleport { position, .. } => {
                    assert_eq!(*position, (-20.0, 70.0, 8.5))
                }
                other => panic!("expected teleport, got {other:?}"),
            }
        }
        // Coord form with @a: every target gets the absolute position.
        let output = handle_command("/tp @a 0 80 0", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 2);
        for action in &output.actions {
            match action {
                CommandAction::Teleport { position, .. } => assert_eq!(*position, (0.0, 80.0, 0.0)),
                other => panic!("expected teleport, got {other:?}"),
            }
        }
        // @p as destination picks the nearest player (to the sender).
        let near_source = player_source("Steve", (0.0, 64.0, 0.0));
        let output = handle_command("/tp Alex @p", &near_source, &three_players()).expect("native");
        assert_eq!(output.actions.len(), 1);
        match &output.actions[0] {
            CommandAction::Teleport { position, .. } => assert_eq!(*position, (2.0, 64.0, 2.0)),
            other => panic!("expected teleport, got {other:?}"),
        }
    }

    #[test]
    fn selector_gamemode_xp_effect_and_title_expand() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));
        // Spectator avoids the already-in-mode failure (Alex is creative).
        let output = handle_command("/gamemode spectator @a", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 2);
        let output = handle_command("/xp 100 @a", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 2);
        let output = handle_command("/effect @a speed 30", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 2);
        assert_eq!(output.feedback.len(), 2);
        let output =
            handle_command("/title @a title Hola mundo", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 2);
        let output = handle_command("/spawnpoint @a 0 70 0", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 2);
        let output = handle_command("/spawn @a", &source, &players()).expect("native");
        assert_eq!(output.actions.len(), 2);
    }

    #[test]
    fn selector_only_players_match() {
        let source = player_source("Steve", (0.0, 64.0, 0.0));
        let output = handle_command("/give @z stone", &source, &players()).expect("native");
        assert!(output.actions.is_empty());
        assert!(output.feedback[0].contains("Cannot find a player"));
    }
}
