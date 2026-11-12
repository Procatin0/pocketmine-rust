//! Scoreboard: port of `pocketmine\scoreboard\Scoreboard` (axolotl-pm
//! `src/scoreboard/*`: `Scoreboard`, `ScoreboardObjective`,
//! `ScoreboardLine`, `ScoreboardSlot`, `ColorRange`).
//!
//! A scoreboard is a per-player container of objectives. Each objective
//! has a name, a display name, a [`ScoreboardSlot`] (where clients show
//! it), a background tint ([`ColorRange`]) and a set of lines keyed by
//! their numeric score. Bedrock renders it with
//! `RemoveObjectivePacket` (0x6A) / `SetDisplayObjectivePacket` (0x6B) /
//! `SetScorePacket` (0x6C) — see [`Scoreboard::bedrock_packets`]. There is
//! deliberately NO `SetScoreboardIdentityPacket` (0x70): ScoreHud's
//! ScoreFactory never sends one, and the fake-player rows are created by
//! the `CHANGE_FAKE_PLAYER` entries themselves. Java renders the same state with
//! `scoreboard_objective` / `scoreboard_score` /
//! `scoreboard_display_objective` (+ `reset_score` for removals); the
//! Java builders live in `java-bridge` (which cannot depend on this
//! crate), fed through the `take_scoreboard` hook poll.
//!
//! Display-slot shape differs per platform and is the classic trap:
//! Bedrock `SetDisplayObjectivePacket` carries the slot as a string
//! (`"sidebar"` / `"belowname"` / `"list"`); Java
//! `display_objective.position` is list=0 / sidebar=1 / below-name=2.
//! [`ScoreboardSlot::bedrock_string_value`] and
//! [`ScoreboardSlot::java_position`] keep both straight. (The numeric
//! `bedrock_value` byte survives only as the PHP DTO slot, mirroring
//! php-plugin-bridge's `ScoreboardSlot` ints.)
//!
//! Persistence: [`Scoreboard::to_json`] / [`Scoreboard::from_json`]
//! (format `scoreboard.json`, see `SessionManager::save_scoreboards`)
//! store the global/default scoreboard plus every per-player copy, so
//! objectives survive a restart and are re-sent on join. PocketMine
//! itself keeps scoreboards in memory only (`Player::setScoreboard`);
//! the on-disk form is this port's JSON snapshot, like `world.json`.

use bedrock_protocol::{
    display_slot as bedrock_display_slot, sort_order as bedrock_sort_order, Packet,
    RemoveObjectivePacket, ScorePacketEntry, SetDisplayObjectivePacket, SetScorePacket,
    SetScoreboardIdentityPacket,
};
use php_plugin_bridge::{ScoreboardData, ScoreboardLineData, ScoreboardObjectiveData};
use serde::{Deserialize, Serialize};

/// Where a scoreboard objective is displayed.
///
/// Custom "ObjectScreen" slots from the Bedrock command API are not
/// modelled: PocketMine only exposes the three vanilla slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreboardSlot {
    Sidebar,
    BelowName,
    List,
}

impl Default for ScoreboardSlot {
    fn default() -> Self {
        Self::Sidebar
    }
}

impl ScoreboardSlot {
    /// Bedrock `SetDisplayObjectivePacket` slot string
    /// (`"sidebar"` / `"belowname"` / `"list"`).
    pub fn bedrock_string_value(self) -> &'static str {
        match self {
            Self::Sidebar => bedrock_display_slot::SIDEBAR,
            Self::BelowName => bedrock_display_slot::BELOW_NAME,
            Self::List => bedrock_display_slot::LIST,
        }
    }

    /// Bedrock `DisplaySlot` byte (sidebar=1, below-name=2, list=3),
    /// kept for the PHP DTO slot; the wire packet itself carries a
    /// string (see [`ScoreboardSlot::bedrock_string_value`]).
    pub fn bedrock_value(self) -> u8 {
        match self {
            Self::Sidebar => 1,
            Self::BelowName => 2,
            Self::List => 3,
        }
    }

    /// Java `display_objective` position (list=0, sidebar=1,
    /// below-name=2). Not a permutation of the Bedrock values.
    pub fn java_position(self) -> i32 {
        match self {
            Self::List => 0,
            Self::Sidebar => 1,
            Self::BelowName => 2,
        }
    }

    pub fn from_bedrock(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Sidebar),
            2 => Some(Self::BelowName),
            3 => Some(Self::List),
            _ => None,
        }
    }

    pub fn from_java(position: i32) -> Option<Self> {
        match position {
            0 => Some(Self::List),
            1 => Some(Self::Sidebar),
            2 => Some(Self::BelowName),
            _ => None,
        }
    }

    pub fn from_str_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "sidebar" | "side" => Some(Self::Sidebar),
            "below_name" | "belowname" | "below" => Some(Self::BelowName),
            "list" => Some(Self::List),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sidebar => "sidebar",
            Self::BelowName => "below_name",
            Self::List => "list",
        }
    }
}

