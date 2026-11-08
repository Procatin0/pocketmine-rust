//! Update checking modelled on `updater/UpdateChecker.php`.
//!
//! PM constructs `UpdateChecker` in `Server::__construct` (after worlds,
//! before network) and submits an async task to `update.axolotl-pm.org`.
//! The only console output is:
//!
//! * `DEBUG "Async update check failed due to ..."` on network failure
//!   (`checkUpdateError`),
//! * `DEBUG` for invalid/older versions (`checkUpdate`),
//! * `WARNING` x3 when a newer version exists (`showConsoleUpdate`),
//! * `INFO` x2 for channel suggestions (`showChannelSuggestionStable/Beta`).
//!
//! The Rust port has no update client, so **all network activity is
//! suppressed by design**: [`UpdateChecker`] never spawns a task, never
//! logs at `INFO`/`WARNING`, and only emits the same `DEBUG` lines PM
//! would emit when the check is disabled/failed. Setting
//! `auto-updater.enabled: false` in `pocketmine.yml` (or simply not
//! implementing the HTTP client) therefore matches PM's observable behaviour
//! for the empty-update case: silence.

use tracing::debug;

/// Stub of `updater/UpdateChecker.php`.
pub struct UpdateChecker {
    enabled: bool,
    channel: String,
}

impl UpdateChecker {
    /// `new UpdateChecker($server, $host)`: the check only runs when
    /// `auto-updater.enabled` (default `true`); otherwise PM is silent.
    pub fn new(enabled: bool, channel: &str) -> Self {
        Self {
            enabled,
            channel: channel.to_string(),
        }
    }

    /// `doCheck()`: PM submits `UpdateCheckTask` to the async pool. The Rust
    /// port suppresses the HTTP request entirely; this only records the
    /// debug line PM would log for a disabled checker.
    pub fn do_check(&self) {
        if !self.enabled {
            debug!("Async update check failed due to \"auto-updater disabled\"");
        } else {
            debug!("Update check suppressed (no updater client in Rust port)");
        }
    }

    /// `checkUpdateError($error)`.
    pub fn check_update_error(&self, error: &str) {
        debug!("Async update check failed due to \"{error}\"");
    }

    pub fn channel(&self) -> &str {
        &self.channel
    }

    /// There is never a known update without an HTTP client.
    pub fn has_update(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_checker_never_reports_an_update() {
        let checker = UpdateChecker::new(false, "stable");
        checker.do_check();
        checker.check_update_error("offline");
        assert!(!checker.has_update());
        assert_eq!(checker.channel(), "stable");
    }
}
