//! Port of `plugins/JavaBridge/tests/modern_test.php`, listener scope:
//! status per protocol, login_success (binary UUID), configuration
//! (registry_data, update_tags 0x0d, brand 0x01, finish_configuration),
//! join_game fields + SpawnInfo, 25 chunks for radius 2 with
//! chunk_batch_start/finished, chat_command /list, block dig/place,
//! plugin inbox, disconnect, plus the wire-level checks the PHP test
//! does on the listener's own packets: `player_info_update` gamemode
//! bitflag 0x04, `spawn_entity` lpVec3 (773+) vs vec3i16 (769/770),
//! `entity_metadata` Optional component (type 6 + presence byte), and
//! `update_time` per version.
//!
//! Chunk *contents* (sections, palettes, fluid counts) are NOT covered:
//! they belong to the `ModernChunk` agent; the stub here returns dummy
//! column bytes and this test only asserts framing/batch behaviour.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{handshake, start_server, StubHooks, TestClient};
use java_bridge::profile;
use java_bridge::wire::{self, Reader};

async fn login_modern(
    addr: std::net::SocketAddr,
    protocol: i32,
    name: &str,
) -> (TestClient, Vec<(i32, Vec<u8>)>, Vec<(i32, Vec<u8>)>) {
    let mut client = TestClient::connect(addr).await;
    client.send(&handshake(protocol, 2, name)).await;
    let login = client
        .collect_until(Duration::from_secs(2), |f| !f.is_empty())
        .await;
    let ack = profile::packet_id(protocol, "login_acknowledged").unwrap();
    client.send_frame(ack, &[]).await;
    let config = client
        .collect_until(Duration::from_secs(2), |f| {
            f.iter()
                .any(|(id, _)| *id == profile::packet_id(protocol, "finish_configuration").unwrap())
        })
        .await;
    (client, login, config)
}

