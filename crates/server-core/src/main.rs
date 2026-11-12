use nethernet_rs::{NetherNetIdentity, PeerDataChannels, ServerInfo, SignalingServer};
use php_plugin_bridge::{
    CommandSender, Event, PluginEnableOrder, PluginManager, ServerCommandEvent,
};
use raklib_rs::{RakNetEvent, RakNetListener};
use server_core::query::{QueryHandler, QueryInfo, QueryListener};
use server_core::translations::Translator;
use server_core::{ServerConfig, ServerPluginHost, SessionManager};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// `VersionInfo::NAME` (Axolotl branding).
const SERVER_NAME: &str = "Axolotl-PM";
/// Display full version for dev builds (`5.51.2-dev` per Axolotl branding;
/// `VersionInfo::BASE_VERSION` is `5.51.2`).
const FULL_VERSION: &str = "5.51.2-dev";
/// `ProtocolInfo::MINECRAFT_VERSION`.
const MINECRAFT_VERSION: &str = "1.26.60";
/// `VersionInfo::GITHUB_URL`.
const GITHUB_URL: &str = "https://github.com/axolotl-pm/PocketMine-MP";

#[cfg(windows)]
fn init_windows_timer() {
    #[link(name = "winmm")]
    extern "system" {
        fn timeBeginPeriod(uPeriod: u32) -> u32;
    }
    unsafe {
        let _ = timeBeginPeriod(1);
    }
}

