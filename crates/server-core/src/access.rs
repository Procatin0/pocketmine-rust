//! Name/IP bans and the whitelist, ported from PocketMine's `BanList` and
//! `white-list.txt` handling.
//!
//! Files keep PocketMine's names (`banned-players.txt`, `banned-ips.txt`,
//! `white-list.txt`). Bans are stored as `entry|reason` lines and the whitelist
//! as one name per line, which is the closest simple format to PM's YAML
//! config lists that this port can round-trip without a config parser.

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use parking_lot::RwLock;

use crate::command::lang;

#[derive(Debug, Default)]
struct AccessData {
    /// Lowercase player name -> reason (may be empty).
    name_bans: BTreeMap<String, String>,
    /// Lowercase IP -> reason (may be empty).
    ip_bans: BTreeMap<String, String>,
    /// Lowercase player names.
    whitelist: BTreeSet<String>,
    whitelist_enabled: bool,
    /// Directory the lists were loaded from; `None` keeps everything in memory
    /// (tests and embedded servers never touch the disk).
    dir: Option<PathBuf>,
}

/// Thread-safe ban and whitelist registry.
#[derive(Debug)]
pub struct AccessList {
    inner: RwLock<AccessData>,
}

impl PartialEq for AccessList {
    fn eq(&self, other: &Self) -> bool {
        let left = self.inner.read();
        let right = other.inner.read();
        left.name_bans == right.name_bans
            && left.ip_bans == right.ip_bans
            && left.whitelist == right.whitelist
            && left.whitelist_enabled == right.whitelist_enabled
    }
}

impl Default for AccessList {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessList {
    /// In-memory list, nothing is read or written.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(AccessData::default()),
        }
    }

    /// Loads `banned-players.txt`, `banned-ips.txt` and `white-list.txt` from
    /// `dir`, and makes later changes persist there.
    pub fn load_from_dir(&self, dir: &Path) {
        let mut data = self.inner.write();
        data.dir = Some(dir.to_path_buf());
        data.name_bans = read_bans(&dir.join("banned-players.txt"));
        data.ip_bans = read_bans(&dir.join("banned-ips.txt"));
        data.whitelist = read_names(&dir.join("white-list.txt"));
        data.whitelist_enabled = read_whitelist_flag(&dir.join("server.properties"));
    }

    /// Re-reads the whitelist from disk, like `/whitelist reload`.
    pub fn reload_whitelist(&self) {
        let mut data = self.inner.write();
        if let Some(dir) = data.dir.clone() {
            data.whitelist = read_names(&dir.join("white-list.txt"));
        }
    }

    pub fn ban_name(&self, name: &str, reason: &str) {
        let mut data = self.inner.write();
        data.name_bans
            .insert(name.to_lowercase(), reason.to_string());
        data.persist_names();
    }

    pub fn pardon_name(&self, name: &str) {
        let mut data = self.inner.write();
        data.name_bans.remove(&name.to_lowercase());
        data.persist_names();
    }

    pub fn ban_ip(&self, ip: &str, reason: &str) {
        let mut data = self.inner.write();
        data.ip_bans.insert(ip.to_lowercase(), reason.to_string());
        data.persist_ips();
    }

    pub fn pardon_ip(&self, ip: &str) {
        let mut data = self.inner.write();
        data.ip_bans.remove(&ip.to_lowercase());
        data.persist_ips();
    }

    pub fn is_ip_banned(&self, ip: &str) -> bool {
        self.inner.read().ip_bans.contains_key(&ip.to_lowercase())
    }

    /// Reason of a name ban, `Some("")` when banned without reason.
    pub fn name_ban_reason(&self, name: &str) -> Option<String> {
        self.inner
            .read()
            .name_bans
            .get(&name.to_lowercase())
            .cloned()
    }

    pub fn ip_ban_reason(&self, ip: &str) -> Option<String> {
        self.inner.read().ip_bans.get(&ip.to_lowercase()).cloned()
    }

    pub fn banned_names(&self) -> Vec<String> {
        self.inner.read().name_bans.keys().cloned().collect()
    }

    pub fn banned_ips(&self) -> Vec<String> {
        self.inner.read().ip_bans.keys().cloned().collect()
    }

    pub fn whitelist_entries(&self) -> Vec<String> {
        self.inner.read().whitelist.iter().cloned().collect()
    }

    pub fn whitelist_enabled(&self) -> bool {
        self.inner.read().whitelist_enabled
    }

    /// Enabling/disabling the whitelist changes `white-list` in
    /// `server.properties` in PocketMine; this port keeps the flag in memory.
    pub fn set_whitelist_enabled(&self, enabled: bool) {
        self.inner.write().whitelist_enabled = enabled;
    }

    pub fn add_whitelist(&self, name: &str) {
        let mut data = self.inner.write();
        data.whitelist.insert(name.to_lowercase());
        data.persist_whitelist();
    }

    pub fn remove_whitelist(&self, name: &str) {
        let mut data = self.inner.write();
        data.whitelist.remove(&name.to_lowercase());
        data.persist_whitelist();
    }

    pub fn is_whitelisted(&self, name: &str) -> bool {
        self.inner.read().whitelist.contains(&name.to_lowercase())
    }

    /// Login reply (`pocketmine.disconnect.*`) when the player may not join.
    ///
    /// Operators bypass the whitelist, exactly like `Server::isWhitelisted()`.
    pub fn denial(&self, name: &str, ip: &str, is_op: bool) -> Option<String> {
        let data = self.inner.read();
        if let Some(reason) = data.name_bans.get(&name.to_lowercase()) {
            return Some(if reason.is_empty() {
                lang::DISCONNECT_BAN_NOREASON.to_string()
            } else {
                lang::DISCONNECT_BAN.replace("{%reason}", reason)
            });
        }
        if let Some(reason) = data.ip_bans.get(&ip.to_lowercase()) {
            let reason = if reason.is_empty() {
                lang::DISCONNECT_BAN_IP.to_string()
            } else {
                reason.clone()
            };
            return Some(lang::DISCONNECT_BAN.replace("{%reason}", &reason));
        }
        if data.whitelist_enabled
            && !is_op
            && !data.whitelist.contains(&name.to_lowercase())
        {
            return Some(lang::DISCONNECT_WHITELISTED.to_string());
        }
        None
    }
}

