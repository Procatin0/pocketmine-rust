use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{debug, warn};

use crate::translations::Translator;

/// `Server::MAX_ACTIVE_TRANSPORTS`.
const MAX_ACTIVE_TRANSPORTS: usize = 8;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub motd: String,
    pub server_ip: String,
    pub server_ip_v6: String,
    pub enable_ipv6: bool,
    pub enable_query: bool,
    pub server_port_udp: u16,
    pub server_port_v6_udp: u16,
    pub signaling_port_tcp: u16,
    pub max_players: i32,
    pub xbox_auth: bool,
    pub white_list: bool,
    /// `auto-save`: persists `/save-on` and `/save-off`.
    pub auto_save: bool,
    /// `allow-cheats`: when enabled, every player can use administrative
    /// commands such as `/tp` and `/gamemode`.
    pub allow_cheats: bool,
    pub level_type: String,
    pub level_name: String,
    pub level_seed: String,
    pub default_world_generator: String,
    pub default_world_generator_settings: String,
    pub gamemode: i32,
    pub difficulty: i32,
    pub force_game_mode: bool,
    pub hardcore: bool,
    pub view_distance: i32,
    pub transports: Vec<String>,
    pub builtin_signaling_enabled: bool,
    pub builtin_signaling_port: u16,
    pub nethernet_key_file: String,
    pub nethernet_identity_domain: String,
    pub nethernet_ice_udp_mux: bool,
    pub nethernet_port_range: Option<(u16, u16)>,
    pub nethernet_mtu: u16,
    pub nethernet_tls_certificate: String,
    pub nethernet_tls_key: String,
    pub nethernet_tls_passphrase: String,
    pub compression_level: i32,
    pub batch_threshold: i32,
    pub async_compression: bool,
    pub async_compression_threshold: i32,
    pub max_mtu_size: u16,
    pub enable_encryption: bool,
    pub upnp_forwarding: bool,
    pub debug_level: i32,
    pub enable_dev_builds: bool,
    pub shutdown_message: String,
    pub force_language: bool,
    pub language: String,
    pub verify_xuid: bool,
    pub save_player_data: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            motd: "Axolotl-PM Server".to_string(),
            server_ip: "0.0.0.0".to_string(),
            server_ip_v6: "::".to_string(),
            enable_ipv6: true,
            enable_query: true,
            server_port_udp: 19133,       // WebRTC DataChannel / RakNet UDP gameplay
            server_port_v6_udp: 19133,
            signaling_port_tcp: 19132,    // Primary port 19132 for TCP NetherNet signaling
            max_players: 20,
            xbox_auth: true,
            white_list: false,            // Whitelist disabled
            auto_save: true,
            allow_cheats: false,          // PM default: cheats off
            level_type: "DEFAULT".to_string(),
            level_name: "world".to_string(),
            level_seed: String::new(),
            default_world_generator: "DEFAULT".to_string(),
            default_world_generator_settings: String::new(),
            gamemode: 0,
            difficulty: 1,
            force_game_mode: false,
            hardcore: false,
            view_distance: 16,
            transports: vec!["nethernet".to_string(), "raknet".to_string()],
            builtin_signaling_enabled: true,
            builtin_signaling_port: 0,
            nethernet_key_file: "nethernet.key".to_string(),
            nethernet_identity_domain: String::new(),
            nethernet_ice_udp_mux: false,
            nethernet_port_range: None,
            nethernet_mtu: 1200,
            nethernet_tls_certificate: String::new(),
            nethernet_tls_key: String::new(),
            nethernet_tls_passphrase: String::new(),
            compression_level: 6,
            batch_threshold: 256,
            async_compression: false,
            async_compression_threshold: 10000,
            max_mtu_size: 1492,
            enable_encryption: true,
            upnp_forwarding: false,
            debug_level: 1,
            // Rust-port divergence: PM defaults this to `false` and refuses to
            // run dev builds until the operator opts in. The Rust port is
            // itself a dev build (`5.51.2-dev`), so it defaults to `true` to
            // start out of the box; set it to `false` in `pocketmine.yml` for
            // strict PM parity (5x EMERGENCY + exit). See `main.rs`.
            enable_dev_builds: true,
            shutdown_message: "Server closed".to_string(),
            force_language: false,
            language: "eng".to_string(),
            verify_xuid: true,
            save_player_data: true,
        }
    }
}