#[tokio::test]
async fn modern_flow_all_protocols() {
    let protocols = [
        (769, "Steve"),
        (770, "Bob"),
        (773, "Carol"),
        (774, "Alex"),
        (775, "Diana"),
    ];
    for (protocol, name) in protocols {
        let hooks = StubHooks::new(1).arc();
        let (addr, _server, _handle) = start_server(Arc::clone(&hooks)).await;

        // ---- status ----
        let mut status = TestClient::connect(addr).await;
        let mut hello = handshake(protocol, 1, "");
        hello.extend_from_slice(&wire::frame(0x00, &[]));
        status.send(&hello).await;
        let frames = status
            .collect_until(Duration::from_secs(2), |f| !f.is_empty())
            .await;
        let mut r = Reader::new(&frames[0].1);
        let json = r.read_string().unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value["version"]["protocol"], protocol,
            "protocol {protocol}"
        );
        assert_eq!(
            value["version"]["name"],
            profile::version_name(protocol).unwrap()
        );

        // ---- login + configuration ----
        let (mut client, login, config) = login_modern(addr, protocol, name).await;
        assert_eq!(login.len(), 1);
        assert_eq!(
            login[0].0,
            profile::packet_id(protocol, "login_success").unwrap()
        );
        let uuid = &login[0].1[..16];
        assert_eq!(uuid, &wire::offline_uuid(name));
        let mut r = Reader::new(&login[0].1[16..]);
        assert_eq!(r.read_string().as_deref(), Some(name));
        assert_eq!(r.read_varint(), Some(0), "properties");

        let registry_id = profile::packet_id(protocol, "registry_data").unwrap();
        let mut registry_ids = Vec::new();
        let mut tags_payload = None;
        let mut brand_payload = None;
        let mut brand_before_finish = false;
        let mut finish_seen = false;
        for (id, payload) in &config {
            if *id == registry_id {
                let mut r = Reader::new(payload);
                registry_ids.push(r.read_string().unwrap());
            }
            if *id == profile::packet_id(protocol, "update_tags").unwrap() {
                tags_payload = Some(payload.clone());
            }
            if *id == profile::packet_id(protocol, "config_brand").unwrap() {
                brand_payload = Some(payload.clone());
                brand_before_finish = !finish_seen;
            }
            if *id == profile::packet_id(protocol, "finish_configuration").unwrap() {
                finish_seen = true;
            }
        }
        assert_eq!(
            registry_ids,
            [
                "minecraft:dimension_type",
                "minecraft:worldgen/biome",
                "minecraft:chat_type"
            ],
            "protocol {protocol} registries"
        );
        assert!(finish_seen, "protocol {protocol} finish_configuration");

        // Tags 0x0d: parse registries/tags/members.
        assert_eq!(profile::packet_id(protocol, "update_tags"), Some(13));
        let tags = tags_payload.expect("update_tags sent");
        let mut r = Reader::new(&tags);
        assert_eq!(r.read_varint(), Some(1));
        assert_eq!(r.read_string().as_deref(), Some("minecraft:damage_type"));
        assert_eq!(r.read_varint(), Some(1));
        assert_eq!(r.read_string().as_deref(), Some("minecraft:is_fire"));
        assert_eq!(r.read_varint_array(), Some(vec![1, 2]));
        assert_eq!(r.remaining(), 0);

        // Brand 0x01 before finish_configuration.
        assert_eq!(profile::packet_id(protocol, "config_brand"), Some(1));
        let brand = brand_payload.expect("brand sent");
        let mut r = Reader::new(&brand);
        assert_eq!(r.read_string().as_deref(), Some("minecraft:brand"));
        assert_eq!(r.read_string().as_deref(), Some("PocketMine"));
        assert_eq!(r.remaining(), 0);
        assert!(brand_before_finish);

        // ---- play: join_game + chunks ----
        let settings = profile::packet_id(protocol, "config_settings").unwrap();
        let mut settings_payload = wire::string("en_US");
        settings_payload.extend_from_slice(&[10u8, 0, 1, 0x7F]);
        settings_payload.extend_from_slice(&wire::varint(1));
        client.send_frame(settings, &settings_payload).await;
        let finish = profile::packet_id(protocol, "config_finish").unwrap();
        client.send_frame(finish, &[]).await;

        let join_id = profile::packet_id(protocol, "join_game").unwrap();
        let map_id = profile::packet_id(protocol, "map_chunk").unwrap();
        let frames = client
            .collect_until(Duration::from_secs(4), |f| {
                f.iter().any(|(id, _)| *id == join_id)
                    && f.iter().filter(|(id, _)| *id == map_id).count() >= 25
            })
            .await;
        let join = frames
            .iter()
            .find(|(id, _)| *id == join_id)
            .map(|(_, p)| p.clone())
            .expect("join_game");
        let mut r = Reader::new(&join);
        let entity_id = u32::from_be_bytes(r.data[r.pos..r.pos + 4].try_into().unwrap());
        r.pos += 4;
        assert!(entity_id > 0);
        assert_eq!(r.data[r.pos], 0, "hardcore");
        r.pos += 1;
        assert_eq!(r.read_varint(), Some(1), "world count");
        assert_eq!(r.read_string().as_deref(), Some("minecraft:overworld"));
        assert_eq!(r.read_varint(), Some(20), "max players");
        assert_eq!(r.read_varint(), Some(2), "view distance clamped to 2");
        assert_eq!(r.read_varint(), Some(2), "simulation distance");
        r.pos += 3; // reduced, respawn screen, limited crafting
        assert_eq!(r.read_varint(), Some(0), "dimension type");
        assert_eq!(r.read_string().as_deref(), Some("minecraft:overworld"));
        assert_eq!(r.read_i64(), Some(0), "seed");
        assert_eq!(r.data[r.pos], 0, "gamemode");
        r.pos += 1;
        assert_eq!(r.data[r.pos], 0xFF, "previous gamemode");
        r.pos += 1;
        assert_eq!(r.data[r.pos], 0, "debug");
        r.pos += 1;
        assert_eq!(r.data[r.pos], 1, "flat");
        r.pos += 1;
        assert_eq!(r.data[r.pos], 0, "death location");
        r.pos += 1;
        assert_eq!(r.read_varint(), Some(0), "portal cooldown");
        assert_eq!(r.read_varint(), Some(63), "sea level");
        assert_eq!(r.data[r.pos], 0, "secure chat");
        r.pos += 1;
        assert_eq!(r.remaining(), 0, "join_game fully parsed");

        let chunks: Vec<_> = frames.iter().filter(|(id, _)| *id == map_id).collect();
        assert_eq!(chunks.len(), 25, "protocol {protocol}: 25 columns");
        assert!(frames
            .iter()
            .any(|(id, _)| *id == profile::packet_id(protocol, "chunk_batch_start").unwrap()));
        let finished: Vec<_> = frames
            .iter()
            .filter(|(id, _)| *id == profile::packet_id(protocol, "chunk_batch_finished").unwrap())
            .collect();
        assert!(!finished.is_empty(), "batch closed");
        let mut r = Reader::new(&finished[0].1);
        assert_eq!(r.read_varint(), Some(25), "finished count");

        // ---- chat_command /list ----
        let chat_command = profile::packet_id(protocol, "chat_command").unwrap();
        client.send_frame(chat_command, &wire::string("list")).await;
        let chat_id = profile::packet_id(protocol, "system_chat").unwrap();
        let frames = client
            .collect_until(Duration::from_secs(2), |f| {
                f.iter().any(|(id, _)| *id == chat_id)
            })
            .await;
        assert!(frames
            .iter()
            .any(|(id, p)| { *id == chat_id && p.windows(6).any(|w| w == b"Online") }));

        // ---- block dig ----
        let dig = profile::packet_id(protocol, "block_dig").unwrap();
        let mut dig_payload = wire::varint(2);
        dig_payload.extend_from_slice(&wire::position(0, 4, 0));
        client.send_frame(dig, &dig_payload).await;
        let change = profile::packet_id(protocol, "block_change").unwrap();
        let frames = client
            .collect_until(Duration::from_secs(2), |f| {
                f.iter().any(|(id, _)| *id == change)
            })
            .await;
        assert!(frames.iter().any(|(id, p)| {
            *id == change && {
                let mut r = Reader::new(p);
                r.read_position() == Some((0, 4, 0)) && r.read_varint() == Some(0)
            }
        }));

        // ---- disconnect ----
        drop(client);
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(hooks
            .state
            .lock()
            .unwrap()
            .quits
            .contains(&name.to_string()));
    }
}

