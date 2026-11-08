//! Console and file logging modelled on Axolotl-PM's `MainLogger`.
//!
//! Format (console and file):
//!
//! ```text
//! [18:26:43.210] [Server thread/INFO]: Loading resource packs...
//! ```
//!
//! PocketMine/Axolotl `MainLogger::send()` builds the console line as
//! `AQUA + "[%s] " + RESET + "%s[%s/%s]: %s"` (see
//! `target/axolotl-pm/src/utils/MainLogger.php`) and writes the same
//! string, stripped of colour codes, to `server.log` prefixed with
//! `Y-m-d `. This module reproduces both sides:
//!
//! * 8 levels: `EMERGENCY`/`ALERT`/`CRITICAL`/`ERROR`/`WARNING`/`NOTICE`/
//!   `INFO`/`DEBUG` with PM colours (`§c` red, `§4` dark red, `§e` yellow,
//!   `§b` aqua, `§f` white, `§8` gray).
//! * `[HH:MM:SS.vvv] [Server thread/PREFIX]:` — the main thread is always
//!   `Server thread`; named worker threads render as `Name thread`, matching
//!   PM's `getThreadName() . " thread"` rule.
//! * `server.log` in the current directory, `YYYY-MM-DD` prefix, no ANSI.
//!   Files >= 32 MiB are archived to `log_archive/` with the header line
//!   `--- Starting new log file - old log file archived as {name} ---`,
//!   mirroring `MainLoggerThread`.
//!
//! Standard `tracing::{info,warn,error,debug}!` keep working and map to
//! `INFO`/`WARNING`/`ERROR`/`DEBUG`. The extra PM levels are emitted with
//! the `emergency!`/`alert!`/`critical!`/`warning!`/`notice!`/`error_pm!`
//! macros from this module, which attach a `pm_level` field consumed by the
//! formatter. `DEBUG` output is gated by [`set_log_debug`] (PM's
//! `setLogDebug(debug.level > 1)`); the initial value comes from
//! `POCKETMINE_LOG_LEVEL`.

use std::borrow::Cow;
use std::fmt;
use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

const RESET: &str = "\x1b[0m";
const AQUA: &str = "\x1b[36m";
const RED: &str = "\x1b[91m";
const DARK_RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[93m";
const WHITE: &str = "\x1b[97m";
const GRAY: &str = "\x1b[90m";

/// `MainLoggerThread::$maxFileSize`.
const MAX_LOG_FILE_SIZE: u64 = 32 * 1024 * 1024;

/// Whether ANSI escapes are enabled for stdout (and for console messages).
static ANSI: AtomicBool = AtomicBool::new(false);

/// PocketMine `MainLogger::$logDebug`. When false, `DEBUG` events are dropped.
static LOG_DEBUG: AtomicBool = AtomicBool::new(false);

/// `MainLogger` thread name argument (`"Server"` -> `"Server thread"`).
const MAIN_THREAD_NAME: &str = "Server";

/// Initialised `server.log` writer (None when logging to file is disabled).
static LOG_FILE: std::sync::OnceLock<Mutex<Option<LogFileWriter>>> =
    std::sync::OnceLock::new();

struct LogFileWriter {
    file: std::fs::File,
    size: u64,
    path: std::path::PathBuf,
    archive_dir: Option<std::path::PathBuf>,
}

impl LogFileWriter {
    /// Appends one already-formatted line; archives first when the 32 MiB
    /// limit would be exceeded, mirroring `MainLoggerThread::archiveLogFile`.
    fn write_line(&mut self, line: &str) {
        let bytes = line.as_bytes().len() as u64;
        if self.size + bytes >= MAX_LOG_FILE_SIZE {
            if let Some(dir) = self.archive_dir.clone() {
                let _ = self.archive(dir);
            }
        }
        let _ = self.file.write_all(line.as_bytes());
        let _ = self.file.flush();
        self.size += bytes;
    }

    fn archive(&mut self, archive_dir: std::path::PathBuf) {
        let _ = self.file.flush();
        let _ = self.file.sync_all();
        let (year, month, day, hour, minute, second, _) = local_datetime();
        let date = format!(
            "{year:04}-{month:02}-{day:02}T{hour:02}.{minute:02}.{second:02}"
        );
        let base = self
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "server".to_string());
        let ext = self
            .path
            .extension()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut i = 0u32;
        let file_name = loop {
            let name = if ext.is_empty() {
                format!("{base}.{date}.{i}")
            } else {
                format!("{base}.{date}.{i}.{ext}")
            };
            if !archive_dir.join(&name).exists() {
                break name;
            }
            i += 1;
        };
        let _ = std::fs::create_dir_all(&archive_dir);
        let _ = std::fs::rename(&self.path, archive_dir.join(&file_name));
        // Re-create `server.log` fresh, with the PM archive header line.
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)
        {
            self.file = file;
            self.size = 0;
            let header = format!(
                "--- Starting new log file - old log file archived as {file_name} ---\n"
            );
            let _ = self.file.write_all(header.as_bytes());
            self.size = header.len() as u64;
        }
    }
}

