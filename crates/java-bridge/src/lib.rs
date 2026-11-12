//! Java Edition TCP listener for PocketMine-Rust.
//!
//! Port of `plugins/JavaBridge/src/javabridge/JavaServer.php` +
//! `JavaClient.php`: Tokio listener on TCP 25565, handshake / status /
//! login / configuration / play for legacy 1.12.2 (340) and modern
//! 1.21.4-26.1 (769-775), keep-alive, budgeted chunk streaming,
//! plugin-message delivery, and entity spawn/sync for Java observers.
//!
//! Ownership note: `Protocol.php` wire primitives live in [`wire`],
//! the modern packet-id table in [`profile`], but chunk *contents*
//! (`ChunkBuilder` / `ModernChunk`) belong to other agents and arrive
//! through [`hooks::ServerHooks`]. Likewise command/chat dispatch and
//! the player registry stay in `server-core`; this crate only frames
//! what they decide.
//!
//! ```
//! // Wiring (production, in server-core main.rs):
//! // let hooks: Arc<dyn ServerHooks> = Arc::new(CoreHooks { ... });
//! // let server = Arc::new(JavaServer::new(hooks));
//! // tokio::spawn(server.run_arc("0.0.0.0:25565")); // accept/flush only
//! // // central 20 TPS loop: session_manager.tick(); server.tick_at(tick);
//! ```

pub mod bossbar;
pub mod display;
pub mod hooks;
pub mod profile;
pub mod server;
pub mod session;
pub mod wire;

pub use hooks::{
    BlockChange, BossBarFrame, ChatOutcome, LegacyChunk, LoadedData, ModernChunkData,
    PlayerSnapshot, ScoreboardFrame, ServerHooks,
};
pub use server::JavaServer;
pub use session::{EntityTarget, JavaSession, SessionState, TabTarget};
