use std::sync::Arc;
use rtc::ice::network_type::NetworkType;
use rtc::peer_connection::transport::RTCDtlsRole;
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler,
    RTCConfigurationBuilder, SettingEngineBuilder,
};

struct DummyHandler;

#[async_trait::async_trait]
impl PeerConnectionEventHandler for DummyHandler {}

#[tokio::test]
async fn test_sequential_connections_on_same_port() {
    let udp_port = 19145;

    // Connection 1
    let setting_engine1 = SettingEngineBuilder::new()
        .with_answering_dtls_role(RTCDtlsRole::Server)
        .with_multicast_dns_mode(rtc::ice::mdns::MulticastDnsMode::Disabled)
        .with_network_types(vec![NetworkType::Udp4])
        .build();
    let config1 = RTCConfigurationBuilder::new().build();
    let pc1 = PeerConnectionBuilder::new()
        .with_configuration(config1)
        .with_setting_engine(setting_engine1)
        .with_handler(Arc::new(DummyHandler))
        .with_udp_addrs(vec![format!("0.0.0.0:{}", udp_port)])
        .build()
        .await
        .expect("PC 1 build");

    // Close Connection 1
    pc1.close().await.expect("PC 1 close");
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Connection 2 on the same port
    let setting_engine2 = SettingEngineBuilder::new()
        .with_answering_dtls_role(RTCDtlsRole::Server)
        .with_multicast_dns_mode(rtc::ice::mdns::MulticastDnsMode::Disabled)
        .with_network_types(vec![NetworkType::Udp4])
        .build();
    let config2 = RTCConfigurationBuilder::new().build();
    let pc2 = PeerConnectionBuilder::new()
        .with_configuration(config2)
        .with_setting_engine(setting_engine2)
        .with_handler(Arc::new(DummyHandler))
        .with_udp_addrs(vec![format!("0.0.0.0:{}", udp_port)])
        .build()
        .await
        .expect("PC 2 build on same port");

    pc2.close().await.expect("PC 2 close");
}
