//! `BossEventPacket` (0x4A / 74): Bedrock boss bars.
//!
//! Byte-for-byte port of PocketMine `pmmp/protocol/src/BossEventPacket.php`
//! ( BedrockProtocol, protocol 1.26 / 2225 — see `data/protocol_info.json`
//! `"BossEventPacket": 74` = `0x4A`):
//!
//! ```text
//! bossActorUniqueId : VarInt64 (zigzag signed, `CommonTypes::putActorUniqueId`)
//! playerActorUniqueId: VarInt64 (zigzag signed, usually 0 server -> client)
//! eventType          : unsigned byte
//! title              : string (VarUInt32 len + bytes)
//! filteredTitle      : string
//! healthPercent      : f32 little-endian (0..1)
//! color              : unsigned byte (see [`BossBarColor`])
//! overlay            : unsigned byte (see [`BossBarOverlay`])
//! ```
//!
//! The packet is both clientbound and serverbound: the client only ever
//! sends `TYPE_REGISTER_PLAYER` (1) / `TYPE_UNREGISTER_PLAYER` (3) /
//! `TYPE_QUERY` (8) — `PacketListener` in apiBossBar logs those and kicks
//! on anything else. The server sends `TYPE_SHOW` (0), `TYPE_HIDE` (2),
//! `TYPE_HEALTH_PERCENT` (4), `TYPE_TITLE` (5), `TYPE_PROPERTIES` (6).
//!
//! Colours/overlays come from the resource-pack definitions file and (as
//! apiBossBar's README notes, `#blamemojang`) the client may ignore them —
//! they are still encoded verbatim so a resource pack can implement them.

use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID `0x4A` (74): `BossEventPacket` in `data/protocol_info.json`.
pub const BOSS_EVENT_PACKET_ID: u32 = 0x4A;

/// Bedrock boss-bar colours (`types/BossBarColor.php`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum BossBarColor {
    Pink = 0,
    Blue = 1,
    Red = 2,
    Green = 3,
    Yellow = 4,
    Purple = 5,
    RebeccaPurple = 6,
    White = 7,
}

impl BossBarColor {
    /// `PINK..=WHITE` (0..=7), `None` outside.
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Pink,
            1 => Self::Blue,
            2 => Self::Red,
            3 => Self::Green,
            4 => Self::Yellow,
            5 => Self::Purple,
            6 => Self::RebeccaPurple,
            7 => Self::White,
            _ => return None,
        })
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Bedrock boss-bar overlays / dividers (progress style). The values are
/// the Java `dividers` order shared with the overlay texture names
/// (`progress`, `10`, `20`, `6`, `4`, `3` segments).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum BossBarOverlay {
    Progress = 0,
    Ten = 1,
    Twenty = 2,
    Six = 3,
    Four = 4,
    Three = 5,
}

impl BossBarOverlay {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Progress,
            1 => Self::Ten,
            2 => Self::Twenty,
            3 => Self::Six,
            4 => Self::Four,
            5 => Self::Three,
            _ => return None,
        })
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// `BossEventPacket` (`0x4A`).
#[derive(Debug, Clone, PartialEq)]
pub struct BossEventPacket {
    /// Unique id of the boss actor the bar is bound to (`actorId` in
    /// apiBossBar; a synthetic `Entity::nextRuntimeId()` unless
    /// `BossBar::setEntity()` bound a real entity).
    pub boss_actor_unique_id: i64,
    /// Unique id of the player actor for register/unregister/query
    /// (0 for all server -> client events).
    pub player_actor_unique_id: i64,
    /// One of the `TYPE_*` constants.
    pub event_type: u8,
    pub title: String,
    pub filtered_title: String,
    /// Fill fraction `0..1` (`setPercentage` in apiBossBar).
    pub health_percent: f32,
    pub color: u8,
    pub overlay: u8,
}

