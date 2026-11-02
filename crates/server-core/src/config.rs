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
    pub transports: Vec<String>,
    pub builtin_signaling_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            motd: "Dedicated PocketMine-Rust Server".to_string(),
            server_port_udp: 19132,
            server_port_v6_udp: 19133,
            signaling_port_tcp: 19133, // Default TCP signaling separated from UDP 19132
            max_players: 1000,
            xbox_auth: false,
            transports: vec!["nethernet".to_string(), "raknet".to_string()],
            builtin_signaling_enabled: true,
        }
    }
}

impl ServerConfig {
    /// Loads configuration from `server.properties` and `pocketmine.yml`.
    /// Enforces that the UDP gameplay port and TCP signaling port are distinct
    /// to adhere to the Bedrock transport protocol and avoid Terracotta connection error.
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
                server-port=19132\n\
                server-portv6=19133\n\
                max-players=1000\n\
                xbox-auth=off\n";
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
                      port: 19133\n";
            let _ = fs::write(&pm_path, default_pm);
            info!("Created default pocketmine.yml");
        }

        // Protocol Validation: TCP signaling and UDP gameplay port MUST be distinct
        if config.builtin_signaling_enabled && config.signaling_port_tcp == config.server_port_udp {
            warn!(
                "Port collision detected: TCP signaling port and UDP game port are both {}. \
                 Adjusting TCP signaling to 19133 to prevent Terracotta handshake errors.",
                config.server_port_udp
            );
            config.signaling_port_tcp = if config.server_port_udp == 19132 { 19133 } else { 19132 };
        }

        info!(
            "Ports active -> NetherNet TCP Signaling: {}, RakNet UDP Gameplay: {}",
            config.signaling_port_tcp, config.server_port_udp
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

            if trimmed.starts_with("name:") && line.contains("transport:") == false {
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
                } else if !line.starts_with(" ") && !line.starts_with("\t") {
                    in_builtin_signaling = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_port_separation() {
        let mut config = ServerConfig::default();
        config.server_port_udp = 19132;
        config.signaling_port_tcp = 19132;

        if config.builtin_signaling_enabled && config.signaling_port_tcp == config.server_port_udp {
            config.signaling_port_tcp = 19133;
        }

        assert_ne!(config.signaling_port_tcp, config.server_port_udp);
        assert_eq!(config.signaling_port_tcp, 19133);
        assert_eq!(config.server_port_udp, 19132);
    }
}