#[cfg(not(windows))]
fn init_windows_timer() {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    server_core::logging::init();
    init_windows_timer();

    let start_time = Instant::now();

    // `Server::__construct`: "Loading server configuration".
    info!("Loading server configuration");
    let config = ServerConfig::load_or_create(Path::new("."));
    server_core::logging::set_log_debug(config.debug_level > 1);

    // `Server::__construct`: language selection (`language.selected`).
    let lang = Translator::english();
    {
        let selected_lang = config.language.to_lowercase();
        if selected_lang != "eng" {
            // PM `Language::__construct` throws `LanguageNotFoundException`
            // ("Language \"{}\" not found") and falls back to `eng`.
            tracing::error!("Language \"{selected_lang}\" not found");
            info!("{}", config.language_selected_message());
        } else {
            info!("{}", config.language_selected_message());
        }
    }

    // `Server::__construct`: development-build gate. Axolotl is always a dev
    // build (`VersionInfo::IS_DEVELOPMENT_BUILD = true`).
    // When `settings.enable-dev-builds` is false PM emits 5x EMERGENCY and
    // force-shuts down; when true it emits the warning banner below.
    // NOTE: the Rust port defaults `enable-dev-builds` to true so the dev
    // server starts out of the box; PM upstream defaults to false.
    if !config.enable_dev_builds {
        server_core::emergency!(
            "{}",
            lang.translate("pocketmine.server.devBuild.error1", &[SERVER_NAME])
        );
        server_core::emergency!(
            "{}",
            lang.translate("pocketmine.server.devBuild.error2", &[])
        );
        server_core::emergency!(
            "{}",
            lang.translate("pocketmine.server.devBuild.error3", &[])
        );
        server_core::emergency!(
            "{}",
            lang.translate(
                "pocketmine.server.devBuild.error4",
                &["settings.enable-dev-builds"]
            )
        );
        server_core::emergency!(
            "{}",
            lang.translate(
                "pocketmine.server.devBuild.error5",
                &[&format!("{GITHUB_URL}/releases")]
            )
        );
        shutdown_sequence(ShutdownContextKind::FailedStartup).await;
        stop_other_threads();
        std::process::exit(1);
    }

    warn!("{}", "-".repeat(40));
    warn!(
        "{}",
        lang.translate("pocketmine.server.devBuild.warning1", &[SERVER_NAME])
    );
    warn!(
        "{}",
        lang.translate("pocketmine.server.devBuild.warning2", &[])
    );
    warn!(
        "{}",
        lang.translate("pocketmine.server.devBuild.warning3", &[])
    );
    warn!("{}", "-".repeat(40));

    // `Server::__construct`: `pocketmine.server.start` (AQUA version).
    info!(
        "{}",
        lang.translate(
            "pocketmine.server.start",
            &[&format!("\u{a7}b{MINECRAFT_VERSION}\u{a7}r")]
        )
    );

    // `Server::__construct`: Xbox auth gate.
    if config.xbox_auth {
        info!("{}", lang.translate("pocketmine.server.auth.enabled", &[]));
    } else {
        warn!("{}", lang.translate("pocketmine.server.auth.disabled", &[]));
        warn!("{}", lang.translate("pocketmine.server.authWarning", &[]));
        warn!(
            "{}",
            lang.translate("pocketmine.server.authProperty.disabled", &[])
        );
    }

    // `Server::__construct`: server/machine ids (DEBUG-only).
    let server_id = machine_unique_id(&format!("{}:{}", config.server_ip, config.server_port_udp));
    debug!("Server unique id: {server_id}");
    let machine_id = machine_unique_id("");
    debug!("Machine unique id: {machine_id}");

    // `Server::__construct`: `pocketmine.server.info` + `license`.
    info!(
        "{}",
        lang.translate(
            "pocketmine.server.info",
            &[SERVER_NAME, &format!("\u{a7}e{FULL_VERSION}\u{a7}r")]
        )
    );
    info!(
        "{}",
        lang.translate("pocketmine.server.license", &[SERVER_NAME])
    );

    // Session manager + ops/bans/whitelist (silent; PM logs nothing for these).
    let world_generator = config.create_world_generator();
    let session_manager = Arc::new(SessionManager::with_generator_gamemode_and_view_distance(
        world_generator,
        config.gamemode,
        config.view_distance,
    ));
    session_manager.set_allow_cheats(config.allow_cheats);
    session_manager.set_max_players(config.max_players);
    session_manager.set_auto_save_enabled(config.auto_save);
    // PM `Server::__construct` has no "Loaded N operators" line; ops are
    // loaded silently from `ops.txt`.
    let ops = load_ops(Path::new("ops.txt"));
    session_manager.set_ops(ops);
    session_manager.access().load_from_dir(Path::new("."));
    session_manager.enable_persistence(std::path::PathBuf::from("."));
    // PM has no `world.json`; the Rust snapshot loads silently.
    let _ = session_manager
        .world_state()
        .load_from(Path::new("world.json"));
    // Persisted scoreboard objectives (`scoreboard.json`, global +
    // per-player); missing file = no objectives, like `world.json`.
    let restored_scoreboards = session_manager.load_scoreboards();
    if restored_scoreboards > 0 {
        tracing::debug!("restored {restored_scoreboards} persisted scoreboard(s)");
    }

    // Plugin manager + host (created before transports so sessions immediately
    // have events/commands; PM creates `PluginManager` before worlds).
    let plugin_manager = Arc::new(PluginManager::new());
    let host = Arc::new(ServerPluginHost::new(
        Arc::clone(session_manager.world()),
        Arc::clone(session_manager.registry()),
        Arc::clone(session_manager.ops()),
        config.motd.clone(),
        config.max_players,
    ));
    // Virtual saves go to the same `players/` dir as Bedrock sessions
    // (`enable_persistence(".")` above): one canonical location, no staging.
    host.set_data_dir(std::path::PathBuf::from("."));
    let host_dyn: Arc<dyn php_plugin_bridge::PluginHost> = host.clone();
    plugin_manager.set_host(host_dyn);
    // Virtual (Java bridge) players have no Bedrock session: host mutations
    // (`player.register` / `player.update_position` / `player.unregister`)
    // notify the session manager so Bedrock viewers get spawn / move /
    // despawn packets without waiting for the 20-tick poll.
    {
        let session_manager = Arc::clone(&session_manager);
        host.set_virtual_notify(move || {
            session_manager.poll_virtual_players();
        });
    }
    session_manager.set_plugin_manager(Arc::clone(&plugin_manager));

    let _ = plugin_manager.init_runtime().await;

    // `Server::__construct`: `ResourcePackManager` logs
    // `info "Loading resource packs..."` (hard-coded, not translated).
    ensure_resource_packs_dir();
    info!("Loading resource packs...");
    debug!("Successfully loaded 0 resource packs");

    // `Server::__construct`: `loadPlugins` + `enablePlugins(STARTUP)`.
    let load_result = match plugin_manager.load_plugins_with_result("plugins").await {
        Ok(result) => result,
        Err(error) => {
            tracing::error!("Could not load plugins directory: {error}");
            shutdown_sequence(ShutdownContextKind::FailedStartup).await;
            stop_other_threads();
            std::process::exit(1);
        }
    };
    if load_result.errors > 0 {
        server_core::emergency!(
            "{}",
            lang.translate("pocketmine.plugin.someLoadErrors", &[])
        );
        shutdown_sequence(ShutdownContextKind::FailedStartup).await;
        stop_other_threads();
        std::process::exit(1);
    }
    match plugin_manager
        .enable_plugins_for(PluginEnableOrder::Startup)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            server_core::emergency!(
                "{}",
                lang.translate("pocketmine.plugin.someEnableErrors", &[])
            );
            shutdown_sequence(ShutdownContextKind::FailedStartup).await;
            stop_other_threads();
            std::process::exit(1);
        }
        Err(error) => {
            tracing::error!("Could not enable plugins: {error}");
            server_core::emergency!(
                "{}",
                lang.translate("pocketmine.plugin.someEnableErrors", &[])
            );
            shutdown_sequence(ShutdownContextKind::FailedStartup).await;
            stop_other_threads();
            std::process::exit(1);
        }
    }

    // `Server::__construct`: `startupPrepareWorlds` -> `Preparing world "world"`.
    info!(
        "{}",
        lang.translate("pocketmine.level.preparing", &[&config.level_name])
    );

    // `Server::__construct`: `enablePlugins(POSTWORLD)`.
    match plugin_manager
        .enable_plugins_for(PluginEnableOrder::PostWorld)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            server_core::emergency!(
                "{}",
                lang.translate("pocketmine.plugin.someEnableErrors", &[])
            );
            shutdown_sequence(ShutdownContextKind::FailedStartup).await;
            stop_other_threads();
            std::process::exit(1);
        }
        Err(error) => {
            tracing::error!("Could not enable plugins: {error}");
            server_core::emergency!(
                "{}",
                lang.translate("pocketmine.plugin.someEnableErrors", &[])
            );
            shutdown_sequence(ShutdownContextKind::FailedStartup).await;
            stop_other_threads();
            std::process::exit(1);
        }
    }

    // Updater stub: PM constructs `UpdateChecker` here; only DEBUG output.
    let updater = server_core::updater::UpdateChecker::new(true, "stable");
    updater.do_check();

    // `Server::__construct`: `startupPrepareNetworkInterfaces`.
    // The registry of live WebRTC reliable data channels: the signaling
    // server publishes each peer's channel to it on connect, and the
    // central tick loop below pushes the tick-queued envelopes
    // (`SessionManager::drain_pending_outgoing_bytes`, the
    // `NetworkSession::flushGamePacketQueue()` half of
    // `NetworkSession::tick()`) through it — so chunks stream to a
    // client that is still loading and sends no packets of its own.
    let peer_data_channels = Arc::new(PeerDataChannels::new());
    if !start_network_interfaces(
        &config,
        &session_manager,
        &plugin_manager,
        &lang,
        Arc::clone(&peer_data_channels),
    )
    .await
    {
        shutdown_sequence(ShutdownContextKind::FailedStartup).await;
        stop_other_threads();
        std::process::exit(1);
    }

    // Native Java Edition listener (`java-bridge`): the `plugins/JavaBridge`
    // PHP plugin is retired; Tokio serves `JavaServer` on TCP 25565 with
    // the core as its `ServerHooks` (`server_core::java_hooks::CoreHooks`).
    // Keep the concrete `Arc<CoreHooks>` for the chunk prefetch task below
    // while the listener owns the trait object.
    //
    // `run_arc` is accept/flush only. The logical tick for Java runs on
    // the single central clock below: `JavaServer::tick_at` with the
    // same tick number as `SessionManager::tick` (Bedrock). No second
    // 50 ms interval exists for Java anymore.
    let core_hooks = Arc::new(server_core::java_hooks::CoreHooks::new(
        Arc::clone(&session_manager),
        config.motd.clone(),
        config.view_distance,
        config.gamemode,
        config.level_type.clone(),
    ));
    let java_server = {
        let hooks: Arc<dyn java_bridge::ServerHooks> = core_hooks.clone();
        Arc::new(java_bridge::JavaServer::new(hooks))
    };
    info!("Java Edition listener starting on 0.0.0.0:25565 (java-bridge, native Rust, prioritized for Java 26.1)");
    {
        let server = Arc::clone(&java_server);
        tokio::spawn(async move {
            if let Err(e) = server.run_arc("0.0.0.0:25565").await {
                tracing::error!("FATAL: Failed to bind Java Edition listener on 0.0.0.0:25565: {e}. Check if another server is already using port 25565!");
            }
        });
    }

    // Chunk prefetch sidecar: warms the Java chunk cache (objects plus
    // converted legacy/modern bytes) on the blocking pool around every
    // online player, so `streamChunks` in a Java tick consumes bytes that
    // are already generated instead of paying ~18 ms per cold column. It
    // never runs on the tick task: the 20 TPS clock cannot slow down no
    // matter how many columns are cold.
    {
        let manager = Arc::clone(&session_manager);
        let java_server = Arc::clone(&java_server);
        let radius = config.view_distance.clamp(2, 12);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            let mut running: Option<tokio::task::JoinHandle<usize>> = None;
            loop {
                interval.tick().await;
                if running.as_ref().is_some_and(|h| !h.is_finished()) {
                    continue;
                }
                running = None;
                // Only Java clients consume the converted Java payload
                // cache; with none in play there is nothing to warm
                // (Bedrock streams its own cached payloads).
                let protocols = java_server.active_protocols();
                if protocols.is_empty() {
                    continue;
                }
                // Columns around every online player, nearest first. The
                // registry is the shared source both platforms tick from.
                let mut wanted: std::collections::HashMap<(i32, i32), i32> =
                    std::collections::HashMap::new();
                for player in manager.registry().all() {
                    let center_x = (player.x / 16.0).floor() as i32;
                    let center_z = (player.z / 16.0).floor() as i32;
                    for cx in (center_x - radius)..=(center_x + radius) {
                        for cz in (center_z - radius)..=(center_z + radius) {
                            let distance = (cx - center_x).pow(2) + (cz - center_z).pow(2);
                            wanted
                                .entry((cx, cz))
                                .and_modify(|d| *d = (*d).min(distance))
                                .or_insert(distance);
                        }
                    }
                }
                if wanted.is_empty() {
                    continue;
                }
                let mut columns: Vec<((i32, i32), i32)> =
                    wanted.into_iter().map(|(c, d)| (c, d)).collect();
                columns.sort_by_key(|(_, d)| *d);
                let columns: Vec<(i32, i32)> = columns.into_iter().map(|(c, _)| c).collect();
                // Pass the whole sorted list: `prefetch_blocking` skips
                // warm columns with cache lookups and generates at most
                // a bounded number of cold ones per pass, so each
                // 500 ms pass progresses to the next batch. (The old
                // `truncate(32)` here re-warmed the same nearest 32
                // columns forever; outer columns stayed cold and the
                // Java tick paid ~18 ms per column for the whole load.)
                // Converted bytes are warmed only for the protocols
                // actually connected — all five used to cost ~5x.
                let hooks = Arc::clone(&core_hooks);
                running = Some(tokio::task::spawn_blocking(move || {
                    hooks.prefetch_blocking(&columns, &protocols)
                }));
            }
        });
    }

    // `Server::__construct`: `configGroup->save()` (silent), default gamemode.
    info!(
        "{}",
        lang.translate(
            "pocketmine.server.defaultGameMode",
            &[&gamemode_translated_name(config.gamemode, lang)]
        )
    );

    // `Server::__construct`: splash (single info with 7 URL lines).
    // `message = %` preserves leading `\n\n` (the `"{}"` form escapes them).
    let splash = build_splash(lang);
    info!(message = %splash);

    // `Server::__construct`: `pocketmine.server.startFinished`.
    info!(
        "{}",
        lang.translate(
            "pocketmine.server.startFinished",
            &[&format_elapsed(start_time.elapsed())]
        )
    );

    // Single server tick (núcleo): one 20 TPS clock drives both
    // connectors on the same pulse. There is exactly one interval in
    // this process — the one in the logical-tick task below.
    //
    //   ┌─ ONE 50 ms clock (main.rs logical tick) ─────────────────┐
    //   │ SessionManager::tick()   → Bedrock core (world.tick,     │
    //   │                              SetTime envelope, Fase 2     │
    //   │                              UpdateBlock, virtual sync,   │
    //   │                              doChunkRequests; queue only) │
    //   │ flush tick queues      → drain_pending_outgoing_bytes +  │
    //   │                              send on the peer's WebRTC     │
    //   │                              reliable data channel       │
    //   │ JavaServer::tick_at(N)   → Java connector (same N, same   │
    //   │                              shared WorldState::time;     │
    //   │                              queue only, budgeted)        │
    //   │ tick_tx.send(N)          → plugin sidecar                 │
    //   └───────────────────────────────────────────────────────────┘
    //        │                       │                      │
    //        ▼                       ▼                      ▼
    //   Bedrock transport      Java socket flush       PHP sidecar
    //   (tick queue → drain +  (handle_stream:        (tick_sync on
    //   encode + send on the   poll_bytes + 20 ms     spawn_blocking,
    //   peer's data channel;   drain flush; no tick   serialized)
    //   dispatch still drains  interval left)       + prefetch
    //   on its path too)                            sidecar (500 ms
    //                                                    spawn_blocking)
    //
    // The tick mutates state and queues envelopes (`pending_outgoing`
    // for Bedrock, `out_buf` for Java). Right after the Bedrock tick the
    // loop flushes that queue (`drain_pending_outgoing_bytes` +
    // `PeerDataChannels::send_to_peer`) — the
    // `NetworkSession::flushGamePacketQueue()` half of the reference
    // `NetworkSession::tick()` — so a client that sends nothing after
    // `RequestChunkRadius` still receives its terrain chunks and the
    // `PLAYER_SPAWN` that follows them. Blocking work (`tick_sync` PHP
    // IPC, save) still runs in the sidecars below / on
    // `spawn_blocking`, so the 50 ms pacing holds at 20 TPS no matter
    // how slow a plugin is. The old second clocks — the 20 Hz flush
    // interval for Bedrock polls and the `run_arc` 50 ms `Skip` interval
    // for Java — are gone: the polls now live in `SessionManager::tick`
    // and the Java tick is driven from this same loop with the same
    // number.
    {
        // Plugin-tick sidecar: PHP ticks stay serialized here, in order,
        // instead of being awaited inline by the loop. Actions land one
        // cycle later than before (the price of never stalling the
        // clock); a wedged plugin (30 s IPC timeout) now only backs up
        // this queue, not the TPS.
        let (tick_tx, mut tick_rx) = mpsc::unbounded_channel::<u64>();
        {
            let plugin_manager = Arc::clone(&plugin_manager);
            let session_manager = Arc::clone(&session_manager);
            tokio::spawn(async move {
                while let Some(tick) = tick_rx.recv().await {
                    // Stop as soon as shutdown begins: pending heartbeats
                    // must not interleave with (or restart the worker
                    // during) plugin disabling.
                    if plugin_manager.php_worker().is_shutting_down() {
                        break;
                    }
                    let manager = Arc::clone(&plugin_manager);
                    let result = tokio::task::spawn_blocking(move || manager.tick_sync(tick)).await;
                    match result {
                        Ok(Ok(actions)) => {
                            if !actions.is_empty() {
                                session_manager.apply_actions(&actions, 0);
                            }
                        }
                        Ok(Err(error)) => warn!("Plugin tick {} failed: {}", tick, error),
                        Err(error) => warn!("Plugin tick task panicked: {}", error),
                    }
                }
            });
        }

        // Logical tick: the single clock. It records, advances both
        // connectors with the same tick number, flushes the Bedrock
        // tick's queued envelopes onto each peer's data channel, and
        // queues the PHP tick.
        {
            let plugin_manager = Arc::clone(&plugin_manager);
            let session_manager = Arc::clone(&session_manager);
            let java_server = Arc::clone(&java_server);
            let peer_data_channels = Arc::clone(&peer_data_channels);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(50));
                // Skip, never burst-catch-up: a slow tick must not queue
                // hundreds of catch-up ticks after one slow column build.
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    // Stop ticking as soon as shutdown begins: pending
                    // heartbeats must not interleave with (or restart the
                    // worker during) plugin disabling.
                    if plugin_manager.php_worker().is_shutting_down() {
                        break;
                    }
                    let started = Instant::now();
                    session_manager.metrics().record_tick();

                    // Bedrock connector tick (state + envelopes):
                    // world clock (queues the 1 Hz `SetTime`, or
                    // immediately on a jump), Fase 2 block forwarding,
                    // virtual-player sync, and `doChunkRequests` for
                    // every PreSpawn/InGame session with a view distance
                    // (`NetworkSession::tick()` in the reference).
                    let report = session_manager.tick();
                    let tick = report.tick;

                    // `NetworkSession::flushGamePacketQueue()`: encode
                    // everything this tick queued (terrain chunks and
                    // the `PLAYER_SPAWN` that follows the threshold
                    // chunk included) and write it to each peer's
                    // reliable data channel, without waiting for that
                    // peer to send a packet. A peer with no registered
                    // channel (not yet connected, or already gone) is
                    // skipped — its drain already happened here, so the
                    // batch is dropped rather than re-queued; sessions
                    // only exist after their first dispatch, which
                    // requires the channel, so this is the disconnect
                    // race, not the normal path.
                    for (peer, batches) in session_manager.drain_pending_outgoing_bytes() {
                        for batch in batches {
                            let len = batch.len();
                            if !peer_data_channels.send_to_peer(peer, batch).await {
                                debug!(
                                    "Tick flush: no WebRTC data channel for {peer}; dropped {len} byte batch"
                                );
                            }
                        }
                    }

                    // Java connector tick with the same number, after
                    // Bedrock so Java sees the world time Bedrock just
                    // advanced (e.g. `/time set 6000` on Bedrock reaches
                    // Java in this same tick via jump detection there,
                    // and vice versa). Bounded by the Java tick budget
                    // and fed by the prefetch-warmed cache; the heavy
                    // part runs on the blocking pool so the Tokio worker
                    // serving accepts/flushes never stalls, while the
                    // `await` keeps order and 20 TPS pacing (no second
                    // interval, no lost ticks).
                    {
                        let server = Arc::clone(&java_server);
                        let _ = tokio::task::spawn_blocking(move || server.tick_at(tick)).await;
                    }

                    // Queue the PHP tick. If the sidecar died the channel
                    // is closed, but the clock must keep running regardless
                    // (game logic is the sidecar's job, not the clock's).
                    let _ = tick_tx.send(tick);

                    // PocketMine's autosave interval is 6000 ticks (5
                    // minutes). Fire-and-forget on the blocking pool: the
                    // old code awaited the whole save inline, stalling
                    // the clock for the duration of every file write.
                    if tick % 6000 == 0 && session_manager.auto_save_enabled() {
                        let saver = Arc::clone(&session_manager);
                        tokio::task::spawn_blocking(move || {
                            if let Err(error) = saver.save_all() {
                                warn!("Autosave failed: {}", error);
                            }
                        });
                    }

                    if started.elapsed() > Duration::from_millis(10) {
                        debug!(
                            "Logical tick {tick} took {:?} (must stay << 50 ms)",
                            started.elapsed()
                        );
                    }
                }
            });
        }
    }

    // Console: ServerCommandEvent + comandos de plugins.
    {
        let plugin_manager = Arc::clone(&plugin_manager);
        let session_manager = Arc::clone(&session_manager);
        tokio::spawn(async move {
            let mut lines = BufReader::new(tokio::io::stdin()).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let line = line.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        let manager = Arc::clone(&plugin_manager);
                        let sessions = Arc::clone(&session_manager);
                        let result = tokio::task::spawn_blocking(move || {
                            let mut event = ServerCommandEvent::new(line.clone());
                            let mut actions =
                                manager.dispatch_event_sync(&mut event).unwrap_or_default();
                            if !event.is_cancelled() {
                                let command = event.command;
                                // Native commands first (tp, gamemode, say, ...).
                                if let Some(feedback) = sessions.execute_console_command(&command) {
                                    for line in feedback {
                                        server_core::logging::console_message(&line);
                                    }
                                    return actions;
                                }
                                match manager
                                    .execute_command_sync(&CommandSender::Console, &command)
                                {
                                    Ok(Some(outcome)) => actions.extend(outcome.actions),
                                    Ok(None) => server_core::logging::console_message(
                                        &server_core::command::not_found_message(
                                            command.split_whitespace().next().unwrap_or(""),
                                        ),
                                    ),
                                    Err(error) => {
                                        tracing::error!("Console command failed: {}", error)
                                    }
                                }
                            }
                            actions
                        })
                        .await;
                        if let Ok(actions) = result {
                            session_manager.apply_actions(&actions, 0);
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        debug!("Console input closed: {}", error);
                        break;
                    }
                }
            }
        });
    }

    // Keep running until Ctrl+C (PM SignalHandler) or `/stop`.
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result?;
            // PM `SignalHandler`: "Received signal interrupt, stopping the server".
            info!("Received signal interrupt, stopping the server");
        }
        _ = wait_for_stop(Arc::clone(&session_manager)) => {}
    }

    // `Server::forceShutdown` sequence; the `Stopping other threads` line
    // after it comes from `PocketMine.php` (`new Server(...); $logger->info(...)`).
    let context = ShutdownContext {
        plugin_manager: Arc::clone(&plugin_manager),
        session_manager: Arc::clone(&session_manager),
        config: config.clone(),
    };
    shutdown_sequence(context).await;
    stop_other_threads();
    // PM `forceShutdown()` ends the process with `exit` instead of returning:
    // returning instead would hand control back to the Tokio runtime, whose
    // drop waits indefinitely for the blocking pool (e.g. the console stdin
    // read) and makes the server hang forever in interactive terminals.
    std::process::exit(0);
}

