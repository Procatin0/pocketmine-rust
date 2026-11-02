use nethernet_rs::{ServerInfo, SignalingServer};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::test]
async fn test_nethernet_probe_join() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    drop(listener);

    let info = ServerInfo {
        name: "Test PocketMine-Rust Server".to_string(),
        max_players: 500,
        ..Default::default()
    };

    let server = SignalingServer::new(local_addr, info, 19132);
    tokio::spawn(async move {
        server.run().await.unwrap();
    });

    // Wait a brief moment for server to bind
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // 1. Send HTTP GET /v1/join probe
    let mut client = TcpStream::connect(local_addr).await.unwrap();
    let request = "GET /v1/join HTTP/1.1\r\nHost: localhost\r\n\r\n";
    client.write_all(request.as_bytes()).await.unwrap();

    let mut response = Vec::new();
    client.read_to_end(&mut response).await.unwrap();
    let response_str = String::from_utf8(response).unwrap();

    assert!(response_str.contains("HTTP/1.1 200 OK"));
    assert!(response_str.contains("Test PocketMine-Rust Server"));
    assert!(response_str.contains("\"maxPlayers\":500"));
    assert!(response_str.contains("\"transportLayer\":2"));

    // 2. Send HTTP GET / (browser dashboard)
    let mut client_root = TcpStream::connect(local_addr).await.unwrap();
    let request_root = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    client_root.write_all(request_root.as_bytes()).await.unwrap();

    let mut response_root = Vec::new();
    client_root.read_to_end(&mut response_root).await.unwrap();
    let response_root_str = String::from_utf8(response_root).unwrap();

    assert!(response_root_str.contains("HTTP/1.1 200 OK"));
    assert!(response_root_str.contains("PocketMine-Rust"));
    assert!(response_root_str.contains("text/html"));

    // 3. Send HTTP POST /v1/join/12345678 (SDP offer negotiation)
    let mut client_sdp = TcpStream::connect(local_addr).await.unwrap();
    let request_sdp = "POST /v1/join/12345678 HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/sdp\r\nContent-Length: 4\r\n\r\nv=0\r\n";
    client_sdp.write_all(request_sdp.as_bytes()).await.unwrap();

    let mut response_sdp = Vec::new();
    client_sdp.read_to_end(&mut response_sdp).await.unwrap();
    let response_sdp_str = String::from_utf8(response_sdp).unwrap();

    assert!(response_sdp_str.contains("HTTP/1.1 200 OK"));
    assert!(response_sdp_str.contains("application/sdp"));
    assert!(response_sdp_str.contains("a=candidate:1 1 UDP"));
}
