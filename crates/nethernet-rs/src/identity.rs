//! NetherNet RFC 8827 Self-Signed Identity Assertion Generator
//!
//! Generates valid P-384 ECDSA self-signed JWT identity assertions required by Minecraft Bedrock
//! when connecting via NetherNet signaling.

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use p384::ecdsa::{signature::Signer, Signature, SigningKey, VerifyingKey};
use p384::pkcs8::EncodePublicKey;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct NetherNetIdentity {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    pub_key_b64_der: String,
}

impl NetherNetIdentity {
    pub fn new() -> Self {
        // P-384 private key is 48 bytes
        // Use a static deterministic seed or getrandom if available
        let mut seed = [0u8; 48];
        for (i, b) in seed.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(0x42);
        }
        let signing_key = SigningKey::from_slice(&seed).expect("Valid P-384 private key seed");
        let verifying_key = *signing_key.verifying_key();
        let der = verifying_key
            .to_public_key_der()
            .expect("Failed to encode P-384 public key to DER");
        let pub_key_b64_der = STANDARD.encode(der.as_bytes());

        Self {
            signing_key,
            verifying_key,
            pub_key_b64_der,
        }
    }

    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }

    /// Sign data using ES384 (ECDSA with P-384 and SHA-384)
    fn sign_es384(&self, data: &[u8]) -> Signature {
        self.signing_key.sign(data)
    }

    /// Generate an RFC 8827 `a=identity` assertion value bound to the given DTLS fingerprint digest.
    ///
    /// The digest should be in format `AA:BB:CC:...` or hex string.
    pub fn create_assertion(&self, fingerprint_digest: &str) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 1. Build JWT Token
        let jwt_header = json!({
            "alg": "ES384",
            "x5u": self.pub_key_b64_der
        })
        .to_string();
        let jwt_header_b64 = URL_SAFE_NO_PAD.encode(jwt_header.as_bytes());

        let jwt_claims = json!({
            "exp": now + 3600,
            "iat": now,
            "cpk": self.pub_key_b64_der
        })
        .to_string();
        let jwt_claims_b64 = URL_SAFE_NO_PAD.encode(jwt_claims.as_bytes());

        let jwt_signing_input = format!("{}.{}", jwt_header_b64, jwt_claims_b64);
        let jwt_sig: Signature = self.sign_es384(jwt_signing_input.as_bytes());
        let jwt_sig_b64 = URL_SAFE_NO_PAD.encode(jwt_sig.to_bytes());
        let compact_jwt = format!("{}.{}", jwt_signing_input, jwt_sig_b64);

        // 2. Build Detached JWS for DTLS Fingerprint
        let canonical_fp = format!(
            r#"{{"fingerprint":[{{"algorithm":"sha-256","digest":"{}"}}]}}"#,
            fingerprint_digest
        );
        let jws_header = json!({"alg": "ES384"}).to_string();
        let jws_header_b64 = URL_SAFE_NO_PAD.encode(jws_header.as_bytes());
        let canonical_fp_b64 = URL_SAFE_NO_PAD.encode(canonical_fp.as_bytes());

        let jws_signing_input = format!("{}.{}", jws_header_b64, canonical_fp_b64);
        let jws_sig: Signature = self.sign_es384(jws_signing_input.as_bytes());
        let jws_sig_b64 = URL_SAFE_NO_PAD.encode(jws_sig.to_bytes());
        let detached_jws = format!("{}..{}", jws_header_b64, jws_sig_b64);

        // 3. Build Inner Assertion JSON string
        let inner_assertion = json!({
            "fingerprints": detached_jws,
            "token": compact_jwt
        })
        .to_string();

        // 4. Build Envelope JSON
        let envelope = json!({
            "assertion": inner_assertion,
            "idp": {
                "domain": "self",
                "protocol": "default"
            }
        })
        .to_string();

        // 5. Standard base64 encoding for the `a=identity:` attribute
        STANDARD.encode(envelope.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_assertion_generation() {
        let identity = NetherNetIdentity::new();
        let fp = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";
        let encoded = identity.create_assertion(fp);

        // Verify base64 decoding
        let decoded_bytes = STANDARD.decode(&encoded).expect("Should be valid base64");
        let envelope: serde_json::Value =
            serde_json::from_slice(&decoded_bytes).expect("Should be valid JSON");

        assert_eq!(envelope["idp"]["domain"], "self");
        assert_eq!(envelope["idp"]["protocol"], "default");

        let assertion_str = envelope["assertion"]
            .as_str()
            .expect("assertion must be a JSON string");
        let inner: serde_json::Value =
            serde_json::from_str(assertion_str).expect("inner assertion must be JSON");

        assert!(inner["token"].as_str().is_some());
        assert!(inner["fingerprints"].as_str().is_some());

        let token_parts: Vec<&str> = inner["token"].as_str().unwrap().split('.').collect();
        assert_eq!(token_parts.len(), 3);

        let jws_parts: Vec<&str> = inner["fingerprints"].as_str().unwrap().split('.').collect();
        assert_eq!(jws_parts.len(), 3);
        assert_eq!(jws_parts[1], ""); // detached
    }
}