#[derive(Clone)]
struct ShutdownContext {
    plugin_manager: Arc<PluginManager>,
    session_manager: Arc<SessionManager>,
    config: ServerConfig,
}

/// Which shutdown is running: `Server::forceShutdown` after the tick loop, or
/// `forceShutdownExit` while still in the constructor (no worlds/network yet).
enum ShutdownContextKind {
    Full(ShutdownContext),
    FailedStartup,
}

type ShutdownContextAlias = ShutdownContextKind;

/// Implements `Server::forceShutdown()` log order. `FailedStartup` skips
/// plugin/world/network steps that were never initialised (PM `isset()` guards).
async fn shutdown_sequence(context: impl Into<ShutdownContextAlias>) {
    let context = context.into();
    let lang = Translator::english();
    server_core::emergency!(
        "{}",
        lang.translate("pocketmine.server.forcingShutdown", &[])
    );

    match context {
        ShutdownContextAlias::Full(ctx) => {
            // Stop ticking and forbid worker restarts *before* disabling, so a
            // plugin that kills the PHP worker fails fast (with an error log)
            // instead of respawning an empty runtime mid-shutdown and silently
            // skipping the remaining plugins' `onDisable`.
            ctx.plugin_manager.php_worker().begin_shutdown();

            debug!("Disabling all plugins");
            // Bounded: a wedged plugin must not hang shutdown forever. On
            // timeout the PHP `shutdown` action still disables the remaining
            // plugins worker-side before the process is force-killed.
            if tokio::time::timeout(
                Duration::from_secs(15),
                ctx.plugin_manager.disable_plugins(),
            )
            .await
            .is_err()
            {
                warn!("Disabling plugins timed out; continuing shutdown");
            }
            let worker = ctx.plugin_manager.php_worker().clone();
            let _ = tokio::time::timeout(
                Duration::from_secs(10),
                tokio::task::spawn_blocking(move || worker.shutdown_sync()),
            )
            .await;

            // `NetworkSessionManager::close("Server closed")`.
            close_sessions(&ctx.session_manager, &ctx.config.shutdown_message);

            debug!("Unloading all worlds");
            info!(
                "{}",
                lang.translate("pocketmine.level.unloading", &[&ctx.config.level_name])
            );
        }
        ShutdownContextAlias::FailedStartup => {
            // Constructor failed before the manager existed; PM still runs
            // `forceShutdown()` but every `isset()` guard is false. Only the
            // `EMERGENCY` + generic debug lines appear here.
        }
    }

    debug!("Removing event handlers");
    debug!("Shutting down async task worker pool");
    debug!("Saving properties");
    debug!("Closing console");
    debug!("Stopping network interfaces");
    debug!("Stopping network interface RakLibInterface");
    debug!("Stopping network interface NetherNetInterface");
    debug!("Stopping network interface DedicatedQueryNetworkInterface");
}

