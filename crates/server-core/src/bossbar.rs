//! Boss bars: Rust port of `xenialdan\apibossbar` (apiBossBar virion).
//!
//! Upstream (`target/inxomnyaa-apiBossBar/src/xenialdan/apibossbar/`):
//!
//! * `BossBar.php` — single shared bar for all viewers: `addPlayer` /
//!   `removePlayer` / `addPlayers` / `removePlayers` / `removeAllPlayers`,
//!   `setTitle` / `setSubTitle` / `getFullTitle`, `setPercentage` (0..1,
//!   via the `Attribute::HEALTH` map, max 100), `setColor`, `hideFrom` /
//!   `showTo` (+ `*All`), `getEntity` / `setEntity` / `resetEntity`, fluent
//!   setters, `BossEventPacket::show/hide/title/healthPercent` sends.
//! * `DiverseBossBar.php` — per-player title/subtitle/percentage/color
//!   overrides falling back to the shared defaults (`setTitleFor`,
//!   `setPercentageFor`, `setColorFor`, `resetFor`/`resetForAll`, ...).
//! * `API.php` / `PacketListener.php` — `API::load` registers a listener
//!   that logs client `BossEventPacket` register/unregister packets.
//!
//! Mapping here:
//!
//! * [`BossBar`] — shared bar: fluent `set_title` / `set_subtitle` /
//!   `set_percentage` / `set_color` / `set_overlay` / `set_style` /
//!   `set_flags` / `add_player` / `remove_player` / `hide_from` /
//!   `show_to` / `set_entity` / `reset_entity` + packet builders
//!   [`BossBar::show_packet`] (Bedrock) and [`BossBar::java_add_frame`]
//!   (Java). Viewer state is kept here; actual delivery is done by
//!   `SessionManager` (see `session.rs`), which resolves names to
//!   Bedrock sessions or Java virtual players.
//! * [`DiverseBossBar`] — wraps a [`BossBar`] plus per-player override
//!   maps with the upstream `*For` methods, including the upstream
//!   `getFullTitleFor` fallback bug-for-bug: the per-player full title
//!   is only the override title/subtitle when at least one is set,
//!   otherwise the shared full title.
//! * [`BossBarManager`] — thread-safe registry (`Arc`, `RwLock`) owning
//!   every bar by its Bedrock actor id, with the synthetic actor-id
//!   allocator (`Entity::nextRuntimeId()` in PHP) and per-player Java
//!   frame queues drained by `java-bridge` (`take_bossbar_frames`).
//! * Free helpers [`clamp_percentage`] and [`full_title`] mirror
//!   `setPercentage`'s clamp and `getFullTitle`'s `"\n\n"` join.
//!
//! Viewers are keyed by lowercase player name so Bedrock sessions and
//! Java virtual players (`PlayerRegistry`) share one namespace, exactly
//! like `queue_packet` (name -> session id) and the registry do.
//! "Add player" always shows the bar (upstream `addPlayer` sends
//! `BossEventPacket::show`); "hide" only hides without unsubscribing,
//! "remove" sends `hide` and unsubscribes.
//!
//! Colours default to `BossBarColor::Purple` and overlays to
//! `BossBarOverlay::Progress`, matching the upstream constructor;
//! `BossBar::set_color` re-sends `show` (as upstream does — Bedrock has
//! no standalone colour event in apiBossBar), while `properties` events
//! are available via [`BossBar::properties_packet`].

use std::collections::{HashMap, HashSet};

use bedrock_protocol::{BossBarColor, BossBarOverlay, BossEventPacket};
use java_bridge::BossBarFrame;
use parking_lot::RwLock;

pub use java_bridge::bossbar as java;

/// Clamp a fill fraction exactly like `setPercentage`:
/// `min(1.0, max(0.0, percentage))`.
pub fn clamp_percentage(percentage: f32) -> f32 {
    percentage.clamp(0.0, 1.0)
}

/// `getFullTitle()`: title plus `"\n\n" + subtitle` when the subtitle is
/// non-empty (single-line titles + subtitle below the bar).
pub fn full_title(title: &str, subtitle: &str) -> String {
    if subtitle.is_empty() {
        title.to_string()
    } else {
        format!("{title}\n\n{subtitle}")
    }
}

/// Lowercase player name (registry / queue namespace).
pub fn viewer_key(name: &str) -> String {
    name.to_lowercase()
}

/// One queued Java `boss_bar` frame helper (kept here so `session.rs`
/// and `java_hooks.rs` share the constructor).
pub fn java_frame(frame: Vec<u8>) -> BossBarFrame {
    BossBarFrame { frame }
}

