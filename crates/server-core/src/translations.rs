//! Spanish translation catalogue copied from PocketMine-MP's `spa.ini`.
//!
//! Commands keep their messages as constants so the parsing stays pure, but this
//! module makes the exact locale available at runtime and lets the tests prove
//! that every constant still matches what PocketMine ships. That comparison is
//! the parity rule of the port: "the same behaviour, only the language
//! translated".

use std::collections::BTreeMap;

/// `resources/spa.ini`, copied from `axolotl-pm`. The reference clone lives in
/// `target/` and is not versioned, so the locale is embedded to keep the parity
/// test reproducible.
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

    /// The embedded Spanish locale.
    pub fn embedded() -> &'static Self {
        static TRANSLATOR: std::sync::OnceLock<Translator> = std::sync::OnceLock::new();
        TRANSLATOR.get_or_init(|| Translator::parse(SPA_INI))
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
    /// Grows as more commands move off ad-hoc strings.
    #[test]
    fn constants_match_the_shipped_locale() {
        let translator = Translator::embedded();
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
    fn fills_positional_and_named_placeholders() {
        let translator = Translator::embedded();
        assert_eq!(
            translator.fill("commands.players.list", &["2", "1000"]).as_deref(),
            Some("Hay 2/1000 jugadores en línea:")
        );
        assert_eq!(
            translator
                .fill_named(
                    "pocketmine.command.notFound",
                    &[("commandName", "nope"), ("helpCommand", "/help")],
                )
                .as_deref(),
            Some(concat!(
                "Comando desconocido: nope. Utilice /help para obtener una lista de ",
                "comandos disponibles."
            ))
        );
    }

    #[test]
    fn parses_the_shipped_locale() {
        let translator = Translator::embedded();
        assert!(translator.len() > 300, "only {} entries", translator.len());
        assert!(translator.get("chat.type.admin").is_some());
        assert!(translator.get("missing.key").is_none());
        assert!(translator.fill("missing.key", &[]).is_none());
    }
}