/// Background tint of an objective (`ColorRange` in PHP).
///
/// Bedrock's `SetDisplayObjectivePacket` carries no colour field, so
/// the core encodes the tint as the matching `§` colour code prepended
/// to the display name (see [`ScoreboardObjective::wire_display_name`]).
/// Java display names are sent as plain text components and ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColorRange {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Vanilla `§` palette in `§0`-`§f` order, for nearest-code mapping.
const VANILLA_PALETTE: [(u8, u8, u8); 16] = [
    (0, 0, 0),       // 0 black
    (0, 0, 170),     // 1 dark_blue
    (0, 170, 0),     // 2 dark_green
    (0, 170, 170),   // 3 dark_aqua
    (170, 0, 0),     // 4 dark_red
    (170, 0, 170),   // 5 dark_purple
    (255, 170, 0),   // 6 gold
    (170, 170, 170), // 7 gray
    (85, 85, 85),    // 8 dark_gray
    (85, 85, 255),   // 9 blue
    (85, 255, 85),   // a green
    (85, 255, 255),  // b aqua
    (255, 85, 85),   // c red
    (255, 85, 255),  // d light_purple
    (255, 255, 85),  // e yellow
    (255, 255, 255), // f white
];

impl ColorRange {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn red() -> Self {
        Self::rgb(255, 85, 85)
    }

    pub fn black() -> Self {
        Self::rgb(0, 0, 0)
    }

    pub fn white() -> Self {
        Self::rgb(255, 255, 255)
    }

    /// Accepts `[r, g, b]`, mirroring the PHP `ColorRange` wire form.
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() == 3 {
            Some(Self::rgb(slice[0], slice[1], slice[2]))
        } else {
            None
        }
    }

    pub fn to_vec(self) -> Vec<u8> {
        vec![self.r, self.g, self.b]
    }

    /// Nearest vanilla `§` code (`0`-`f`) by euclidean RGB distance.
    pub fn color_code(self) -> char {
        let mut best = 0usize;
        let mut best_dist = u32::MAX;
        for (index, &(pr, pg, pb)) in VANILLA_PALETTE.iter().enumerate() {
            let dr = self.r as i32 - pr as i32;
            let dg = self.g as i32 - pg as i32;
            let db = self.b as i32 - pb as i32;
            let dist = (dr * dr + dg * dg + db * db) as u32;
            if dist < best_dist {
                best_dist = dist;
                best = index;
            }
        }
        char::from_digit(best as u32, 16).unwrap_or('f')
    }

    /// `§` code prefix, e.g. `"§c"` for red.
    pub fn prefix(self) -> String {
        format!("\u{a7}{}", self.color_code())
    }
}

impl Default for ColorRange {
    fn default() -> Self {
        Self::red()
    }
}

/// One scoreboard line: a text shown with a numeric score.
///
/// The score is the sort key (with descending sort order, higher scores
/// render on top). Lines are unique per objective by score value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreboardLine {
    pub score: u32,
    pub text: String,
}

impl ScoreboardLine {
    pub fn new(score: u32, text: impl Into<String>) -> Self {
        Self {
            score,
            text: text.into(),
        }
    }

    /// Fake-player name sent as `customName` on the line's
    /// `CHANGE_FAKE_PLAYER` `SetScorePacket` entry: the line text verbatim,
    /// exactly like ScoreFactory (`ScorePacketEntry.customName = message`).
    /// The old `§r<score>§<text>` form duplicated the score the text
    /// already carries (e.g. ScoreboardDemo `"Oro: 100"`) and orphaned a
    /// new client row on every text edit.
    pub fn fake_player_id(&self) -> String {
        self.text.clone()
    }