/// Installs the global subscriber using the PocketMine console layout.
///
/// The log level can be controlled with the `POCKETMINE_LOG_LEVEL` environment
/// variable. Accepted values: `trace`, `debug`, `info` (default), `warn`,
/// `error`. Anything below `debug` starts with `LOG_DEBUG = false`; the
/// caller is expected to call [`set_log_debug`] once `pocketmine.yml`'s
/// `debug.level` is known, exactly like `Server.php` does.
///
/// Set `POCKETMINE_NO_LOG_FILE=1` to skip the `server.log` writer (PM's
/// `--no-log-file` flag). `POCKETMINE_LOG_FILE` overrides the file path.
pub fn init() {
    let ansi = std::io::stdout().is_terminal();
    ANSI.store(ansi, Ordering::Relaxed);

    // The PHP worker formats its own console lines; it inherits this offset so
    // plugin output carries the same timestamp as server output.
    std::env::set_var(
        "PM_CONSOLE_UTC_OFFSET_SECS",
        local_utc_offset_secs().to_string(),
    );

    let env_level = std::env::var("POCKETMINE_LOG_LEVEL")
        .as_deref()
        .unwrap_or("info")
        .to_ascii_lowercase();
    LOG_DEBUG.store(env_level == "debug" || env_level == "trace", Ordering::Relaxed);
    let max_level = match env_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    // The formatter additionally gates DEBUG on LOG_DEBUG so the config's
    // `debug.level` can enable it after startup. Never let the subscriber
    // discard DEBUG/TRACE before the formatter sees them when the env asked
    // for them; otherwise a `set_log_debug(true)` later would stay invisible.
    let subscriber_max = if max_level <= Level::DEBUG {
        Level::DEBUG
    } else {
        max_level
    };

    init_log_file();

    tracing_subscriber::fmt()
        .event_format(ConsoleFormat {
            timer: Clock,
            ansi,
        })
        .with_max_level(subscriber_max)
        .init();
}

/// PocketMine `MainLogger::setLogDebug()`: `debug.level > 1` enables DEBUG.
pub fn set_log_debug(enabled: bool) {
    LOG_DEBUG.store(enabled, Ordering::Relaxed);
}

/// Whether DEBUG output is currently enabled.
pub fn is_log_debug() -> bool {
    LOG_DEBUG.load(Ordering::Relaxed)
}

/// Flushes `server.log` (PM's `syncFlushBuffer()`).
pub fn flush_log_file() {
    if let Some(cell) = LOG_FILE.get() {
        if let Ok(mut guard) = cell.lock() {
            if let Some(writer) = guard.as_mut() {
                let _ = writer.file.flush();
            }
        }
    }
}

/// PocketMine `MainLogger::shutdownLogWriterThread()`: flush and release.
pub fn shutdown_log_writer() {
    flush_log_file();
}

fn init_log_file() {
    let no_log = std::env::var("POCKETMINE_NO_LOG_FILE").as_deref() == Ok("1");
    if no_log {
        let _ = LOG_FILE.set(Mutex::new(None));
        return;
    }
    let path = std::env::var("POCKETMINE_LOG_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("server.log"));
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    let archive_dir = std::path::PathBuf::from("log_archive");
    let _ = std::fs::create_dir_all(&archive_dir);
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(file) => {
            let size = file.metadata().map(|m| m.len()).unwrap_or(0);
            let writer = LogFileWriter {
                file,
                size,
                path,
                archive_dir: Some(archive_dir),
            };
            let _ = LOG_FILE.set(Mutex::new(Some(writer)));
        }
        Err(_) => {
            let _ = LOG_FILE.set(Mutex::new(None));
        }
    }
}

