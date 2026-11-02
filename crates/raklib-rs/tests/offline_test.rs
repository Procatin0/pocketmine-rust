use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

use raklib_rs::{
    format_bedrock_motd, OfflinePacket, OpenConnectionReply1, OpenConnectionReply2,
    OpenConnectionRequest1, OpenConnectionRequest2, PacketError, RakNetEvent, RakNetListener,
    UnconnectedPing, UnconnectedPong, OFFLINE_MESSAGE_DATA_ID,
};

#[tokio::test]
async fn test_unconnected_ping_pong_loopback() {
    let server_id: u64 = 0x11223344_55667788;
    let listener = Arc::new(
        RakNetListener::bind("127.0.0.1:0".parse().unwrap(), server_id)
            .await
            .expect("Failed to bind RakNetListener"),
    );
    let server_addr = listener.local_addr().expect("Failed to get local addr");

    // Spawn listener polling loop
    let listener_clone = Arc::clone(&listener);
    let listener_task = tokio::spawn(async move {
        listener_clone.poll_event().await
    });

    // Create client socket
    let client = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind client socket");

    let ping_time: u64 = 1_700_000_123_456;
    let client_guid: u64 = 0xdeadbeef_cafebabe;
    let ping = UnconnectedPing {
        ping_time,
        client_guid,
    };
    let ping_bytes = ping.encode();
    // Validate ping packet contains standard RakNet offline magic
    assert_eq!(&ping_bytes[9..25], &OFFLINE_MESSAGE_DATA_ID);

    client
        .send_to(&ping_bytes, server_addr)
        .await
        .expect("Failed to send ping datagram");

    // Await listener processing
    let event = timeout(Duration::from_secs(2), listener_task)
        .await
        .expect("Listener timed out")
        .expect("Listener task panicked")
        .expect("Listener poll error");

    match event {
        RakNetEvent::Offline { packet, src, response } => {
            assert_eq!(packet, OfflinePacket::UnconnectedPing(ping.clone()));
            assert_eq!(src, client.local_addr().unwrap());
            assert!(response.is_some());
        }
    }

    // Client receives Unconnected Pong
    let mut recv_buf = [0u8; 2048];
    let (len, from_addr) = timeout(Duration::from_secs(2), client.recv_from(&mut recv_buf))
        .await
        .expect("Client receive timed out")
        .expect("Client receive error");

    assert_eq!(from_addr, server_addr);

    let pong = UnconnectedPong::decode(&recv_buf[..len]).expect("Failed to decode UnconnectedPong");
    assert_eq!(pong.ping_time, ping_time);
    assert_eq!(pong.server_id, server_id);

    let expected_motd = format_bedrock_motd(server_id);
    assert_eq!(pong.motd, expected_motd);
    assert_eq!(
        pong.motd,
        format!("MCPE;Dedicated PocketMine-Rust Server;748;1.21.20;0;1000;{server_id};Survival;1;19132;19133;")
    );
}

#[tokio::test]
async fn test_open_connection_request1_and_reply1() {
    let server_id: u64 = 0xaabbccdd_11223344;
    let listener = Arc::new(
        RakNetListener::bind("127.0.0.1:0".parse().unwrap(), server_id)
            .await
            .expect("Failed to bind RakNetListener"),
    );
    let server_addr = listener.local_addr().expect("Failed to get local addr");

    let listener_clone = Arc::clone(&listener);
    let listener_task = tokio::spawn(async move {
        listener_clone.poll_event().await
    });

    let client = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind client socket");

    let req1 = OpenConnectionRequest1 {
        protocol_version: 11,
        mtu_size: 1492,
    };
    let req1_bytes = req1.encode();
    assert_eq!(req1_bytes.len(), 1492);

    client
        .send_to(&req1_bytes, server_addr)
        .await
        .expect("Failed to send req1");

    let _ = timeout(Duration::from_secs(2), listener_task)
        .await
        .expect("Timed out")
        .expect("Panicked")
        .expect("Poll error");

    let mut recv_buf = [0u8; 2048];
    let (len, _) = timeout(Duration::from_secs(2), client.recv_from(&mut recv_buf))
        .await
        .expect("Client receive timed out")
        .expect("Client receive error");

    let reply1 = OpenConnectionReply1::decode(&recv_buf[..len])
        .expect("Failed to decode OpenConnectionReply1");
    assert_eq!(reply1.server_id, server_id);
    assert_eq!(reply1.mtu_size, 1492);
    assert!(!reply1.server_security);
}

