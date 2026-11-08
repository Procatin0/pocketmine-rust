//! Embedded PHP runtime files.
//!
//! The PHP runtime is compiled into the bridge as a set of `include_str!`
//! constants. At startup the files are written to a temporary directory and
//! `bootstrap.php` is executed by the worker process.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Every PHP runtime file, in load order.
///
/// Paths are flat because the runtime directory contains all files side by
/// side; `bootstrap.php` is responsible for requiring them in this order.
pub const PHP_RUNTIME_FILES: &[(&str, &str)] = &[
    ("math.php", include_str!("../../php/math.php")),
    ("text.php", include_str!("../../php/text.php")),
    ("permission.php", include_str!("../../php/permission.php")),
    ("config.php", include_str!("../../php/config.php")),
    ("scheduler.php", include_str!("../../php/scheduler.php")),
    ("command.php", include_str!("../../php/command.php")),
    ("item.php", include_str!("../../php/item.php")),
    ("world.php", include_str!("../../php/world.php")),
    ("player.php", include_str!("../../php/player.php")),
    ("chat.php", include_str!("../../php/chat.php")),
    ("core.php", include_str!("../../php/core.php")),
    ("event.php", include_str!("../../php/event.php")),
    ("plugin.php", include_str!("../../php/plugin.php")),
    ("autoload.php", include_str!("../../php/autoload.php")),
    ("ipc.php", include_str!("../../php/ipc.php")),
    ("bootstrap.php", include_str!("../../php/bootstrap.php")),
];

/// Default location used by the worker: `<temp>/pocketmine-rust-php`.
pub fn default_runtime_dir() -> PathBuf {
    std::env::temp_dir().join("pocketmine-rust-php")
}

/// Write every embedded PHP file into `dir` and return the path to
/// `bootstrap.php`.
///
/// On Windows an existing runtime may still be mapped by another process (for
/// example a previous server instance), in which case rewriting the files
/// fails with a sharing violation. When that happens we transparently retry in
/// a per-process directory (`<dir>-<pid>`) so a fresh worker can still start.
pub fn write_runtime(dir: &Path) -> io::Result<PathBuf> {
    match write_runtime_at(dir) {
        Ok(path) => Ok(path),
        Err(first_error) => {
            let fallback = fallback_runtime_dir(dir);
            match write_runtime_at(&fallback) {
                Ok(path) => {
                    tracing::warn!(
                        "Could not refresh PHP runtime in {} ({}); using {} instead",
                        dir.display(),
                        first_error,
                        fallback.display()
                    );
                    Ok(path)
                }
                Err(second_error) => Err(io::Error::new(
                    second_error.kind(),
                    format!(
                        "failed to write PHP runtime to {} ({}); fallback {} also failed ({})",
                        dir.display(),
                        first_error,
                        fallback.display(),
                        second_error
                    ),
                )),
            }
        }
    }
}

fn write_runtime_at(dir: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;

    for (name, contents) in PHP_RUNTIME_FILES {
        fs::write(dir.join(name), contents.as_bytes())?;
    }

    Ok(dir.join("bootstrap.php"))
}

fn fallback_runtime_dir(dir: &Path) -> PathBuf {
    let name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("pocketmine-rust-php");
    dir.with_file_name(format!("{}-{}", name, std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_all_runtime_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "pmrust-bootstrap-test-{}-{}",
            std::process::id(),
            unique
        ));

        let bootstrap = write_runtime(&dir).expect("runtime written");
        assert_eq!(bootstrap, dir.join("bootstrap.php"));
        assert!(bootstrap.exists());
        for (name, _) in PHP_RUNTIME_FILES {
            assert!(dir.join(name).exists(), "missing {}", name);
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
