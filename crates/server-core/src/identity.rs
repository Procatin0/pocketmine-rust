//! Minimal Bedrock login identity extraction.
//!
//! The login payload is a JSON string containing the JWT chain plus a client
//! data JWT. We only need `displayName` and the XUID, so a tiny base64url
//! decoder and JSON walker are enough (no signature verification: offline
//! servers cannot validate Xbox chains anyway).

use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoginIdentity {
    pub display_name: String,
    pub xuid: String,
    pub uuid: String,
    pub language: String,
}

impl LoginIdentity {
    /// Stable positive id derived from the XUID (or the display name).
    pub fn unique_id(&self) -> i64 {
        let source = if self.xuid.is_empty() {
            &self.display_name
        } else {
            &self.xuid
        };
        // FNV-1a 64-bit, masked into a positive i64.
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in source.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        (hash & 0x7fff_ffff_ffff_ffff) as i64
    }
}

/// Parses the identity from `LoginPacket.connection_request`.
///
/// The connection_request normally contains JWT tokens (chain + client data),
/// but in offline/local mode it may be a plain JSON object or even a bare
/// player name. This function falls back through several strategies so the
/// player's real name is used whenever possible.
pub fn parse_login_identity(connection_request: &str) -> LoginIdentity {
    tracing::info!("Login connection_request raw len={} head={}", connection_request.len(), &connection_request.chars().take(300).collect::<String>());
    // The connection_request from the GDK client is length-prefixed binary:
    // `T\0\0\0{"AuthenticationType":0,"Token":"<jwt>",...}` — first strip any
    // non-JSON prefix and try parsing the inner JSON for JWT tokens.
    let json_part = match connection_request.find('{') {
        Some(idx) => &connection_request[idx..],
        None => connection_request,
    };
    let connection_request = json_part;

    let mut identity = LoginIdentity::default();

    // Strategy 1: Extract JWT tokens (online mode with Xbox auth).
    let tokens = collect_jwt_tokens(connection_request);
    tracing::info!("Login identity: {} jwt tokens found", tokens.len());
    for (idx, token) in tokens.iter().enumerate() {
        if let Some(payload) = decode_jwt_payload(token) {
            let raw = payload.to_string();
            tracing::info!("Login identity token {} payload len={} preview={}", idx, raw.len(), &raw.chars().take(300).collect::<String>());
            // Save huge token payload for external analysis (skin data is ~440k)
            if idx == 1 {
                std::fs::write("C:\\Users\\matic\\AppData\\Local\\Temp\\opencode\\clientdata_payload.json", raw.as_bytes()).ok();
                // also save raw raw client_token to inspect
                std::fs::write("C:\\Users\\matic\\AppData\\Local\\Temp\\opencode\\clientdata_jwt.txt", token.as_bytes()).ok();
            }
            // Debug: show if displayName exists anywhere nested
            fn contains_key(v: &Value, key: &str) -> bool {
                match v {
                    Value::Object(map) => map.keys().any(|k| k == key) || map.values().any(|x| contains_key(x, key)),
                    Value::Array(a) => a.iter().any(|x| contains_key(x, key)),
                    _ => false,
                }
            }
            tracing::info!("Login identity token {} has displayName={} extraData={} ThirdPartyName={} XUID={}", idx, contains_key(&payload, "displayName"), contains_key(&payload, "extraData"), contains_key(&payload, "ThirdPartyName"), contains_key(&payload, "XUID"));
            apply_claims(&payload, &mut identity);
        } else {
            tracing::info!("Login identity token {} payload decode FAILED", idx);
        }
    }

    // Strategy 2: If no JWT was found, try parsing as plain JSON and look for
    // common name fields (offline/local mode).
    if identity.display_name.is_empty() {
        if let Ok(value) = serde_json::from_str::<Value>(connection_request) {
            if let Some(name) = value
                .get("displayName")
                .and_then(Value::as_str)
                .or_else(|| value.get("name").and_then(Value::as_str))
                .or_else(|| value.get("username").and_then(Value::as_str))
                .filter(|n| !n.is_empty())
            {
                identity.display_name = name.to_string();
            }
            if identity.xuid.is_empty() {
                if let Some(xuid) = value.get("xuid").and_then(Value::as_str) {
                    identity.xuid = xuid.to_string();
                }
            }
        }
    }

    // Strategy 3: Last resort — a bare string is a player name only when it is a
    // valid username (`Player::isValidUserName`), so malformed login data falls
    // through to the `Player` default instead of leaking as a nickname.
    if identity.display_name.is_empty() {
        let trimmed = connection_request.trim();
        let valid = !trimmed.is_empty()
            && trimmed.chars().count() <= 16
            && trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ' ');
        if valid {
            identity.display_name = trimmed.to_string();
        }
    }

    if identity.display_name.is_empty() {
        identity.display_name = "Player".to_string();
    }
    identity
}