/// Writes one line to `server.log` (already `YYYY-MM-DD [..] [..]: msg\n`,
/// stripped). File writes never affect console output.
fn write_log_file_line(line: String) {
    if let Some(cell) = LOG_FILE.get() {
        if let Ok(mut guard) = cell.lock() {
            if let Some(writer) = guard.as_mut() {
                writer.write_line(&line);
            }
        }
    }
}

/// PocketMine `MainLogger` file date (`Y-m-d`).
fn file_date_prefix() -> String {
    let (year, month, day, ..) = local_datetime();
    format!("{year:04}-{month:02}-{day:02} ")
}

/// Prints a message to the server console using PocketMine's console format:
///
/// ```text
/// [18:26:43.123] [Server thread/INFO]: mensaje
/// ```
///
/// Unlike `tracing::info!`, this does not add a tracing event — it is a plain
/// console line, exactly as PocketMine-MP writes command feedback to its
/// console. Colour codes (`§a`, `§c`, ...) are converted to ANSI when stdout
/// is a terminal, otherwise they are stripped. The same stripped line is
/// appended to `server.log`.
pub fn console_message(text: &str) {
    let ansi = ANSI.load(Ordering::Relaxed);
    let (hour, minute, second, millis) = local_time();
    let prefix = if ansi {
        format!(
            "{AQUA}[{hour:02}:{minute:02}:{second:02}.{millis:03}]{RESET} [{MAIN_THREAD_NAME} thread/INFO]: "
        )
    } else {
        format!(
            "[{hour:02}:{minute:02}:{second:02}.{millis:03}] [{MAIN_THREAD_NAME} thread/INFO]: "
        )
    };
    let body = to_ansi(text);
    // `MainLogger` terminates every console line with `RESET`: a message
    // that opens a colour (`§c...` from `red()`, `§e...`, ...) and never
    // closes it must not tint the next line, which may not even start with
    // the AQUA prefix (PHP `Logger` writes to the same terminal).
    let reset = if ansi { RESET } else { "" };
    println!("{prefix}{body}{reset}");
    let _ = std::io::stdout().flush();

    let plain = to_plain(text);
    let file_line = format!(
        "{}[{hour:02}:{minute:02}:{second:02}.{millis:03}] [{MAIN_THREAD_NAME} thread/INFO]: {plain}\n",
        file_date_prefix(),
    );
    write_log_file_line(file_line);
}

/// Strips every `§` colour code (PM `TextFormat::clean`); used for files.
fn to_plain(text: &str) -> String {
    if !text.contains('\u{a7}') && !text.contains('\x1b') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{a7}' {
            let _ = chars.next();
            continue;
        }
        if ch == '\x1b' {
            // Skip ESC [ ... m
            let esc = chars.next();
            if esc == Some('[') {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

/// Converts Bedrock colour codes (`§a`, `§l`, ...) to ANSI escapes, or strips
/// them when the console cannot render ANSI. Consoles never understand `§`.
pub fn to_ansi(text: &str) -> String {
    if !text.contains('\u{a7}') {
        return text.to_string();
    }

    let ansi = ANSI.load(Ordering::Relaxed);
    let mut out = String::with_capacity(text.len() + 8);
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\u{a7}' {
            out.push(ch);
            continue;
        }
        let Some(code) = chars.next() else {
            break;
        };
        if !ansi {
            continue;
        }
        match code.to_ascii_lowercase() {
            '0' => out.push_str("\x1b[30m"),
            '1' => out.push_str("\x1b[34m"),
            '2' => out.push_str("\x1b[32m"),
            '3' => out.push_str("\x1b[36m"),
            '4' => out.push_str("\x1b[31m"),
            '5' => out.push_str("\x1b[35m"),
            '6' => out.push_str("\x1b[33m"),
            '7' => out.push_str("\x1b[37m"),
            '8' => out.push_str("\x1b[90m"),
            '9' => out.push_str("\x1b[94m"),
            'a' => out.push_str("\x1b[92m"),
            'b' => out.push_str("\x1b[96m"),
            'c' => out.push_str("\x1b[91m"),
            'd' => out.push_str("\x1b[95m"),
            'e' => out.push_str("\x1b[93m"),
            'f' => out.push_str("\x1b[97m"),
            'l' => out.push_str("\x1b[1m"),
            'm' => out.push_str("\x1b[9m"),
            'n' => out.push_str("\x1b[4m"),
            'o' => out.push_str("\x1b[3m"),
            'r' => out.push_str(RESET),
            // Material colours and obfuscated text have no console equivalent.
            _ => {}
        }
    }
    out
}

