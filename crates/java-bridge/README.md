# java-bridge

Rust port of the Java Edition TCP listener:
`plugins/JavaBridge/src/javabridge/JavaServer.php` + `JavaClient.php`
(Tokio, TCP 25565 by default).

## Mapping PHP -> Rust

| PHP | Rust |
| --- | ---- |
| `JavaServer` (socket, `tick()`, caches, entity ids, TAB/time/entity/block phases) | [`server::JavaServer`] (`run_arc` accept loop + 50 ms tick loop, `tick()` gating) |
| `JavaClient` (one TCP connection, state machine, `poll`/`tick`) | [`session::JavaSession`] (socket-free: `poll_bytes` in, `take_out` out) |
| `PlayerSession` (Bedrock session in `server-core`) | `JavaSession` mirrors its fields (identity, position, gamemode, `sent_chunks`, viewers/seen entities, tick counters, pending output) |
| `Protocol.php` primitives | [`wire`] (VarInt/VarLong, strings, packed position, `lpVec3`, yaw conversion, frames, offline UUID, minimal NBT text) |
| `ModernProfile.php` packet-id table | [`profile`] (769/770/773/774/775 ids, `player_entity_type`, `spawn_velocity_is_lp_vec3`, brand) |
| `Bridge::hostCall(...)` (`player.register`, `player.load`, `player.get`, `player.update_position`, `player.take_messages`, `world.get_chunk`, `server.get_players_full`, `world.take_block_changes`, ...) | [`hooks::ServerHooks`] trait — production impl in `server-core` delegates to `SessionManager` / `PlayerRegistry` / `WorldState` |
| `sync_virtuals_locked` / `broadcast_add_player` (Bedrock viewers see virtuals) | mirrored in reverse: [`server::entity_targets_for`] + `JavaSession::sync_entities` (Java observers see Bedrock + virtual snapshots, chunk-gated, Bedrock snapshot wins on name collision) |
| `deliverPluginMessages` (`player.take_messages`) | `JavaSession::deliver_plugin_messages` via `ServerHooks::take_plugin_messages`, every 5 ticks |
| `streamChunks` budget (12 ms, 64/batch, 256 KiB backpressure, 20-attempt rotation, `chunk_batch_received` ceiling) | `JavaSession::stream_chunks` |
| keep-alive 10 s / timeout 30 s | `JavaSession::keep_alive` / `tick` |
| entity spawn via `lpVec3` (773+), `vec3i16` (769/770) | `modern_spawn_payload` |
| `entity_metadata` Optional component (type 6 + presence byte) | `modern_nametag_payload` |
| `player_info_update` gamemode bitflag `0x04` | `modern_player_info_gamemode(_batch)` |
| tags `0x0d`, brand `0x01` in configuration | `begin_configuration` via `profile::packet_id` |

## Out of scope (other agents)

- `Protocol.php` chunk/state translation and `ModernChunk.php` column
  bytes: supplied through `ServerHooks::legacy_chunk` /
  `modern_chunk` / `modern_block_state` / `legacy_block_state`, so the
  `Protocol` / `ModernChunk` crates can be plugged in without changing
  the listener. This crate does not duplicate them.
- Full NBT writer (`Nbt.php`): only the anonymous text component
  (`system_chat`, nametags, kick) is implemented here.
- Chat/command dispatch (`PlayerChatEvent`, `/list`, `/tp`, ...) stays
  in `server-core`: `ServerHooks::handle_chat` returns a
  [`hooks::ChatOutcome`] (lines + sender teleport/gamemode) and the
  session only sends/applies it.
- Registry/tags JSON payloads (`resources/registry/*.json`,
  `resources/tags/*.json`): supplied through
  `ServerHooks::registry_payloads` / `tags_payload`.

## Tests

- `tests/wire_test.rs` — port of `wire_test.php` (legacy 340 over real
  TCP): status, login sequence, 9 chunks, `/list`, plugin inbox, chat
  broadcast, `/spawn`, block dig/place, `/tp` (+64), duplicate name,
  wrong protocol, disconnect.
- `tests/modern_test.rs` — port of `modern_test.php`, listener scope:
  status/login/configuration/join_game/chunks for 769-775,
  `player_info_update` `0x04`, `spawn_entity` `lpVec3`,
  `entity_metadata` Optional, `update_time` per version. Chunk
  *contents* are intentionally not asserted (ModernChunk agent).

Run: `cargo test -p java-bridge`
