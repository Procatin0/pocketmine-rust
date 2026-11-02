//! NetherNet SDP Negotiation and ICE Candidate Management
//!
//! Faithful translation of Axolotl's PHP implementation:
//! - pocketmine\nethernet\negotiation\IceCandidate
//! - pocketmine\nethernet\negotiation\IceCandidateFormatter
//! - pocketmine\nethernet\negotiation\AnswerBuilder

use crate::identity::NetherNetIdentity;

/// RFC 8445 section 5.1.2.1 priority for a server-reflexive candidate
pub const SERVER_REFLEXIVE_PRIORITY: u32 = (100 << 24) + (65535 << 8) + 255; // 1694498815

/// Foundations are numbers in every stack clients are written against; these stay clear of gathered ones.
pub const ADVERTISED_FOUNDATION: u32 = 80000000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceCandidate {
    pub foundation: String,
    pub component: String,
    pub protocol: String,
    pub priority: u32,
    pub address: String,
    pub port: u16,
    pub candidate_type: String, // "host", "srflx", "relay"
    pub related_address: Option<String>,
    pub related_port: Option<u16>,
}

impl IceCandidate {
    pub const COMPONENT: &'static str = "1";
    pub const TYPE_HOST: &'static str = "host";
    pub const TYPE_SERVER_REFLEXIVE: &'static str = "srflx";
    pub const TYPE_RELAY: &'static str = "relay";

    /// Parse candidate string without the "candidate:" prefix (or with it, stripped automatically)
    pub fn parse(value: &str) -> Option<Self> {
        let clean = value
            .strip_prefix("a=candidate:")
            .unwrap_or(value)
            .strip_prefix("candidate:")
            .unwrap_or(value)
            .trim();

        let parts: Vec<&str> = clean.split_whitespace().collect();
        if parts.len() < 8 || parts[6] != "typ" {
            return None;
        }

        let foundation = parts[0].to_string();
        let component = parts[1].to_string();
        let protocol = parts[2].to_lowercase();
        let priority = parts[3].parse::<u32>().ok()?;
        let address = parts[4].to_string();
        let port = parts[5].parse::<u16>().ok()?;
        let candidate_type = parts[7].to_string();

        let mut related_address = None;
        let mut related_port = None;

        let mut i = 8;
        while i + 1 < parts.len() {
            if parts[i] == "raddr" {
                related_address = Some(parts[i + 1].to_string());
            } else if parts[i] == "rport" {
                related_port = parts[i + 1].parse::<u16>().ok();
            }
            i += 2;
        }

        Some(Self {
            foundation,
            component,
            protocol,
            priority,
            address,
            port,
            candidate_type,
            related_address,
            related_port,
        })
    }

    /// Builds a candidate for an address this host is reached at but did not gather.
    pub fn server_reflexive(
        foundation: &str,
        address: &str,
        port: u16,
        priority: u32,
    ) -> Self {
        Self {
            foundation: foundation.to_string(),
            component: Self::COMPONENT.to_string(),
            protocol: "udp".to_string(),
            priority,
            address: address.to_string(),
            port,
            candidate_type: Self::TYPE_SERVER_REFLEXIVE.to_string(),
            related_address: Some(if address.contains(':') { "::".to_string() } else { "0.0.0.0".to_string() }),
            related_port: Some(0),
        }
    }

    pub fn without_related_address(&self) -> Self {
        Self {
            foundation: self.foundation.clone(),
            component: self.component.clone(),
            protocol: self.protocol.clone(),
            priority: self.priority,
            address: self.address.clone(),
            port: self.port,
            candidate_type: self.candidate_type.clone(),
            related_address: None,
            related_port: None,
        }
    }

    pub fn is_host(&self) -> bool {
        self.candidate_type == Self::TYPE_HOST
    }

    pub fn is_udp_host(&self) -> bool {
        self.is_host() && self.protocol == "udp"
    }

    pub fn is_ipv6(&self) -> bool {
        self.address.contains(':')
    }