/// Extracts strings that look like JWTs (three dot-separated segments).
fn collect_jwt_tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    if let Ok(value) = serde_json::from_str::<Value>(input) {
        collect_from_value(&value, &mut tokens, 0);
    }

    // Also scan raw text for dot-separated base64url runs. The login payload
    // is normally `{"chain":[...]}.<clientDataJwt>`, which is not valid JSON
    // as a whole, so the trailing client-data JWT would be glued to `]}`
    // without broader delimiters (e.g. `]}.h2.p2.sig`). Split on every JSON
    // structural char and trim stray dots so both tokens are recovered.
    // However the GDK client sends length-prefixed binary headers like
    // `T\0\0\0{"AuthenticationType":0,"Token":"eyJ..."}` where the client-data
    // token sits directly after the chain payload without a clean delimiter
    // (leading `\?\?`). For candidates with base64 chars wrapped around junk,
    // trim to the first valid base64 boundary manually.
    let mut scanned = 0usize;
    let mut rejected = 0usize;
    for candidate in input.split(|c: char| {
        c == '"'
            || c == '\''
            || c.is_whitespace()
            || c == ','
            || c == '{'
            || c == '}'
            || c == '['
            || c == ']'
            || c == ':'
    }) {
        let mut candidate = candidate.trim_matches(|c: char| {
            c == '{' || c == '}' || c == '[' || c == ']' || c == '.' || c == ':' || c == ';' || c == '?'
        });
        // If the candidate has a leading non-base64 prefix before the JWT header
        // (GDK `??\b\0eyJ...`), cut to the first 'ey' that decodes to {"alg":
        // otherwise is_jwt correctly rejects it but we lose the token.
        if !is_jwt(candidate) {
            if let Some(pos) = candidate.find("eyJ") {
                candidate = &candidate[pos..];
            }
        }
        scanned += 1;
        if is_jwt(candidate) && !tokens.iter().any(|t| t == candidate) {
            tokens.push(candidate.to_string());
        } else if candidate.split('.').count() == 3 && candidate.len() > 200 {
            rejected += 1;
            tracing::info!("Login identity: rejecting long candidate len={} head={}", candidate.len(), &candidate.chars().take(120).collect::<String>());
        }
    }
    tracing::info!("Login identity: scanned {} candidates, rejected large-dot {} ones", scanned, rejected);

    tokens
}

fn collect_from_value(value: &Value, tokens: &mut Vec<String>, depth: usize) {
    if depth > 8 {
        return;
    }
    match value {
        Value::String(text) => {
            if is_jwt(text) && !tokens.iter().any(|t| t == text) {
                tokens.push(text.clone());
            } else if text.contains("chain") || text.starts_with('{') {
                if let Ok(nested) = serde_json::from_str::<Value>(text) {
                    collect_from_value(&nested, tokens, depth + 1);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_from_value(item, tokens, depth + 1);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_from_value(item, tokens, depth + 1);
            }
        }
        _ => {}
    }
}

fn is_jwt(candidate: &str) -> bool {
    let segments: Vec<&str> = candidate.split('.').collect();
    segments.len() == 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'=')
        })
}

