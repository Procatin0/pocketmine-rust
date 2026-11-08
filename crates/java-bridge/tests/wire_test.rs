//! Port of `plugins/JavaBridge/tests/wire_test.php` (legacy 1.12.2,
//! protocol 340) over a real Tokio TCP listener on loopback.
//!
//! PHP checks: listener active, status JSON + pong, login packet
//! sequence (login_success, join_game, difficulty, spawn_position,
//! abilities, update_health, update_time, position, player_info, chat),
//! 9 chunks with bitmap 3, `/list`, plugin inbox lines, chat broadcast,
//! `/spawn`, block dig/place (creative gate), `/tp` with the +64 legacy
//! offset, duplicate-name and wrong-protocol rejection, disconnect.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{handshake, start_server, StubHooks, TestClient};
use java_bridge::wire::{self, Reader};

fn frames_with<'a>(frames: &'a [(i32, Vec<u8>)], id: i32) -> Vec<&'a [u8]> {
    frames
        .iter()
        .filter(|(fid, _)| *fid == id)
        .map(|(_, payload)| payload.as_slice())
        .collect()
}

#[tokio::test]
async fn legacy_wire_flow() {
    let hooks = StubHooks::new(1).arc();
    let (addr, _server, _handle) = start_server(Arc::clone(&hooks)).await;

    // ---------------- status ----------------
    let mut status = TestClient::connect(addr).await;
    let mut hello = handshake(340, 1, "");
    hello.extend_from_slice(&wire::frame(0x00, &[]));
    status.send(&hello).await;
    let frames = status
        .collect_until(Duration::from_secs(2), |f| f.len() >= 1)
        .await;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].0, wire::S2C_STATUS_RESPONSE);
    let mut r = Reader::new(&frames[0].1);
    let json = r.read_string().unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["version"]["protocol"], 340);
    assert_eq!(value["description"]["text"], "JavaBridge test");
    assert_eq!(value["players"]["max"], 20);

    status
        .send_frame(wire::C2S_STATUS_PING, &wire::long(123456789))
        .await;
    let frames = status
        .collect_until(Duration::from_secs(2), |f| {
            f.iter().any(|(id, _)| *id == wire::S2C_STATUS_PONG)
        })
        .await;
    let pong = frames_with(&frames, wire::S2C_STATUS_PONG);
    assert_eq!(pong.len(), 1);
    let mut r = Reader::new(pong[0]);
    assert_eq!(r.read_i64(), Some(123456789));

    // ---------------- login ----------------
    let mut client = TestClient::connect(addr).await;
    client.send(&handshake(340, 2, "Steve")).await;
    let frames = client
        .collect_until(Duration::from_secs(3), |f| {
            f.iter().any(|(id, _)| *id == wire::S2C_CHAT)
        })
        .await;
    let ids: Vec<i32> = frames.iter().map(|(id, _)| *id).collect();
    let expected = [
        wire::S2C_LOGIN_SUCCESS,
        wire::S2C_JOIN_GAME,
        wire::S2C_DIFFICULTY,
        wire::S2C_SPAWN_POSITION,
        wire::S2C_ABILITIES,
        wire::S2C_UPDATE_HEALTH,
        wire::S2C_UPDATE_TIME,
        wire::S2C_POSITION,
        wire::S2C_PLAYER_INFO,
        wire::S2C_CHAT,
    ];
    assert_eq!(&ids[..expected.len()], &expected[..], "ids: {ids:?}");

    let join_game = frames[1].1.as_slice();
    let entity_id = u32::from_be_bytes(join_game[0..4].try_into().unwrap());
    assert!(entity_id > 0);
    assert_eq!(join_game[4], 0, "gamemode survival");
    let dimension = u32::from_be_bytes(join_game[5..9].try_into().unwrap());
    assert_eq!(dimension, 0);
    assert_eq!(join_game[9], 2, "difficulty");
    assert_eq!(join_game[10], 20, "max players");
    let mut r = Reader::new(&join_game[11..]);
    assert_eq!(r.read_string().as_deref(), Some("flat"));

    // Chunks keep streaming on the tick loop: 9 columns for radius 1.
    let frames = client
        .collect_until(Duration::from_secs(3), |f| {
            f.iter()
                .filter(|(id, _)| *id == wire::S2C_MAP_CHUNK)
                .count()
                >= 9
        })
        .await;
    let chunks = frames_with(&frames, wire::S2C_MAP_CHUNK);
    assert_eq!(chunks.len(), 9, "9 columns");
    for chunk in &chunks {
        let mut r = Reader::new(chunk);
        let _cx = r.data[r.pos..r.pos + 4].to_vec();
        r.pos += 8; // x, z
        r.pos += 1; // groundUp
        assert_eq!(r.read_varint(), Some(3), "bitmap");
        let size = r.read_varint().unwrap();
        assert!(size > 1000, "size {size}");
        r.pos += size as usize;
        assert_eq!(r.read_varint(), Some(0), "block entities");
    }

    // ---------------- /list ----------------
    client
        .send_frame(wire::C2S_CHAT, &wire::string("/list"))
        .await;
    let frames = client
        .collect_until(Duration::from_secs(2), |f| {
            f.iter().any(|(id, _)| *id == wire::S2C_CHAT)
        })
        .await;
    let chats = frames_with(&frames, wire::S2C_CHAT);
    assert!(chats.iter().any(|payload| {
        let mut r = Reader::new(payload);
        r.read_string()
            .map(|s| s.contains("Online (1)"))
            .unwrap_or(false)
    }));

    // Plugin inbox lines reach the socket (deliverPluginMessages).
    hooks
        .state
        .lock()
        .unwrap()
        .inbox
        .push("plugin line here".to_string());
    let frames = client
        .collect_until(Duration::from_secs(3), |f| {
            f.iter()
                .any(|(id, p)| *id == wire::S2C_CHAT && p.windows(9).any(|w| w == b"plugin li"))
        })
        .await;
    assert!(
        frames.iter().any(|(id, p)| *id == wire::S2C_CHAT && p.windows(16).any(|w| w == b"plugin line here")),
        "plugin line delivered"
    );

    // Chat goes through the broadcast hook.
    client
        .send_frame(wire::C2S_CHAT, &wire::string("hola mundo"))
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(hooks
        .state
        .lock()
        .unwrap()
        .broadcasts
        .contains(&"<Steve> hola mundo".to_string()));

    // ---------------- /spawn ----------------
    client
        .send_frame(wire::C2S_CHAT, &wire::string("/spawn"))
        .await;
    let frames = client
        .collect_until(Duration::from_secs(2), |f| {
            f.iter().any(|(id, _)| *id == wire::S2C_POSITION)
        })
        .await;
    assert_eq!(frames_with(&frames, wire::S2C_POSITION).len(), 1);

    // ---------------- block dig ----------------
    client
        .send_frame(
            wire::C2S_BLOCK_DIG,
            &[wire::varint(2), wire::position(0, 68, 0)].concat(),
        )
        .await;
    let frames = client
        .collect_until(Duration::from_secs(2), |f| {
            f.iter().any(|(id, _)| *id == wire::S2C_BLOCK_CHANGE)
        })
        .await;
    assert!(frames_with(&frames, wire::S2C_BLOCK_CHANGE)
        .iter()
        .any(|payload| {
            let mut r = Reader::new(payload);
            r.read_position() == Some((0, 68, 0)) && r.read_varint() == Some(0)
        }));

    // ---------------- block place (creative gate) ----------------
    let legacy_place = [
        wire::position(0, 68, 0),
        vec![1u8],                   // face up
        1u16.to_be_bytes().to_vec(), // held item: stone
        vec![8u8, 8, 8],             // cursor
    ]
    .concat();
    client
        .send_frame(wire::C2S_BLOCK_PLACE, &legacy_place)
        .await;
    let frames = client.collect(Duration::from_millis(300)).await;
    assert!(frames
        .iter()
        .any(|(id, p)| { *id == wire::S2C_CHAT && p.windows(13).any(|w| w == b"modo creativo") }));
    assert_eq!(
        hooks.state.lock().unwrap().blocks.get(&(0, 5, 0)),
        None,
        "survival must not place"
    );

    client
        .send_frame(wire::C2S_CHAT, &wire::string("/gamemode creative"))
        .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    client
        .send_frame(wire::C2S_BLOCK_PLACE, &legacy_place)
        .await;
    let frames = client
        .collect_until(Duration::from_secs(2), |f| {
            f.iter().any(|(id, _)| *id == wire::S2C_BLOCK_CHANGE)
        })
        .await;
    assert!(frames_with(&frames, wire::S2C_BLOCK_CHANGE)
        .iter()
        .any(|payload| {
            let mut r = Reader::new(payload);
            r.read_position() == Some((0, 69, 0)) && r.read_varint() == Some(1)
        }));
    assert_eq!(
        hooks.state.lock().unwrap().blocks.get(&(0, 5, 0)),
        Some(&1u32),
        "legacy stone placed at canonical (0,5,0)"
    );

    // ---------------- /tp (+64 legacy offset) ----------------
    client
        .send_frame(wire::C2S_CHAT, &wire::string("/tp 10 66 -30"))
        .await;
    let frames = client
        .collect_until(Duration::from_secs(2), |f| {
            f.iter().any(|(id, _)| *id == wire::S2C_POSITION)
        })
        .await;
    let positions = frames_with(&frames, wire::S2C_POSITION);
    let last = positions.last().unwrap();
    let mut r = Reader::new(last);
    assert_eq!(r.read_f64(), Some(10.0));
    assert_eq!(r.read_f64(), Some(130.0), "66 + 64 legacy offset");
    assert_eq!(r.read_f64(), Some(-30.0));

    // ---------------- duplicate name ----------------
    let mut dup = TestClient::connect(addr).await;
    dup.send(&handshake(340, 2, "Steve")).await;
    let frames = dup
        .collect_until(Duration::from_secs(2), |f| !f.is_empty())
        .await;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].0, wire::S2C_LOGIN_DISCONNECT);

    // ---------------- wrong protocol ----------------
    let mut old = TestClient::connect(addr).await;
    old.send(&handshake(47, 2, "Alex")).await;
    let frames = old
        .collect_until(Duration::from_secs(2), |f| !f.is_empty())
        .await;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].0, wire::S2C_LOGIN_DISCONNECT);

    // ---------------- disconnect ----------------
    drop(client);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(hooks
        .state
        .lock()
        .unwrap()
        .quits
        .contains(&"Steve".to_string()));
    assert!(!hooks
        .state
        .lock()
        .unwrap()
        .snapshots
        .iter()
        .any(|s| s.name == "Steve"));
}