impl ServerConfig {
    /// Loads configuration from `server.properties` and `pocketmine.yml`.
    ///
    /// Missing files are materialised silently, exactly like `Server.php`:
    /// `pocketmine.yml` is copied from the reference resource (with the
    /// dev-build channel swap) and `server.properties` falls back to the
    /// `ServerProperties` defaults without any log line. Only genuine
    /// validation failures (`compression-level`, unknown transports) log, at
    /// the same `WARNING` level and wording as `Server.php`.
    pub fn load_or_create(base_dir: &Path) -> Self {
        let mut config = Self::default();

        let props_path = base_dir.join("server.properties");
        if props_path.exists() {
            if let Ok(content) = fs::read_to_string(&props_path) {
                config.parse_server_properties(&content);
            }
        } else {
            let default_props = "#Minecraft: Bedrock Edition Server Properties\n\
                motd=Axolotl-PM Server\n\
                server-port=19132\n\
                server-portv6=19133\n\
                server-ip=0.0.0.0\n\
                enable-ipv6=true\n\
                gametype=survival\n\
                force-gamemode=false\n\
                hardcore=false\n\
                pvp=true\n\
                difficulty=1\n\
                level-type=DEFAULT\n\
                level-seed=\n\
                level-name=world\n\
                view-distance=16\n\
                max-players=20\n\
                enable-query=true\n\
                auto-save=true\n\
                white-list=false\n\
                xbox-auth=true\n\
                language=eng\n\
                allow-cheats=false\n\
                ";
            let _ = fs::write(&props_path, default_props);
        }

        let pm_path = base_dir.join("pocketmine.yml");
        if pm_path.exists() {
            if let Ok(content) = fs::read_to_string(&pm_path) {
                config.parse_pocketmine_yml(&content);
            }
        } else {
            let default_pm = include_str!("pocketmine_default.yml");
            let _ = fs::write(&pm_path, default_pm);
        }

        // `Server.php`: validate `network.compression-level`.
        // "Invalid network compression level $n set, setting to default 6"
        if config.compression_level < 1 || config.compression_level > 9 {
            warn!(
                "Invalid network compression level {} set, setting to default 6",
                config.compression_level
            );
            config.compression_level = 6;
        }

        // `Server.php::startupPrepareNetworkInterfaces`: cap transports.
        // Unknown-transport warnings are emitted in PM order by `main.rs`
        // during network startup, not here during config parsing.
        if config.transports.len() > MAX_ACTIVE_TRANSPORTS {
            config.transports.truncate(MAX_ACTIVE_TRANSPORTS);
        }

        // TCP signaling (19132) and UDP gameplay (19132 from server.properties)
        // may share the same port number: they are different protocols, so
        // there is no socket collision (PocketMine's own default is both
        // 19132). The earlier code "fixed" that equality by moving gameplay
        // to 19133, which collided with the WebRTC UDP socket (RakNet and
        // WebRTC both bound UDP 19133). Only guarantee a nonzero signaling
        // port here; the distinct WebRTC UDP port is chosen in main.rs
        // (port-range first, otherwise signaling+1).
        if config.signaling_port_tcp == 0 {
            config.signaling_port_tcp = 19132;
        }

        debug!(
            "ServerConfig loaded (xbox-auth: {}, transports: {:?})",
            if config.xbox_auth { "on" } else { "off" },
            config.transports
        );

        config
    }

    /// Resolves the HTTP signaling bind port: `0` means server-port
    /// (`NetherNetTransport::createBuiltinSignaling`).
    pub fn effective_signaling_port(&self) -> u16 {
        if self.builtin_signaling_port == 0 {
            self.server_port_udp.max(self.signaling_port_tcp.min(65535) as u16)
                .min(65535)
        } else {
            self.builtin_signaling_port
        }
    }

    /// `language.selected` line, always English (`eng`).
    pub fn language_selected_message(&self) -> String {
        let lang = Translator::english();
        let name = lang
            .get("language.name")
            .unwrap_or("English")
            .to_string();
        lang.fill("language.selected", &[&name, &self.language])
            .unwrap_or_else(|| format!("Selected {name} ({}) as the base language", self.language))
    }

