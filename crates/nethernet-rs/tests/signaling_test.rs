use nethernet_rs::{NetherNetIdentity, ServerInfo, SignalingServer};
use std::net::SocketAddr;
use std::sync::Arc;
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

    let identity = Arc::new(NetherNetIdentity::new());
    let server = SignalingServer::new(local_addr, info, 0, identity);
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
    assert!(response_root_str.contains("Test PocketMine-Rust Server"));
    assert!(response_root_str.contains("application/json"));

    // 3. Send HTTP POST /v1/join/12345678 (SDP offer negotiation)
    let sdp_offer = "v=0\r\n\
o=- 7752464448080962985 2 IN IP4 127.0.0.1\r\n\
s=-\r\n\
t=0 0\r\n\
a=group:BUNDLE 0\r\n\
m=application 39240 UDP/DTLS/SCTP webrtc-datachannel\r\n\
c=IN IP4 127.0.0.1\r\n\
a=ice-ufrag:+/RF\r\n\
a=ice-pwd:cLVVKgIMcsGZGxK7pW5G+B7J\r\n\
a=fingerprint:sha-256 22:AC:EC:B0:D1:62:E9:CA:BE:0B:FC:9B:8A:2A:3F:A2:FB:05:1C:80:A7:6C:85:25:1C:40:5D:C7:19:8B:AF:66\r\n\
a=setup:actpass\r\n\
a=mid:0\r\n\
a=sctp-port:5000\r\n\
a=max-message-size:262144\r\n\
a=candidate:1 1 udp 2122260199 127.0.0.1 39240 typ host\r\n";

    let mut client_sdp = TcpStream::connect(local_addr).await.unwrap();
    let request_sdp = format!(
        "POST /v1/join/12345678 HTTP/1.1\r\nHost: 192.168.100.51\r\nContent-Type: application/sdp\r\nContent-Length: {}\r\n\r\n{}",
        sdp_offer.len(),
        sdp_offer
    );
    client_sdp.write_all(request_sdp.as_bytes()).await.unwrap();

    let mut response_sdp = Vec::new();
    client_sdp.read_to_end(&mut response_sdp).await.unwrap();
    let response_sdp_str = String::from_utf8(response_sdp).unwrap();
    println!("DEBUG RESPONSE SDP: >>>\n{}\n<<<", response_sdp_str);

    assert!(response_sdp_str.contains("HTTP/1.1 200 OK"));
    assert!(response_sdp_str.contains("application/sdp"));
    assert!(response_sdp_str.contains("a=identity:"));
    assert!(response_sdp_str.contains("a=candidate:80000000 1 udp 1694498815 192.168.100.51"));
}
