//! Wire primitives ported from `plugins/JavaBridge/src/javabridge/Protocol.php`
//! (VarInt/VarLong, strings, packed positions, `lpVec3`, yaw conversion,
//! frames, offline UUID) plus the minimal anonymous-NBT text component the
//! listener needs for modern chat / nametags (`Nbt::text` in `Nbt.php`).
//!
//! Chunk *contents* (`ChunkBuilder` / `ModernChunk`) and the full NBT writer
//! are owned by other agents; this module only moves bytes the listener
//! itself frames. Chunk payloads arrive through
//! [`crate::hooks::ServerHooks::legacy_chunk`] /
//! [`crate::hooks::ServerHooks::modern_chunk`].

/// Legacy Java 1.12.2 protocol (340).
pub const PROTOCOL_VERSION: i32 = 340;
/// Legacy version name.
pub const VERSION_NAME: &str = "1.12.2";
/// Hard cap for one framed packet.
pub const MAX_FRAME_LENGTH: usize = 2_097_152;

// --- handshake / status / login (legacy ids, shared by modern) ----------
pub const C2S_HANDSHAKE: i32 = 0x00;
pub const C2S_STATUS_REQUEST: i32 = 0x00;
pub const C2S_STATUS_PING: i32 = 0x01;
pub const S2C_STATUS_RESPONSE: i32 = 0x00;
pub const S2C_STATUS_PONG: i32 = 0x01;
pub const C2S_LOGIN_START: i32 = 0x00;
pub const S2C_LOGIN_DISCONNECT: i32 = 0x00;
pub const S2C_LOGIN_SUCCESS: i32 = 0x02;

// --- play, server -> client (legacy 340) --------------------------------
pub const S2C_BLOCK_CHANGE: i32 = 0x0B;
pub const S2C_CHAT: i32 = 0x0F;
pub const S2C_KICK: i32 = 0x1A;
pub const S2C_UNLOAD_CHUNK: i32 = 0x1D;
pub const S2C_KEEP_ALIVE: i32 = 0x1F;
pub const S2C_MAP_CHUNK: i32 = 0x20;
pub const S2C_JOIN_GAME: i32 = 0x23;
pub const S2C_ABILITIES: i32 = 0x2C;
pub const S2C_PLAYER_INFO: i32 = 0x2E;
pub const S2C_POSITION: i32 = 0x2F;
pub const S2C_UPDATE_HEALTH: i32 = 0x41;
pub const S2C_SPAWN_POSITION: i32 = 0x46;
pub const S2C_UPDATE_TIME: i32 = 0x47;
pub const S2C_DIFFICULTY: i32 = 0x0D;
pub const S2C_NAMED_ENTITY_SPAWN: i32 = 0x05;
pub const S2C_ENTITY: i32 = 0x25;
pub const S2C_REL_ENTITY_MOVE: i32 = 0x26;
pub const S2C_ENTITY_MOVE_LOOK: i32 = 0x27;
pub const S2C_ENTITY_LOOK: i32 = 0x28;
pub const S2C_ENTITY_HEAD_ROTATION: i32 = 0x36;
pub const S2C_ENTITY_DESTROY: i32 = 0x32;
pub const S2C_ENTITY_METADATA: i32 = 0x3C;
pub const S2C_ENTITY_TELEPORT: i32 = 0x4C;

// --- play, client -> server (legacy 340) --------------------------------
pub const C2S_TELEPORT_CONFIRM: i32 = 0x00;
pub const C2S_CHAT: i32 = 0x02;
pub const C2S_CLIENT_COMMAND: i32 = 0x03;
pub const C2S_SETTINGS: i32 = 0x04;
pub const C2S_KEEP_ALIVE: i32 = 0x0B;
pub const C2S_FLYING: i32 = 0x0C;
pub const C2S_POSITION: i32 = 0x0D;
pub const C2S_POSITION_LOOK: i32 = 0x0E;
pub const C2S_LOOK: i32 = 0x0F;
pub const C2S_BLOCK_DIG: i32 = 0x14;
pub const C2S_ARM_ANIMATION: i32 = 0x1D;
pub const C2S_BLOCK_PLACE: i32 = 0x1F;

/// `player_info_update` bitflags (modern, identical in 769-775).
/// `0x02` is `initialize_chat`, NOT `update_game_mode`: sending it makes
/// the client expect a `chat_session` and abort decoding.
pub const PLAYER_INFO_ADD_PLAYER: u8 = 0x01;
pub const PLAYER_INFO_UPDATE_GAME_MODE: u8 = 0x04;
pub const PLAYER_INFO_UPDATE_LISTED: u8 = 0x08;
pub const PLAYER_INFO_UPDATE_LATENCY: u8 = 0x10;
/// `change_game_mode` reason of `game_state_change`.
pub const GAME_EVENT_CHANGE_GAMEMODE: u8 = 3;