#[tokio::test]
async fn modern_entity_spawn_lpvec3_and_metadata_optional() {
    // Direct session check (no socket): the same assertions as the PHP
    // spawn_entity / entity_metadata sections of modern_test.php.
    use java_bridge::hooks::{PlayerSnapshot, ServerHooks};
    use java_bridge::session::{EntityTarget, JavaSession, SessionState};

    struct Hooks;
    impl ServerHooks for Hooks {
        fn view_distance(&self) -> i32 {
            1
        }
        fn spawn(&self) -> (f64, f64, f64) {
            (0.5, 5.62, 0.5)
        }
        fn snapshots(&self) -> Vec<PlayerSnapshot> {
            vec![]
        }
    }
    let hooks = Hooks;

    for (protocol, lp) in [(769, false), (770, false), (773, true), (775, true)] {
        let mut session = JavaSession::new(1, "127.0.0.1:25565".parse().unwrap());
        session.protocol = protocol;
        session.username = Some("Steve".to_string());
        session.state = SessionState::Play;
        session.playing = true;
        session.sync_entities(
            &hooks,
            &[EntityTarget {
                name: "Alex".to_string(),
                x: 1.0,
                y: 64.0,
                z: 2.0,
                yaw: 0.0,
                pitch: 0.0,
                head_yaw: 0.0,
                is_sneaking: false,
                gamemode: 0,
                ping: 0,
            }],
        );
        let bytes = session.take_out();
        let mut frames = Vec::new();
        let mut rest = bytes.as_slice();
        while let Some((id, payload, consumed)) = wire::try_split_frame(rest) {
            frames.push((id, payload));
            rest = &rest[consumed..];
        }
        let spawn_id = profile::packet_id(protocol, "spawn_entity").unwrap();
        assert_eq!(spawn_id, 1);
        let spawn = frames.iter().find(|(id, _)| *id == spawn_id).unwrap();
        let mut r = Reader::new(&spawn.1);
        let _entity = r.read_varint();
        r.pos += 16; // uuid
        assert_eq!(r.read_varint(), Some(profile::player_entity_type(protocol)));
        r.pos += 24; // xyz
        if lp {
            assert_eq!(r.data[r.pos], 0x00, "protocol {protocol}: lpVec3 zero");
            r.pos += 1;
        } else {
            // angles + objectData + trailing vec3i16 zeros.
            assert!(spawn.1.ends_with(&[0, 0, 0, 0, 0, 0]));
        }
        let meta_id = profile::packet_id(protocol, "entity_metadata").unwrap();
        let meta = frames.iter().find(|(id, _)| *id == meta_id).unwrap();
        let mut r = Reader::new(&meta.1);
        let _entity = r.read_varint();
        assert_eq!(r.data[r.pos], 2, "custom_name index");
        r.pos += 1;
        assert_eq!(r.read_varint(), Some(6), "optional_component, not direct 5");
        assert_eq!(r.data[r.pos], 1, "Optional presence byte");
    }
}

