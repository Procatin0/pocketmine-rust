//! Cross-platform display-name convention: PocketMine (Bedrock host) style visibility.
//!
//! On PocketMine-Rust, Bedrock is the native host platform, while Java Edition
//! players connect via the java-bridge as guest/virtual players.
//! This is the exact inverse of GeyserMC/Floodgate (where Java is the host and
//! Bedrock is the guest):
//!
//! * **Bedrock players (native) NEVER have a prefix `.`**:
//!   Bedrock viewers see `Procatin0` as `Procatin0`.
//!   Java viewers see `Procatin0` as `Procatin0`.
//!   Scores, nametags, TAB entries, and chat for Bedrock players are always verbatim.
//!
//! * **Java players (virtual/guest) are named with the `.` prefix from the
//!   start**, on every surface: their canonical display name already carries
//!   it, so Bedrock TAB (`PlayerListPacket`), nametag metadata
//!   (`AddPlayerPacket` / `SetActorDataPacket`), Bedrock and Java chat,
//!   scoreboards, Java TAB and Java nametags all agree (`sunkiler008` is
//!   always shown as `.sunkiler008`).
//!
//! Single source of truth: [`JAVA_PREFIX_IN_BEDROCK`] and
//! [`java_player_name`] (idempotent, so applying it twice never yields `..`).

/// Prefix carried by every Java player's name (inverse of GeyserMC/Floodgate).
pub const JAVA_PREFIX_IN_BEDROCK: &str = ".";

/// Deprecated / compatibility alias: Bedrock players never have a prefix in Java.
pub const BEDROCK_PREFIX_IN_JAVA: &str = "";

/// Canonical name of a Java player: the wire username with the `.` prefix.
///
/// Idempotent: an already-prefixed (or empty) name is returned unchanged, so
/// every surface can treat the player "as if the name already included the
/// prefix from the start" without producing `.` `.` `name`.
pub fn java_player_name(raw_name: &str) -> String {
    if raw_name.is_empty() || raw_name.starts_with(JAVA_PREFIX_IN_BEDROCK) {
        raw_name.to_string()
    } else {
        format!("{JAVA_PREFIX_IN_BEDROCK}{raw_name}")
    }
}

/// Display name of a Bedrock player as seen by a Java viewer (verbatim, native platform).
pub fn bedrock_to_java_display(raw_name: &str) -> String {
    raw_name.to_string()
}

/// Display name of a Java player (prefixed with ".").
pub fn java_to_bedrock_display(raw_name: &str) -> String {
    java_player_name(raw_name)
}

/// Observer-relative display name for one target.
///
/// Bedrock players are always verbatim; Java players always carry their `.`
/// prefix, regardless of who is looking, because the prefix is part of their
/// canonical name.
pub fn display_name(raw_name: &str, is_bedrock: bool, _viewer_is_java: bool) -> String {
    if is_bedrock {
        raw_name.to_string()
    } else {
        java_player_name(raw_name)
    }
}

/// Display name for a Java viewer from a snapshot flag (`is_virtual`).
/// Bedrock players stay verbatim; Java players keep their `.` prefix.
pub fn java_view_display_name(raw_name: &str, is_virtual: bool) -> String {
    if is_virtual {
        java_player_name(raw_name)
    } else {
        raw_name.to_string()
    }
}

/// Display name for a Bedrock viewer.
/// Java players (`is_virtual == true`) get the "." prefix; Bedrock players are verbatim.
pub fn bedrock_view_display_name(raw_name: &str, is_virtual: bool) -> String {
    if is_virtual {
        java_player_name(raw_name)
    } else {
        raw_name.to_string()
    }
}

/// True when `display` is a prefixed Java display name.
pub fn is_java_display_name(display: &str) -> bool {
    display.len() > JAVA_PREFIX_IN_BEDROCK.len()
        && display.starts_with(JAVA_PREFIX_IN_BEDROCK)
        && !display[JAVA_PREFIX_IN_BEDROCK.len()..].is_empty()
}

/// Strips the Java prefix, returning the raw Java name.
pub fn strip_java_prefix(display: &str) -> &str {
    display.strip_prefix(JAVA_PREFIX_IN_BEDROCK).unwrap_or(display)
}

/// Compatibility alias (Bedrock names have no prefix).
pub fn is_bedrock_display_name(_display: &str) -> bool {
    false
}

/// Compatibility alias (Bedrock names have no prefix).
pub fn strip_bedrock_prefix(display: &str) -> &str {
    display
}

/// Bedrock names must never be prefixed in text. Returns input unchanged.
pub fn prefix_bedrock_names_in_text(text: &str, _raw_bedrock_names: &[String]) -> String {
    text.to_string()
}

