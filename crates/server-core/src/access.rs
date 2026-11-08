//! Name/IP bans and the whitelist, ported from PocketMine's `BanList`,
//! `BanEntry` and `white-list.txt` handling.
//!
//! Files keep PocketMine's names (`banned-players.txt`, `banned-ips.txt`,
//! `white-list.txt`). Each ban line is the PM `BanEntry::getString()` format:
//! `name|created|source|expires|reason`. Legacy lines written by this port
//! before `BanEntry` existed (`name|reason`, or bare `name`) are still read:
//! with two fields the second is the reason (PM would read it as the creation
//! date and fail — we treat unparseable dates as "no date").
//!
//! Expiration mirrors `BanList::isBanned`/`getEntry`: `remove_expired()` runs
//! before every check, so an expired entry is invisible without an explicit
//! pardon. `source` defaults to `(Unknown)` and `reason` to
//! `Banned by an operator.`, exactly like PM's `BanEntry` defaults.

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use parking_lot::RwLock;

use crate::command::lang;

/// `BanEntry::$format` (`Y-m-d H:i:s O`, simplified: a UTC-normalised
/// timestamp string so `from_string`/`get_string` round-trip without a date
/// parser).
pub const BAN_DATE_FORMAT: &str = "Y-m-d H:i:s";

/// One PM `BanEntry`.
#[derive(Debug, Clone, PartialEq)]
pub struct BanEntry {
    pub name: String,
    pub created: i64,
    pub source: String,
    pub expires: Option<i64>,
    pub reason: String,
}