/// A shared boss bar (upstream `BossBar`).
#[derive(Debug, Clone, PartialEq)]
pub struct BossBar {
    /// Bedrock boss actor unique id (`$actorId`; synthetic when no
    /// entity is bound). Also the registry key.
    pub actor_id: i64,
    /// Name of the bound entity/player, when `set_entity` bound one.
    pub bound_entity: Option<String>,
    title: String,
    subtitle: String,
    percentage: f32,
    color: u8,
    overlay: u8,
    /// Java-only flags (`FLAG_DARKEN_SCREEN | FLAG_PLAY_MUSIC |
    /// FLAG_CREATE_WORLD_FOG`).
    flags: u8,
    /// Lowercase viewer names (`$players` by player id in PHP).
    viewers: HashSet<String>,
}

impl BossBar {
    /// `new BossBar()`: no viewers, empty titles, 100% fill, purple,
    /// progress overlay — and *no* spawn (upstream creates nothing
    /// until the first `addPlayer`).
    pub fn new(actor_id: i64) -> Self {
        Self {
            actor_id,
            bound_entity: None,
            title: String::new(),
            subtitle: String::new(),
            percentage: 1.0,
            color: BossBarColor::Purple as u8,
            overlay: BossBarOverlay::Progress as u8,
            flags: 0,
            viewers: HashSet::new(),
        }
    }

    /// `BossBar::createForEntity($entity)`.
    pub fn create_for_entity(actor_id: i64, entity: impl Into<String>) -> Self {
        let mut bar = Self::new(actor_id);
        bar.bound_entity = Some(entity.into());
        bar
    }

    // ---- identity / viewers -----------------------------------------

    pub fn actor_id(&self) -> i64 {
        self.actor_id
    }

    pub fn is_viewer(&self, name: &str) -> bool {
        self.viewers.contains(&viewer_key(name))
    }

    /// Lowercase viewer names, sorted (deterministic for tests/logs).
    pub fn viewers(&self) -> Vec<String> {
        let mut viewers: Vec<String> = self.viewers.iter().cloned().collect();
        viewers.sort();
        viewers
    }

    /// `getPlayers()` (lowercase names; PHP returns `Player[]`).
    pub fn players(&self) -> Vec<String> {
        self.viewers()
    }

    pub fn viewer_count(&self) -> usize {
        self.viewers.len()
    }

    /// `addPlayer($player)` — show (idempotent).
    pub fn add_player(&mut self, name: &str) -> &mut Self {
        self.viewers.insert(viewer_key(name));
        self
    }

    /// `addPlayers($players)`.
    pub fn add_players<I>(&mut self, names: I) -> &mut Self
    where
        I: IntoIterator<Item = String>,
    {
        for name in names {
            self.add_player(&name);
        }
        self
    }

    /// `removePlayer($player)` — hide + unsubscribe.
    pub fn remove_player(&mut self, name: &str) -> &mut Self {
        self.viewers.remove(&viewer_key(name));
        self
    }

    /// `removePlayers($players)`.
    pub fn remove_players<I>(&mut self, names: I) -> &mut Self
    where
        I: IntoIterator<Item = String>,
    {
        for name in names {
            self.remove_player(&name);
        }
        self
    }

    /// `removeAllPlayers()`.
    pub fn remove_all_players(&mut self) -> &mut Self {
        self.viewers.clear();
        self
    }

    // ---- titles ------------------------------------------------------

    pub fn title(&self) -> &str {
        &self.title
    }

    /// `setTitle($title = "")` (fluent; delivery via `SessionManager`).
    pub fn set_title(&mut self, title: impl Into<String>) -> &mut Self {
        self.title = title.into();
        self
    }

    pub fn subtitle(&self) -> &str {
        &self.subtitle
    }

    /// `setSubTitle($subTitle = "")`.
    pub fn set_subtitle(&mut self, subtitle: impl Into<String>) -> &mut Self {
        self.subtitle = subtitle.into();
        self
    }

    /// `getFullTitle()`.
    pub fn full_title(&self) -> String {
        full_title(&self.title, &self.subtitle)
    }

    // ---- fill --------------------------------------------------------

    pub fn percentage(&self) -> f32 {
        self.percentage
    }

    /// `setPercentage($percentage)` (0..1, clamped).
    pub fn set_percentage(&mut self, percentage: f32) -> &mut Self {
        self.percentage = clamp_percentage(percentage);
        self
    }

    // ---- style -------------------------------------------------------