/// Rewrites whole-word occurrences of each *raw* Java name in `text` to
/// its Bedrock-view form (`"." + raw`), skipping occurrences that are
/// already prefixed.
pub fn prefix_java_names_in_text(text: &str, raw_java_names: &[String]) -> String {
    let mut result = text.to_string();
    let mut names: Vec<&String> = raw_java_names
        .iter()
        .filter(|name| !name.is_empty())
        .collect();
    names.sort_by(|a, b| b.len().cmp(&a.len()));
    for raw in names {
        result = prefix_one_java_name(&result, raw);
    }
    result
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn prefix_one_java_name(text: &str, raw: &str) -> String {
    let bytes = text.as_bytes();
    let raw_bytes = raw.as_bytes();
    if raw_bytes.is_empty() || raw_bytes.len() > bytes.len() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 1);
    let mut i = 0;
    while i < bytes.len() {
        if text[i..].starts_with(raw) {
            let before_ok = i == 0 || !is_name_byte(bytes[i - 1]);
            let after = i + raw_bytes.len();
            let after_ok = after >= bytes.len() || !is_name_byte(bytes[after]);
            let already_prefixed = i > 0 && bytes[i - 1] == b'.';
            if before_ok && after_ok && !already_prefixed {
                out.push_str(JAVA_PREFIX_IN_BEDROCK);
                out.push_str(raw);
                i += raw_bytes.len();
                continue;
            }
        }
        let ch_len = utf8_len(bytes[i]);
        out.push_str(&text[i..i + ch_len]);
        i += ch_len;
    }
    out
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >= 0xF0 {
        4
    } else if first >= 0xE0 {
        3
    } else if first >= 0xC0 {
        2
    } else {
        1
    }
}

/// Default chat line (`"<name> msg"`) for one viewer platform.
pub fn chat_line(sender_raw: &str, sender_is_bedrock: bool, viewer_is_java: bool, message: &str) -> String {
    let display = display_name(sender_raw, sender_is_bedrock, viewer_is_java);
    format!("<{display}> {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_prefix_is_part_of_the_name_for_every_viewer() {
        assert_eq!(JAVA_PREFIX_IN_BEDROCK, ".");
        assert_eq!(java_to_bedrock_display("sunkiler008"), ".sunkiler008");
        assert_eq!(bedrock_to_java_display("Procatin0"), "Procatin0");

        // Matrix:
        // Bedrock target ("Procatin0", is_bedrock=true):
        assert_eq!(display_name("Procatin0", true, false), "Procatin0"); // seen by Bedrock
        assert_eq!(display_name("Procatin0", true, true), "Procatin0");  // seen by Java

        // Java target ("sunkiler008", is_bedrock=false): the prefix is part
        // of the name, so it never disappears from any surface.
        assert_eq!(display_name("sunkiler008", false, false), ".sunkiler008"); // seen by Bedrock
        assert_eq!(display_name("sunkiler008", false, true), ".sunkiler008");  // seen by Java

        // View helpers:
        assert_eq!(java_view_display_name("Procatin0", false), "Procatin0");
        assert_eq!(java_view_display_name("sunkiler008", true), ".sunkiler008");
        assert_eq!(java_view_display_name("sunkiler008", false), "sunkiler008");
        assert_eq!(bedrock_view_display_name("Procatin0", false), "Procatin0");
        assert_eq!(bedrock_view_display_name("sunkiler008", true), ".sunkiler008");
        // Already-prefixed input is never prefixed twice.
        assert_eq!(java_player_name(".sunkiler008"), ".sunkiler008");
        assert_eq!(java_player_name(""), "");
    }

    #[test]
    fn strip_roundtrips_prefixed_and_leaves_raw_alone() {
        assert_eq!(strip_java_prefix(".sunkiler008"), "sunkiler008");
        assert_eq!(strip_java_prefix("sunkiler008"), "sunkiler008");
        assert!(is_java_display_name(".sunkiler008"));
        assert!(!is_java_display_name("sunkiler008"));
        assert!(!is_java_display_name("."));
    }

    #[test]
    fn bedrock_names_are_never_prefixed_in_text() {
        let names = vec!["Procatin0".to_string(), "Steve".to_string()];
        assert_eq!(
            prefix_bedrock_names_in_text("<Procatin0> hola", &names),
            "<Procatin0> hola"
        );
        assert_eq!(
            prefix_bedrock_names_in_text("Nombre: Procatin0", &names),
            "Nombre: Procatin0"
        );
    }

    #[test]
    fn java_names_prefixed_in_text_respects_boundaries() {
        let names = vec!["sunkiler008".to_string()];
        assert_eq!(
            prefix_java_names_in_text("<sunkiler008> hola", &names),
            "<.sunkiler008> hola"
        );
        assert_eq!(
            prefix_java_names_in_text("sunkiler008 joined the game", &names),
            ".sunkiler008 joined the game"
        );
        assert_eq!(
            prefix_java_names_in_text("<.sunkiler008> hola", &names),
            "<.sunkiler008> hola"
        );
    }

    #[test]
    fn chat_line_matches_display_matrix() {
        assert_eq!(chat_line("Procatin0", true, false, "hola"), "<Procatin0> hola");
        assert_eq!(chat_line("Procatin0", true, true, "hola"), "<Procatin0> hola");
        assert_eq!(chat_line("sunkiler008", false, false, "hola"), "<.sunkiler008> hola");
        assert_eq!(chat_line("sunkiler008", false, true, "hola"), "<.sunkiler008> hola");
    }
}