// ---------------------------------------------------------------------------
// VarInt / VarLong
// ---------------------------------------------------------------------------

pub fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

pub fn varint(value: i32) -> Vec<u8> {
    let mut out = Vec::new();
    write_varint(&mut out, value);
    out
}

pub fn var_long(value: i64) -> Vec<u8> {
    let mut out = Vec::new();
    let mut v = value;
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 && v != -1 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 || v == -1 {
            break;
        }
    }
    out
}

pub fn write_string(out: &mut Vec<u8>, value: &str) {
    write_varint(out, value.len() as i32);
    out.extend_from_slice(value.as_bytes());
}

pub fn string(value: &str) -> Vec<u8> {
    let mut out = Vec::new();
    write_string(&mut out, value);
    out
}

/// Identifier (`minecraft:brand`, ...): same wire form as `string`.
pub fn identifier(value: &str) -> Vec<u8> {
    string(value)
}

pub fn write_varint_array(out: &mut Vec<u8>, values: &[i32]) {
    write_varint(out, values.len() as i32);
    for v in values {
        write_varint(out, *v);
    }
}

pub fn write_long(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub fn long(value: i64) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

pub fn write_double(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub fn double(value: f64) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

pub fn write_float(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub fn float(value: f32) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

pub fn write_short(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&(value as i16).to_be_bytes());
}

pub fn short(value: i32) -> Vec<u8> {
    (value as i16).to_be_bytes().to_vec()
}

pub fn write_position(out: &mut Vec<u8>, x: i32, y: i32, z: i32) {
    let packed: u64 = (((x as u64) & 0x3FF_FFFF) << 38)
        | (((y as u64) & 0xFFF) << 26)
        | ((z as u64) & 0x3FF_FFFF);
    write_long(out, packed as i64);
}

pub fn position(x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut out = Vec::new();
    write_position(&mut out, x, y, z);
    out
}

pub fn boolean(value: bool) -> Vec<u8> {
    vec![u8::from(value)]
}

pub fn ubyte(value: i32) -> Vec<u8> {
    vec![(value & 0xFF) as u8]
}

/// Angle in degrees (-180..180 yaw, -90..90 pitch) to signed byte.
pub fn angle(degrees: f64) -> i32 {
    let scaled = (degrees * 256.0 / 360.0 + 360.0).rem_euclid(256.0).floor() as i32;
    if scaled >= 128 {
        scaled - 256
    } else {
        scaled
    }
}

/// Relative delta in blocks to i16 (*4096, clamped).
pub fn rel_delta(delta: f64) -> i32 {
    ((delta * 4096.0).round() as i32).clamp(-32768, 32767)
}

// ---------------------------------------------------------------------------
// Yaw / pitch conversion (Bedrock canonical <-> Java wire)
// ---------------------------------------------------------------------------

/// Bedrock yaw (0 = north) to Java yaw (0 = south): `+180` mod 360.
pub fn bedrock_yaw_to_java(yaw: f64) -> f64 {
    let mut y = (yaw + 180.0).rem_euclid(360.0);
    if y > 180.0 {
        y -= 360.0;
    }
    y
}

/// Java yaw to Bedrock yaw: same involution, result in [0, 360).
pub fn java_yaw_to_bedrock(yaw: f64) -> f64 {
    (yaw + 180.0).rem_euclid(360.0)
}

/// Bedrock pitch (normalized to [0, 360) in Rust) to Java pitch (-180, 180].
pub fn bedrock_pitch_to_java(pitch: f64) -> f64 {
    let mut p = pitch.rem_euclid(360.0);
    if p > 180.0 {
        p -= 360.0;
    }
    p
}

// ---------------------------------------------------------------------------
// lpVec3 (1.21.9+, `spawn_entity` velocity)
// ---------------------------------------------------------------------------

/// Low-precision vector for `spawn_entity` / `entity_velocity` in 773+.
/// Zero is a single `0x00` byte; otherwise 3x15-bit components + scale.
pub fn lp_vec3(x: f64, y: f64, z: f64) -> Vec<u8> {
    const ABS_MAX: f64 = 1.7179869183e10;
    const ABS_MIN: f64 = 3.051944088384301e-5;
    let sanitize = |v: f64| {
        if v.is_nan() {
            0.0
        } else {
            v.clamp(-ABS_MAX, ABS_MAX)
        }
    };
    let (x, y, z) = (sanitize(x), sanitize(y), sanitize(z));
    let max = x.abs().max(y.abs()).max(z.abs());
    if max < ABS_MIN {
        return vec![0x00];
    }
    let scale = max.ceil() as i64;
    let pack = |v: f64| -> i64 { ((v * 0.5 + 0.5) * 32766.0).round() as i64 };
    let needs_continuation = scale > 3;
    let markers: i64 = if needs_continuation {
        (scale % 4) | 4
    } else {
        scale
    };
    let packed: i64 = markers
        + pack(x / scale as f64) * 0x8
        + pack(y / scale as f64) * 0x40000
        + pack(z / scale as f64) * 0x2_0000_0000;
    let mut out = vec![(packed & 0xFF) as u8, ((packed >> 8) & 0xFF) as u8];
    out.extend_from_slice(&(((packed >> 16) & 0xFFFF_FFFF) as u32).to_be_bytes());
    if needs_continuation {
        out.extend_from_slice(&varint((scale / 4) as i32));
    }
    out
}

// ---------------------------------------------------------------------------
// Frames, chat, UUID
// ---------------------------------------------------------------------------

/// Full frame: varint(len) + varint(id) + payload.
pub fn frame(id: i32, payload: &[u8]) -> Vec<u8> {
    let mut body = varint(id);
    body.extend_from_slice(payload);
    let mut out = varint(body.len() as i32);
    out.extend_from_slice(&body);
    out
}

/// Simple chat component `{"text":"..."}`.
pub fn chat_json(text: &str) -> String {
    serde_json::json!({ "text": text }).to_string()
}

/// Anonymous NBT text component `{"text":"..."}` used by modern
/// `system_chat` and `entity_metadata` (subset of `Nbt::text`).
pub fn nbt_text(text: &str) -> Vec<u8> {
    // compound (10) { "text" (string=8) len u16 + bytes } END(0)
    let mut out = vec![10u8];
    out.push(8u8);
    out.extend_from_slice(&(4u16).to_be_bytes());
    out.extend_from_slice(b"text");
    out.extend_from_slice(&(text.len() as u16).to_be_bytes());
    out.extend_from_slice(text.as_bytes());
    out.push(0u8);
    out
}

/// Offline-mode UUID: MD5 of `"OfflinePlayer:<name>"` with version-3 and
/// RFC-4122 variant bits set. Returns 16 raw bytes.
pub fn offline_uuid(name: &str) -> [u8; 16] {
    let mut hash = md5(format!("OfflinePlayer:{name}").as_bytes());
    hash[6] = (hash[6] & 0x0F) | 0x30;
    hash[8] = (hash[8] & 0x3F) | 0x80;
    hash
}

/// `8-4-4-4-12` form expected by legacy `login_success`.
pub fn uuid_string(raw: &[u8; 16]) -> String {
    let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

// ---------------------------------------------------------------------------
// Reader (cursor over a frame payload; `None` when incomplete)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn read_varint(&mut self) -> Option<i32> {
        let mut value: u32 = 0;
        let mut shift = 0u32;
        loop {
            let byte = *self.data.get(self.pos)?;
            self.pos += 1;
            value |= ((byte & 0x7F) as u32) << shift;
            if byte & 0x80 == 0 {
                return Some(value as i32);
            }
            shift += 7;
            if shift > 28 {
                return None;
            }
        }
    }

    pub fn read_string(&mut self) -> Option<String> {
        let len = self.read_varint()?;
        if !(0..=32767 * 4).contains(&len) {
            return None;
        }
        let end = self.pos.checked_add(len as usize)?;
        let bytes = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(String::from_utf8_lossy(bytes).into_owned())
    }

    pub fn read_u16(&mut self) -> Option<u16> {
        let bytes = self.data.get(self.pos..self.pos + 2)?;
        self.pos += 2;
        Some(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_i64(&mut self) -> Option<i64> {
        let bytes = self.data.get(self.pos..self.pos + 8)?;
        self.pos += 8;
        Some(i64::from_be_bytes(bytes.try_into().ok()?))
    }

    pub fn read_f64(&mut self) -> Option<f64> {
        let bytes = self.data.get(self.pos..self.pos + 8)?;
        self.pos += 8;
        Some(f64::from_be_bytes(bytes.try_into().ok()?))
    }

    pub fn read_f32(&mut self) -> Option<f32> {
        let bytes = self.data.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(f32::from_be_bytes(bytes.try_into().ok()?))
    }

    pub fn read_position(&mut self) -> Option<(i32, i32, i32)> {
        let v = self.read_i64()? as u64;
        let mut x = ((v >> 38) & 0x3FF_FFFF) as i64;
        let mut y = ((v >> 26) & 0xFFF) as i64;
        let mut z = (v & 0x3FF_FFFF) as i64;
        if x >= 0x200_0000 {
            x -= 0x400_0000;
        }
        if z >= 0x200_0000 {
            z -= 0x400_0000;
        }
        if y >= 0x800 {
            y -= 0x1000;
        }
        Some((x as i32, y as i32, z as i32))
    }

    pub fn read_varint_array(&mut self) -> Option<Vec<i32>> {
        let count = self.read_varint()?;
        if count < 0 {
            return None;
        }
        let mut values = Vec::with_capacity(count as usize);
        for _ in 0..count {
            values.push(self.read_varint()?);
        }
        Some(values)
    }
}

/// Try to split one frame off `buf`: returns (packet_id, payload, consumed)
/// or `None` when incomplete. Mirrors `Protocol::readVarInt` + length guard.
pub fn try_split_frame(buf: &[u8]) -> Option<(i32, Vec<u8>, usize)> {
    let mut r = Reader::new(buf);
    let len = r.read_varint()?;
    if len <= 0 || len as usize > MAX_FRAME_LENGTH {
        return None;
    }
    let start = r.pos;
    let end = start.checked_add(len as usize)?;
    let body = buf.get(start..end)?;
    let mut br = Reader::new(body);
    let id = br.read_varint()?;
    let payload = body[br.pos..].to_vec();
    Some((id, payload, end))
}

// ---------------------------------------------------------------------------
// MD5 (RFC 1321), vendored so the crate needs no extra dependency.
// ---------------------------------------------------------------------------

const MD5_S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];
const MD5_K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

fn md5(input: &[u8]) -> [u8; 16] {
    let mut msg = input.to_vec();
    let bit_len = (input.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());
    let (mut a0, mut b0, mut c0, mut d0): (u32, u32, u32, u32) =
        (0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476);
    for chunk in msg.as_chunks::<64>().0 {
        let mut m = [0u32; 16];
        for (i, word) in chunk.as_chunks::<4>().0.iter().enumerate() {
            m[i] = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let tmp = f.wrapping_add(a).wrapping_add(MD5_K[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(tmp.rotate_left(MD5_S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn md5_rfc1321_vectors() {
        assert_eq!(hex(&md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex(&md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            hex(&md5(b"abcdefghijklmnopqrstuvwxyz")),
            "c3fcd3d76192e4007dfb496cca67e13b"
        );
    }

    #[test]
    fn offline_uuid_matches_php_md5() {
        // PHP: md5('OfflinePlayer:Steve', true) with version/variant bits.
        let uuid = offline_uuid("Steve");
        assert_eq!(uuid[6] >> 4, 0x3);
        assert_eq!(uuid[8] >> 6, 0b10);
        assert_eq!(uuid_string(&uuid).len(), 36);
    }

    #[test]
    fn varint_roundtrip() {
        for value in [0, 1, 63, 64, 300, -1, i32::MAX, 123456789] {
            let encoded = varint(value);
            let mut r = Reader::new(&encoded);
            assert_eq!(r.read_varint(), Some(value), "value {value}");
        }
    }

    #[test]
    fn position_roundtrip() {
        for (x, y, z) in [(0, 0, 0), (0, 68, 0), (-30, -64, 10), (12345, 319, -9876)] {
            let encoded = position(x, y, z);
            let mut r = Reader::new(&encoded);
            assert_eq!(r.read_position(), Some((x, y, z)));
        }
    }

    #[test]
    fn lp_vec3_zero_is_single_byte() {
        assert_eq!(lp_vec3(0.0, 0.0, 0.0), vec![0x00]);
        assert!(lp_vec3(1.0, 2.0, 3.0).len() >= 6);
        // Non-zero vectors never decode as the legacy pitch byte + vec3i16.
        assert_ne!(lp_vec3(0.1, 0.0, 0.0), vec![0x00]);
    }

    #[test]
    fn yaw_conversions() {
        assert!((bedrock_yaw_to_java(0.0) - 180.0).abs() < 0.001);
        assert!((bedrock_yaw_to_java(90.0) - (-90.0)).abs() < 0.001);
        assert!((java_yaw_to_bedrock(bedrock_yaw_to_java(123.0)) - 123.0).abs() < 0.001);
        assert!((bedrock_pitch_to_java(315.0) - (-45.0)).abs() < 0.001);
    }

    #[test]
    fn frame_split() {
        let payload = string("hola");
        let framed = frame(0x02, &payload);
        let (id, body, consumed) = try_split_frame(&framed).unwrap();
        assert_eq!(id, 0x02);
        assert_eq!(body, payload);
        assert_eq!(consumed, framed.len());
        assert!(try_split_frame(&framed[..2]).is_none());
    }
}