/// Parses `entry|reason` lines, ignoring blanks and `#` comments.
fn read_bans(path: &Path) -> BTreeMap<String, String> {
    let mut bans = BTreeMap::new();
    let Ok(contents) = std::fs::read_to_string(path) else {
        return bans;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_once('|') {
            Some((entry, reason)) => {
                bans.insert(entry.trim().to_lowercase(), reason.trim().to_string());
            }
            None => {
                bans.insert(line.to_lowercase(), String::new());
            }
        }
    }
    bans
}

/// Reads `white-list`/`allow-list` from `server.properties`.
fn read_whitelist_flag(path: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return false;
    };
    for line in contents.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if key == "white-list" || key == "whitelist" || key == "allow-list" {
                return matches!(value.trim().to_lowercase().as_str(), "true" | "on" | "1");
            }
        }
    }
    false
}

fn read_names(path: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Ok(contents) = std::fs::read_to_string(path) else {
        return names;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        names.insert(line.to_lowercase());
    }
    names
}

impl AccessData {
    fn persist_names(&self) {
        let Some(dir) = &self.dir else {
            return;
        };
        let contents: String = self
            .name_bans
            .iter()
            .map(|(name, reason)| format!("{name}|{reason}\n"))
            .collect();
        let _ = std::fs::write(dir.join("banned-players.txt"), contents);
    }

    fn persist_ips(&self) {
        let Some(dir) = &self.dir else {
            return;
        };
        let contents: String = self
            .ip_bans
            .iter()
            .map(|(ip, reason)| format!("{ip}|{reason}\n"))
            .collect();
        let _ = std::fs::write(dir.join("banned-ips.txt"), contents);
    }

    fn persist_whitelist(&self) {
        let Some(dir) = &self.dir else {
            return;
        };
        let contents: String = self.whitelist.iter().map(|name| format!("{name}\n")).collect();
        let _ = std::fs::write(dir.join("white-list.txt"), contents);
    }
}

/// `inet_pton()` semantics: both IPv4 and IPv6 literals are accepted.
pub fn is_ip_address(value: &str) -> bool {
    value.parse::<IpAddr>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bans_and_whitelist_decide_who_may_join() {
        let access = AccessList::new();
        assert!(access.denial("Steve", "10.0.0.1", false).is_none());

        access.ban_name("Steve", "griefing");
        assert_eq!(
            access.denial("steve", "10.0.0.1", false),
            Some("Estás baneado de este servidor. Motivo: griefing".to_string())
        );

        access.ban_ip("10.0.0.2", "");
        assert_eq!(
            access.denial("Alex", "10.0.0.2", false),
            Some("Estás baneado de este servidor. Motivo: Baneado de IP".to_string())
        );

        access.set_whitelist_enabled(true);
        assert_eq!(
            access.denial("Alex", "10.0.0.3", false),
            Some("El servidor está en modo lista blanca".to_string())
        );
        // Operators bypass the whitelist, like PocketMine.
        assert!(access.denial("Alex", "10.0.0.3", true).is_none());

        access.add_whitelist("Alex");
        assert!(access.denial("alex", "10.0.0.3", false).is_none());
        assert_eq!(access.whitelist_entries(), vec!["alex".to_string()]);

        access.pardon_name("Steve");
        access.pardon_ip("10.0.0.2");
        assert!(access.denial("Steve", "10.0.0.2", true).is_none());
    }

    #[test]
    fn detects_ip_literals_like_inet_pton() {
        assert!(is_ip_address("192.168.0.1"));
        assert!(is_ip_address("::1"));
        assert!(!is_ip_address("Steve"));
        assert!(!is_ip_address("999.1.1.1"));
    }
}