    pub fn color(&self) -> u8 {
        self.color
    }

    /// `setColor($color)` (a `BossBarColor::*` byte; stored verbatim).
    pub fn set_color(&mut self, color: u8) -> &mut Self {
        self.color = color;
        self
    }

    /// With a typed colour (Bedrock set, `0..=7`).
    pub fn set_color_typed(&mut self, color: BossBarColor) -> &mut Self {
        self.set_color(color as u8)
    }

    pub fn overlay(&self) -> u8 {
        self.overlay
    }

    /// Overlay / divider style (`BossBarOverlay`, `0..=5`).
    pub fn set_overlay(&mut self, overlay: u8) -> &mut Self {
        self.overlay = overlay;
        self
    }

    /// `setStyle(color, overlay)` — Bedrock colour + overlay.
    pub fn set_style(&mut self, color: u8, overlay: u8) -> &mut Self {
        self.color = color;
        self.overlay = overlay;
        self
    }

    /// Java-only flags (`FLAG_DARKEN_SCREEN | ...`).
    pub fn flags(&self) -> u8 {
        self.flags
    }

    pub fn set_flags(&mut self, flags: u8) -> &mut Self {
        self.flags = flags;
        self
    }

    // ---- entity binding ----------------------------------------------

    /// `getEntity()` (name when bound).
    pub fn entity(&self) -> Option<&str> {
        self.bound_entity.as_deref()
    }

    /// `setEntity($entity)` — bind the bar to an entity by name. When
    /// `bound_entity` is `None` the bar keeps its synthetic actor id
    /// (upstream `Entity::nextRuntimeId()`); bind a real session's
    /// `entity_unique_id` by passing it as `actor_id` at construction
    /// for the true boss-mob case.
    pub fn set_entity(&mut self, entity: Option<String>) -> &mut Self {
        self.bound_entity = entity;
        self
    }

    /// `resetEntity($removeEntity = false)` — unbind (upstream also
    /// closes the entity when asked; this Rust core has no entity
    /// lifecycle, so the flag only documents intent).
    pub fn reset_entity(&mut self, _remove_entity: bool) -> &mut Self {
        self.bound_entity = None;
        self
    }

    // ---- Bedrock packets (upstream `send*Packet`) ---------------------

    /// `BossEventPacket::show(actorId, fullTitle, percentage, color, overlay)`.
    /// Sent on `addPlayer` / `showTo` / `setColor` / `setEntity`.
    pub fn show_packet(&self) -> BossEventPacket {
        BossEventPacket::show(
            self.actor_id,
            self.full_title(),
            self.percentage,
            self.color,
            self.overlay,
        )
    }

    /// `BossEventPacket::hide(actorId)` — `removePlayer` / `hideFrom`.
    pub fn hide_packet(&self) -> BossEventPacket {
        BossEventPacket::hide(self.actor_id)
    }

    /// `BossEventPacket::title(actorId, fullTitle)` — `setTitle` /
    /// `setSubTitle` (`sendBossTextPacket`).
    pub fn title_packet(&self) -> BossEventPacket {
        BossEventPacket::title(self.actor_id, self.full_title())
    }

    /// `BossEventPacket::healthPercent(actorId, percentage)` —
    /// `setPercentage` (`sendBossHealthPacket`).
    pub fn health_packet(&self) -> BossEventPacket {
        BossEventPacket::health_percent(self.actor_id, self.percentage)
    }

    /// `BossEventPacket::properties(actorId, color, overlay)`.
    pub fn properties_packet(&self) -> BossEventPacket {
        BossEventPacket::properties(self.actor_id, self.color, self.overlay)
    }

    // ---- Java packets -------------------------------------------------

    /// Java `boss_bar ADD` frame for `protocol` (340/769-775).
    pub fn java_add_frame(&self, protocol: i32) -> Option<Vec<u8>> {
        java::add_frame(
            &java::bossbar_uuid(self.actor_id),
            protocol,
            &self.full_title(),
            self.percentage,
            java::bedrock_color_to_java(self.color),
            self.overlay as i32,
            self.flags,
        )
    }

    /// Java `boss_bar REMOVE` frame.
    pub fn java_remove_frame(&self, protocol: i32) -> Option<Vec<u8>> {
        java::remove_frame(&java::bossbar_uuid(self.actor_id), protocol)
    }

    pub fn java_update_health_frame(&self, protocol: i32) -> Option<Vec<u8>> {
        let id = java::boss_bar_packet_id(protocol)?;
        Some(java_bridge::wire::frame(
            id,
            &java::update_health_payload(
                &java::bossbar_uuid(self.actor_id),
                self.percentage,
            ),
        ))
    }