/// Local wall clock, so console timestamps match the operator's watch.
struct Clock;

impl FormatTime for Clock {
    fn format_time(&self, writer: &mut Writer<'_>) -> fmt::Result {
        let (hour, minute, second, millis) = local_time();
        write!(writer, "{hour:02}:{minute:02}:{second:02}.{millis:03}")
    }
}

#[cfg(windows)]
fn local_time() -> (u16, u16, u16, u16) {
    #[repr(C)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(system_time: *mut SystemTime);
    }

    let mut time = SystemTime {
        year: 0,
        month: 0,
        day_of_week: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        milliseconds: 0,
    };
    // SAFETY: `GetLocalTime` only writes into the provided SYSTEMTIME.
    unsafe { GetLocalTime(&mut time) };
    (time.hour, time.minute, time.second, time.milliseconds)
}

/// `(year, month, day, hour, minute, second, millis)` in local time.
fn local_datetime() -> (u16, u16, u16, u16, u16, u16, u16) {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct SystemTime {
            year: u16,
            month: u16,
            day_of_week: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            milliseconds: u16,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLocalTime(system_time: *mut SystemTime);
        }
        let mut time = SystemTime {
            year: 0,
            month: 0,
            day_of_week: 0,
            day: 0,
            hour: 0,
            minute: 0,
            second: 0,
            milliseconds: 0,
        };
        // SAFETY: `GetLocalTime` only writes into the provided SYSTEMTIME.
        unsafe { GetLocalTime(&mut time) };
        (
            time.year,
            time.month,
            time.day,
            time.hour,
            time.minute,
            time.second,
            time.milliseconds,
        )
    }
    #[cfg(not(windows))]
    {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let secs = elapsed.as_secs() % 86_400;
        (
            1970,
            1,
            1,
            (secs / 3_600) as u16,
            ((secs % 3_600) / 60) as u16,
            (secs % 60) as u16,
            (elapsed.subsec_millis() % 1_000) as u16,
        )
    }
}

/// Difference between the local wall clock and UTC, in seconds.
fn local_utc_offset_secs() -> i32 {
    let (hour, minute, second, _) = local_time();
    let local = hour as i64 * 3_600 + minute as i64 * 60 + second as i64;
    let utc = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() % 86_400) as i64;

    let mut diff = local - utc;
    if diff < -43_200 {
        diff += 86_400;
    } else if diff > 43_200 {
        diff -= 86_400;
    }
    diff as i32
}

/// Fallback for non-Windows hosts: UTC derived from the system clock.
#[cfg(not(windows))]
fn local_time() -> (u16, u16, u16, u16) {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = elapsed.as_secs() % 86_400;
    (
        (secs / 3_600) as u16,
        ((secs % 3_600) / 60) as u16,
        (secs % 60) as u16,
        (elapsed.subsec_millis() % 1_000) as u16,
    )
}

struct ConsoleFormat {
    timer: Clock,
    ansi: bool,
}

/// Extracts the human message and the optional `pm_level` override.
#[derive(Default)]
struct FieldCollector {
    message: Option<String>,
    pm_level: Option<String>,
}

