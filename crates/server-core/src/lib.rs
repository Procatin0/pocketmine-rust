//! PocketMine-Rust Server Core: Session and Player Management

pub mod access;
pub mod anvil;
pub mod auth_keys;
pub mod block_container;
pub mod block_info;
pub mod bossbar;
pub mod chat;
pub mod chunk_pipeline;
pub mod crafting;
pub mod command;
pub mod entity;
pub mod entity_types;
pub mod fluid;
pub mod config;
pub mod crash;
pub mod identity;
pub mod inventory;
pub mod java_hooks;
pub mod java_sync;
pub mod legacy_meta;
pub mod logging;
pub mod metrics;
pub mod nbt_json;
pub mod packets_wire;
pub mod permission;
pub mod physics;
pub mod plugin_host;
pub mod profiler;
pub mod query;
pub mod random_tick;
pub mod redstone;
pub mod scoreboard;
pub mod session;
pub mod static_packets;
pub mod translations;
pub mod updater;
pub mod world_state;

pub use anvil::{AnvilInventory, AnvilState};
pub use block_container::BlockContainer;
pub use config::ServerConfig;
pub use crafting::{
    CraftingGrid, CraftingInventory, CraftingManager, CraftingTableInventory,
    PlayerCraftingInventory, RecipeIngredient, ShapedRecipe, ShapelessRecipe,
};
pub use entity::{
    Aabb, DamageOutcome, Entity, EntityKind, EntitySize, Mob, MobAi, Projectile, ProjectileKind,
};
pub use entity_types::{EntityType, ENTITY_TYPES};
pub use identity::{parse_login_identity, LoginIdentity};
pub use bossbar::{BossBar, BossBarManager, DiverseBossBar};
pub use physics::{
    calculate_fall_damage, is_fallable, is_on_ground, is_supported, support_type, LandingEffect,
    SupportType,
};
pub use plugin_host::{PlayerRegistry, ServerPluginHost};
pub use scoreboard::{
    ColorRange, PacketBytes as ScoreboardPacket, Scoreboard, ScoreboardFile, ScoreboardLine,
    ScoreboardObjective, ScoreboardSlot, SCOREBOARD_FILE_VERSION,
};
pub use session::{
    ActorSound, OpenContainer, OutgoingPacket, PlayerSession, SessionError, SessionManager,
    SessionState, TickReport,
};
pub use world_state::WorldState;
/// Geyser/Floodgate-style display-name helpers (Bedrock -> Java is
/// `"." + raw`, Java -> Bedrock is verbatim). Re-exported so plugins/tests
/// can render `"<nombre> msg"` / `"Nombre: ..."` lines per viewer.
pub use java_bridge::display::{
    bedrock_view_display_name, chat_line, display_name, java_view_display_name,
    prefix_bedrock_names_in_text, BEDROCK_PREFIX_IN_JAVA,
};
