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

    let server = SignalingServer::new(local_addr, info);
    tokio::spawn(async move {
        server.run().await.unwrap();
    });

    // Wait a brief moment for server to bind
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Send HTTP GET /v1/join probe
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
}
