//! English translation catalogue from Axolotl-PM's `eng.ini`.
//!
//! The server runs with `language "eng"` (`ServerProperties::LANGUAGE`,
//! default `"eng"`; `pocketmine.yml` may force another locale later, but the
//! embedded fallback is always English). All log lines that carry a
//! `pocketmine.*` translation go through [`Translator::fill`] instead of
//! hard-coded English, exactly like `Server.php` does via
//! `Language::translate(KnownTranslationFactory::...)`.
//!
//! Command messages in [`crate::command::lang`] mirror the same `eng.ini`
//! catalogue, so parity tests compare them against the English locale. The
//! Spanish locale (`spa.ini`, copied from `axolotl-pm`) is kept as reference;
//! main runtime/user-visible strings use English via
//! [`Translator::english`] / [`Translator::embedded`].

use std::collections::BTreeMap;

/// `resources/eng.ini`, copied byte-for-byte from
/// `target/axolotl-pm/resources/translations/eng.ini`. The reference clone
/// lives in `target/` and is not versioned, so the locale is embedded to keep
/// the parity test reproducible.
pub const ENG_INI: &str = include_str!("../resources/eng.ini");

/// `resources/spa.ini`, copied from `axolotl-pm`. Kept for command-message
/// parity tests only; the runtime language is English (`eng`).
pub const SPA_INI: &str = include_str!("../resources/spa.ini");

/// PocketMine's `BaseLang` for a `.ini` locale.
#[derive(Debug, Default)]
pub struct Translator {
    entries: BTreeMap<String, String>,
}

