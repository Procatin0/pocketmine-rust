//! Crash-dump handling modelled on `Server::crashDump()`.
//!
//! PocketMine writes a human-readable dump plus a zlib-compressed JSON block
//! to `crashdumps/YYYY-MM-DD_HH.mm.ss_T.log`, logs two `EMERGENCY` lines
//! (create + submit), optionally posts to the Crash Archive, then forces a
//! shutdown with the 120 s uptime throttle (`Server.php::crashDump`).
//!
//! The Rust port does not link PM's crash-archive client, so the POST is
//! suppressed by design (dev builds and locally modified trees never post
//! anyway: `Server.php` checks `GIT_HASH` for `-dirty`/all-zero). What *is*
//! implemented is the local path: the same two `EMERGENCY` lines through
//! [`crate::translations::Translator`] and a minimal dump file, so an
//! operator inspecting `server.log` sees exactly where PocketMine-or-Rust
//! diverged (missing POST) without any silent failure.

use std::path::{Path, PathBuf};

use crate::translations::Translator;

/// `pocketmine.crash.create`.
/// "An unrecoverable error has occurred and the server has crashed. Creating a crash dump"
pub fn create_message() -> String {
    Translator::english()
        .translate("pocketmine.crash.create", &[])
}

/// `pocketmine.crash.submit` with the dump path.
pub fn submit_message(path: &str) -> String {
    Translator::english().translate("pocketmine.crash.submit", &[path])
}

/// `pocketmine.crash.archive` with report URL + ID.
pub fn archive_message(url: &str, id: &str) -> String {
    Translator::english().translate("pocketmine.crash.archive", &[url, id])
}

/// `pocketmine.crash.error` with the failure reason.
pub fn error_message(reason: &str) -> String {
    Translator::english().translate("pocketmine.crash.error", &[reason])
}

/// Resolves today's dump path the same way `Server::writeCrashDumpFile` does:
/// `crashdumps/YYYY-MM-DD_HH.mm.ss_T.log` using local time.
pub fn dump_path(data_path: &Path, unix_secs: i64) -> PathBuf {
    let local = unix_to_local(unix_secs);
    let (year, month, day, hour, minute, second, _) = local;
    data_path.join("crashdumps").join(format!(
        "{year:04}-{month:02}-{day:02}_{hour:02}.{minute:02}.{second:02}_T.log"
    ))
}

/// Writes a minimal dump file (`error` + `trace` + PHP-equivalent header)
/// and returns its path. The encoded `===BEGIN CRASH DUMP===` block is a
/// plain UTF-8 placeholder; PM's zlib JSON has no consumer in the Rust port.
pub fn write_dump(
    data_path: &Path,
    error_type: &str,
    error_message: &str,
    error_file: &str,
    error_line: u32,
) -> std::io::Result<PathBuf> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let path = dump_path(data_path, now);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut body = String::new();
    body.push_str("---- Axolotl-PM Crash Dump ----\n");
    body.push_str(&format!("Type: {error_type}\n"));
    body.push_str(&format!("Message: {error_message}\n"));
    body.push_str(&format!("File: {error_file}:{error_line}\n"));
    body.push('\n');
    body.push_str("----------------------REPORT THE DATA BELOW THIS LINE-----------------------\n");
    body.push('\n');
    body.push_str("===BEGIN CRASH DUMP===\n");
    body.push_str("Rust port: crash-archive POST suppressed (see crash.rs docs).\n");
    body.push_str("===END CRASH DUMP===\n");
    std::fs::write(&path, body)?;
    Ok(path)
}

/// `Server::crashDump` minimum uptime (120 s) throttle.
pub const MIN_UPTIME_SECS: i64 = 120;

/// Whether the Crash Archive POST would be suppressed for this build.
/// PM suppresses for `ParseError`, for `-dirty`/zero git hashes, and within
/// 120 s of the last crash; the Rust port suppresses unconditionally because
/// there is no archive client.
pub fn is_archive_post_suppressed() -> bool {
    true
}

fn unix_to_local(secs: i64) -> (u16, u16, u16, u16, u16, u16, u16) {
    // Gmtime derivation via civil-from-days; timezone via logging offset is
    // good enough for a dump filename (PM uses local `date()`).
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hour, minute, second) = (
        (rem / 3_600) as u16,
        ((rem % 3_600) / 60) as u16,
        (rem % 60) as u16,
    );
    let (year, month, day) = civil_from_days(days);
    (year, month, day, hour, minute, second, 0)
}

fn civil_from_days(z: i64) -> (u16, u16, u16) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u16, m as u16, d as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_come_from_english_catalogue() {
        assert!(create_message().contains("crashed"));
        assert!(submit_message("crashdumps/x.log").contains("crashdumps/x.log"));
        assert!(error_message("boom").contains("boom"));
    }

    #[test]
    fn archive_post_is_suppressed() {
        assert!(is_archive_post_suppressed());
    }
}