impl BossEventPacket {
    /// S2C: shows the boss bar to the player.
    pub const TYPE_SHOW: u8 = 0;
    /// C2S: registers a player to a boss fight.
    pub const TYPE_REGISTER_PLAYER: u8 = 1;
    /// S2C: removes the boss bar from the client (`hide` / `removePlayer`).
    pub const TYPE_HIDE: u8 = 2;
    /// C2S: unregisters a player from a boss fight.
    pub const TYPE_UNREGISTER_PLAYER: u8 = 3;
    /// S2C: sets the bar percentage (`setPercentage`).
    pub const TYPE_HEALTH_PERCENT: u8 = 4;
    /// S2C: sets the title of the bar (`setTitle` / `setSubTitle`).
    pub const TYPE_TITLE: u8 = 5;
    /// S2C: updates misc properties (colour + overlay).
    pub const TYPE_PROPERTIES: u8 = 6;
    /// Alias of [`TYPE_PROPERTIES`] in older PM naming (colour/overlay
    /// texture update; apiBossBar never sends it — `setColor` re-sends
    /// `TYPE_SHOW` — but the wire constant is kept for parity).
    pub const TYPE_TEXTURE: u8 = Self::TYPE_PROPERTIES;
    /// C2S: client asking the server to resend all boss data.
    pub const TYPE_QUERY: u8 = 8;

    fn base(boss_actor_unique_id: i64, event_type: u8) -> Self {
        Self {
            boss_actor_unique_id,
            player_actor_unique_id: 0,
            event_type,
            title: String::new(),
            filtered_title: String::new(),
            health_percent: 0.0,
            color: BossBarColor::Purple as u8,
            overlay: BossBarOverlay::Progress as u8,
        }
    }

    /// `BossEventPacket::show($bossId, $title, $healthPercent, $color, $overlay)`
    /// — the apiBossBar `sendBossPacket()` payload (`addPlayer` / `showTo` /
    /// `setColor` / `setEntity` all funnel here).
    pub fn show(
        boss_actor_unique_id: i64,
        title: impl Into<String>,
        health_percent: f32,
        color: u8,
        overlay: u8,
    ) -> Self {
        let title = title.into();
        let mut packet = Self::base(boss_actor_unique_id, Self::TYPE_SHOW);
        packet.filtered_title = title.clone();
        packet.title = title;
        packet.health_percent = health_percent.clamp(0.0, 1.0);
        packet.color = color;
        packet.overlay = overlay;
        packet
    }

    /// `BossEventPacket::hide($bossId)` — `removePlayer` / `hideFrom`.
    pub fn hide(boss_actor_unique_id: i64) -> Self {
        Self::base(boss_actor_unique_id, Self::TYPE_HIDE)
    }

    /// `BossEventPacket::registerPlayer($bossId, $playerId)` (C2S).
    pub fn register_player(boss_actor_unique_id: i64, player_actor_unique_id: i64) -> Self {
        let mut packet = Self::base(boss_actor_unique_id, Self::TYPE_REGISTER_PLAYER);
        packet.player_actor_unique_id = player_actor_unique_id;
        packet
    }

    /// `BossEventPacket::unregisterPlayer($bossId, $playerId)` (C2S).
    pub fn unregister_player(boss_actor_unique_id: i64, player_actor_unique_id: i64) -> Self {
        let mut packet = Self::base(boss_actor_unique_id, Self::TYPE_UNREGISTER_PLAYER);
        packet.player_actor_unique_id = player_actor_unique_id;
        packet
    }

    /// `BossEventPacket::healthPercent($bossId, $healthPercent)`
    /// (`setPercentage` / `setPercentageFor` / `sendBossHealthPacket`).
    pub fn health_percent(boss_actor_unique_id: i64, health_percent: f32) -> Self {
        let mut packet = Self::base(boss_actor_unique_id, Self::TYPE_HEALTH_PERCENT);
        packet.health_percent = health_percent.clamp(0.0, 1.0);
        packet
    }

    /// `BossEventPacket::title($bossId, $title)` (`setTitle` / `setSubTitle` /
    /// `sendBossTextPacket`). `filteredTitle` mirrors `title`, exactly like
    /// the PM factory (and apiBossBar's combined full title).
    pub fn title(boss_actor_unique_id: i64, title: impl Into<String>) -> Self {
        let title = title.into();
        let mut packet = Self::base(boss_actor_unique_id, Self::TYPE_TITLE);
        packet.filtered_title = title.clone();
        packet.title = title;
        packet
    }

    /// `BossEventPacket::properties($bossId, $color, $overlay)`.
    pub fn properties(boss_actor_unique_id: i64, color: u8, overlay: u8) -> Self {
        let mut packet = Self::base(boss_actor_unique_id, Self::TYPE_PROPERTIES);
        packet.color = color;
        packet.overlay = overlay;
        packet
    }