#[tokio::test]
async fn modern_player_info_update_gamemode_and_update_time() {
    use java_bridge::hooks::PlayerSnapshot;
    use java_bridge::hooks::ServerHooks;
    use java_bridge::session::{JavaSession, SessionState};

    struct Hooks;
    impl ServerHooks for Hooks {
        fn view_distance(&self) -> i32 {
            1
        }
        fn spawn(&self) -> (f64, f64, f64) {
            (0.5, 5.62, 0.5)
        }
        fn snapshots(&self) -> Vec<PlayerSnapshot> {
            vec![]
        }
    }
    let hooks = Hooks;

    for protocol in [769, 775] {
        let mut session = JavaSession::new(1, "127.0.0.1:25565".parse().unwrap());
        session.protocol = protocol;
        session.username = Some("Steve".to_string());
        session.state = SessionState::Play;
        session.playing = true;
        session.set_gamemode(1);
        session.send_time(6000, true);
        let bytes = session.take_out();
        let mut frames = Vec::new();
        let mut rest = bytes.as_slice();
        while let Some((id, payload, consumed)) = wire::try_split_frame(rest) {
            frames.push((id, payload));
            rest = &rest[consumed..];
        }
        let info = frames
            .iter()
            .find(|(id, _)| *id == profile::packet_id(protocol, "player_info").unwrap())
            .unwrap();
        assert_eq!(info.1[0], 0x04, "player_info_update update_game_mode");
        // `game_state_change` carries the gamemode as its float parameter
        // (survival 0.0, creative 1.0, ...). Packing it as a byte left the
        // value at 0.0, so Java clients stayed in survival after
        // `/gamemode creative`.
        let gsc = frames
            .iter()
            .find(|(id, _)| *id == profile::packet_id(protocol, "game_state_change").unwrap())
            .expect("game_state_change");
        assert_eq!(gsc.1[0], 3, "change_game_mode reason");
        assert_eq!(&gsc.1[1..5], &1.0f32.to_be_bytes(), "gamemode as f32");
        let time = frames
            .iter()
            .find(|(id, _)| *id == profile::packet_id(protocol, "update_time").unwrap())
            .unwrap();
        let mut r = Reader::new(&time.1);
        assert_eq!(r.read_i64(), Some(0), "age");
        if protocol >= 775 {
            // clockUpdates[]: count, id, totalTicks (varlong), partial, rate.
            assert_eq!(r.read_varint(), Some(1));
            assert_eq!(r.read_varint(), Some(0));
            assert_eq!(r.read_varint(), Some(6000), "totalTicks varlong");
            assert_eq!(r.read_f32(), Some(0.0));
            assert_eq!(r.read_f32(), Some(1.0));
        } else {
            assert_eq!(r.read_i64(), Some(6000));
            assert_eq!(r.data[r.pos], 1, "tickDayTime");
        }
    }
}
