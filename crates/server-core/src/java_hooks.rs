//! Production [`ServerHooks`](java_bridge::ServerHooks) implementation.
//!
//! The Java listener (`java-bridge`'s `JavaServer`) owns frames, ids and
//! tick pacing; everything stateful is delegated here to the canonical
//! store: [`SessionManager`] (sessions + [`PlayerRegistry`]) and its
//! [`WorldState`]. This replaces the PHP `plugins/JavaBridge` server:
//! chat routes through [`crate::chat::handle_java_chat`], chunk bytes are
//! produced by [`bedrock_world::java_chunk`] over the shared chunk column,
//! and block edits enqueue in the same `WorldState` queues that already
//! feed Bedrock (`poll_block_changes`) — so one edit reaches both
//! platforms without any new IPC.

use std::net::SocketAddr;
use std::sync::Arc;

use bedrock_world::block_id;
use bedrock_world::java_chunk::{ChunkBuilder, ModernChunk, ModernProfile};
use java_bridge::{
    BlockChange, ChatOutcome, LegacyChunk, LoadedData, ModernChunkData, PlayerSnapshot,
    ServerHooks,
};
use php_plugin_bridge::{PlayerContext, PlayerJoinEvent, PlayerQuitEvent};

use crate::session::SessionManager;

/// `ServerHooks` backed by the live server core.
pub struct CoreHooks {
    manager: Arc<SessionManager>,
    motd: String,
    view_distance: i32,
    default_gamemode: i32,
    level_type: String,
}

impl CoreHooks {
    pub fn new(
        manager: Arc<SessionManager>,
        motd: String,
        view_distance: i32,
        default_gamemode: i32,
        level_type: String,
    ) -> Self {
        Self {
            manager,
            motd,
            view_distance,
            default_gamemode,
            level_type,
        }
    }
}

impl ServerHooks for CoreHooks {
    fn log(&self, message: String) {
        tracing::info!("Java bridge: {message}");
    }

    fn view_distance(&self) -> i32 {
        self.view_distance
    }

    fn spawn(&self) -> (f64, f64, f64) {
        let (x, y, z) = self.manager.world().spawn();
        (x as f64, y as f64, z as f64)
    }

    fn default_gamemode(&self) -> i32 {
        self.default_gamemode
    }

    fn difficulty(&self) -> i32 {
        self.manager.world().difficulty()
    }

    fn level_type(&self) -> String {
        self.level_type.clone()
    }

    fn motd(&self) -> String {
        self.motd.clone()
    }

    fn max_players(&self) -> i32 {
        self.manager.max_players()
    }

    fn world_time(&self) -> i64 {
        i64::from(self.manager.world().time())
    }

    fn world_time_running(&self) -> bool {
        self.manager.world().is_time_running()
    }

    fn load_player_data(&self, name: &str) -> Option<LoadedData> {
        // Same canonical `players/<name>.json` (JSON v2, 26 fields) that
        // `PlayerSession::save_player_data` writes.
        let path = std::path::Path::new("players")
            .join(format!("{}.json", name.to_lowercase()));
        let bytes = std::fs::read(path).ok()?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        let number = |key: &str| value.get(key).and_then(serde_json::Value::as_f64);
        let (x, y, z) = (number("x")?, number("y")?, number("z")?);
        value_get(&value, "gamemode").map(|gamemode| LoadedData {
            x,
            y,
            z,
            yaw: number("yaw").unwrap_or(0.0),
            pitch: number("pitch").unwrap_or(0.0),
            gamemode: gamemode as i32,
            health: number("health").unwrap_or(20.0) as f32,
        })
    }

    fn is_name_available(&self, name: &str, _except_session: u64) -> bool {
        // A live Bedrock entry wins; Java-vs-Java duplicates are rejected
        // by the listener's own session registry before this hook.
        !matches!(
            self.manager.registry().get(name),
            Some(player) if !player.is_virtual
        )
    }

    fn snapshots(&self) -> Vec<PlayerSnapshot> {
        self.manager
            .registry()
            .all()
            .into_iter()
            .map(|player| PlayerSnapshot {
                name: player.name.clone(),
                x: player.x,
                y: player.y,
                z: player.z,
                yaw: player.yaw,
                pitch: player.pitch,
                gamemode: player.gamemode as i32,
                ping: player.ping as i32,
                is_virtual: player.is_virtual,
            })
            .collect()
    }