impl Visit for FieldCollector {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let text = format!("{value:?}");
        if field.name() == "message" {
            if self.message.is_none() {
                let trimmed = strip_debug_quotes(&text);
                self.message = Some(trimmed);
            }
        } else if field.name() == "pm_level" {
            self.pm_level = Some(strip_debug_quotes(&text));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            if self.message.is_none() {
                self.message = Some(value.to_string());
            }
        } else if field.name() == "pm_level" {
            self.pm_level = Some(value.to_string());
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name() == "message" && self.message.is_none() {
            self.message = Some(value.to_string());
        } else if field.name() == "pm_level" {
            self.pm_level = Some(value.to_string());
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if field.name() == "message" && self.message.is_none() {
            self.message = Some(value.to_string());
        } else if field.name() == "pm_level" {
            self.pm_level = Some(value.to_string());
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "message" && self.message.is_none() {
            self.message = Some(value.to_string());
        } else if field.name() == "pm_level" {
            self.pm_level = Some(value.to_string());
        }
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if field.name() == "message" && self.message.is_none() {
            self.message = Some(value.to_string());
        } else if field.name() == "pm_level" {
            self.pm_level = Some(value.to_string());
        }
    }
}

/// `record_debug` renders `&str` with quotes; unquote the common case.
fn strip_debug_quotes(text: &str) -> String {
    let trimmed = text.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"')) && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

/// PocketMine prefix for a tracing level (no `pm_level` override).
fn default_pm_prefix(level: Level) -> &'static str {
    match level {
        Level::ERROR => "ERROR",
        Level::WARN => "WARNING",
        Level::INFO => "INFO",
        Level::DEBUG | Level::TRACE => "DEBUG",
    }
}

/// PocketMine colour for a prefix.
fn prefix_color(prefix: &str) -> &'static str {
    match prefix {
        "EMERGENCY" | "ALERT" | "CRITICAL" => RED,
        "ERROR" => DARK_RED,
        "WARNING" => YELLOW,
        "NOTICE" => AQUA,
        "INFO" => WHITE,
        _ => GRAY,
    }
}

impl<S, N> FormatEvent<S, N> for ConsoleFormat
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let level = *event.metadata().level();
        let mut collector = FieldCollector::default();
        event.record(&mut collector);
        let prefix =
            collector
                .pm_level
                .as_deref()
                .unwrap_or_else(|| default_pm_prefix(level));
        if prefix == "DEBUG" && !is_log_debug() {
            return Ok(());
        }

        let message = collector.message.unwrap_or_default();
        let thread = thread_label();
        let (hour, minute, second, millis) = local_time();

        if self.ansi {
            let color = prefix_color(prefix);
            write!(writer, "{AQUA}[")?;
            self.timer.format_time(&mut writer)?;
            write!(writer, "] {RESET}{color}[{thread}/{prefix}]:")?;
            write!(writer, " {}{RESET}", to_ansi(&message))?;
            writeln!(writer)
        } else {
            // Non-terminal: strip `§` codes like PM's `TextFormat::clean`
            // (consoles never render `§`; files are always stripped).
            let plain_message = to_plain(&message);
            write!(writer, "[")?;
            self.timer.format_time(&mut writer)?;
            write!(writer, "] [{thread}/{prefix}]: {plain_message}\n")
        }
        .and_then(|()| {
            // File copy: `Y-m-d [..] [..]: msg\n`, never ANSI.
            let plain = to_plain(&message);
            let file_line = format!(
                "{}[{hour:02}:{minute:02}:{second:02}.{millis:03}] [{thread}/{prefix}]: {plain}\n",
                file_date_prefix(),
            );
            write_log_file_line(file_line);
            Ok(())
        })
    }
}

/// Mirrors `MainLogger::send()`: the main thread is `"Server thread"`, other
/// `Thread`/`Worker` instances render as `"{name} thread"`. Rust's tokio
/// workers all map to the single `"Server thread"` PocketMine uses.
fn thread_label() -> Cow<'static, str> {
    match std::thread::current().name() {
        Some(name) if name != "main" && !name.starts_with("tokio") => {
            Cow::Owned(format!("{name} thread"))
        }
        _ => Cow::Borrowed("Server thread"),
    }
}

/// Emits an `EMERGENCY` event (`MainLogger::emergency`, `§c` red).
#[macro_export]
macro_rules! emergency {
    ($($arg:tt)*) => {
        tracing::event!(tracing::Level::ERROR, pm_level = "EMERGENCY", $($arg)*)
    };
}

/// Emits an `ALERT` event (`MainLogger::alert`, `§c` red).
#[macro_export]
macro_rules! alert {
    ($($arg:tt)*) => {
        tracing::event!(tracing::Level::ERROR, pm_level = "ALERT", $($arg)*)
    };
}