impl From<ShutdownContext> for ShutdownContextAlias {
    fn from(value: ShutdownContext) -> Self {
        Self::Full(value)
    }
}

/// `PocketMine.php`: `info "Stopping other threads"` after `new Server(...)`.
fn stop_other_threads() {
    info!("Stopping other threads");
    server_core::logging::shutdown_log_writer();
}

/// `NetworkSessionManager::close($reason)` logs
/// `pocketmine.network.session.close` per session via the session logger.
fn close_sessions(session_manager: &SessionManager, reason: &str) {
    let lang = Translator::english();
    let count = session_manager.session_count();
    // `{%reason}` is named in eng.ini.
    let message = lang
        .fill_named("pocketmine.network.session.close", &[("reason", reason)])
        .unwrap_or_else(|| format!("Session closed: {reason}"));
    for _ in 0..count {
        info!("{message}");
    }
}

/// Resolves once `/stop` sets the shutdown flag.
async fn wait_for_stop(manager: Arc<server_core::SessionManager>) {
    loop {
        if manager.stop_requested() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Reads `ops.txt`, one player name per line. Missing file means no operators.
fn load_ops(path: &Path) -> HashSet<String> {
    let mut ops = HashSet::new();
    let Ok(contents) = std::fs::read_to_string(path) else {
        return ops;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        ops.insert(line.to_lowercase());
    }
    ops
}

/// `ResourcePackManager::__construct` preconditions (silent): create the dir
/// and `resource_packs.yml` when missing. Only the `info "Loading resource
/// packs..."` line is observable.
fn ensure_resource_packs_dir() {
    let _ = std::fs::create_dir_all("resource_packs");
    let yml = Path::new("resource_packs/resource_packs.yml");
    if !yml.exists() {
        let default = "force_resources: false\nresource_stack: []\n";
        let _ = std::fs::write(yml, default);
    }
}

/// `GameMode::getTranslatableName()` translated (`gameMode.*`).
fn gamemode_translated_name(gamemode: i32, lang: &Translator) -> String {
    let key = match gamemode {
        1 => "gameMode.creative",
        2 => "gameMode.adventure",
        3 => "gameMode.spectator",
        _ => "gameMode.survival",
    };
    lang.translate(key, &[])
}

/// `Server.php` splash: `"\n\n" + "- {translated}\n" x7` in one `info`.
/// Params are named (`{%discordUrl}` etc.), not positional.
fn build_splash(lang: &Translator) -> String {
    let highlight = |url: &str| format!("\u{a7}b{url}\u{a7}r");
    let mut splash = String::from("\n\n");
    for (key, param, url) in [
        (
            "pocketmine.server.url.discord",
            "discordUrl",
            "https://discord.axolotl-pm.org",
        ),
        (
            "pocketmine.server.url.docs",
            "docsUrl",
            "https://axolotl-pm.org/docs",
        ),
        ("pocketmine.server.url.sourceCode", "sourceUrl", GITHUB_URL),
        (
            "pocketmine.server.url.freePlugins",
            "pluginsUrl",
            "https://poggit.pmmp.io/plugins",
        ),
        (
            "pocketmine.server.url.donations",
            "donationsUrl",
            "https://patreon.com/pocketminemp",
        ),
        (
            "pocketmine.server.url.translations",
            "translationsUrl",
            "https://translate.axolotl-pm.org",
        ),
        (
            "pocketmine.server.url.bugReporting",
            "bugReportingUrl",
            &format!("{GITHUB_URL}/issues"),
        ),
    ] {
        splash.push_str("- ");
        splash.push_str(
            &lang
                .fill_named(key, &[(param, &highlight(url))])
                .unwrap_or_else(|| key.to_string()),
        );
        splash.push('\n');
    }
    splash
}

/// `strval(round(elapsed, 3))` without trailing zeros.
fn format_elapsed(elapsed: Duration) -> String {
    let secs = elapsed.as_secs_f64();
    let rounded = (secs * 1000.0).round() / 1000.0;
    let mut text = format!("{rounded:.3}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

/// Deterministic pseudo-UUID (`Utils::getMachineUniqueId` stand-in).
fn machine_unique_id(extra: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    std::process::id().hash(&mut hasher);
    extra.hash(&mut hasher);
    let h1 = hasher.finish();
    let mut hasher = DefaultHasher::new();
    extra.hash(&mut hasher);
    (h1 ^ 0x9e37_79b9_7f4a_7c15_u64).hash(&mut hasher);
    let h2 = hasher.finish();
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (h1 >> 32) as u32,
        ((h1 >> 16) & 0xffff) as u16,
        0x4000 | ((h1 & 0x0fff) as u16),
        0x8000 | ((h2 >> 48) & 0x0fff) as u16,
        h2 & 0xffff_ffff_ffff,
    )
}

/// `Server::startupPrepareNetworkInterfaces` log parity.
///
/// Returns `false` when no usable transport started (`noneConfigured`).
async fn start_network_interfaces(
    config: &ServerConfig,
    session_manager: &Arc<SessionManager>,
    plugin_manager: &Arc<PluginManager>,
    lang: &Translator,
    peer_data_channels: Arc<PeerDataChannels>,
) -> bool {
    // PM validates transports here (unknown -> warning, cap 8). Config
    // already capped the list; warnings happen in PM order here.
    let known = ["nethernet", "raknet"];
    let mut active: Vec<String> = Vec::new();
    for name in config.transports.iter().take(8) {
        if name.is_empty() {
            continue;
        }
        if !known.contains(&name.as_str()) {
            warn!(
                "{}",
                lang.translate("pocketmine.server.transport.unknown", &[name])
            );
            continue;
        }
        active.push(name.clone());
    }
    if active.is_empty() {
        server_core::emergency!(
            "{}",
            lang.translate("pocketmine.server.transport.noneConfigured", &[])
        );
        return false;
    }

    // `ServerInfo` for NetherNet HTTP signaling (probe fields).
    let server_info = ServerInfo {
        name: config.motd.clone(),
        level: config.level_name.clone(),
        max_players: config.max_players,
        game_type: config.gamemode,
        hardcore: config.hardcore,
        online_auth: config.xbox_auth,
        self_signed_auth: !config.xbox_auth,
        whitelist: config.white_list,
        ..Default::default()
    };

    let mut started_any = false;

    // Effective TCP signaling port (`builtin-signaling.port` wins when set).
    let signaling_port = if config.builtin_signaling_port != 0 {
        config.builtin_signaling_port
    } else {
        config.signaling_port_tcp
    };
    // WebRTC gameplay UDP port: `port-range` first, otherwise one above the
    // TCP signaling port so it never collides with the RakNet UDP socket
    // (server_port_udp) or the signaling TCP socket.
    let webrtc_udp_port = config
        .nethernet_port_range
        .map(|(first, _)| first)
        .unwrap_or_else(|| signaling_port.saturating_add(1));
    let webrtc_active = active.iter().any(|t| t == "nethernet");

    // `Server::startupPrepareQueryInterface`: GS4 Query handler shared by
    // the RakNet socket (when RakLib listens) or a dedicated socket.
    let query_handler = if config.enable_query {
        Some(Arc::new(QueryHandler::new(build_query_info(
            config,
            session_manager,
            plugin_manager,
        ))))
    } else {
        None
    };
    let mut raknet_started = false;

    for transport in &active {
        match transport.as_str() {
            "nethernet" => {
                // `NetherNetTransport::createInterfaces` notices/warnings.
                if !config.builtin_signaling_enabled {
                    server_core::notice!(
                        "{}",
                        lang.translate("pocketmine.server.nethernet.builtinSignalingDisabled", &[])
                    );
                }
                ensure_identity_key_file(&config.nethernet_key_file);
                if config.nethernet_tls_certificate.is_empty() {
                    server_core::notice!(
                        "{}",
                        lang.translate("pocketmine.server.nethernet.tls.disabled", &[])
                    );
                } else {
                    server_core::notice!(
                        "{}",
                        lang.translate("pocketmine.server.nethernet.tls.enabled", &[])
                    );
                }
                if config.nethernet_ice_udp_mux && config.nethernet_port_range.is_none() {
                    warn!(
                        "{}",
                        lang.translate(
                            "pocketmine.server.nethernet.udpMuxWithoutPortRange",
                            &["transport.nethernet.ice-udp-mux"]
                        )
                    );
                }

                // Bind wildcards (0.0.0.0, plus [::] dual-stack when IPv6 is
                // enabled) instead of a concrete local IP such as the
                // Hyper-V vEthernet address (172.19.16.1). A concrete source
                // has no route to LAN/loopback clients and produces
                // Windows `10049`/`10051` on every ICE write.
                let signaling_addr: SocketAddr = format!("0.0.0.0:{}", signaling_port)
                    .parse()
                    .unwrap_or_else(|_| "0.0.0.0:19132".parse().unwrap());

                struct BedrockPacketHandler {
                    session_manager: Arc<SessionManager>,
                }

                impl nethernet_rs::NetherNetPacketHandler for BedrockPacketHandler {
                    fn handle_message(
                        &self,
                        peer: SocketAddr,
                        channel: &str,
                        data: bytes::Bytes,
                    ) -> Vec<bytes::Bytes> {
                        if channel != "ReliableDataChannel" && channel != "UnreliableDataChannel" {
                            return Vec::new();
                        }
                        self.session_manager.dispatch_webrtc_packet(peer, &data)
                    }

                    fn on_disconnect(&self, peer: SocketAddr) {
                        debug!("WebRTC peer {} disconnected", peer);
                        self.session_manager.handle_peer_disconnect(peer);
                    }
                }

                let identity = Arc::new(NetherNetIdentity::new());
                // Dual-stack signaling when IPv6 is enabled: `[::]` alongside
                // `0.0.0.0` so IPv6 clients reach the same TCP port.
                let signaling_server = if config.enable_ipv6 {
                    let v6_addr: SocketAddr = format!("[::]:{}", signaling_port)
                        .parse()
                        .unwrap_or_else(|_| "[::]:19132".parse().unwrap());
                    SignalingServer::with_multiple_ports(
                        vec![signaling_addr, v6_addr],
                        server_info.clone(),
                        vec![webrtc_udp_port],
                        identity,
                    )
                } else {
                    SignalingServer::new(
                        signaling_addr,
                        server_info.clone(),
                        webrtc_udp_port,
                        identity,
                    )
                };
                let mut signaling_server = signaling_server;
                signaling_server.set_packet_handler(Arc::new(BedrockPacketHandler {
                    session_manager: Arc::clone(session_manager),
                }));
                // Share the host's registry so the central tick loop can
                // push tick-queued envelopes (chunks included) to a peer
                // without waiting for that peer to send a packet first.
                signaling_server.set_peer_data_channels(Arc::clone(&peer_data_channels));

                tokio::spawn(async move {
                    if let Err(error) = signaling_server.run().await {
                        tracing::error!("NetherNet signaling server failed: {error}");
                    }
                });
                started_any = true;
            }
            "raknet" => {
                warn!(
                    "{}",
                    lang.translate("pocketmine.server.raknet.deprecated", &[])
                );
                let server_id: u64 = 0x1234_5678_aabb_ccdd;
                // RakNet listens on the wildcard gameplay port, never a
                // concrete local IP (see signaling bind above).
                let raknet_addr: SocketAddr = format!("0.0.0.0:{}", config.server_port_udp)
                    .parse()
                    .unwrap_or_else(|_| "0.0.0.0:19132".parse().unwrap());
                // Task collision guard: never share the WebRTC gameplay UDP
                // port between two listening sockets — gameplay datagrams
                // landing on the RakNet "listening" socket would be dropped
                // (it only speaks RakNet/STUN) and never reach ICE/DTLS.
                let raknet_collides_webrtc = webrtc_active && raknet_addr.port() == webrtc_udp_port;
                if raknet_collides_webrtc {
                    tracing::warn!(
                        "RakNet and NetherNet both claim UDP port {}; keeping NetherNet on {} and skipping RakNet bind so gameplay packets are not lost",
                        raknet_addr.port(),
                        webrtc_udp_port
                    );
                    // Skip the bind entirely; NetherNet keeps the port.
                    // `started_any` already reflects NetherNet above.
                    continue;
                }
                match RakNetListener::bind(raknet_addr, server_id).await {
                    Ok(listener) => {
                        let listener = Arc::new(listener);
                        let gamemode_name = gamemode_english_name(config.gamemode);
                        listener.set_motd(format!(
                            "MCPE;{};2225;{MINECRAFT_VERSION};0;{};{};{};1;{};{};",
                            config.motd,
                            config.max_players,
                            server_id,
                            gamemode_name,
                            raknet_addr.port(),
                            raknet_addr.port(),
                        ));
                        if let Some(handler) = query_handler.clone() {
                            let handler = Arc::clone(&handler);
                            listener.set_raw_handler(Arc::new(move |peer, packet| {
                                handler.handle(peer, packet)
                            }));
                        }
                        raknet_started = true;
                        let raknet_runner = Arc::clone(&listener);
                        let session_mgr = Arc::clone(session_manager);
                        let raknet_port = raknet_addr.port();
                        tokio::spawn(async move {
                            loop {
                                match raknet_runner.poll_event().await {
                                    Ok(event) => match event {
                                        RakNetEvent::Offline { packet, src, .. } => {
                                            debug!(
                                                "RakNet offline handshake on :{} from {}: {:?}",
                                                raknet_port, src, packet
                                            );
                                            let session_id = session_mgr.create_session(src);
                                            debug!(
                                                "Registered session {} for RakNet peer {}",
                                                session_id, src
                                            );
                                        }
                                    },
                                    Err(error) => {
                                        debug!(
                                            "RakNet listener error on :{}: {}",
                                            raknet_port, error
                                        );
                                    }
                                }
                            }
                        });
                        info!(
                            "{}",
                            lang.translate(
                                "pocketmine.server.networkStart",
                                &[&config.server_ip, &raknet_addr.port().to_string()]
                            )
                        );
                        if config.enable_ipv6 {
                            info!(
                                "{}",
                                lang.translate(
                                    "pocketmine.server.networkStart",
                                    &[
                                        &format!("[{}]", config.server_ip_v6),
                                        &config.server_port_v6_udp.to_string()
                                    ]
                                )
                            );
                        }
                        started_any = true;
                    }
                    Err(error) => {
                        tracing::error!(
                            "{}",
                            lang.translate(
                                "pocketmine.server.transport.unavailable",
                                &["raknet", &error.to_string()]
                            )
                        );
                    }
                }
            }
            _ => {}
        }
    }

    // `Server::startupPrepareQueryInterface` (+ IPv6 branch).
    // PM registers a `DedicatedQueryNetworkInterface` only when RakLib is
    // not listening on the address; otherwise `QueryHandler` rides on the
    // RakNet socket (done above via `set_raw_handler`).
    if config.enable_query {
        info!(
            "{}",
            lang.translate(
                "pocketmine.server.query.running",
                &[&config.server_ip, &config.server_port_udp.to_string()]
            )
        );
        if config.enable_ipv6 {
            info!(
                "{}",
                lang.translate(
                    "pocketmine.server.query.running",
                    &[
                        &format!("[{}]", config.server_ip_v6),
                        &config.server_port_v6_udp.to_string()
                    ]
                )
            );
        }
        if let Some(handler) = query_handler.clone() {
            // Live refresh: players / plugins / map like `QueryInfo`.
            {
                let handler = Arc::clone(&handler);
                let session_manager = Arc::clone(session_manager);
                let plugin_manager = Arc::clone(plugin_manager);
                let config = config.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        handler.set_info(build_query_info(
                            &config,
                            &session_manager,
                            &plugin_manager,
                        ));
                    }
                });
            }
            // Dedicated interface (e.g. NetherNet-only / IPv6): the RakNet
            // socket only covers IPv4 `server_ip:server_port_udp`, so the
            // IPv4 dedicated socket is skipped when RakLib listens there
            // (PM `isListeningOn`), while IPv6 always gets its own socket.
            if !raknet_started {
                match QueryListener::bind_with_handler(
                    &config.server_ip,
                    config.server_port_udp,
                    Arc::clone(&handler),
                )
                .await
                {
                    Ok(listener) => {
                        debug!(
                            "Query listener bound to {}",
                            listener.local_addr().unwrap_or_else(|_| config
                                .server_ip
                                .parse()
                                .unwrap_or("0.0.0.0:0".parse().unwrap()))
                        );
                        tokio::spawn(async move {
                            listener.serve().await;
                        });
                    }
                    Err(error) => {
                        tracing::error!(
                            "Could not bind dedicated Query listener on {}:{}: {}",
                            config.server_ip,
                            config.server_port_udp,
                            error
                        );
                    }
                }
            }
            if config.enable_ipv6 {
                match QueryListener::bind_with_handler(
                    &config.server_ip_v6,
                    config.server_port_v6_udp,
                    Arc::clone(&handler),
                )
                .await
                {
                    Ok(listener) => {
                        debug!(
                            "Query listener bound to {}",
                            listener
                                .local_addr()
                                .map(|a| a.to_string())
                                .unwrap_or_else(|_| "unknown".to_string())
                        );
                        tokio::spawn(async move {
                            listener.serve().await;
                        });
                    }
                    Err(error) => {
                        debug!("Could not bind IPv6 Query listener: {}", error);
                    }
                }
            }
        }
    }

    // `Server::startupPrepareNetworkInterfaces`: UPnP deprecation (PM checks
    // `network.upnp-forwarding`).
    if config.upnp_forwarding {
        warn!(
            "{}",
            lang.translate("pocketmine.server.upnp.deprecated", &[])
        );
    }

    if !started_any {
        server_core::emergency!(
            "{}",
            lang.translate("pocketmine.server.transport.noneConfigured", &[])
        );
        return false;
    }
    true
}