    pub fn java_update_title_frame(&self, protocol: i32) -> Option<Vec<u8>> {
        let id = java::boss_bar_packet_id(protocol)?;
        Some(java_bridge::wire::frame(
            id,
            &java::update_title_payload(
                &java::bossbar_uuid(self.actor_id),
                protocol,
                &self.full_title(),
            ),
        ))
    }

    pub fn java_update_style_frame(&self, protocol: i32) -> Option<Vec<u8>> {
        let id = java::boss_bar_packet_id(protocol)?;
        Some(java_bridge::wire::frame(
            id,
            &java::update_style_payload(
                &java::bossbar_uuid(self.actor_id),
                java::bedrock_color_to_java(self.color),
                self.overlay as i32,
            ),
        ))
    }

    pub fn java_update_flags_frame(&self, protocol: i32) -> Option<Vec<u8>> {
        let id = java::boss_bar_packet_id(protocol)?;
        Some(java_bridge::wire::frame(
            id,
            &java::update_flags_payload(
                &java::bossbar_uuid(self.actor_id),
                self.flags,
            ),
        ))
    }

    /// Upstream `__toString()`.
    pub fn describe(&self) -> String {
        format!(
            "BossBar ID: {}, Players: {}, Title: {:?}, Subtitle: {:?}, Percentage: {:?}, Color: {:?}",
            self.actor_id,
            self.viewers.len(),
            self.title,
            self.subtitle,
            self.percentage,
            self.color
        )
    }
}

/// Per-player overrides (upstream `DiverseBossBar`).
#[derive(Debug, Clone, PartialEq)]
pub struct DiverseBossBar {
    /// Shared defaults + viewer list.
    pub bar: BossBar,
    titles: HashMap<String, String>,
    subtitles: HashMap<String, String>,
    percentages: HashMap<String, f32>,
    colors: HashMap<String, u8>,
}

impl DiverseBossBar {
    pub fn new(actor_id: i64) -> Self {
        Self {
            bar: BossBar::new(actor_id),
            titles: HashMap::new(),
            subtitles: HashMap::new(),
            percentages: HashMap::new(),
            colors: HashMap::new(),
        }
    }

    pub fn actor_id(&self) -> i64 {
        self.bar.actor_id()
    }

    // ---- viewers (upstream clones the attribute map on add) ----------

    pub fn add_player(&mut self, name: &str) -> &mut Self {
        self.bar.add_player(name);
        self
    }

    pub fn add_players<I>(&mut self, names: I) -> &mut Self
    where
        I: IntoIterator<Item = String>,
    {
        for name in names {
            self.add_player(&name);
        }
        self
    }

    pub fn remove_player(&mut self, name: &str) -> &mut Self {
        let key = viewer_key(name);
        self.titles.remove(&key);
        self.subtitles.remove(&key);
        self.percentages.remove(&key);
        self.colors.remove(&key);
        self.bar.remove_player(name);
        self
    }

    pub fn remove_players<I>(&mut self, names: I) -> &mut Self
    where
        I: IntoIterator<Item = String>,
    {
        for name in names {
            self.remove_player(&name);
        }
        self
    }

    pub fn remove_all_players(&mut self) -> &mut Self {
        self.titles.clear();
        self.subtitles.clear();
        self.percentages.clear();
        self.colors.clear();
        self.bar.remove_all_players();
        self
    }

    pub fn viewers(&self) -> Vec<String> {
        self.bar.viewers()
    }

    pub fn is_viewer(&self, name: &str) -> bool {
        self.bar.is_viewer(name)
    }

    // ---- shared defaults (upstream `BossBar` methods) -----------------

    pub fn title(&self) -> &str {
        self.bar.title()
    }

    pub fn set_title(&mut self, title: impl Into<String>) -> &mut Self {
        self.bar.set_title(title);
        self
    }

    pub fn subtitle(&self) -> &str {
        self.bar.subtitle()
    }

    pub fn set_subtitle(&mut self, subtitle: impl Into<String>) -> &mut Self {
        self.bar.set_subtitle(subtitle);
        self
    }

    pub fn full_title(&self) -> String {
        self.bar.full_title()
    }

    pub fn percentage(&self) -> f32 {
        self.bar.percentage()
    }

    pub fn set_percentage(&mut self, percentage: f32) -> &mut Self {
        self.bar.set_percentage(percentage);
        self
    }

    pub fn color(&self) -> u8 {
        self.bar.color()
    }

