use std::sync::Arc;
use tokio::sync::Notify;
use rtc::peer_connection::transport::{RTCDtlsRole, RTCIceCandidateType};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler,
    RTCConfigurationBuilder, RTCIceGatheringState, RTCSessionDescription, SettingEngineBuilder,
};

struct NatHandler {
    complete: Arc<Notify>,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for NatHandler {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        println!("Gathering state: {:?}", state);
        if state == RTCIceGatheringState::Complete {
            self.complete.notify_waiters();
        }
    }
}

#[tokio::test]
async fn test_nat_gathering() {
    let complete = Arc::new(Notify::new());
    let handler = Arc::new(NatHandler { complete: complete.clone() });

    let setting_engine = SettingEngineBuilder::new()
        .with_answering_dtls_role(RTCDtlsRole::Server)
        .with_multicast_dns_mode(rtc::ice::mdns::MulticastDnsMode::Disabled)
        .with_nat_1to1_ips(vec!["192.168.100.51".to_string()], RTCIceCandidateType::Host)
        .build();

    let config = RTCConfigurationBuilder::new().build();
    let pc = PeerConnectionBuilder::new()
        .with_configuration(config)
        .with_setting_engine(setting_engine)
        .with_handler(handler)
        .with_udp_addrs(vec!["0.0.0.0:19138"])
        .build()
        .await
        .expect("build pc");

    let client_offer_sdp = "v=0\r\n\
o=- 7752464448080962985 2 IN IP4 127.0.0.1\r\n\
s=-\r\n\
t=0 0\r\n\
a=group:BUNDLE 0\r\n\
m=application 39240 UDP/DTLS/SCTP webrtc-datachannel\r\n\
c=IN IP4 192.168.100.205\r\n\
a=ice-ufrag:+/RF\r\n\
a=ice-pwd:cLVVKgIMcsGZGxK7pW5G+B7J\r\n\
a=fingerprint:sha-256 22:AC:EC:B0:D1:62:E9:CA:BE:0B:FC:9B:8A:2A:3F:A2:FB:05:1C:80:A7:6C:85:25:1C:40:5D:C7:19:8B:AF:66\r\n\
a=setup:actpass\r\n\
a=mid:0\r\n\
a=sctp-port:5000\r\n\
a=max-message-size:262144\r\n";

    let offer = RTCSessionDescription::offer(client_offer_sdp.to_string()).expect("offer");
    pc.set_remote_description(offer).await.expect("remote");

    let answer = pc.create_answer(None).await.expect("answer");
    pc.set_local_description(answer).await.expect("local");

    // Wait for gathering complete with timeout
    tokio::select! {
        _ = complete.notified() => println!("Gathering completed!"),
        _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => println!("Timed out"),
    }

    if let Some(desc) = pc.local_description().await {
        println!("Local description:\n{}", desc.sdp);
    }
}