/// Emits a `CRITICAL` event (`MainLogger::critical`, `§c` red).
#[macro_export]
macro_rules! critical {
    ($($arg:tt)*) => {
        tracing::event!(tracing::Level::ERROR, pm_level = "CRITICAL", $($arg)*)
    };
}

/// Emits an `ERROR` event with the dark-red PM prefix.
#[macro_export]
macro_rules! error_pm {
    ($($arg:tt)*) => {
        tracing::event!(tracing::Level::ERROR, pm_level = "ERROR", $($arg)*)
    };
}

/// Emits a `WARNING` event (yellow). Alias for `tracing::warn!` with the PM
/// prefix so `WARNING` is spelled out in the log.
#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {
        tracing::event!(tracing::Level::WARN, pm_level = "WARNING", $($arg)*)
    };
}

/// Emits a `NOTICE` event (`MainLogger::notice`, `§b` aqua).
#[macro_export]
macro_rules! notice {
    ($($arg:tt)*) => {
        tracing::event!(tracing::Level::INFO, pm_level = "NOTICE", $($arg)*)
    };
}

/// Emits an `INFO` event with the PM prefix spelled out.
#[macro_export]
macro_rules! info_pm {
    ($($arg:tt)*) => {
        tracing::event!(tracing::Level::INFO, pm_level = "INFO", $($arg)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These tests share the global `ANSI` flag, so they run one at a time.
    static ANSI_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_ansi() -> std::sync::MutexGuard<'static, ()> {
        ANSI_TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn strips_color_codes_without_ansi() {
        let _guard = lock_ansi();
        ANSI.store(false, Ordering::Relaxed);
        assert_eq!(to_ansi("\u{a7}b[TestKit] \u{a7}rhola"), "[TestKit] hola");
    }

    #[test]
    fn leaves_plain_text_untouched() {
        let _guard = lock_ansi();
        ANSI.store(false, Ordering::Relaxed);
        assert_eq!(to_ansi("sin colores"), "sin colores");
        assert_eq!(to_ansi("final suelto \u{a7}"), "final suelto ");
    }

    #[test]
    fn emits_ansi_when_enabled() {
        let _guard = lock_ansi();
        ANSI.store(true, Ordering::Relaxed);
        assert_eq!(to_ansi("\u{a7}ahola\u{a7}r"), "\x1b[92mhola\x1b[0m");
        ANSI.store(false, Ordering::Relaxed);
    }

    #[test]
    fn clock_formats_milliseconds() {
        let (hour, minute, second, millis) = local_time();
        assert!(hour < 24 && minute < 60 && second < 60 && millis < 1_000);
    }

    #[test]
    fn pm_prefix_mapping_is_complete() {
        assert_eq!(default_pm_prefix(Level::ERROR), "ERROR");
        assert_eq!(default_pm_prefix(Level::WARN), "WARNING");
        assert_eq!(default_pm_prefix(Level::INFO), "INFO");
        assert_eq!(default_pm_prefix(Level::DEBUG), "DEBUG");
        assert_eq!(prefix_color("EMERGENCY"), RED);
        assert_eq!(prefix_color("ALERT"), RED);
        assert_eq!(prefix_color("CRITICAL"), RED);
        assert_eq!(prefix_color("ERROR"), DARK_RED);
        assert_eq!(prefix_color("WARNING"), YELLOW);
        assert_eq!(prefix_color("NOTICE"), AQUA);
        assert_eq!(prefix_color("INFO"), WHITE);
        assert_eq!(prefix_color("DEBUG"), GRAY);
    }

    #[test]
    fn strips_ansi_and_section_codes_for_files() {
        assert_eq!(to_plain("\x1b[36mhi\x1b[0m"), "hi");
        assert_eq!(to_plain("\u{a7}bhi\u{a7}r"), "hi");
        assert_eq!(to_plain("plain"), "plain");
    }

    #[test]
    fn named_thread_label_appends_thread_suffix() {
        // `MainLogger::send()` renders workers as `"{name} thread"`. Verify
        // the formatter rule by spawning a named thread.
        let handle = std::thread::Builder::new()
            .name("NetherNet".to_string())
            .spawn(thread_label)
            .expect("spawn named thread");
        assert_eq!(handle.join().expect("join"), "NetherNet thread");
    }
}