    /// `BossEventPacket::query($bossId, $playerId)` (C2S resend request).
    pub fn query(boss_actor_unique_id: i64, player_actor_unique_id: i64) -> Self {
        let mut packet = Self::base(boss_actor_unique_id, Self::TYPE_QUERY);
        packet.player_actor_unique_id = player_actor_unique_id;
        packet
    }
}

impl Packet for BossEventPacket {
    fn packet_id(&self) -> u32 {
        BOSS_EVENT_PACKET_ID
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        // `CommonTypes::putActorUniqueId` = `VarInt::writeSignedLong` (zigzag
        // var_i64). `Writer::write_byte` for the three unsigned bytes,
        // `LE::writeFloat` for the health percent.
        writer.write_var_i64(self.boss_actor_unique_id);
        writer.write_var_i64(self.player_actor_unique_id);
        writer.write_byte(self.event_type);
        writer.write_string(&self.title);
        writer.write_string(&self.filtered_title);
        writer.write_f32_le(self.health_percent);
        writer.write_byte(self.color);
        writer.write_byte(self.overlay);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            boss_actor_unique_id: reader.read_var_i64()?,
            player_actor_unique_id: reader.read_var_i64()?,
            event_type: reader.read_byte()?,
            title: reader.read_string()?,
            filtered_title: reader.read_string()?,
            health_percent: reader.read_f32_le()?,
            color: reader.read_byte()?,
            overlay: reader.read_byte()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(packet: &BossEventPacket) {
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = BossEventPacket::decode(&mut reader).expect("decode");
        assert_eq!(&decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn packet_id_is_0x4a() {
        assert_eq!(BOSS_EVENT_PACKET_ID, 0x4A);
        assert_eq!(BossEventPacket::hide(1).packet_id(), 0x4A);
        // 74 in data/protocol_info.json.
        assert_eq!(BOSS_EVENT_PACKET_ID, 74);
    }

    #[test]
    fn show_roundtrips_and_fills_defaults_like_pm() {
        let packet = BossEventPacket::show(
            42,
            "Hello world!",
            0.5,
            BossBarColor::Red as u8,
            BossBarOverlay::Progress as u8,
        );
        assert_eq!(packet.event_type, BossEventPacket::TYPE_SHOW);
        assert_eq!(packet.title, "Hello world!");
        assert_eq!(packet.filtered_title, "Hello world!");
        assert_eq!(packet.health_percent, 0.5);
        roundtrip(&packet);
    }

    #[test]
    fn every_constructor_roundtrips() {
        for packet in [
            BossEventPacket::hide(7),
            BossEventPacket::register_player(7, 99),
            BossEventPacket::unregister_player(7, 99),
            BossEventPacket::health_percent(7, 0.25),
            BossEventPacket::title(7, "Title\n\nSub"),
            BossEventPacket::properties(
                7,
                BossBarColor::White as u8,
                BossBarOverlay::Ten as u8,
            ),
            BossEventPacket::query(7, 99),
        ] {
            roundtrip(&packet);
        }
    }

    #[test]
    fn health_is_clamped_like_set_percentage() {
        assert_eq!(BossEventPacket::health_percent(1, 1.5).health_percent, 1.0);
        assert_eq!(BossEventPacket::health_percent(1, -0.5).health_percent, 0.0);
        assert_eq!(
            BossEventPacket::show(1, "x", 2.0, 0, 0).health_percent,
            1.0
        );
    }

    #[test]
    fn color_overlay_enums_cover_pm_range() {
        assert_eq!(BossBarColor::from_u8(0), Some(BossBarColor::Pink));
        assert_eq!(BossBarColor::from_u8(6), Some(BossBarColor::RebeccaPurple));
        assert_eq!(BossBarColor::from_u8(7), Some(BossBarColor::White));
        assert_eq!(BossBarColor::from_u8(8), None);
        assert_eq!(BossBarOverlay::from_u8(5), Some(BossBarOverlay::Three));
        assert_eq!(BossBarOverlay::from_u8(6), None);
        // PM defaults: purple bar, progress overlay (apiBossBar constructor).
        assert_eq!(BossEventPacket::hide(1).color, BossBarColor::Purple as u8);
        assert_eq!(BossEventPacket::hide(1).overlay, 0);
    }
}