impl BanEntry {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_lowercase(),
            created: now_unix(),
            source: "(Unknown)".to_string(),
            expires: None,
            reason: "Banned by an operator.".to_string(),
        }
    }

    pub fn with_reason(name: &str, reason: &str) -> Self {
        let mut entry = Self::new(name);
        entry.reason = reason.to_string();
        entry
    }

    pub fn has_expired(&self) -> bool {
        self.expires.is_some_and(|expires| expires < now_unix())
    }

    fn has_expired_at(&self, now: i64) -> bool {
        self.expires.is_some_and(|expires| expires < now)
    }

    /// `BanEntry::getString()`: `name|created|source|expires|reason`,
    /// with `Forever` for no expiration (PM's literal).
    pub fn get_string(&self) -> String {
        let expires = self
            .expires
            .map(|expires| format_unix(expires))
            .unwrap_or_else(|| "Forever".to_string());
        format!(
            "{}|{}|{}|{}|{}",
            self.name,
            format_unix(self.created),
            self.source,
            expires,
            self.reason
        )
    }

    /// `BanEntry::fromString`, tolerant of the legacy `name|reason` format:
    /// a second field that parses as a date is the creation date (PM
    /// behaviour); anything else is the reason.
    pub fn from_string(line: &str) -> Option<Self> {
        let line = line.trim();
        if line.len() < 2 {
            return None;
        }
        let mut parts: Vec<&str> = line.splitn(6, '|').collect();
        let name = parts.remove(0).trim().to_lowercase();
        if name.is_empty() {
            return None;
        }
        let mut entry = Self::new(&name);
        if parts.is_empty() {
            return Some(entry);
        }
        // Legacy `name|reason`: the second field is not a date.
        if parts.len() == 1 {
            if let Some(created) = parse_unix(parts[0]) {
                entry.created = created;
            } else if !parts[0].trim().is_empty() {
                entry.reason = parts[0].trim().to_string();
            }
            return Some(entry);
        }
        if let Some(created) = parse_unix(parts.remove(0)) {
            entry.created = created;
        }
        if !parts.is_empty() {
            entry.source = parts.remove(0).trim().to_string();
        }
        if !parts.is_empty() {
            let expire = parts.remove(0).trim();
            if !expire.is_empty() && !expire.eq_ignore_ascii_case("Forever") {
                entry.expires = parse_unix(expire);
            }
        }
        if !parts.is_empty() {
            entry.reason = parts.remove(0).trim().to_string();
        }
        Some(entry)
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Formats a unix timestamp as `Y-m-d H:i:s` in UTC. PM uses local time
/// with an offset (`Y-m-d H:i:s O`); UTC keeps the round-trip stable across
/// machines. `parse_unix` accepts both that format and a raw unix integer.
fn format_unix(unix: i64) -> String {
    // Civil-from-days (Howard Hinnant's algorithm) in UTC.
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    let (hour, minute, second) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

fn parse_unix(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("Forever") {
        return None;
    }
    if let Ok(unix) = text.parse::<i64>() {
        return Some(unix);
    }
    // `Y-m-d H:i:s` (with or without an `+0000` offset suffix).
    let date_part = text.split(' ').take(2).collect::<Vec<_>>().join(" ");
    let date_part = date_part.trim();
    let (date, time) = date_part.split_once(' ')?;
    let (year, month, day) = (
        date.split_once('-')?.0.parse::<i64>().ok()?,
        date.split_once('-')?.1.split_once('-')?.0.parse::<i64>().ok()?,
        date.rsplit_once('-')?.1.parse::<i64>().ok()?,
    );
    let (hour, minute, second) = (
        time.split_once(':')?.0.parse::<i64>().ok()?,
        time.split_once(':')?.1.split_once(':')?.0.parse::<i64>().ok()?,
        time.rsplit_once(':')?.1.parse::<i64>().ok()?,
    );
    // Days-from-civil.
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
}

#[derive(Debug, Default)]
struct AccessData {
    /// Lowercase player name -> entry.
    name_bans: BTreeMap<String, BanEntry>,
    /// Lowercase IP -> entry.
    ip_bans: BTreeMap<String, BanEntry>,
    /// Lowercase player names.
    whitelist: BTreeSet<String>,
    whitelist_enabled: bool,
    /// Directory the lists were loaded from; `None` keeps everything in memory
    /// (tests and embedded servers never touch the disk).
    dir: Option<PathBuf>,
}

impl AccessData {
    fn remove_expired(&mut self, now: i64) {
        self.name_bans.retain(|_, entry| !entry.has_expired_at(now));
        self.ip_bans.retain(|_, entry| !entry.has_expired_at(now));
    }
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
        self.ban_name_with(name, reason, "(Unknown)", None);
    }

    /// `BanList::addBan`: entry with an explicit source and expiration.
    pub fn ban_name_with(&self, name: &str, reason: &str, source: &str, expires: Option<i64>) {
        let mut entry = BanEntry::with_reason(name, reason);
        entry.source = source.to_string();
        entry.expires = expires;
        let mut data = self.inner.write();
        data.name_bans.insert(entry.name.clone(), entry);
        data.persist_names();
    }

    pub fn pardon_name(&self, name: &str) {
        let mut data = self.inner.write();
        data.name_bans.remove(&name.to_lowercase());
        data.persist_names();
    }

    pub fn ban_ip(&self, ip: &str, reason: &str) {
        self.ban_ip_with(ip, reason, "(Unknown)", None);
    }

    pub fn ban_ip_with(&self, ip: &str, reason: &str, source: &str, expires: Option<i64>) {
        let mut entry = BanEntry::with_reason(ip, reason);
        entry.source = source.to_string();
        entry.expires = expires;
        let mut data = self.inner.write();
        data.ip_bans.insert(entry.name.clone(), entry);
        data.persist_ips();
    }

    pub fn pardon_ip(&self, ip: &str) {
        let mut data = self.inner.write();
        data.ip_bans.remove(&ip.to_lowercase());
        data.persist_ips();
    }

    pub fn is_ip_banned(&self, ip: &str) -> bool {
        let mut data = self.inner.write();
        data.remove_expired(now_unix());
        data.ip_bans.contains_key(&ip.to_lowercase())
    }

    /// Reason of a name ban, `Some("")` when banned without reason.
    pub fn name_ban_reason(&self, name: &str) -> Option<String> {
        let mut data = self.inner.write();
        data.remove_expired(now_unix());
        data.name_bans
            .get(&name.to_lowercase())
            .map(|entry| entry.reason.clone())
    }

    pub fn ip_ban_reason(&self, ip: &str) -> Option<String> {
        let mut data = self.inner.write();
        data.remove_expired(now_unix());
        data.ip_bans
            .get(&ip.to_lowercase())
            .map(|entry| entry.reason.clone())
    }

    /// Full entry for a name ban (`BanList::getEntry`).
    pub fn name_ban_entry(&self, name: &str) -> Option<BanEntry> {
        let mut data = self.inner.write();
        data.remove_expired(now_unix());
        data.name_bans.get(&name.to_lowercase()).cloned()
    }

    pub fn ip_ban_entry(&self, ip: &str) -> Option<BanEntry> {
        let mut data = self.inner.write();
        data.remove_expired(now_unix());
        data.ip_bans.get(&ip.to_lowercase()).cloned()
    }

    pub fn banned_names(&self) -> Vec<String> {
        let mut data = self.inner.write();
        data.remove_expired(now_unix());
        data.name_bans.keys().cloned().collect()
    }

    pub fn banned_ips(&self) -> Vec<String> {
        let mut data = self.inner.write();
        data.remove_expired(now_unix());
        data.ip_bans.keys().cloned().collect()
    }

    /// All entries, for PHP `BanList::getEntries()` parity.
    pub fn name_ban_entries(&self) -> Vec<BanEntry> {
        let mut data = self.inner.write();
        data.remove_expired(now_unix());
        data.name_bans.values().cloned().collect()
    }

    pub fn ip_ban_entries(&self) -> Vec<BanEntry> {
        let mut data = self.inner.write();
        data.remove_expired(now_unix());
        data.ip_bans.values().cloned().collect()
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
        let mut data = self.inner.write();
        data.remove_expired(now_unix());
        if let Some(entry) = data.name_bans.get(&name.to_lowercase()) {
            return Some(if entry.reason.is_empty() {
                lang::DISCONNECT_BAN_NOREASON.to_string()
            } else {
                lang::DISCONNECT_BAN.replace("{%reason}", &entry.reason)
            });
        }
        if let Some(entry) = data.ip_bans.get(&ip.to_lowercase()) {
            let reason = if entry.reason.is_empty() {
                lang::DISCONNECT_BAN_IP.to_string()
            } else {
                entry.reason.clone()
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

/// Parses `BanEntry` lines, ignoring blanks and `#` comments.
fn read_bans(path: &Path) -> BTreeMap<String, BanEntry> {
    let mut bans = BTreeMap::new();
    let Ok(contents) = std::fs::read_to_string(path) else {
        return bans;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(entry) = BanEntry::from_string(line) {
            bans.insert(entry.name.clone(), entry);
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
            .values()
            .map(|entry| format!("{}\n", entry.get_string()))
            .collect();
        let _ = std::fs::write(dir.join("banned-players.txt"), contents);
    }

    fn persist_ips(&self) {
        let Some(dir) = &self.dir else {
            return;
        };
        let contents: String = self
            .ip_bans
            .values()
            .map(|entry| format!("{}\n", entry.get_string()))
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
            Some("You are banned from this server. Reason: griefing".to_string())
        );

        access.ban_ip("10.0.0.2", "");
        assert_eq!(
            access.denial("Alex", "10.0.0.2", false),
            Some("You are banned from this server. Reason: IP banned".to_string())
        );

        access.set_whitelist_enabled(true);
        assert_eq!(
            access.denial("Alex", "10.0.0.3", false),
            Some("Server is whitelisted".to_string())
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
    fn empty_reason_means_no_reason() {
        let access = AccessList::new();
        access.ban_name("Steve", "");
        assert_eq!(
            access.denial("Steve", "10.0.0.1", false),
            Some("You are banned from this server".to_string())
        );
    }

    #[test]
    fn detects_ip_literals_like_inet_pton() {
        assert!(is_ip_address("192.168.0.1"));
        assert!(is_ip_address("::1"));
        assert!(!is_ip_address("Steve"));
        assert!(!is_ip_address("999.1.1.1"));
    }

    #[test]
    fn ban_entry_round_trips_and_reads_legacy_lines() {
        let entry = BanEntry::from_string("steve|2024-01-02 03:04:05|CONSOLE|Forever|griefing")
            .expect("parse PM line");
        assert_eq!(entry.name, "steve");
        assert_eq!(entry.source, "CONSOLE");
        assert_eq!(entry.expires, None);
        assert_eq!(entry.reason, "griefing");
        assert_eq!(entry.get_string(), "steve|2024-01-02 03:04:05|CONSOLE|Forever|griefing");

        let legacy = BanEntry::from_string("alex|griefing").expect("legacy reason");
        assert_eq!(legacy.reason, "griefing");
        let bare = BanEntry::from_string("creeper").expect("bare name");
        assert_eq!(bare.name, "creeper");
    }

    #[test]
    fn expired_bans_disappear_without_pardon() {
        let access = AccessList::new();
        access.ban_name_with("Steve", "temp", "CONSOLE", Some(now_unix() - 10));
        assert!(access.denial("Steve", "10.0.0.1", false).is_none());
        assert!(access.name_ban_entry("Steve").is_none());
    }
}