/// Live `QueryInfo` snapshot (`QueryInfo::__construct`): MOTD, online
/// players, loaded plugins, gametype, map, players/limits and address.
fn build_query_info(
    config: &ServerConfig,
    session_manager: &SessionManager,
    plugin_manager: &PluginManager,
) -> QueryInfo {
    let players: Vec<String> = session_manager
        .registry()
        .all()
        .into_iter()
        .map(|player| player.name)
        .collect();
    let mut info = QueryInfo::new(
        config.motd.clone(),
        plugin_manager.load_order(),
        players,
        config.max_players,
        config.server_port_udp as i32,
        config.server_ip.clone(),
    );
    // `QueryInfo`: SMP for survival/adventure, CMP otherwise.
    info.gametype = if config.gamemode == 0 || config.gamemode == 2 {
        "SMP".to_string()
    } else {
        "CMP".to_string()
    };
    info.map = session_manager.world().name.clone();
    info.whitelist = config.white_list;
    info
}

fn gamemode_english_name(gamemode: i32) -> &'static str {
    match gamemode {
        1 => "Creative",
        2 => "Adventure",
        3 => "Spectator",
        _ => "Survival",
    }
}

/// `NetherNetTransport::loadOrCreateIdentityPem` file side-effect (silent):
/// materialise `nethernet.key` so the data dir matches PM. Contents are a
/// placeholder; the Rust identity is ephemeral per process.
fn ensure_identity_key_file(key_file: &str) {
    if key_file.is_empty() || Path::new(key_file).exists() {
        return;
    }
    let _ = std::fs::create_dir_all(Path::new(key_file).parent().unwrap_or(Path::new(".")));
    let _ = std::fs::write(
        key_file,
        "-----BEGIN EC PRIVATE KEY-----\n# Rust port placeholder\n-----END EC PRIVATE KEY-----\n",
    );
}