    fn join_api(&self, _session_id: u64, peer: SocketAddr, snapshot: &PlayerSnapshot) {
        let mut player = PlayerContext::new(snapshot.name.clone(), peer);
        player.x = snapshot.x;
        player.y = snapshot.y;
        player.z = snapshot.z;
        player.yaw = snapshot.yaw;
        player.pitch = snapshot.pitch;
        player.gamemode = snapshot.gamemode.max(0) as u32;
        player.is_virtual = true;
        self.manager.registry().upsert(player.clone());
        // Let Bedrock viewers see the join immediately (spawn packet).
        self.manager.poll_virtual_players();
        if let Some(plugins) = self.manager.plugin_manager() {
            let mut event = PlayerJoinEvent::new(player, format!("{} joined the game", snapshot.name));
            match plugins.dispatch_event_sync(&mut event) {
                Ok(actions) => {
                    self.manager.apply_actions(&actions, 0);
                }
                Err(error) => tracing::warn!("Java PlayerJoinEvent failed: {error}"),
            }
        }
    }

    fn leave_api(&self, name: &str) {
        let player = self.manager.registry().get(name);
        self.manager.registry().remove_if_virtual(name);
        if let Some(player) = player {
            if let Some(plugins) = self.manager.plugin_manager() {
                let mut event =
                    PlayerQuitEvent::new(player, format!("{} left the game", name), "left");
                if let Err(error) = plugins.dispatch_event_sync(&mut event) {
                    tracing::warn!("Java PlayerQuitEvent failed: {error}");
                }
            }
        }
        self.manager.poll_virtual_players();
    }

    fn update_virtual_position(
        &self,
        name: &str,
        x: f64,
        y: f64,
        z: f64,
        yaw: f64,
        pitch: f64,
    ) {
        if let Some(mut player) = self.manager.registry().get(name) {
            player.apply_position(x, y, z, yaw, pitch);
            self.manager.registry().upsert(player);
            self.manager.poll_virtual_players();
        }
    }

    fn take_plugin_messages(&self, name: &str) -> Vec<String> {
        self.manager.registry().take_messages(name)
    }

    fn handle_chat(&self, _session_id: u64, name: &str, message: &str) -> ChatOutcome {
        let sender = self.manager.registry().get(name).unwrap_or_else(|| {
            PlayerContext::new(
                name,
                "0.0.0.0:0".parse().unwrap_or(SocketAddr::from(([0, 0, 0, 0], 0))),
            )
        });
        let outcome = crate::chat::handle_java_chat(&self.manager, &sender, message);
        ChatOutcome {
            lines: outcome.feedback,
            teleport: None,
            gamemode: None,
        }
    }

    fn break_block(&self, x: i32, y: i32, z: i32) -> bool {
        self.manager.world().set_block(x, y, z, block_id::AIR)
    }

    fn place_block(&self, x: i32, y: i32, z: i32, network_id: u32) -> bool {
        self.manager.world().set_block(x, y, z, network_id)
    }

    fn block_network_id(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        Some(self.manager.world().block_at(x, y, z))
    }

    fn stone_network_id(&self) -> u32 {
        block_id::STONE
    }

    fn modern_block_state(&self, network_id: u32, protocol: i32) -> i32 {
        match ModernProfile::for_protocol(protocol) {
            Some(profile) => ModernChunk::bedrock_state(network_id, &profile),
            None => {
                if network_id == block_id::AIR {
                    0
                } else {
                    1
                }
            }
        }
    }

    fn legacy_block_state(&self, network_id: u32) -> i32 {
        ChunkBuilder::java_state(network_id)
    }

    fn legacy_chunk(&self, cx: i32, cz: i32) -> Option<LegacyChunk> {
        let chunk = self.manager.world().get_chunk(cx, cz);
        let (bitmap, data) = ChunkBuilder::from_chunk(&chunk);
        Some(LegacyChunk {
            bitmap: bitmap as i32,
            data,
        })
    }

    fn modern_chunk(&self, protocol: i32, cx: i32, cz: i32) -> Option<ModernChunkData> {
        let profile = ModernProfile::for_protocol(protocol)?;
        let chunk = self.manager.world().get_chunk(cx, cz);
        let data = ModernChunk::from_chunk(&chunk, &profile);
        let heights = ModernChunk::heights_from_chunk(&chunk);
        Some(ModernChunkData {
            heightmaps: ModernChunk::heightmaps(&heights, &profile),
            light: ModernChunk::light(),
            data,
        })
    }

    fn drain_block_changes(&self) -> Vec<BlockChange> {
        // Fase 1 queue (Bedrock -> Java); the Bedrock-facing duplicate
        // (`take_broadcast_changes`) is drained separately by
        // `poll_block_changes`, so draining here starves nobody.
        self.manager
            .world()
            .take_changes()
            .into_iter()
            .map(|change| BlockChange {
                x: change.x,
                y: change.y,
                z: change.z,
                network_id: Some(change.block_id),
                kind: ModernChunk::block_name(change.block_id)
                    .unwrap_or("block")
                    .to_string(),
            })
            .collect()
    }
}

fn value_get(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(serde_json::Value::as_i64)
}