#[tokio::test]
async fn test_open_connection_request2_and_reply2() {
    let server_id: u64 = 0x99887766_55443322;
    let listener = Arc::new(
        RakNetListener::bind("127.0.0.1:0".parse().unwrap(), server_id)
            .await
            .expect("Failed to bind RakNetListener"),
    );
    let server_addr = listener.local_addr().expect("Failed to get local addr");

    let listener_clone = Arc::clone(&listener);
    let listener_task = tokio::spawn(async move {
        listener_clone.poll_event().await
    });

    let client = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind client socket");
    let client_addr = client.local_addr().unwrap();

    let req2 = OpenConnectionRequest2 {
        server_address: server_addr,
        mtu_size: 1400,
        client_id: 0xfeedface_12345678,
    };
    let req2_bytes = req2.encode();

    client
        .send_to(&req2_bytes, server_addr)
        .await
        .expect("Failed to send req2");

    let _ = timeout(Duration::from_secs(2), listener_task)
        .await
        .expect("Timed out")
        .expect("Panicked")
        .expect("Poll error");

    let mut recv_buf = [0u8; 2048];
    let (len, _) = timeout(Duration::from_secs(2), client.recv_from(&mut recv_buf))
        .await
        .expect("Client receive timed out")
        .expect("Client receive error");

    let reply2 = OpenConnectionReply2::decode(&recv_buf[..len])
        .expect("Failed to decode OpenConnectionReply2");
    assert_eq!(reply2.server_id, server_id);
    assert_eq!(reply2.client_address, client_addr);
    assert_eq!(reply2.mtu_size, 1400);
    assert!(!reply2.server_security);
}

#[tokio::test]
async fn test_invalid_magic_rejected() {
    let mut corrupt_ping = vec![0x01];
    corrupt_ping.extend_from_slice(&12345u64.to_be_bytes());
    // Put wrong magic
    corrupt_ping.extend_from_slice(&[0xaa; 16]);
    corrupt_ping.extend_from_slice(&9999u64.to_be_bytes());

    let err = UnconnectedPing::decode(&corrupt_ping).unwrap_err();
    match err {
        PacketError::InvalidMagic => (),
        other => panic!("Expected InvalidMagic, got {:?}", other),
    }

    let mut corrupt_req1 = vec![0x05];
    corrupt_req1.extend_from_slice(&[0x00; 16]); // wrong magic
    corrupt_req1.push(11);

    let err = OpenConnectionRequest1::decode(&corrupt_req1).unwrap_err();
    match err {
        PacketError::InvalidMagic => (),
        other => panic!("Expected InvalidMagic, got {:?}", other),
    }
}

#[tokio::test]
async fn test_custom_motd() {
    let server_id: u64 = 42;
    let listener = RakNetListener::bind("127.0.0.1:0".parse().unwrap(), server_id)
        .await
        .unwrap();

    let custom = "MCPE;Custom Server;748;1.21.20;0;50;42;Creative;1;19132;19133;";
    listener.set_motd(custom);
    assert_eq!(listener.motd(), custom);

    let ping = OfflinePacket::UnconnectedPing(UnconnectedPing {
        ping_time: 100,
        client_guid: 200,
    });
    let client_addr = "127.0.0.1:54321".parse().unwrap();
    let reply = listener.handle_offline_packet(&ping, client_addr).unwrap();

    match reply {
        OfflinePacket::UnconnectedPong(pong) => {
            assert_eq!(pong.motd, custom);
        }
        _ => panic!("Expected UnconnectedPong"),
    }
}
