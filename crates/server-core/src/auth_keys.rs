//! Xbox authentication key handling modelled on
//! `network/mcpe/auth/AuthKeyProvider.php`.
//!
//! PM's `AuthKeyProvider` (logger prefix `Minecraft Auth Key Provider`)
//! lazily fetches Mojang's auth keys from the async pool, logging:
//!
//! * `NOTICE "Fetching new authentication keys"` (`fetchKeys`),
//! * `DEBUG "Key refresh was requested, but it's already in progress"`,
//! * `DEBUG "Key {id} not recognised!"` / `DEBUG "Key {id} found in keychain"`
//!   (`resolveKey`),
//! * `ERROR "The following errors occurred while fetching new keys: ..."` +
//!   `CRITICAL "Failed to fetch authentication keys from Mojang's API. Xbox
//!   players may not be able to authenticate!"` on fetch failure,
//! * `INFO "Successfully fetched N new authentication keys from issuer ..."`
//!   on success.
//!
//! The Rust port parses the Xbox chain **locally** in
//! [`crate::identity::parse_login_identity`] and never fetches Mojang keys
//! over the network, so **all of the above log lines are suppressed by
//! design**. This module exists to document that suppression and to give the
//! `Minecraft Auth Key Provider` prefix a home if the fetch is ever
//! implemented. [`AuthKeyProvider::prefixed_log_lines`] is deliberately
//! empty.

/// Logger prefix PM uses for this component.
pub const LOG_PREFIX: &str = "Minecraft Auth Key Provider";

/// Refresh interval `AuthKeyProvider::ALLOWED_REFRESH_INTERVAL` (30 min).
pub const ALLOWED_REFRESH_INTERVAL_SECS: i64 = 30 * 60;

/// Identifies a cached auth key (`AuthKeyring::getKey` equivalent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedKey {
    pub key_id: String,
    pub issuer: String,
}

/// Stub of `auth/AuthKeyProvider.php`: same API shape, no network, no logs.
#[derive(Debug, Default)]
pub struct AuthKeyProvider {
    keys: std::collections::HashMap<String, CachedKey>,
}

impl AuthKeyProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getKey($keyId)`: `None` always (no fetch); callers fall back to
    /// local `identity.rs` parsing. Emits no log line (PM would log
    /// `DEBUG`/`NOTICE`, suppressed here).
    pub fn get_key(&self, key_id: &str) -> Option<&CachedKey> {
        self.keys.get(key_id)
    }

    /// `fetchKeys()`: suppressed; always no-op.
    pub fn fetch_keys(&self) {}

    /// `onKeysFetched()`: suppressed; always no-op.
    pub fn on_keys_fetched(&self) {}

    /// Documents the suppressed lines: PM would emit the `PrefixedLogger`
    /// lines listed in the module docs. Returns an empty slice so callers can
    /// iterate without branching.
    pub fn prefixed_log_lines() -> &'static [&'static str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_is_suppressed_and_silent() {
        let provider = AuthKeyProvider::new();
        provider.fetch_keys();
        provider.on_keys_fetched();
        assert_eq!(provider.get_key("missing"), None);
        assert!(AuthKeyProvider::prefixed_log_lines().is_empty());
        assert_eq!(LOG_PREFIX, "Minecraft Auth Key Provider");
    }
}