    pub fn set_color(&mut self, color: u8) -> &mut Self {
        self.bar.set_color(color);
        self
    }

    pub fn set_style(&mut self, color: u8, overlay: u8) -> &mut Self {
        self.bar.set_style(color, overlay);
        self
    }

    pub fn set_overlay(&mut self, overlay: u8) -> &mut Self {
        self.bar.set_overlay(overlay);
        self
    }

    pub fn set_flags(&mut self, flags: u8) -> &mut Self {
        self.bar.set_flags(flags);
        self
    }

    pub fn set_entity(&mut self, entity: Option<String>) -> &mut Self {
        self.bar.set_entity(entity);
        self
    }

    pub fn reset_entity(&mut self, remove_entity: bool) -> &mut Self {
        self.bar.reset_entity(remove_entity);
        self
    }

    // ---- per-player overrides -----------------------------------------

    /// `resetFor($player)` (drop overrides; caller re-sends the bar).
    pub fn reset_for(&mut self, name: &str) -> &mut Self {
        let key = viewer_key(name);
        self.titles.remove(&key);
        self.subtitles.remove(&key);
        self.percentages.remove(&key);
        self.colors.remove(&key);
        self
    }

    /// `resetForAll()`.
    pub fn reset_for_all(&mut self) -> &mut Self {
        self.titles.clear();
        self.subtitles.clear();
        self.percentages.clear();
        self.colors.clear();
        self
    }

    /// `getTitleFor($player)` — override or shared default.
    pub fn title_for(&self, name: &str) -> String {
        self.titles
            .get(&viewer_key(name))
            .cloned()
            .unwrap_or_else(|| self.bar.title().to_string())
    }

    /// `setTitleFor($players, $title)`.
    pub fn set_title_for<I>(&mut self, names: I, title: impl Into<String>) -> &mut Self
    where
        I: IntoIterator<Item = String>,
    {
        let title = title.into();
        for name in names {
            self.titles.insert(viewer_key(&name), title.clone());
        }
        self
    }

    /// `getSubTitleFor($player)`.
    pub fn subtitle_for(&self, name: &str) -> String {
        self.subtitles
            .get(&viewer_key(name))
            .cloned()
            .unwrap_or_else(|| self.bar.subtitle().to_string())
    }

    /// `setSubTitleFor($players, $subTitle)`.
    pub fn set_subtitle_for<I>(&mut self, names: I, subtitle: impl Into<String>) -> &mut Self
    where
        I: IntoIterator<Item = String>,
    {
        let subtitle = subtitle.into();
        for name in names {
            self.subtitles.insert(viewer_key(&name), subtitle.clone());
        }
        self
    }

    /// `getFullTitleFor($player)`.
    ///
    /// Upstream quirk kept verbatim: the text is the *override* title
    /// (or `""`), plus the override subtitle when one is set; only
    /// when the resulting text is empty does it fall back to the
    /// shared [`Self::full_title`]. It deliberately does *not*
    /// interpolate the shared default title with an override subtitle.
    pub fn full_title_for(&self, name: &str) -> String {
        let key = viewer_key(name);
        let mut text = self.titles.get(&key).cloned().unwrap_or_default();
        if let Some(subtitle) = self.subtitles.get(&key) {
            if !subtitle.is_empty() {
                text.push_str("\n\n");
                text.push_str(subtitle);
            }
        }
        if text.is_empty() {
            text = self.full_title();
        }
        text
    }

    /// `getPercentageFor($player)`.
    pub fn percentage_for(&self, name: &str) -> f32 {
        self.percentages
            .get(&viewer_key(name))
            .copied()
            .unwrap_or_else(|| self.bar.percentage())
    }

    /// `setPercentageFor($players, $percentage)`.
    pub fn set_percentage_for<I>(&mut self, names: I, percentage: f32) -> &mut Self
    where
        I: IntoIterator<Item = String>,
    {
        let percentage = clamp_percentage(percentage);
        for name in names {
            self.percentages.insert(viewer_key(&name), percentage);
        }
        self
    }

    /// `getColorFor($player)`.
    pub fn color_for(&self, name: &str) -> u8 {
        self.colors
            .get(&viewer_key(name))
            .copied()
            .unwrap_or_else(|| self.bar.color())
    }

    /// `setColorFor($players, $color)`.
    pub fn set_color_for<I>(&mut self, names: I, color: u8) -> &mut Self
    where
        I: IntoIterator<Item = String>,
    {
        for name in names {
            self.colors.insert(viewer_key(&name), color);
        }
        self
    }

    // ---- packets -------------------------------------------------------