    /// `scoreboardId` for this line: the `i64` row key handed to
    /// `SetScorePacket`. ScoreFactory uses `scoreboardId = line = score`
    /// (1..15, small and stable across text edits), so an update
    /// overwrites the same client row instead of orphaning the previous
    /// one behind a new FNV-of-text id. No `SetScoreboardIdentityPacket`
    /// (0x70) is sent alongside (see [`Scoreboard::bedrock_packets`]).
    pub fn identity(&self) -> i64 {
        self.score as i64
    }
}

/// One objective: named, displayed in one slot, with scored lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreboardObjective {
    pub name: String,
    pub display_name: String,
    pub slot: ScoreboardSlot,
    #[serde(default = "default_sort_order")]
    pub sort_order: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ColorRange>,
    #[serde(default)]
    pub lines: Vec<ScoreboardLine>,
}

fn default_sort_order() -> u8 {
    bedrock_sort_order::DESCENDING as u8
}

impl ScoreboardObjective {
    pub fn new(
        name: impl Into<String>,
        display_name: impl Into<String>,
        slot: ScoreboardSlot,
    ) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            slot,
            sort_order: default_sort_order(),
            background: None,
            lines: Vec::new(),
        }
    }

    pub fn sidebar(name: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self::new(name, display_name, ScoreboardSlot::Sidebar)
    }

    /// Adds (or replaces) the line with this score. Returns `true` when
    /// the line set changed.
    pub fn add_score(&mut self, score: u32, text: impl Into<String>) -> bool {
        let text = text.into();
        if let Some(line) = self.lines.iter_mut().find(|line| line.score == score) {
            if line.text == text {
                return false;
            }
            line.text = text;
        } else {
            self.lines.push(ScoreboardLine::new(score, text));
        }
        true
    }

    /// Removes the line with this score. Returns `true` when present.
    pub fn remove_score(&mut self, score: u32) -> bool {
        let before = self.lines.len();
        self.lines.retain(|line| line.score != score);
        self.lines.len() != before
    }

    pub fn get_score(&self, score: u32) -> Option<&ScoreboardLine> {
        self.lines.iter().find(|line| line.score == score)
    }

    /// Lines in display order: descending score for the default
    /// descending sort, ascending otherwise.
    pub fn sorted_lines(&self) -> Vec<&ScoreboardLine> {
        let mut lines: Vec<&ScoreboardLine> = self.lines.iter().collect();
        if self.sort_order == bedrock_sort_order::DESCENDING as u8 {
            lines.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.text.cmp(&b.text)));
        } else {
            lines.sort_by(|a, b| a.score.cmp(&b.score).then_with(|| a.text.cmp(&b.text)));
        }
        lines
    }

    pub fn clear_lines(&mut self) {
        self.lines.clear();
    }

    /// Display name as sent to Bedrock clients: background tint as a
    /// `§` prefix (Bedrock has no separate colour field).
    pub fn wire_display_name(&self) -> String {
        match self.background {
            Some(color) => format!("{}{}", color.prefix(), self.display_name),
            None => self.display_name.clone(),
        }
    }
}

/// A player's whole scoreboard: a title plus ordered objectives.
///
/// Objective names must be unique; adding a second objective with the
/// same name replaces the first (PocketMine `Scoreboard::addObjective`
/// semantics).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Scoreboard {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub objectives: Vec<ScoreboardObjective>,
}