    /// Returns candidate string without the "candidate:" prefix.
    pub fn to_candidate_string(&self) -> String {
        let mut parts = vec![
            self.foundation.clone(),
            self.component.clone(),
            self.protocol.clone(),
            self.priority.to_string(),
            self.address.clone(),
            self.port.to_string(),
            "typ".to_string(),
            self.candidate_type.clone(),
        ];

        if self.candidate_type == Self::TYPE_SERVER_REFLEXIVE || self.candidate_type == Self::TYPE_RELAY {
            parts.push("raddr".to_string());
            parts.push(
                self.related_address
                    .clone()
                    .unwrap_or_else(|| if self.is_ipv6() { "::".to_string() } else { "0.0.0.0".to_string() }),
            );
            parts.push("rport".to_string());
            parts.push(self.related_port.unwrap_or(0).to_string());
        }

        parts.join(" ")
    }
}

pub struct IceCandidateFormatter;

impl IceCandidateFormatter {
    pub const PREFIX: &'static str = "candidate:";

    /// Builds a server-reflexive candidate for a signaling peer if the offer does not carry it.
    pub fn signaling_peer(offer_sdp: &str, peer_address: &str) -> Option<String> {
        let mut host_port = None;
        let peer_clean = peer_address.trim();

        for line in offer_sdp.lines() {
            let line = line.trim();
            if let Some(cand_str) = line.strip_prefix("a=candidate:").or_else(|| line.strip_prefix("candidate:")) {
                if let Some(cand) = IceCandidate::parse(cand_str) {
                    if cand.address == peer_clean {
                        return None;
                    }
                    if host_port.is_none() && cand.is_udp_host() {
                        host_port = Some(cand.port);
                    }
                }
            }
        }

        let port = host_port?;
        let candidate = IceCandidate::server_reflexive("signaling", peer_clean, port, SERVER_REFLEXIVE_PRIORITY);
        Some(format!("{}{}", Self::PREFIX, candidate.to_candidate_string()))
    }

    /// Rewrites gathered candidates:
    /// - Drops internal host candidates that are not listed in advertised addresses
    /// - Formats each advertised address as a server-reflexive candidate with foundation 80000000 + n
    pub fn advertise(candidates: &[String], addresses: &[String]) -> Vec<String> {
        let mut kept = Vec::new();
        let mut matched_addresses = std::collections::HashSet::new();
        let mut udp_host_port: Option<u16> = None;

        for val in candidates {
            if let Some(cand) = IceCandidate::parse(val) {
                if cand.is_udp_host() && udp_host_port.is_none() {
                    udp_host_port = Some(cand.port);
                }
                let is_listed = addresses.iter().any(|a| a == &cand.address);
                if is_listed {
                    matched_addresses.insert(cand.address.clone());
                }
                if !cand.is_host() {
                    kept.push(cand.without_related_address().to_candidate_string());
                } else if is_listed {
                    kept.push(val.clone());
                }
                // Host candidates on other addresses (e.g. internal docker IPs) are DROPPED.
            }
        }

        let port = udp_host_port.unwrap_or(19133);
        let mut announced = 0u32;
        for address in addresses {
            if matched_addresses.contains(address) {
                continue;
            }

            let foundation = format!("{}", ADVERTISED_FOUNDATION + announced);
            let priority = SERVER_REFLEXIVE_PRIORITY - (announced << 8);
            let candidate = IceCandidate::server_reflexive(&foundation, address, port, priority);
            kept.push(candidate.to_candidate_string());
            announced += 1;
        }

        kept
    }

    /// Returns a gathered candidate in the form it is signaled to a client in.
    pub fn format(candidate: &str, ufrag: &str, index: usize) -> Option<String> {
        let parsed = IceCandidate::parse(candidate)?;
        Some(format!(
            "{}{} generation 0 ufrag {} network-id {} network-cost 0",
            Self::PREFIX,
            parsed.to_candidate_string(),
            ufrag,
            index
        ))
    }
}

pub struct AnswerBuilder {
    pub advertised_addresses: Vec<String>,
}

impl AnswerBuilder {
    pub fn new(advertised_addresses: Vec<String>) -> Self {
        Self { advertised_addresses }
    }