    /// Per-player `BossEventPacket::show` (upstream `showTo` override).
    pub fn show_packet_for(&self, name: &str) -> BossEventPacket {
        BossEventPacket::show(
            self.bar.actor_id(),
            self.full_title_for(name),
            self.percentage_for(name),
            self.color_for(name),
            self.bar.overlay(),
        )
    }

    pub fn hide_packet(&self) -> BossEventPacket {
        self.bar.hide_packet()
    }

    pub fn title_packet_for(&self, name: &str) -> BossEventPacket {
        BossEventPacket::title(self.bar.actor_id(), self.full_title_for(name))
    }

    pub fn health_packet_for(&self, name: &str) -> BossEventPacket {
        BossEventPacket::health_percent(self.bar.actor_id(), self.percentage_for(name))
    }

    pub fn java_add_frame_for(&self, name: &str, protocol: i32) -> Option<Vec<u8>> {
        java::add_frame(
            &java::bossbar_uuid(self.bar.actor_id()),
            protocol,
            &self.full_title_for(name),
            self.percentage_for(name),
            java::bedrock_color_to_java(self.color_for(name)),
            self.bar.overlay() as i32,
            self.bar.flags(),
        )
    }

    pub fn java_remove_frame(&self, protocol: i32) -> Option<Vec<u8>> {
        self.bar.java_remove_frame(protocol)
    }
}

/// Last Java state one viewer was sent for one bar, so the drain can
/// emit ADD / UPDATE_* / REMOVE deltas instead of full resends.
/// `None` entry means the client has no bar (next drain sends ADD).
#[derive(Debug, Clone, PartialEq)]
pub struct JavaSentState {
    pub title: String,
    pub percentage: f32,
    pub color: u8,
    pub overlay: u8,
    pub flags: u8,
}

/// Thread-safe registry of shared bars (upstream has no manager: plugins
/// keep their `BossBar` objects; here `SessionManager` owns them so
/// `apply_actions`, quit cleanup and Java delivery share one store).
pub struct BossBarManager {
    bars: RwLock<HashMap<i64, BossBar>>,
    diverse: RwLock<HashMap<i64, DiverseBossBar>>,
    /// Next synthetic actor id (`Entity::nextRuntimeId()`).
    next_actor_id: parking_lot::Mutex<i64>,
    /// Queued Java frames per lowercase player name.
    java_queues: RwLock<HashMap<String, std::collections::VecDeque<BossBarFrame>>>,
    /// Last-sent Java state per (lowercase viewer, boss actor id).
    java_sent: RwLock<HashMap<(String, i64), JavaSentState>>,
}

impl Default for BossBarManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BossBarManager {
    pub fn new() -> Self {
        Self {
            bars: RwLock::new(HashMap::new()),
            diverse: RwLock::new(HashMap::new()),
            // Bedrock session entity ids start at 1..N; synthetic boss
            // actors live far above them so they never collide.
            next_actor_id: parking_lot::Mutex::new(1_000_000),
            java_queues: RwLock::new(HashMap::new()),
            java_sent: RwLock::new(HashMap::new()),
        }
    }

    /// Allocate a synthetic actor id.
    pub fn next_actor_id(&self) -> i64 {
        let mut next = self.next_actor_id.lock();
        let id = *next;
        *next += 1;
        id
    }

    /// `new BossBar()` + register. Returns the actor id.
    pub fn create(&self) -> i64 {
        let id = self.next_actor_id();
        self.bars.write().insert(id, BossBar::new(id));
        id
    }

    /// `BossBar::createForEntity($entity)`: actor id is the entity's
    /// unique id (pass a Bedrock session's `entity_unique_id`).
    pub fn create_for_entity(&self, actor_id: i64, entity: impl Into<String>) -> i64 {
        self.bars
            .write()
            .insert(actor_id, BossBar::create_for_entity(actor_id, entity));
        actor_id
    }

    /// `new DiverseBossBar()` + register. Returns the actor id.
    pub fn create_diverse(&self) -> i64 {
        let id = self.next_actor_id();
        self.diverse.write().insert(id, DiverseBossBar::new(id));
        id
    }

    pub fn remove(&self, actor_id: i64) -> bool {
        self.bars.write().remove(&actor_id).is_some()
            || self.diverse.write().remove(&actor_id).is_some()
    }

    pub fn contains(&self, actor_id: i64) -> bool {
        self.bars.read().contains_key(&actor_id)
            || self.diverse.read().contains_key(&actor_id)
    }

    pub fn get(&self, actor_id: i64) -> Option<BossBar> {
        self.bars.read().get(&actor_id).cloned()
    }