fn decode_jwt_payload(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64url_decode(payload)?;
    serde_json::from_slice(&bytes).ok()
}

fn apply_claims(payload: &Value, identity: &mut LoginIdentity) {
    if identity.display_name.is_empty() {
        if let Some(name) = payload.get("ThirdPartyName").and_then(Value::as_str) {
            identity.display_name = name.to_string();
        } else if let Some(name) = payload.get("displayName").and_then(Value::as_str) {
            identity.display_name = name.to_string();
        } else if let Some(name) = payload
            .get("extraData")
            .and_then(|extra| extra.get("displayName"))
            .and_then(Value::as_str)
        {
            identity.display_name = name.to_string();
        } else if let Some(name) = payload
            .get("extraData")
            .and_then(|extra| extra.get("displayName"))
            .and_then(Value::as_str)
        {
            identity.display_name = name.to_string();
        }
    }

    if identity.xuid.is_empty() {
        if let Some(xuid) = payload.get("xuid").and_then(Value::as_str) {
            identity.xuid = xuid.to_string();
        } else if let Some(xuid) = payload
            .get("extraData")
            .and_then(|extra| extra.get("XUID"))
            .and_then(Value::as_str)
        {
            identity.xuid = xuid.to_string();
        }
    }

    if identity.uuid.is_empty() {
        if let Some(identity_uuid) = payload.get("identity").and_then(Value::as_str) {
            identity.uuid = identity_uuid.to_string();
        }
    }

    if identity.language.is_empty() {
        if let Some(language) = payload
            .get("languageCode")
            .or_else(|| payload.get("extraData").and_then(|e| e.get("languageCode")))
            .and_then(Value::as_str)
        {
            identity.language = language.to_string();
        }
    }
}

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;

    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => (byte - b'A') as u32,
            b'a'..=b'z' => (byte - b'a' + 26) as u32,
            b'0'..=b'9' => (byte - b'0' + 52) as u32,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }

    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_base64url(data: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = *chunk.get(1).unwrap_or(&0) as u32;
            let b2 = *chunk.get(2).unwrap_or(&0) as u32;
            let triple = (b0 << 16) | (b1 << 8) | b2;
            out.push(TABLE[(triple >> 18) as usize & 63] as char);
            out.push(TABLE[(triple >> 12) as usize & 63] as char);
            if chunk.len() > 1 {
                out.push(TABLE[(triple >> 6) as usize & 63] as char);
            }
            if chunk.len() > 2 {
                out.push(TABLE[triple as usize & 63] as char);
            }
        }
        out
    }

    fn fake_jwt(claims: &Value) -> String {
        let header = encode_base64url(br#"{"alg":"none","typ":"JWT"}"#);
        let payload = encode_base64url(claims.to_string().as_bytes());
        format!("{}.{}.signature", header, payload)
    }

    #[test]
    fn parses_chain_and_client_data() {
        let chain = serde_json::json!({
            "chain": [fake_jwt(&serde_json::json!({"xuid": "2535412345678901"}))]
        });
        let client = fake_jwt(&serde_json::json!({
            "extraData": {
                "displayName": "Steve",
                "XUID": "2535412345678901",
                "languageCode": "es_ES"
            }
        }));
        let request = format!("{}.{}", chain, client);

        let identity = parse_login_identity(&request);
        assert_eq!(identity.display_name, "Steve");
        assert_eq!(identity.xuid, "2535412345678901");
        assert_eq!(identity.language, "es_ES");
        assert!(identity.unique_id() > 0);
    }

    #[test]
    fn falls_back_to_player_name() {
        let identity = parse_login_identity("not-a-jwt");
        assert_eq!(identity.display_name, "Player");
        assert_ne!(identity.unique_id(), 0);
    }

    #[test]
    fn base64url_roundtrip() {
        let data = b"{\"displayName\":\"Alex\"}";
        let encoded = encode_base64url(data);
        assert_eq!(base64url_decode(&encoded).unwrap(), data);
    }
}