    fn parse_server_properties(&mut self, content: &str) {
        let mut map = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                map.insert(k.trim(), v.trim());
            }
        }

        if let Some(motd) = map.get("motd") {
            self.motd = motd.to_string();
        }
        if let Some(port) = map.get("server-port").and_then(|p| p.parse().ok()) {
            self.server_port_udp = port;
        }
        if let Some(port_v6) = map.get("server-portv6").and_then(|p| p.parse().ok()) {
            self.server_port_v6_udp = port_v6;
        }
        if let Some(ip) = map.get("server-ip") {
            if !ip.is_empty() {
                self.server_ip = ip.to_string();
            }
        }
        if let Some(ip) = map.get("server-ipv6") {
            if !ip.is_empty() {
                self.server_ip_v6 = ip.to_string();
            }
        }
        if let Some(v) = map.get("enable-ipv6") {
            self.enable_ipv6 = parse_bool(v);
        }
        if let Some(v) = map.get("enable-query") {
            self.enable_query = parse_bool(v);
        }
        if let Some(max_p) = map.get("max-players").and_then(|p| p.parse().ok()) {
            self.max_players = max_p;
        }
        if let Some(auth) = map.get("xbox-auth") {
            // PM `Config::getConfigBool(xbox-auth, true)`: anything not
            // explicitly true/false/on/off/1/0 keeps the default. We keep the
            // same truthiness table as the old parser, with the PM default
            // already set to `true` in `Default`.
            if is_bool_literal(auth) {
                self.xbox_auth = parse_bool(auth);
            }
        }
        if let Some(wl) = map.get("white-list").or_else(|| map.get("allow-list")).or_else(|| map.get("whitelist")) {
            self.white_list = parse_bool(wl);
        }
        if let Some(auto) = map.get("auto-save") {
            self.auto_save = parse_bool(auto);
        }
        if let Some(cheats) = map.get("allow-cheats") {
            self.allow_cheats = parse_bool(cheats);
        }
        if let Some(lang) = map.get("language") {
            if !lang.is_empty() {
                self.language = lang.to_lowercase();
            }
        }
        if let Some(lt) = map.get("level-type") {
            if !lt.is_empty() {
                self.level_type = lt.to_string();
            }
        }
        if let Some(name) = map.get("level-name") {
            if !name.is_empty() {
                self.level_name = name.to_string();
            }
        }
        if let Some(seed) = map.get("level-seed") {
            self.level_seed = seed.to_string();
        }
        if let Some(gm) = map.get("gamemode").or_else(|| map.get("gametype")) {
            self.gamemode = match gm.to_lowercase().as_str() {
                "creative" | "c" | "1" => 1,
                "adventure" | "a" | "2" => 2,
                "spectator" | "sp" | "3" | "v" => 3,
                _ => 0,
            };
        }
        if let Some(v) = map.get("force-gamemode") {
            self.force_game_mode = parse_bool(v);
        }
        if let Some(v) = map.get("hardcore") {
            self.hardcore = parse_bool(v);
        }
        if let Some(d) = map.get("difficulty").and_then(|d| d.parse().ok()) {
            self.difficulty = d;
        }
        if let Some(vd) = map.get("view-distance").and_then(|v| v.parse().ok()) {
            self.view_distance = vd;
        }
    }

    /// Creates a world generator instance based on `level-type` and `level-seed`.
    pub fn create_world_generator(&self) -> std::sync::Arc<dyn bedrock_world::WorldGenerator> {
        let seed = self
            .level_seed
            .trim()
            .parse::<u64>()
            .unwrap_or_else(|_| {
                // `Generator::convertSeed("")` picks a random seed; PM keeps the
                // string form. Fall back to a stable 0 for the generator hint;
                // the numeric seed is fully resolved by the world layer.
                let mut h: u64 = 0;
                for b in self.level_seed.bytes() {
                    h = h.wrapping_mul(31).wrapping_add(b as u64);
                }
                h
            });
        bedrock_world::create_generator(&self.level_type, seed)
    }

    fn parse_pocketmine_yml(&mut self, content: &str) {
        // Minimal indentation-aware scan for the keys Server.php reads.
        // Section handling is intentionally lenient: unknown keys are ignored.
        #[derive(PartialEq, Eq, Clone, Copy)]
        enum Section {
            Root,
            Settings,
            Network,
            Transport,
            NetherNet,
            BuiltinSignaling,
            Tls,
            Debug,
            Player,
        }
        let mut section = Section::Root;
        let mut in_transport = false;

        for raw_line in content.lines() {
            if raw_line.trim().is_empty() || raw_line.trim_start().starts_with('#') {
                continue;
            }
            let indent = raw_line.len() - raw_line.trim_start().len();
            let trimmed = raw_line.trim();

            if indent == 0 {
                match trimmed {
                    "settings:" => { section = Section::Settings; in_transport = false; continue; }
                    "network:" => { section = Section::Network; in_transport = false; continue; }
                    "transport:" => { section = Section::Transport; in_transport = true; continue; }
                    "debug:" => { section = Section::Debug; in_transport = false; continue; }
                    "player:" => { section = Section::Player; in_transport = false; continue; }
                    _ => { section = Section::Root; in_transport = false; }
                }
            } else if in_transport && indent <= 2 {
                match trimmed {
                    "nethernet:" => { section = Section::NetherNet; continue; }
                    _ => { section = Section::Transport; }
                }
            } else if section == Section::NetherNet && indent <= 4 {
                if trimmed == "builtin-signaling:" {
                    section = Section::BuiltinSignaling;
                    continue;
                }
                if indent <= 4 && trimmed.starts_with("tls:") {
                    section = Section::Tls;
                    continue;
                }
            } else if section == Section::BuiltinSignaling && indent <= 6 {
                if trimmed.starts_with("tls:") {
                    section = Section::Tls;
                    continue;
                }
            }

            let Some((key, value)) = trimmed.split_once(':') else {
                continue;
            };
            let key = key.trim();
            let value = value.split('#').next().unwrap_or("").trim();
            let value = value.trim_matches('"').trim_matches('\'');

            match section {
                Section::Settings => match key {
                    "force-language" => self.force_language = parse_bool(value),
                    "shutdown-message" => self.shutdown_message = value.to_string(),
                    "enable-dev-builds" => self.enable_dev_builds = parse_bool(value),
                    "language" => {
                        if !value.is_empty() {
                            self.language = value.to_lowercase();
                        }
                    }
                    _ => {}
                },
                Section::Network => match key {
                    "compression-level" => {
                        if let Ok(v) = value.parse::<i32>() {
                            self.compression_level = v;
                        }
                    }
                    "batch-threshold" => {
                        if let Ok(v) = value.parse::<i32>() {
                            self.batch_threshold = v;
                        }
                    }
                    "async-compression" => self.async_compression = parse_bool(value),
                    "async-compression-threshold" => {
                        if let Ok(v) = value.parse::<i32>() {
                            self.async_compression_threshold = v;
                        }
                    }
                    "max-mtu-size" => {
                        if let Ok(v) = value.parse::<u16>() {
                            self.max_mtu_size = v;
                        }
                    }
                    "enable-encryption" => self.enable_encryption = parse_bool(value),
                    "upnp-forwarding" => self.upnp_forwarding = parse_bool(value),
                    _ => {}
                },
                Section::Debug => match key {
                    "level" => {
                        if let Ok(v) = value.parse::<i32>() {
                            self.debug_level = v;
                        }
                    }
                    _ => {}
                },
                Section::Player => match key {
                    "verify-xuid" => self.verify_xuid = parse_bool(value),
                    "save-player-data" => self.save_player_data = parse_bool(value),
                    _ => {}
                },
                Section::Transport => match key {
                    "name" => {
                        let names: Vec<String> = value
                            .split(',')
                            .map(|s| s.trim().to_lowercase())
                            .filter(|s| !s.is_empty())
                            .collect();
                        if !names.is_empty() {
                            self.transports = names;
                        }
                    }
                    _ => {}
                },
                Section::NetherNet => match key {
                    "key-file" => {
                        if !value.is_empty() {
                            self.nethernet_key_file = value.to_string();
                        }
                    }
                    "identity-domain" => self.nethernet_identity_domain = value.to_string(),
                    "ice-udp-mux" => self.nethernet_ice_udp_mux = parse_bool(value),
                    "mtu" => {
                        if let Ok(v) = value.parse::<u16>() {
                            self.nethernet_mtu = v;
                        }
                    }
                    "port-range" => {
                        self.nethernet_port_range = parse_port_range(value);
                    }
                    _ => {}
                },
                Section::BuiltinSignaling => match key {
                    "enabled" => self.builtin_signaling_enabled = parse_bool(value),
                    "port" => {
                        if let Ok(v) = value.parse::<u16>() {
                            self.builtin_signaling_port = v;
                        }
                    }
                    _ => {}
                },
                Section::Tls => match key {
                    "certificate" => self.nethernet_tls_certificate = value.to_string(),
                    "key" => self.nethernet_tls_key = value.to_string(),
                    "passphrase" => self.nethernet_tls_passphrase = value.to_string(),
                    _ => {}
                },
                Section::Root => {}
            }
        }
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "on" | "true" | "1" | "yes"
    )
}