impl Scoreboard {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            objectives: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.objectives.is_empty()
    }

    /// Adds (or replaces, by name) an objective.
    pub fn add_objective(&mut self, objective: ScoreboardObjective) {
        if let Some(existing) = self
            .objectives
            .iter_mut()
            .find(|existing| existing.name == objective.name)
        {
            *existing = objective;
        } else {
            self.objectives.push(objective);
        }
    }

    /// Removes an objective by name. Returns `true` when present.
    pub fn remove_objective(&mut self, name: &str) -> bool {
        let before = self.objectives.len();
        self.objectives.retain(|objective| objective.name != name);
        self.objectives.len() != before
    }

    pub fn get_objective(&self, name: &str) -> Option<&ScoreboardObjective> {
        self.objectives.iter().find(|objective| objective.name == name)
    }

    pub fn get_objective_mut(&mut self, name: &str) -> Option<&mut ScoreboardObjective> {
        self.objectives
            .iter_mut()
            .find(|objective| objective.name == name)
    }

    /// `Scoreboard::addScore(objective, score, text)`: adds a line to an
    /// existing objective. Returns `false` when the objective is
    /// missing (PocketMine throws; the core keeps the no-panic style
    /// of the plugin actions and reports `false` instead).
    pub fn add_score(&mut self, objective_name: &str, score: u32, text: impl Into<String>) -> bool {
        match self.get_objective_mut(objective_name) {
            Some(objective) => objective.add_score(score, text),
            None => false,
        }
    }

    /// `Scoreboard::removeScore(objective, score)`: removes a line.
    pub fn remove_score(&mut self, objective_name: &str, score: u32) -> bool {
        match self.get_objective_mut(objective_name) {
            Some(objective) => objective.remove_score(score),
            None => false,
        }
    }

    pub fn clear(&mut self) {
        self.objectives.clear();
    }

    // -- wire: Bedrock --------------------------------------------------

    /// Every Bedrock packet needed to show this scoreboard from scratch
    /// (after any previous objectives were removed): per objective a
    /// `RemoveObjectivePacket`, a `SetDisplayObjectivePacket`, and a
    /// single batched `SetScorePacket` with one `CHANGE_FAKE_PLAYER`
    /// (0x6C) entry per line (small `scoreboardId = score`, raw
    /// `customName = text`).
    ///
    /// ScoreFactory wire, in order (same grouping, one tick each):
    /// - `sendObjective`: `0x6A RemoveObjective` then `0x6B
    ///   SetDisplayObjective` (objective `"objective"`, slot SIDEBAR,
    ///   sortOrder ASCENDING 0, criteria `"dummy"`). No `0x70` is ever
    ///   sent by ScoreFactory — the fake-player rows are created by the
    ///   `CHANGE_FAKE_PLAYER` entries themselves.
    /// - `setScoreLine` sends nothing (cache only); `sendLines` then emits
    ///   two `0x6C` packets: first the cached entries still as
    ///   `CHANGE_FAKE_PLAYER` (ordinal 3 / `changefakeplayer`,
    ///   `scoreboardId = score = line`, `customName = message`) — this is
    ///   the packet that makes the text visible — and then the same
    ///   entries flipped to `CHANGE_PLAYER` (ordinal 1 /
    ///   `changeplayer`).
    /// The `CHANGE_PLAYER` flip is the virion's legacy binding step: on
    /// the wire its entry body is `objectiveName + score + actorUniqueId`
    /// (`SetScorePacket::encodePayload`, axolotl BedrockProtocol) — it
    /// CANNOT carry `customName`, so no text is delivered by it; the
    /// shown lines are the ones the preceding `CHANGE_FAKE_PLAYER` packet
    /// registered under the same `scoreboardId`s (a `CHANGE_PLAYER` entry
    /// without a matching actor lookup falls back to that registered fake
    /// name on classic clients). Its `actorUniqueId` is
    /// `Player::getId()` (the entity runtime id, e.g. 1), not a spawned
    /// actor's unique id, so on strict clients (1.26, `Block /
    /// ClientDisconnection-90`) this dangling rebinding is a disconnect
    /// risk, not a display step. This state-based port therefore emits
    /// only the `CHANGE_FAKE_PLAYER` packet (the one that determines the
    /// shown wire) — no `0x70`, no `CHANGE_PLAYER` rebinding — with
    /// `RemoveObjective` + display keeping updates idempotent.
    pub fn bedrock_packets(&self) -> Vec<PacketBytes> {
        let mut out = Vec::new();
        for objective in &self.objectives {
            out.push(PacketBytes::RemoveObjective(RemoveObjectivePacket::new(
                objective.name.clone(),
            )));
            out.push(PacketBytes::SetDisplayObjective(SetDisplayObjectivePacket {
                display_slot: objective.slot.bedrock_string_value().to_string(),
                objective_name: objective.name.clone(),
                display_name: objective.wire_display_name(),
                criteria_name: SetDisplayObjectivePacket::CRITERIA_DUMMY.to_string(),
                sort_order: objective.sort_order as i32,
            }));
            let mut entries = Vec::new();
            for line in objective.sorted_lines() {
                entries.push(ScorePacketEntry::change_fake_player(
                    line.identity(),
                    objective.name.clone(),
                    line.score as i32,
                    line.fake_player_id(),
                ));
            }
            if !entries.is_empty() {
                out.push(PacketBytes::SetScore(score_packet(entries)));
            }
        }
        out
    }

    /// Packets that clear this scoreboard on a Bedrock client: one
    /// `RemoveObjectivePacket` per objective.
    pub fn bedrock_remove_packets(&self) -> Vec<PacketBytes> {
        self.objectives
            .iter()
            .map(|objective| PacketBytes::RemoveObjective(RemoveObjectivePacket::new(
                objective.name.clone(),
            )))
            .collect()
    }

    /// Diff against a previous scoreboard, as the packets a client
    /// needs to go from `old` to `self`: removed objectives first,
    /// then the full new state (Bedrock has no in-place objective
    /// update; re-adding a display objective overwrites it).
    pub fn bedrock_diff_packets(&self, old: &Scoreboard) -> Vec<PacketBytes> {
        let mut out = Vec::new();
        for objective in &old.objectives {
            if self.get_objective(&objective.name).is_none() {
                out.push(PacketBytes::RemoveObjective(RemoveObjectivePacket::new(
                    objective.name.clone(),
                )));
            }
        }
        out.extend(self.bedrock_packets());
        out
    }

    // -- wire: PHP DTO ---------------------------------------------------

    pub fn from_data(data: &ScoreboardData) -> Self {
        let mut scoreboard = Scoreboard::new(data.title.clone());
        for objective in &data.objectives {
            let slot = ScoreboardSlot::from_bedrock(objective.slot).unwrap_or_default();
            let mut built = ScoreboardObjective::new(
                objective.name.clone(),
                objective.display_name.clone(),
                slot,
            );
            built.sort_order = objective.sort_order;
            built.background = ColorRange::from_slice(&objective.background);
            for line in &objective.lines {
                built.add_score(line.score, line.text.clone());
            }
            scoreboard.add_objective(built);
        }
        scoreboard
    }

    pub fn to_data(&self) -> ScoreboardData {
        ScoreboardData {
            title: self.title.clone(),
            objectives: self
                .objectives
                .iter()
                .map(|objective| ScoreboardObjectiveData {
                    name: objective.name.clone(),
                    display_name: objective.display_name.clone(),
                    slot: objective.slot.bedrock_value(),
                    sort_order: objective.sort_order,
                    background: objective
                        .background
                        .map(|color| color.to_vec())
                        .unwrap_or_default(),
                    lines: objective
                        .lines
                        .iter()
                        .map(|line| ScoreboardLineData {
                            score: line.score,
                            text: line.text.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    // -- persistence -----------------------------------------------------

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        serde_json::from_value(value.clone()).ok()
    }
}

fn score_packet(entries: Vec<ScorePacketEntry>) -> SetScorePacket {
    SetScorePacket::new(entries)
}

/// Typed Bedrock scoreboard packet, so callers never hand-assemble wire
/// bytes. `SessionManager` maps these onto `OutgoingPacket` variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketBytes {
    RemoveObjective(RemoveObjectivePacket),
    SetDisplayObjective(SetDisplayObjectivePacket),
    SetScore(SetScorePacket),
    SetScoreboardIdentity(SetScoreboardIdentityPacket),
}

impl PacketBytes {
    pub fn packet_id(&self) -> u32 {
        match self {
            Self::RemoveObjective(packet) => packet.packet_id(),
            Self::SetDisplayObjective(packet) => packet.packet_id(),
            Self::SetScore(packet) => packet.packet_id(),
            Self::SetScoreboardIdentity(packet) => packet.packet_id(),
        }
    }

    pub fn encode(&self, writer: &mut bedrock_protocol::Writer) -> Result<(), bedrock_protocol::binary::CodecError> {
        match self {
            Self::RemoveObjective(packet) => packet.encode(writer),
            Self::SetDisplayObjective(packet) => packet.encode(writer),
            Self::SetScore(packet) => packet.encode(writer),
            Self::SetScoreboardIdentity(packet) => packet.encode(writer),
        }
    }
}

/// On-disk `scoreboard.json`: the global (default) scoreboard plus one
/// entry per player that has a personal copy. Written by
/// `SessionManager::save_scoreboards`, loaded at startup.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoreboardFile {
    #[serde(default)]
    pub version: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global: Option<Scoreboard>,
    #[serde(default)]
    pub players: std::collections::HashMap<String, Scoreboard>,
}

pub const SCOREBOARD_FILE_VERSION: i32 = 1;

impl ScoreboardFile {
    pub fn new(global: Option<Scoreboard>, players: std::collections::HashMap<String, Scoreboard>) -> Self {
        Self {
            version: SCOREBOARD_FILE_VERSION,
            global,
            players,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> Scoreboard {
        let mut main = ScoreboardObjective::sidebar("main", "Números");
        main.background = Some(ColorRange::red());
        main.add_score(100, "Oro");
        main.add_score(50, "Plata");
        main.add_score(10, "Cobre");
        let mut board = Scoreboard::new("Economía");
        board.add_objective(main);
        board
    }

    #[test]
    fn slot_values_are_not_interchangeable_between_platforms() {
        assert_eq!(ScoreboardSlot::Sidebar.bedrock_value(), 1);
        assert_eq!(ScoreboardSlot::Sidebar.bedrock_string_value(), "sidebar");
        assert_eq!(ScoreboardSlot::BelowName.bedrock_string_value(), "belowname");
        assert_eq!(ScoreboardSlot::List.bedrock_string_value(), "list");
        assert_eq!(ScoreboardSlot::Sidebar.java_position(), 1);
        assert_eq!(ScoreboardSlot::BelowName.bedrock_value(), 2);
        assert_eq!(ScoreboardSlot::BelowName.java_position(), 2);
        assert_eq!(ScoreboardSlot::List.bedrock_value(), 3);
        assert_eq!(ScoreboardSlot::List.java_position(), 0);
        assert_eq!(ScoreboardSlot::from_bedrock(3), Some(ScoreboardSlot::List));
        assert_eq!(ScoreboardSlot::from_java(0), Some(ScoreboardSlot::List));
        assert_eq!(ScoreboardSlot::from_bedrock(0), None);
        assert_eq!(ScoreboardSlot::from_java(3), None);
        assert_eq!(ScoreboardSlot::from_str_name("sidebar"), Some(ScoreboardSlot::Sidebar));
        assert_eq!(ScoreboardSlot::from_str_name("nope"), None);
    }

    #[test]
    fn color_code_maps_to_nearest_vanilla_code() {
        assert_eq!(ColorRange::red().color_code(), 'c');
        assert_eq!(ColorRange::black().color_code(), '0');
        assert_eq!(ColorRange::white().color_code(), 'f');
        assert_eq!(ColorRange::rgb(0, 0, 170).color_code(), '1');
        assert_eq!(ColorRange::red().prefix(), "\u{a7}c");
    }

    #[test]
    fn add_objective_replaces_by_name_and_remove_reports_presence() {
        let mut board = Scoreboard::new("t");
        board.add_objective(ScoreboardObjective::sidebar("main", "Uno"));
        board.add_objective(ScoreboardObjective::sidebar("main", "Dos"));
        assert_eq!(board.objectives.len(), 1);
        assert_eq!(board.get_objective("main").unwrap().display_name, "Dos");
        assert!(board.remove_objective("main"));
        assert!(!board.remove_objective("main"));
        assert!(board.is_empty());
    }

    #[test]
    fn add_score_replaces_same_score_and_remove_score_reports_presence() {
        let mut board = example();
        assert!(!board.add_score("missing", 1, "x"));
        assert!(board.add_score("main", 50, "Plata v2"));
        assert!(!board.add_score("main", 50, "Plata v2"), "same text: no change");
        assert_eq!(board.get_objective("main").unwrap().lines.len(), 3);
        assert!(board.remove_score("main", 50));
        assert!(!board.remove_score("main", 50));
        assert!(!board.remove_score("missing", 50));
    }

    #[test]
    fn sorted_lines_are_descending_by_default() {
        let objective = example().get_objective("main").unwrap().clone();
        let texts: Vec<&str> = objective
            .sorted_lines()
            .iter()
            .map(|line| line.text.as_str())
            .collect();
        assert_eq!(texts, vec!["Oro", "Plata", "Cobre"]);
    }

    #[test]
    fn bedrock_packets_are_remove_display_then_scores_like_scorefactory() {
        let board = example();
        let packets = board.bedrock_packets();
        assert_eq!(packets.len(), 3, "1 remove + 1 display + 1 score batch (no 0x70): {packets:?}");
        assert_eq!(packets[0].packet_id(), 0x6A);
        assert_eq!(packets[1].packet_id(), 0x6B);
        assert_eq!(packets[2].packet_id(), 0x6C);
        // Display name carries the red §c tint prefix; the slot is a string.
        match &packets[1] {
            PacketBytes::SetDisplayObjective(packet) => {
                assert_eq!(packet.display_name, "\u{a7}cNúmeros");
                assert_eq!(packet.display_slot, bedrock_display_slot::SIDEBAR);
                assert_eq!(packet.criteria_name, "dummy");
            }
            other => panic!("expected SetDisplayObjective, got {other:?}"),
        }
        match &packets[2] {
            PacketBytes::SetScore(packet) => {
                assert_eq!(packet.entries.len(), 3);
                let first = &packet.entries[0];
                assert_eq!(first.score, 100);
                assert_eq!(first.objective_name.as_deref(), Some("main"));
                assert_eq!(first.action, bedrock_protocol::ScorePacketEntryAction::ChangeFakePlayer);
                // ScoreFactory parity: scoreboardId = score, customName = raw text.
                assert_eq!(first.scoreboard_id, 100);
                assert_eq!(first.custom_name.as_deref(), Some("Oro"));
            }
            other => panic!("expected SetScore, got {other:?}"),
        }
        // ScoreFactory never sends SetScoreboardIdentityPacket (0x70).
        assert!(
            packets.iter().all(|packet| packet.packet_id() != 0x70),
            "no identity packet: {packets:?}"
        );
    }

    #[test]
    fn diff_removes_only_dropped_objectives_then_resends() {
        let old = example();
        let mut new = old.clone();
        new.add_objective(ScoreboardObjective::sidebar("extra", "Extra"));
        new.remove_objective("main");
        let packets = new.bedrock_diff_packets(&old);
        assert_eq!(packets[0].packet_id(), 0x6A, "removed objective first");
        assert!(packets.iter().any(|packet| packet.packet_id() == 0x6B));
    }

    #[test]
    fn fake_player_ids_are_raw_text_and_identity_is_the_score() {
        // ScoreFactory: customName = message verbatim, scoreboardId = score.
        let a = ScoreboardLine::new(100, "Oro");
        let b = ScoreboardLine::new(50, "Oro");
        assert_eq!(a.fake_player_id(), "Oro");
        assert_eq!(a.identity(), 100);
        assert_eq!(b.identity(), 50);
        assert_ne!(a.identity(), b.identity());
        // Stable across text edits: an update overwrites the same client row.
        assert_eq!(ScoreboardLine::new(100, "Oro v2").identity(), 100);
        assert_eq!(ScoreboardLine::new(100, "Oro v2").fake_player_id(), "Oro v2");
    }

    #[test]
    fn json_roundtrip_preserves_objectives_lines_and_slot() {
        let board = example();
        let json = board.to_json();
        let restored = Scoreboard::from_json(&json).expect("restore");
        assert_eq!(restored, board);
    }

    #[test]
    fn dto_roundtrip_preserves_everything() {
        let board = example();
        let restored = Scoreboard::from_data(&board.to_data());
        assert_eq!(restored, board);
    }

    #[test]
    fn scoreboard_file_roundtrip() {
        let mut players = std::collections::HashMap::new();
        players.insert("steve".to_string(), example());
        let file = ScoreboardFile::new(Some(example()), players);
        let json = serde_json::to_value(&file).expect("to json");
        let restored: ScoreboardFile = serde_json::from_value(json).expect("from json");
        assert_eq!(restored.version, SCOREBOARD_FILE_VERSION);
        assert_eq!(restored.global, Some(example()));
        assert_eq!(restored.players.get("steve"), Some(&example()));
    }
}