    /// Builds the final SDP answer strictly matching Axolotl's AnswerBuilder.php:
    /// 1. Extracts fingerprint & ice-ufrag from local description
    /// 2. Rewrites candidates using IceCandidateFormatter::advertise
    /// 3. Resets connection address to c=IN IP4 0.0.0.0
    /// 4. Generates RFC 8827 a=identity assertion and inserts it before media line
    pub fn build(
        &self,
        local_sdp: &str,
        identity: &NetherNetIdentity,
    ) -> Result<String, String> {
        let lines: Vec<&str> = local_sdp.lines().collect();

        // 1. Extract fingerprint
        let mut fingerprint = None;
        for line in &lines {
            if let Some(fp) = line.strip_prefix("a=fingerprint:sha-256 ") {
                fingerprint = Some(fp.trim().to_string());
                break;
            }
        }
        let fingerprint = fingerprint.ok_or_else(|| "Local description has no sha-256 fingerprint".to_string())?;

        // 2. Extract local gathered candidates
        let mut local_candidates = Vec::new();
        for line in &lines {
            if let Some(cand) = line.strip_prefix("a=candidate:") {
                local_candidates.push(cand.to_string());
            }
        }

        // 3. Rewrite candidates with advertised addresses
        let advertised = IceCandidateFormatter::advertise(&local_candidates, &self.advertised_addresses);
        if advertised.is_empty() {
            return Err("No candidates available to answer with".to_string());
        }

        // 4. Issue identity assertion
        let assertion = identity.create_assertion(&fingerprint);

        // 5. Reconstruct SDP lines
        let mut result_lines = Vec::new();
        let mut identity_inserted = false;
        let mut candidates_inserted = false;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Insert identity attribute right before the first m= line
            if trimmed.starts_with("m=") && !identity_inserted {
                result_lines.push(format!("a=identity:{}", assertion));
                identity_inserted = true;
            }

            // Replace connection address line: RFC 9429 section 5.2.1 placeholder
            if trimmed.starts_with("c=") {
                result_lines.push("c=IN IP4 0.0.0.0".to_string());
                continue;
            }

            // Replace all candidates with advertised candidates
            if trimmed.starts_with("a=candidate:") || trimmed == "a=end-of-candidates" {
                if !candidates_inserted {
                    for cand in &advertised {
                        result_lines.push(format!("a=candidate:{}", cand));
                    }
                    candidates_inserted = true;
                }
                continue;
            }

            result_lines.push(trimmed.to_string());
        }

        if !identity_inserted {
            result_lines.push(format!("a=identity:{}", assertion));
        }
        if !candidates_inserted {
            for cand in &advertised {
                result_lines.push(format!("a=candidate:{}", cand));
            }
        }

        Ok(result_lines.join("\r\n") + "\r\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ice_candidate_parse_and_server_reflexive() {
        let srflx = IceCandidate::server_reflexive("80000000", "192.168.100.51", 19133, SERVER_REFLEXIVE_PRIORITY);
        assert_eq!(
            srflx.to_candidate_string(),
            "80000000 1 udp 1694498815 192.168.100.51 19133 typ srflx raddr 0.0.0.0 rport 0"
        );

        let parsed = IceCandidate::parse(&srflx.to_candidate_string()).unwrap();
        assert_eq!(parsed.foundation, "80000000");
        assert_eq!(parsed.address, "192.168.100.51");
        assert_eq!(parsed.port, 19133);
        assert_eq!(parsed.candidate_type, "srflx");
    }

    #[test]
    fn test_advertise_drops_internal_and_adds_external() {
        let gathered = vec![
            "2342875175 1 udp 2130706431 172.17.0.3 19133 typ host".to_string(),
            "2342875175 2 udp 2130706431 172.17.0.3 19133 typ host".to_string(),
        ];
        let advertised_addrs = vec!["192.168.100.51".to_string()];

        let result = IceCandidateFormatter::advertise(&gathered, &advertised_addrs);
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("80000000 1 udp 1694498815 192.168.100.51 19133 typ srflx raddr 0.0.0.0 rport 0"));
        assert!(!result[0].contains("172.17.0.3"));
    }

    #[test]
    fn test_signaling_peer_candidate() {
        let offer_sdp = "v=0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=candidate:1 1 udp 2122260199 10.0.0.2 54321 typ host\r\n";
        let cand = IceCandidateFormatter::signaling_peer(offer_sdp, "192.168.100.205");
        assert!(cand.is_some());
        let cand_str = cand.unwrap();
        assert!(cand_str.contains("signaling 1 udp 1694498815 192.168.100.205 54321 typ srflx raddr 0.0.0.0 rport 0"));
    }
}