fn is_bool_literal(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "on" | "true" | "1" | "yes" | "off" | "false" | "0" | "no"
    )
}

fn parse_port_range(value: &str) -> Option<(u16, u16)> {
    let value = value.trim();
    if value.is_empty() || value == "null" || value == "~" {
        return None;
    }
    // YAML flow `[19133, 19140]` or plain `19133`.
    let inner = value.trim_start_matches('[').trim_end_matches(']');
    let mut parts = inner.split(',');
    let first = parts.next()?.trim().parse::<u16>().ok()?;
    let second = parts
        .next()
        .map(|s| s.trim().parse::<u16>())
        .transpose()
        .ok()
        .flatten();
    Some(match second {
        Some(last) => (first, last),
        None => (first, first),
    })
}

/// Writes one `server.properties` entry in place, mirroring
/// `ConfigGroup::setConfigBool()` + `save()`. Comments and untouched keys stay.
pub fn set_server_property(base_dir: &Path, key: &str, value: &str) {
    let path = base_dir.join("server.properties");
    let content = fs::read_to_string(&path).unwrap_or_default();

    let mut lines: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        let matches_key = !trimmed.starts_with('#')
            && line
                .split_once('=')
                .is_some_and(|(name, _)| name.trim() == key);
        if matches_key {
            if !replaced {
                lines.push(format!("{key}={value}"));
                replaced = true;
            }
            continue;
        }
        lines.push(line.to_string());
    }
    if !replaced {
        lines.push(format!("{key}={value}"));
    }

    let mut out = lines.join("\n");
    out.push('\n');
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(error) = fs::write(&path, out) {
        warn!("Could not persist {key} in server.properties: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_defaults() {
        let config = ServerConfig::default();
        assert_eq!(config.signaling_port_tcp, 19132);
        assert_eq!(config.server_port_udp, 19133);
        assert!(!config.white_list);
        assert!(config.xbox_auth);
        assert_eq!(config.language, "eng");
        assert_eq!(config.compression_level, 6);
        assert_eq!(config.level_name, "world");
        assert_eq!(config.transports, vec!["nethernet", "raknet"]);
    }

    #[test]
    fn parses_bool_literals() {
        assert!(parse_bool("on"));
        assert!(parse_bool("true"));
        assert!(parse_bool("1"));
        assert!(!parse_bool("off"));
        assert!(!parse_bool("false"));
        assert!(!parse_bool("0"));
        assert!(is_bool_literal("off"));
        assert!(!is_bool_literal("maybe"));
    }

    #[test]
    fn invalid_compression_level_falls_back_to_six() {
        let dir = std::env::temp_dir().join(format!("pm-rust-cfg-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("server.properties"), "motd=Test\n").unwrap();
        fs::write(
            dir.join("pocketmine.yml"),
            "network:\n  compression-level: 99\n",
        )
        .unwrap();
        let config = ServerConfig::load_or_create(&dir);
        assert_eq!(config.compression_level, 6);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn language_selected_message_uses_english() {
        let config = ServerConfig::default();
        assert_eq!(
            config.language_selected_message(),
            "Selected English (eng) as the base language"
        );
    }

    #[test]
    fn set_server_property_replaces_and_appends() {
        let dir = std::env::temp_dir().join(format!("pm-rust-props-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("server.properties");
        fs::write(&path, "# comment\nmotd=Test\nwhite-list=off\n").unwrap();

        set_server_property(&dir, "white-list", "on");
        set_server_property(&dir, "auto-save", "off");

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# comment"));
        assert!(content.contains("motd=Test"));
        assert!(content.contains("white-list=on"));
        assert!(!content.contains("white-list=off"));
        assert!(content.contains("auto-save=off"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_debug_level_after_transport_section() {
        let dir = std::env::temp_dir().join(format!("pm-rust-dbg-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("server.properties"), "motd=Test\n").unwrap();
        fs::write(
            dir.join("pocketmine.yml"),
            "transport:\n  name: nethernet\n  nethernet:\n    key-file: nethernet.key\n    builtin-signaling:\n      enabled: true\n      tls:\n        certificate: \"\"\ndebug:\n  level: 2\n",
        )
        .unwrap();
        let config = ServerConfig::load_or_create(&dir);
        assert_eq!(config.debug_level, 2, "debug section after transport must parse");
        let _ = fs::remove_dir_all(&dir);
    }
}