    pub fn get_diverse(&self, actor_id: i64) -> Option<DiverseBossBar> {
        self.diverse.read().get(&actor_id).cloned()
    }

    pub fn all_actor_ids(&self) -> Vec<i64> {
        let mut ids: Vec<i64> = self.bars.read().keys().copied().collect();
        ids.extend(self.diverse.read().keys().copied());
        ids.sort();
        ids
    }

    /// Run `f` on the shared bar for `actor_id`; `false` when unknown.
    pub fn with_bar<R>(&self, actor_id: i64, f: impl FnOnce(&mut BossBar) -> R) -> Option<R> {
        let mut bars = self.bars.write();
        bars.get_mut(&actor_id).map(f)
    }

    /// Run `f` on the diverse bar for `actor_id`; `false` when unknown.
    pub fn with_diverse<R>(
        &self,
        actor_id: i64,
        f: impl FnOnce(&mut DiverseBossBar) -> R,
    ) -> Option<R> {
        let mut diverse = self.diverse.write();
        diverse.get_mut(&actor_id).map(f)
    }

    /// Queue a Java frame for `name` (drained by `take_java_frames`).
    pub fn queue_java_frame(&self, name: &str, frame: Vec<u8>) {
        self.java_queues
            .write()
            .entry(viewer_key(name))
            .or_default()
            .push_back(java_frame(frame));
    }

    /// Drain queued Java frames for `name` (`take_bossbar_frames`).
    pub fn take_java_frames(&self, name: &str) -> Vec<BossBarFrame> {
        let key = viewer_key(name);
        self.java_queues
            .write()
            .get_mut(&key)
            .map(|queue| queue.drain(..).collect())
            .unwrap_or_default()
    }

    /// Queued Java frame count for `name` (tests/diagnostics).
    pub fn queued_java_frames(&self, name: &str) -> usize {
        self.java_queues
            .read()
            .get(&viewer_key(name))
            .map(|queue| queue.len())
            .unwrap_or(0)
    }

    /// Replace/insert a shared bar (`bossbar_apply_data`).
    pub fn shared_insert(&self, actor_id: i64, bar: BossBar) {
        self.bars.write().insert(actor_id, bar);
    }

    /// Remove a shared bar without hiding (caller queued the hides).
    pub fn shared_remove_quiet(&self, actor_id: i64) -> bool {
        self.bars.write().remove(&actor_id).is_some()
    }

    /// Replace/insert a diverse bar (`bossbar_apply_data`).
    pub fn diverse_insert(&self, actor_id: i64, bar: DiverseBossBar) {
        self.diverse.write().insert(actor_id, bar);
    }

    /// Remove a diverse bar without hiding (caller queued the hides).
    pub fn diverse_remove_quiet(&self, actor_id: i64) -> bool {
        self.diverse.write().remove(&actor_id).is_some()
    }

    /// Last-sent Java state for `(viewer, boss)` (drain diff).
    pub fn java_sent_get(&self, viewer: &str, boss_id: i64) -> Option<JavaSentState> {
        self.java_sent
            .read()
            .get(&(viewer_key(viewer), boss_id))
            .cloned()
    }

    /// Remember the state the Java client was just sent for a bar.
    pub fn java_sent_set(&self, viewer: &str, boss_id: i64, state: JavaSentState) {
        self.java_sent
            .write()
            .insert((viewer_key(viewer), boss_id), state);
    }

    /// Forget the last-sent state (client has no bar any more).
    pub fn java_sent_remove(&self, viewer: &str, boss_id: i64) {
        self.java_sent
            .write()
            .remove(&(viewer_key(viewer), boss_id));
    }

    /// All boss ids one viewer was sent a bar for (REMOVE diff).
    pub fn java_sent_ids(&self, viewer: &str) -> Vec<i64> {
        let key = viewer_key(viewer);
        self.java_sent
            .read()
            .keys()
            .filter(|(name, _)| name == &key)
            .map(|(_, boss_id)| *boss_id)
            .collect()
    }

