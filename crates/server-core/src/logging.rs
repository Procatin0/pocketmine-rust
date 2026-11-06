//! Console log formatting modelled on PocketMine-MP's console output:
//!
//! ```text
//! [18:26:43.210] [Server thread/INFO]: Loading resource packs...
//! ```
//!
//! Logs always come out on stdout. ANSI colouring depends on whether stdout is
//! a terminal, so redirecting the console to a file produces plain text.

use std::borrow::Cow;
use std::fmt;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

const RESET: &str = "\x1b[0m";
const GRAY: &str = "\x1b[90m";

/// Whether ANSI escapes are enabled for stdout (and for console messages).
static ANSI: AtomicBool = AtomicBool::new(false);

/// Installs the global subscriber using the PocketMine console layout.
///
/// The log level can be controlled with the `POCKETMINE_LOG_LEVEL` environment
/// variable. Accepted values: `trace`, `debug`, `info` (default), `warn`,
/// `error`.
pub fn init() {
    let ansi = std::io::stdout().is_terminal();
    ANSI.store(ansi, Ordering::Relaxed);

    // The PHP worker formats its own console lines; it inherits this offset so
    // plugin output carries the same timestamp as server output.
    std::env::set_var(
        "PM_CONSOLE_UTC_OFFSET_SECS",
        local_utc_offset_secs().to_string(),
    );

    let max_level = match std::env::var("POCKETMINE_LOG_LEVEL")
        .as_deref()
        .unwrap_or("info")
    {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    tracing_subscriber::fmt()
        .event_format(ConsoleFormat {
            timer: Clock,
            ansi,
        })
        .with_max_level(max_level)
        .init();
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
/// is a terminal, otherwise they are stripped.
pub fn console_message(text: &str) {
    let ansi = ANSI.load(Ordering::Relaxed);
    let (hour, minute, second, millis) = local_time();
    let prefix = if ansi {
        format!("\x1b[90m[{hour:02}:{minute:02}:{second:02}.{millis:03}] [Server thread/INFO]:\x1b[0m ")
    } else {
        format!("[{hour:02}:{minute:02}:{second:02}.{millis:03}] [Server thread/INFO]: ")
    };
    let body = to_ansi(text);
    println!("{prefix}{body}");
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

impl<S, N> FormatEvent<S, N> for ConsoleFormat
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let level = *event.metadata().level();
        let (gray, reset) = if self.ansi { (GRAY, RESET) } else { ("", "") };

        write!(writer, "{gray}[")?;
        self.timer.format_time(&mut writer)?;
        write!(writer, "] [{}", thread_label())?;

        let color = if self.ansi { level_color(level) } else { "" };
        write!(writer, "/{color}{level}{gray}]:{reset} ")?;

        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

/// Everything runs on tokio workers once the runtime is up, so all of them are
/// reported as the single "Server thread" PocketMine uses.
fn thread_label() -> Cow<'static, str> {
    match std::thread::current().name() {
        Some(name) if name != "main" && !name.starts_with("tokio") => Cow::Owned(name.to_string()),
        _ => Cow::Borrowed("Server thread"),
    }
}

fn level_color(level: Level) -> &'static str {
    match level {
        Level::ERROR => "\x1b[91m",
        Level::WARN => "\x1b[93m",
        Level::INFO => "\x1b[92m",
        Level::DEBUG => "\x1b[96m",
        Level::TRACE => "\x1b[90m",
    }
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
}
