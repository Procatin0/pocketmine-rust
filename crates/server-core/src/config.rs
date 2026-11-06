use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub motd: String,
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
    pub level_seed: u64,
    pub gamemode: i32,
    pub view_distance: i32,
    pub transports: Vec<String>,
    pub builtin_signaling_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            motd: "Dedicated PocketMine-Rust Server".to_string(),
            server_port_udp: 19133,       // WebRTC DataChannel / RakNet UDP gameplay
            server_port_v6_udp: 19133,
            signaling_port_tcp: 19132,    // Primary port 19132 for TCP NetherNet signaling
            max_players: 1000,
            xbox_auth: false,
            white_list: false,            // Whitelist disabled
            auto_save: true,
            allow_cheats: true,           // Dev-server default: commands usable by everyone
            level_type: "DEFAULT".to_string(),
            level_seed: 12345678,
            gamemode: 0,
            view_distance: 10,
            transports: vec!["nethernet".to_string(), "raknet".to_string()],
            builtin_signaling_enabled: true,
        }
    }
}

impl ServerConfig {
    /// Loads configuration from `server.properties` and `pocketmine.yml`.
    pub fn load_or_create(base_dir: &Path) -> Self {
        let mut config = Self::default();

        let props_path = base_dir.join("server.properties");
        if props_path.exists() {
            if let Ok(content) = fs::read_to_string(&props_path) {
                config.parse_server_properties(&content);
                info!("Loaded server configuration from server.properties");
            }
        } else {
            let default_props = "# Minecraft: Bedrock Edition Server Properties for PocketMine-Rust\n\
                motd=Dedicated PocketMine-Rust Server\n\
                server-port=19133\n\
                server-portv6=19133\n\
                max-players=1000\n\
                white-list=0\n\
                allow-list=0\n\
                xbox-auth=off\n\
                allow-cheats=on\n\
                auto-save=on\n";
            let _ = fs::write(&props_path, default_props);
            info!("Created default server.properties");
        }

        let pm_path = base_dir.join("pocketmine.yml");
        if pm_path.exists() {
            if let Ok(content) = fs::read_to_string(&pm_path) {
                config.parse_pocketmine_yml(&content);
                info!("Loaded network & transport configuration from pocketmine.yml");
            }
        } else {
            let default_pm = "# PocketMine-MP / PocketMine-Rust Configuration\n\
                transport:\n\
                  name: nethernet,raknet\n\
                  nethernet:\n\
                    builtin-signaling:\n\
                      enabled: true\n\
                      port: 19132\n";
            let _ = fs::write(&pm_path, default_pm);
            info!("Created default pocketmine.yml");
        }

        // Protocol Validation: TCP signaling port and UDP gameplay port MUST be distinct
        if config.builtin_signaling_enabled && config.signaling_port_tcp == config.server_port_udp {
            warn!(
                "TCP signaling and UDP gameplay ports both configured as {}. \
                 Assigning TCP signaling to 19132 and UDP gameplay to 19133 to prevent collision.",
                config.signaling_port_tcp
            );
            config.signaling_port_tcp = 19132;
            config.server_port_udp = 19133;
        }

        info!(
            "Configuration active -> Primary TCP Signaling: {}, Gameplay UDP (WebRTC/RakNet): {}, Whitelist: {}",
            config.signaling_port_tcp, config.server_port_udp, if config.white_list { "ON" } else { "OFF" }
        );

        config
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
        if let Some(max_p) = map.get("max-players").and_then(|p| p.parse().ok()) {
            self.max_players = max_p;
        }
        if let Some(auth) = map.get("xbox-auth") {
            self.xbox_auth = *auth == "on" || *auth == "true" || *auth == "1";
        }
        if let Some(wl) = map.get("white-list").or_else(|| map.get("allow-list")).or_else(|| map.get("whitelist")) {
            self.white_list = *wl == "on" || *wl == "true" || *wl == "1";
        }
        if let Some(auto) = map.get("auto-save") {
            self.auto_save = *auto == "on" || *auto == "true" || *auto == "1";
        }
        if let Some(cheats) = map.get("allow-cheats") {
            self.allow_cheats = *cheats == "on" || *cheats == "true" || *cheats == "1";
        }
        if let Some(lt) = map.get("level-type") {
            self.level_type = lt.to_string();
        }
        if let Some(seed) = map.get("level-seed").and_then(|s| s.parse().ok()) {
            self.level_seed = seed;
        }
        if let Some(gm) = map.get("gamemode") {
            self.gamemode = match gm.to_lowercase().as_str() {
                "creative" | "c" | "1" => 1,
                "adventure" | "a" | "2" => 2,
                "spectator" | "sp" | "3" => 3,
                _ => 0,
            };
        }
        if let Some(vd) = map.get("view-distance").and_then(|v| v.parse().ok()) {
            self.view_distance = vd;
        }
    }

    /// Creates a world generator instance based on `level-type` and `level-seed`.
    pub fn create_world_generator(&self) -> std::sync::Arc<dyn bedrock_world::WorldGenerator> {
        bedrock_world::create_generator(&self.level_type, self.level_seed)
    }

    fn parse_pocketmine_yml(&mut self, content: &str) {
        let mut in_builtin_signaling = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with("builtin-signaling:") {
                in_builtin_signaling = true;
                continue;
            }

            if trimmed.starts_with("name:") && !line.contains("transport:") {
                let val = trimmed.trim_start_matches("name:").trim();
                self.transports = val.split(',').map(|s| s.trim().to_lowercase()).collect();
            }

            if in_builtin_signaling {
                if trimmed.starts_with("enabled:") {
                    let val = trimmed.trim_start_matches("enabled:").trim();
                    self.builtin_signaling_enabled = val == "true" || val == "on" || val == "1";
                } else if trimmed.starts_with("port:") {
                    let val = trimmed.trim_start_matches("port:").trim();
                    if let Ok(p) = val.parse::<u16>() {
                        if p != 0 {
                            self.signaling_port_tcp = p;
                        }
                    }
                } else if !line.starts_with(' ') && !line.starts_with('\t') {
                    in_builtin_signaling = false;
                }
            }
        }
    }
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
}