    /// Drop every last-sent entry for a viewer (quit cleanup).
    pub fn java_sent_clear(&self, viewer: &str) {
        let key = viewer_key(viewer);
        self.java_sent.write().retain(|(name, _), _| name != &key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bedrock_protocol::Packet;

    #[test]
    fn defaults_match_upstream_constructor() {
        let bar = BossBar::new(42);
        assert_eq!(bar.title(), "");
        assert_eq!(bar.subtitle(), "");
        assert_eq!(bar.full_title(), "");
        assert_eq!(bar.percentage(), 1.0);
        assert_eq!(bar.color(), BossBarColor::Purple as u8);
        assert_eq!(bar.overlay(), 0);
        assert!(bar.viewers().is_empty());
    }

    #[test]
    fn fluent_setters_and_full_title() {
        let mut bar = BossBar::new(1);
        bar.set_title("Hello world!")
            .set_subtitle("Foo Bar")
            .set_percentage(0.5)
            .add_player("Steve");
        assert_eq!(bar.full_title(), "Hello world!\n\nFoo Bar");
        assert_eq!(bar.percentage(), 0.5);
        assert_eq!(bar.viewers(), vec!["steve".to_string()]);
        assert!(bar.is_viewer("STEVE"));
    }

    #[test]
    fn percentage_clamps_and_viewer_mutation() {
        let mut bar = BossBar::new(1);
        bar.set_percentage(9.0);
        assert_eq!(bar.percentage(), 1.0);
        bar.set_percentage(-1.0);
        assert_eq!(bar.percentage(), 0.0);
        bar.add_players(["Steve".to_string(), "Alex".to_string()]);
        assert_eq!(bar.viewer_count(), 2);
        bar.remove_player("Steve");
        assert_eq!(bar.viewers(), vec!["alex".to_string()]);
        bar.remove_all_players();
        assert!(bar.viewers().is_empty());
    }

    #[test]
    fn show_packet_is_boss_event_show() {
        let mut bar = BossBar::new(42);
        bar.set_title("T").set_subtitle("S").set_percentage(0.5);
        bar.set_style(BossBarColor::Red as u8, BossBarOverlay::Ten as u8);
        let packet = bar.show_packet();
        assert_eq!(packet.packet_id(), 0x4A);
        assert_eq!(packet.event_type, BossEventPacket::TYPE_SHOW);
        assert_eq!(packet.boss_actor_unique_id, 42);
        assert_eq!(packet.title, "T\n\nS");
        assert_eq!(packet.health_percent, 0.5);
        assert_eq!(packet.color, BossBarColor::Red as u8);
        assert_eq!(packet.overlay, BossBarOverlay::Ten as u8);
        assert_eq!(bar.hide_packet().event_type, BossEventPacket::TYPE_HIDE);
        assert_eq!(
            bar.title_packet().event_type,
            BossEventPacket::TYPE_TITLE
        );
        assert_eq!(
            bar.health_packet().event_type,
            BossEventPacket::TYPE_HEALTH_PERCENT
        );
    }

    #[test]
    fn diverse_falls_back_to_shared_defaults() {
        let mut bar = DiverseBossBar::new(1);
        bar.set_title("Default").set_percentage(1.0);
        bar.add_player("Steve");
        assert_eq!(bar.title_for("Steve"), "Default");
        assert_eq!(bar.percentage_for("Steve"), 1.0);
        bar.set_title_for(["Steve".to_string()], "Steve's bar");
        assert_eq!(bar.title_for("Steve"), "Steve's bar");
        assert_eq!(bar.title_for("Alex"), "Default");
        bar.reset_for("Steve");
        assert_eq!(bar.title_for("Steve"), "Default");
        bar.reset_for_all();
        assert_eq!(bar.title_for("Steve"), "Default");
    }

    #[test]
    fn diverse_full_title_for_keeps_upstream_quirk() {
        // Upstream: only the override title/subtitle count; empty
        // override -> shared full title.
        let mut bar = DiverseBossBar::new(1);
        bar.set_title("Default").set_subtitle("Sub");
        bar.add_player("Steve");
        bar.set_title_for(["Steve".to_string()], "T");
        bar.set_subtitle_for(["Steve".to_string()], "ST");
        assert_eq!(bar.full_title_for("Steve"), "T\n\nST");
        assert_eq!(bar.full_title_for("Alex"), "Default\n\nSub");
    }

    #[test]
    fn manager_allocates_unique_ids_and_queues_java() {
        let manager = BossBarManager::new();
        let first = manager.create();
        let second = manager.create();
        assert_ne!(first, second);
        assert!(manager.contains(first));
        assert!(!manager.contains(9_999_999));
        manager.queue_java_frame("Steve", vec![1, 2, 3]);
        assert_eq!(manager.queued_java_frames("steve"), 1);
        let drained = manager.take_java_frames("Steve");
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].frame, vec![1, 2, 3]);
        assert!(manager.take_java_frames("Steve").is_empty());
        assert!(manager.remove(first));
        assert!(!manager.contains(first));
    }
}