impl Translator {
    /// Parses `key=value` lines, skipping blanks and `;`/`#` comments.
    pub fn parse(text: &str) -> Self {
        let mut entries = BTreeMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            entries.insert(key.trim().to_string(), value.trim().to_string());
        }
        Self { entries }
    }

    /// The embedded English locale (`eng`), the default server language.
    pub fn english() -> &'static Self {
        static TRANSLATOR: std::sync::OnceLock<Translator> = std::sync::OnceLock::new();
        TRANSLATOR.get_or_init(|| Translator::parse(ENG_INI))
    }

    /// The embedded Spanish locale (`spa`), for parity tests.
    pub fn spanish() -> &'static Self {
        static TRANSLATOR: std::sync::OnceLock<Translator> = std::sync::OnceLock::new();
        TRANSLATOR.get_or_init(|| Translator::parse(SPA_INI))
    }

    /// The embedded locale used at runtime: English (`eng`).
    ///
    /// Kept as the historical entry point so existing callers keep working;
    /// new code should prefer [`Translator::english`].
    pub fn embedded() -> &'static Self {
        Self::english()
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// `BaseLang::translate()` with positional `{%0}`, `{%1}`, ... parameters.
    pub fn fill(&self, key: &str, args: &[&str]) -> Option<String> {
        self.get(key).map(|template| fill_template(template, args))
    }

    /// `BaseLang::translate()` with named `{%name}` parameters.
    pub fn fill_named(&self, key: &str, args: &[(&str, &str)]) -> Option<String> {
        self.get(key).map(|template| fill_named(template, args))
    }

    /// Translates like `Language::translate()` with positional args, falling
    /// back to the key itself when missing (PM `get()` returns the key).
    pub fn translate(&self, key: &str, args: &[&str]) -> String {
        self.fill(key, args).unwrap_or_else(|| key.to_string())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Replaces `{%0}`, `{%1}`, ... placeholders in a translation string.
pub fn fill_template(template: &str, args: &[&str]) -> String {
    let mut out = template.to_string();
    for (index, value) in args.iter().enumerate() {
        out = out.replace(&format!("{{%{}}}", index), value);
    }
    out
}

/// Replaces named placeholders such as `{%commandName}`.
pub fn fill_named(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for &(key, value) in args {
        out = out.replace(&format!("{{%{}}}", key), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::lang;

    /// Every message the port hard-codes must still equal the shipped locale.
    /// Grows as more commands move off ad-hoc strings. Compared against the
    /// English catalogue because [`crate::command::lang`] mirrors `eng.ini`.
    #[test]
    fn constants_match_the_shipped_locale() {
        let translator = Translator::english();
        for (key, constant) in [
            ("chat.type.admin", lang::CHAT_ADMIN),
            ("chat.type.announcement", lang::CHAT_ANNOUNCEMENT),
            ("chat.type.emote", lang::CHAT_EMOTE),
            ("commands.generic.usage", lang::USAGE),
            ("commands.help.header", lang::HELP_HEADER),
            ("commands.kick.success", lang::KICK_SUCCESS),
            ("commands.kick.success.reason", lang::KICK_SUCCESS_REASON),
            (
                "commands.message.display.incoming",
                lang::MESSAGE_INCOMING,
            ),
            (
                "commands.message.display.outgoing",
                lang::MESSAGE_OUTGOING,
            ),
            ("commands.message.sameTarget", lang::MESSAGE_SAME_TARGET),
            ("commands.players.list", lang::PLAYERS_LIST),
            ("commands.time.added", lang::TIME_ADDED),
            ("commands.time.query", lang::TIME_QUERY),
            ("commands.time.set", lang::TIME_SET),
            ("commands.title.success", lang::TITLE_SUCCESS),
            ("pocketmine.command.error.permission", lang::PERMISSION),
            (
                "pocketmine.command.error.playerUserOnly",
                lang::PLAYER_USER_ONLY,
            ),
            (
                "pocketmine.command.help.specificCommand.aliases",
                lang::HELP_SPECIFIC_ALIASES,
            ),
            (
                "pocketmine.command.help.specificCommand.description",
                lang::HELP_SPECIFIC_DESCRIPTION,
            ),
            (
                "pocketmine.command.help.specificCommand.header",
                lang::HELP_SPECIFIC_HEADER,
            ),
            (
                "pocketmine.command.help.specificCommand.usage",
                lang::HELP_SPECIFIC_USAGE,
            ),
            ("pocketmine.command.notFound", lang::COMMAND_NOT_FOUND),
            (
                "pocketmine.command.version.minecraftVersion",
                lang::VERSION_MINECRAFT,
            ),
            (
                "pocketmine.command.version.serverSoftwareName",
                lang::VERSION_SOFTWARE_NAME,
            ),
            (
                "pocketmine.command.version.serverSoftwareVersion",
                lang::VERSION_SOFTWARE_VERSION,
            ),
        ] {
            assert_eq!(translator.get(key), Some(constant), "key {key}");
        }
    }

    #[test]
    fn fills_spanish_positional_and_named_placeholders() {
        let translator = Translator::english();
        assert_eq!(
            translator.fill("commands.players.list", &["2", "1000"]).as_deref(),
            Some("There are 2/1000 players online:")
        );
        assert_eq!(
            translator
                .fill_named(
                    "pocketmine.command.notFound",
                    &[("commandName", "nope"), ("helpCommand", "/help")],
                )
                .as_deref(),
            Some("Unknown command: nope. Use /help for a list of available commands.")
        );
    }

    #[test]
    fn english_catalogue_has_server_log_keys() {
        let translator = Translator::english();
        // Keys used by the Server.php parity sequence in main.rs.
        for key in [
            "language.selected",
            "pocketmine.server.start",
            "pocketmine.server.auth.enabled",
            "pocketmine.server.auth.disabled",
            "pocketmine.server.authWarning",
            "pocketmine.server.authProperty.disabled",
            "pocketmine.server.info",
            "pocketmine.server.license",
            "pocketmine.server.defaultGameMode",
            "pocketmine.server.startFinished",
            "pocketmine.server.forcingShutdown",
            "pocketmine.server.networkStart",
            "pocketmine.server.query.running",
            "pocketmine.server.transport.unknown",
            "pocketmine.server.transport.unavailable",
            "pocketmine.server.transport.noneConfigured",
            "pocketmine.server.raknet.deprecated",
            "pocketmine.server.nethernet.builtinSignalingDisabled",
            "pocketmine.server.nethernet.identityKeySaveFailed",
            "pocketmine.server.nethernet.tls.disabled",
            "pocketmine.server.nethernet.tls.enabled",
            "pocketmine.server.nethernet.udpMuxWithoutPortRange",
            "pocketmine.plugin.load",
            "pocketmine.plugin.enable",
            "pocketmine.plugin.disable",
            "pocketmine.plugin.loadError",
            "pocketmine.plugin.enableError",
            "pocketmine.plugin.someLoadErrors",
            "pocketmine.plugin.someEnableErrors",
            "pocketmine.level.preparing",
            "pocketmine.level.unloading",
            "pocketmine.network.session.open",
            "pocketmine.network.session.close",
            "pocketmine.network.session.playerName",
            "pocketmine.server.devBuild.warning1",
            "pocketmine.server.devBuild.warning2",
            "pocketmine.server.devBuild.warning3",
            "pocketmine.server.url.discord",
            "pocketmine.server.url.docs",
            "pocketmine.server.url.sourceCode",
            "pocketmine.server.url.freePlugins",
            "pocketmine.server.url.donations",
            "pocketmine.server.url.translations",
            "pocketmine.server.url.bugReporting",
        ] {
            assert!(translator.get(key).is_some(), "missing key {key}");
        }
        assert_eq!(
            translator
                .fill("pocketmine.server.start", &["1.26.60"])
                .as_deref(),
            Some("Starting Minecraft: Bedrock Edition server version 1.26.60")
        );
        assert_eq!(
            translator
                .fill("language.selected", &["English", "eng"])
                .as_deref(),
            Some("Selected English (eng) as the base language")
        );
        assert_eq!(
            translator
                .fill("pocketmine.server.info", &["Axolotl-PM", "5.51.2-dev"])
                .as_deref(),
            Some("This server is running Axolotl-PM version 5.51.2-dev")
        );
        assert_eq!(
            translator
                .fill_named(
                    "pocketmine.network.session.close",
                    &[("reason", "Server closed")],
                )
                .as_deref(),
            Some("Session closed: Server closed")
        );
        assert_eq!(
            translator.fill("commands.players.list", &["2", "1000"]).as_deref(),
            Some("There are 2/1000 players online:")
        );
    }

    #[test]
    fn parses_the_shipped_locales() {
        let english = Translator::english();
        assert!(english.len() > 300, "only {} entries", english.len());
        assert!(english.get("chat.type.admin").is_some());
        assert_eq!(english.get("language.name"), Some("English"));
        assert!(english.get("missing.key").is_none());
        assert!(english.fill("missing.key", &[]).is_none());

        let spanish = Translator::spanish();
        assert!(spanish.len() > 300, "only {} entries", spanish.len());
        assert!(spanish.get("chat.type.admin").is_some());
    }
}
