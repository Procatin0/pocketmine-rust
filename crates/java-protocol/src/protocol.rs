//! Byte-level port of the `javabridge` plugin's binary primitives.
//!
//! Wire format is Java Edition big-endian, byte-for-byte identical to
//! `plugins/JavaBridge/src/javabridge/Protocol.php` (1.12.2 / protocol 340
//! primitives, plus the helpers shared with the modern profiles
//! 769/770/773/774/775).
//!
//! Read helpers use the same convention as PHP: they take the buffer and a
//! mutable offset, return `None` on an incomplete (or invalid) buffer and
//! leave `*offset` untouched in that case, and only advance the offset on
//! success.
//!
//! Exact-byte notes:
//! - VarInt is unsigned LEB128 over the 32-bit two's-complement pattern
//!   (as PHP, which masks to 32 bits). Negative values such as `-1` are
//!   therefore the 5-byte `FF FF FF FF 0F` on the wire. The read helper
//!   returns the signed `i32` view of those bits (Java `int` semantics);
//!   `(-1i32) as u32` is the unsigned value PHP surfaces.
//! - VarLong is signed LEB128 over the 64-bit pattern: `-1` is the single
//!   byte `7F`, unlike VarInt's 5 bytes. For the non-negative times used by
//!   26.1 `update_time` (`totalTicks`) the two encodings coincide. PHP
//!   only encodes `varLong`; the Rust decoder (`read_var_long`) is the
//!   symmetric signed-LEB128 counterpart added for roundtrips.
//! - Long/short/int and IEEE floats/doubles are big-endian (`pack('N'|'n'|'E'|'G'|'J'`).
//!   PHP's `pack('J')` for `long`/`readLong` is big-endian on this build
//!   (see `plugins/JavaBridge/src/javabridge/Protocol.php` and `Nbt.php`),
//!   and `Protocol::long` is big-endian by construction, so the crate uses
//!   `to_be_bytes`/`from_be_bytes` throughout.
//! - `angle()` replicates the PHP formula including its constant offset:
//!   `floor(fmod(degrees * 256 / 360 + 360, 256))` (so a 0-degree angle is
//!   byte 104, not 0), signed to `[-128, 128)`. Changing it to the
//!   textbook `(degrees * 256 / 360) mod 256` would break wire parity with
//!   the plugin even though the modern spawn tests only compare via the
//!   same helper.

/// Legacy 1.12.2 protocol version.
pub const PROTOCOL_VERSION: i32 = 340;
/// Legacy version name.
pub const VERSION_NAME: &str = "1.12.2";
/// Maximum frame body length accepted by the plugin (2 MiB).
pub const MAX_FRAME_LENGTH: usize = 2_097_152;

/// Maximum accepted `read_string` length (PHP: `32767 * 4`).
pub const MAX_STRING_LENGTH: i32 = 32_767 * 4;

// Handshake / status / login (legacy 340).
pub const C2S_HANDSHAKE: i32 = 0x00;
pub const C2S_STATUS_REQUEST: i32 = 0x00;
pub const C2S_STATUS_PING: i32 = 0x01;
pub const S2C_STATUS_RESPONSE: i32 = 0x00;
pub const S2C_STATUS_PONG: i32 = 0x01;
pub const C2S_LOGIN_START: i32 = 0x00;
pub const S2C_LOGIN_DISCONNECT: i32 = 0x00;
pub const S2C_LOGIN_SUCCESS: i32 = 0x02;

// Play: server -> client (legacy 340).
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
pub const S2C_SCOREBOARD_OBJECTIVE: i32 = 0x42;
pub const S2C_SCOREBOARD_SCORE: i32 = 0x45;
pub const S2C_SCOREBOARD_DISPLAY_OBJECTIVE: i32 = 0x3B;
pub const S2C_DIFFICULTY: i32 = 0x0D;
/// Legacy 340 `boss_bar` (apiBossBar Java side). Modern ids (769=0x0A,
/// 770+=0x09) live in `crate::bossbar::boss_bar_packet_id`.
pub const S2C_BOSS_BAR: i32 = 0x0C;

// Play: player entities (legacy 340).
pub const S2C_NAMED_ENTITY_SPAWN: i32 = 0x05;
pub const S2C_ENTITY: i32 = 0x25;
pub const S2C_REL_ENTITY_MOVE: i32 = 0x26;
pub const S2C_ENTITY_MOVE_LOOK: i32 = 0x27;
pub const S2C_ENTITY_LOOK: i32 = 0x28;
pub const S2C_ENTITY_HEAD_ROTATION: i32 = 0x36;
pub const S2C_ENTITY_DESTROY: i32 = 0x32;
pub const S2C_ENTITY_METADATA: i32 = 0x3C;
pub const S2C_ENTITY_TELEPORT: i32 = 0x4C;

// Play: client -> server (legacy 340).
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

/// Read a VarInt without consuming anything when the buffer is incomplete.
///
/// Mirrors `Protocol::readVarInt`: at most 5 bytes are accepted; a 5th byte
/// with the continuation bit set fails, as does running out of buffer.
/// On failure `*offset` is left unchanged.
pub fn read_var_int(buffer: &[u8], offset: &mut usize) -> Option<i32> {
    let mut position = *offset;
    let mut value: u32 = 0;
    let mut shift: u32 = 0;
    while position < buffer.len() {
        let byte = buffer[position];
        position += 1;
        value |= ((byte & 0x7F) as u32) << shift;
        if (byte & 0x80) == 0 {
            *offset = position;
            return Some(value as i32);
        }
        shift += 7;
        if shift > 28 {
            return None;
        }
    }
    None
}

/// Encode a VarInt (masks to 32 bits, like PHP `varInt`).
pub fn var_int(value: i32) -> Vec<u8> {
    let mut remaining = value as u32;
    let mut out = Vec::new();
    loop {
        let mut byte = (remaining & 0x7F) as u8;
        remaining >>= 7;
        if remaining != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if remaining == 0 {
            break;
        }
    }
    out
}

/// Encode a Java VarLong (signed LEB128 over 64 bits, as PHP `varLong`).
///
/// Diverges from `var_int` for negative values (`-1` -> `7F` here vs
/// `FF FF FF FF 0F` for VarInt) and for magnitudes needing more than 32
/// bits. For the non-negative `totalTicks` values of 26.1 `update_time`
/// the two coincide.
pub fn var_long(value: i64) -> Vec<u8> {
    let mut remaining = value;
    let mut out = Vec::new();
    loop {
        let mut byte = (remaining & 0x7F) as u8;
        remaining >>= 7;
        if remaining != 0 && remaining != -1 {
            byte |= 0x80;
        }
        out.push(byte);
        if !(remaining != 0 && remaining != -1) {
            break;
        }
    }
    out
}

/// Decode a VarLong (unsigned LEB128 over 64 bits, at most 10 bytes).
///
/// PHP only encodes `varLong`; this is the symmetric decoder Rust needs to
/// read the 26.1 `update_time` clock fields. Decoding is unsigned (the
/// Minecraft wire semantics and the way `modern_test.php` reads
/// `totalTicks` — via `readVarInt` — for the non-negative times actually
/// used): PHP's signed-LEB128-shaped encoder collapses `127` and `-1` to
/// the same byte `7F` (it stops as soon as the shifted remainder is `0`
/// or `-1` without checking the sign bit), so negative values are not
/// unambiguously recoverable and only the non-negative roundtrip is
/// guaranteed. `var_long(-1)` is still `7F` on the wire, byte-identical
/// to PHP. On an incomplete or over-long buffer it returns `None` and
/// leaves `*offset` unchanged, like `read_var_int`.
pub fn read_var_long(buffer: &[u8], offset: &mut usize) -> Option<i64> {
    let mut position = *offset;
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        if position >= buffer.len() {
            return None;
        }
        let byte = buffer[position] as u64;
        position += 1;
        if shift < 64 {
            result |= (byte & 0x7F) << shift;
        }
        if (byte & 0x80) == 0 {
            *offset = position;
            return Some(result as i64);
        }
        shift += 7;
        if shift >= 70 {
            return None;
        }
    }
}

/// Read raw bytes for a length-prefixed string (VarInt length + bytes).
///
/// Returns `None` (leaving `*offset` unchanged) when the length prefix is
/// missing/invalid, exceeds `MAX_STRING_LENGTH`, or the payload bytes are
/// incomplete.
pub fn read_string_bytes(buffer: &[u8], offset: &mut usize) -> Option<Vec<u8>> {
    let mut position = *offset;
    let length = read_var_int(buffer, &mut position)?;
    if length < 0 || length > MAX_STRING_LENGTH {
        return None;
    }
    let len = length as usize;
    if buffer.len().saturating_sub(position) < len {
        return None;
    }
    let value = buffer[position..position + len].to_vec();
    *offset = position + len;
    Some(value)
}

/// Read a length-prefixed (VarInt) UTF-8 string.
///
/// Like PHP `readString`, length validation matches `read_string_bytes`;
/// additionally, invalid UTF-8 yields `None` with `*offset` unchanged
/// (Java strings are UTF-8 by spec; PHP would surface raw bytes).
pub fn read_string(buffer: &[u8], offset: &mut usize) -> Option<String> {
    let mut position = *offset;
    let bytes = read_string_bytes(buffer, &mut position)?;
    match String::from_utf8(bytes) {
        Ok(s) => {
            *offset = position;
            Some(s)
        }
        Err(_) => None,
    }
}

/// Encode a length-prefixed string (`varInt(len) + bytes`).
pub fn string(value: &str) -> Vec<u8> {
    let mut out = var_int(value.len() as i32);
    out.extend_from_slice(value.as_bytes());
    out
}

/// Encode a namespaced identifier (`minecraft:water`, `minecraft:brand`, ...).
///
/// On the wire this is identical to `string`; the alias exists so callers
/// do not confuse a registry/channel id with free text (PHP `identifier`).
pub fn identifier(value: &str) -> Vec<u8> {
    string(value)
}

/// Encode an array of VarInts with a VarInt count prefix.
pub fn var_int_array(values: &[i32]) -> Vec<u8> {
    let mut out = var_int(values.len() as i32);
    for &value in values {
        out.extend_from_slice(&var_int(value));
    }
    out
}

/// Read a `var_int_array` (VarInt count + VarInt elements).
///
/// Returns `None` with `*offset` unchanged when incomplete.
pub fn read_var_int_array(buffer: &[u8], offset: &mut usize) -> Option<Vec<i32>> {
    let mut position = *offset;
    let count = read_var_int(buffer, &mut position)?;
    if count < 0 {
        return None;
    }
    let count = count as usize;
    // Cap the pre-allocation: a malformed huge count must not OOM before
    // the incomplete-buffer check returns `None`.
    let mut values = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        values.push(read_var_int(buffer, &mut position)?);
    }
    *offset = position;
    Some(values)
}

/// Read a big-endian u16 (`pack('n')`).
pub fn read_u16(buffer: &[u8], offset: &mut usize) -> Option<u16> {
    if buffer.len().saturating_sub(*offset) < 2 {
        return None;
    }
    let value = u16::from_be_bytes([buffer[*offset], buffer[*offset + 1]]);
    *offset += 2;
    Some(value)
}

/// Read a big-endian i64 (`Protocol::long` / `pack('J')`).
pub fn read_long(buffer: &[u8], offset: &mut usize) -> Option<i64> {
    if buffer.len().saturating_sub(*offset) < 8 {
        return None;
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buffer[*offset..*offset + 8]);
    *offset += 8;
    Some(i64::from_be_bytes(bytes))
}

/// Read a big-endian f64 (`pack('E')`).
pub fn read_double(buffer: &[u8], offset: &mut usize) -> Option<f64> {
    if buffer.len().saturating_sub(*offset) < 8 {
        return None;
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buffer[*offset..*offset + 8]);
    *offset += 8;
    Some(f64::from_be_bytes(bytes))
}

/// Read a big-endian f32 (`pack('G'`).
pub fn read_float(buffer: &[u8], offset: &mut usize) -> Option<f32> {
    if buffer.len().saturating_sub(*offset) < 4 {
        return None;
    }
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&buffer[*offset..*offset + 4]);
    *offset += 4;
    Some(f32::from_be_bytes(bytes))
}

/// Read a legacy packed block position (1.8 - 1.13.2 / 340): x (26 bits) | y (12 bits) | z (26 bits).
pub fn read_position_legacy(buffer: &[u8], offset: &mut usize) -> Option<(i32, i32, i32)> {
    if buffer.len().saturating_sub(*offset) < 8 {
        return None;
    }
    let high = u32::from_be_bytes([
        buffer[*offset],
        buffer[*offset + 1],
        buffer[*offset + 2],
        buffer[*offset + 3],
    ]);
    let low = u32::from_be_bytes([
        buffer[*offset + 4],
        buffer[*offset + 5],
        buffer[*offset + 6],
        buffer[*offset + 7],
    ]);
    *offset += 8;
    let mut x = (high >> 6) as i32;
    if x >= 0x2000000 {
        x -= 0x4000000;
    }
    let mut y = (((high & 0x3F) << 6) | (low >> 26)) as i32;
    y &= 0xFFF;
    if y >= 0x800 {
        y -= 0x1000;
    }
    let mut z = (low & 0x3FF_FFFF) as i32;
    if z >= 0x2000000 {
        z -= 0x4000000;
    }
    Some((x, y, z))
}

/// Read a modern packed block position (1.14+ / 763+): x (26 bits) | z (26 bits) | y (12 bits).
pub fn read_position_modern(buffer: &[u8], offset: &mut usize) -> Option<(i32, i32, i32)> {
    if buffer.len().saturating_sub(*offset) < 8 {
        return None;
    }
    let bytes: [u8; 8] = buffer[*offset..*offset + 8].try_into().ok()?;
    let v = u64::from_be_bytes(bytes);
    *offset += 8;
    let mut x = ((v >> 38) & 0x3FF_FFFF) as i32;
    if x >= 0x2000000 {
        x -= 0x4000000;
    }
    let mut z = ((v >> 12) & 0x3FF_FFFF) as i32;
    if z >= 0x2000000 {
        z -= 0x4000000;
    }
    let mut y = (v & 0xFFF) as i32;
    if y >= 0x800 {
        y -= 0x1000;
    }
    Some((x, y, z))
}

/// Default position reader (modern).
pub fn read_position(buffer: &[u8], offset: &mut usize) -> Option<(i32, i32, i32)> {
    read_position_modern(buffer, offset)
}

/// Pack a legacy block position (1.8 - 1.13.2 / 340).
pub fn position_legacy(x: i32, y: i32, z: i32) -> Vec<u8> {
    let packed: u64 = (((x as u32) & 0x3FF_FFFF) as u64) << 38
        | (((y as u32) & 0xFFF) as u64) << 26
        | (((z as u32) & 0x3FF_FFFF) as u64);
    packed.to_be_bytes().to_vec()
}

/// Pack a modern block position (1.14+ / 763+): x (26 bits) | z (26 bits) | y (12 bits).
pub fn position_modern(x: i32, y: i32, z: i32) -> Vec<u8> {
    let packed: u64 = (((x as u32) & 0x3FF_FFFF) as u64) << 38
        | (((z as u32) & 0x3FF_FFFF) as u64) << 12
        | (((y as u32) & 0xFFF) as u64);
    packed.to_be_bytes().to_vec()
}

/// Default position packer (modern).
pub fn position(x: i32, y: i32, z: i32) -> Vec<u8> {
    position_modern(x, y, z)
}

/// Encode a big-endian i64.
pub fn long(value: i64) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

/// Encode a big-endian f64.
pub fn double(value: f64) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

/// Encode a big-endian f32.
///
/// PHP's `Protocol::float(float $value)` takes a double and packs it as
/// `G` (float32). Rust callers hold f32 values (gamemodes, partial ticks);
/// pass `value as f32` when the source is f64 to mirror the PHP conversion.
/// `0.05f32` and `(0.05f64 as f32)` have identical bits (`3D4CCCCD`).
pub fn float(value: f32) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

/// Encode an unsigned byte (masks to 8 bits, as PHP `ubyte`).
pub fn ubyte(value: i32) -> Vec<u8> {
    vec![(value & 0xFF) as u8]
}

/// Encode a boolean as a single byte (1/0).
pub fn boolean(value: bool) -> Vec<u8> {
    vec![u8::from(value)]
}

/// Encode a signed byte (masks to 8 bits, as PHP `byte`; carries `angle`).
pub fn byte(value: i32) -> Vec<u8> {
    vec![(value & 0xFF) as u8]
}

/// Encode a signed short, big-endian (masks to 16 bits, as PHP `short`).
pub fn short(value: i32) -> Vec<u8> {
    ((value & 0xFFFF) as u16).to_be_bytes().to_vec()
}

/// Angle in degrees to a signed byte (textbook `i8` angle):
/// `floor(degrees * 256 / 360 mod 256)`, signed to `[-128, 128)`
/// (0 -> 0, 90 -> 64, 180 -> -128, -90 -> -64).
pub fn angle(degrees: f64) -> i32 {
    let scaled = (degrees * 256.0 / 360.0).rem_euclid(256.0);
    let floored = scaled.floor() as i32;
    if floored >= 128 {
        floored - 256
    } else {
        floored
    }
}

/// Bedrock yaw (0 = south, 90 = west, 180 = north, 270 = east, PocketMine
/// `Entity::getDirectionVector`) to Java yaw (`(-180, 180]`, same compass:
/// 0 = south, 90 = west, -90 = east). Range normalization only — no `+180`
/// (the old offset pointed every facing direction backwards).
pub fn bedrock_yaw_to_java(yaw: f64) -> f64 {
    let mut y = yaw.rem_euclid(360.0);
    if y > 180.0 {
        y -= 360.0;
    }
    y
}

/// Java yaw `(-180, 180]` to Bedrock yaw `[0, 360)`: fold to canonical
/// Bedrock range (inverse of `bedrock_yaw_to_java`).
pub fn java_yaw_to_bedrock(yaw: f64) -> f64 {
    yaw.rem_euclid(360.0)
}

/// Bedrock pitch folded from `[0, 360)` back to Java's `(-180, 180]`.
///
/// Both editions share the pitch sign convention; only the range differs
/// (Bedrock canonical is `[0, 360)` in `session.rs`). Mirrors
/// `Protocol::bedrockPitchToJava`.
pub fn bedrock_pitch_to_java(pitch: f64) -> f64 {
    let mut p = pitch % 360.0;
    if p < 0.0 {
        p += 360.0;
    }
    if p > 180.0 {
        p -= 360.0;
    }
    p
}

/// Relative displacement in blocks to a signed i16 (`*4096`, clamped).
/// Mirrors `Protocol::relDelta` (PHP `round` is half-away-from-zero, as
/// Rust `f64::round`).
pub fn rel_delta(delta: f64) -> i32 {
    let scaled = (delta * 4096.0).round() as i32;
    scaled.clamp(-32768, 32767)
}

/// Low-precision velocity vector `lpVec3` (1.21.9+, `spawn_entity` /
/// `entity_velocity` in `protocol.json`).
///
/// Zero is the single byte `0x00`. Otherwise 6 bytes (three 15-bit
/// quantized components plus a scale marker) and, when the scale exceeds
/// 3, a trailing VarInt with `floor(scale / 4)`. Mirrors
/// `Protocol::lpVec3` (same constants, same
/// `chr(low) + chr(mid) + pack('N', high)` byte split, same sanitization:
/// NaN -> 0, clamp to `+-1.7179869183e10`).
pub fn lp_vec3(x: f64, y: f64, z: f64) -> Vec<u8> {
    const ABS_MAX: f64 = 1.717_986_918_3e10;
    const ABS_MIN: f64 = 3.051_944_088_384_301e-5;
    let sanitize = |v: f64| -> f64 {
        if v.is_nan() {
            0.0
        } else {
            v.clamp(-ABS_MAX, ABS_MAX)
        }
    };
    let x = sanitize(x);
    let y = sanitize(y);
    let z = sanitize(z);
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
        + pack(x / (scale as f64)) * 0x8
        + pack(y / (scale as f64)) * 0x40000
        + pack(z / (scale as f64)) * 0x2000_00000;
    let packed = packed as u64;
    let mut out = Vec::with_capacity(11);
    out.push((packed & 0xFF) as u8);
    out.push(((packed >> 8) & 0xFF) as u8);
    out.extend_from_slice(&(((packed >> 16) & 0xFFFF_FFFF) as u32).to_be_bytes());
    if needs_continuation {
        out.extend_from_slice(&var_int((scale / 4) as i32));
    }
    out
}

/// Build a complete frame: `varInt(body_len) + varInt(id) + payload`.
pub fn frame(id: i32, payload: &[u8]) -> Vec<u8> {
    let mut body = var_int(id);
    body.extend_from_slice(payload);
    let mut out = var_int(body.len() as i32);
    out.extend_from_slice(&body);
    out
}

/// Simple chat component `{"text":"..."}`.
///
/// Matches PHP `Protocol::chatJson` (`json_encode` with
/// `JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE`): slashes and
/// non-ASCII pass through as UTF-8, control characters use the short
/// escapes (`\n`, `\r`, `\t`, `\b`, `\f`) or `\u00xx` (lowercase hex).
/// Input is `&str` (always valid UTF-8), so PHP's
/// `JSON_INVALID_UTF8_SUBSTITUTE` fallback never triggers.
pub fn chat_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 12);
    out.push_str("{\"text\":\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str("\\u");
                out.push_str(&format!("{:04x}", c as u32));
            }
            _ => out.push(c),
        }
    }
    out.push_str("\"}");
    out
}

/// Minecraft offline-mode UUID: MD5 of `"OfflinePlayer:" + name` with the
/// version-3 and RFC 4122 variant bits set. Raw 16 bytes, as PHP
/// `Protocol::offlineUuid`.
pub fn offline_uuid(name: &str) -> [u8; 16] {
    let mut input = Vec::with_capacity(13 + name.len());
    input.extend_from_slice(b"OfflinePlayer:");
    input.extend_from_slice(name.as_bytes());
    let mut hash = md5(&input);
    hash[6] = (hash[6] & 0x0F) | 0x30;
    hash[8] = (hash[8] & 0x3F) | 0x80;
    hash
}

/// Format a raw 16-byte UUID as `8-4-4-4-12`, as PHP `Protocol::uuidString`.
pub fn uuid_string(raw: &[u8; 16]) -> String {
    let hex: Vec<String> = raw.iter().map(|b| format!("{b:02x}")).collect();
    let s: String = hex.concat();
    format!(
        "{}-{}-{}-{}-{}",
        &s[0..8],
        &s[8..12],
        &s[12..16],
        &s[16..20],
        &s[20..32]
    )
}

/// Standard MD5 (RFC 1321), used only for [`offline_uuid`].
fn md5(input: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];
    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut msg = input.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            let j = i * 4;
            *w = u32::from_le_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let tmp = f.wrapping_add(a).wrapping_add(K[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(tmp.rotate_left(S[i]));
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
    fn var_int_vectors() {
        // Wire bytes from Protocol::varInt (genvec).
        assert_eq!(hex(&var_int(0)), "00");
        assert_eq!(hex(&var_int(1)), "01");
        assert_eq!(hex(&var_int(127)), "7f");
        assert_eq!(hex(&var_int(128)), "8001");
        assert_eq!(hex(&var_int(300)), "ac02");
        assert_eq!(hex(&var_int(25565)), "ddc701");
        assert_eq!(hex(&var_int(-1)), "ffffffff0f");
        assert_eq!(hex(&var_int(i32::MAX)), "ffffffff07");
    }

    #[test]
    fn var_int_roundtrip() {
        for value in [0, 1, 127, 128, 300, 25565, -1, i32::MAX, i32::MIN] {
            let encoded = var_int(value);
            let mut offset = 0;
            assert_eq!(read_var_int(&encoded, &mut offset), Some(value));
            assert_eq!(offset, encoded.len());
        }
    }

    #[test]
    fn var_int_incomplete_does_not_consume() {
        let buffer = [0x80u8];
        let mut offset = 0;
        assert_eq!(read_var_int(&buffer, &mut offset), None);
        assert_eq!(offset, 0);
        // 5-byte prefix with continuation on the 5th byte overflows.
        let buffer = [0x80, 0x80, 0x80, 0x80, 0x80, 0x00];
        let mut offset = 0;
        assert_eq!(read_var_int(&buffer, &mut offset), None);
        assert_eq!(offset, 0);
    }

    #[test]
    fn var_long_diverges_for_negatives() {
        assert_eq!(hex(&var_long(-1)), "7f");
        assert_eq!(hex(&var_long(-2)), "7e");
        assert_eq!(hex(&var_long(6000)), "f02e");
        assert_eq!(hex(&var_int(6000)), "f02e");
        // Non-negative roundtrip (the update_time case, incl. 64..127
        // where a signed decoder would misread the sign bit).
        for value in [0i64, 1, 63, 64, 127, 128, 300, 6000, 13000, i64::MAX] {
            let encoded = var_long(value);
            let mut offset = 0;
            assert_eq!(read_var_long(&encoded, &mut offset), Some(value));
            assert_eq!(offset, encoded.len());
        }
        // PHP's signed-shaped encoder is ambiguous below zero
        // (`var_long(-1) == var_long(127) == 7F`); the unsigned decoder
        // surfaces the non-negative reading. Documented, not hidden.
        assert_eq!(var_long(-1), var_long(127));
        let mut offset = 0;
        assert_eq!(read_var_long(&var_long(-1), &mut offset), Some(127));
    }

    #[test]
    fn string_roundtrip() {
        let encoded = string("Steve");
        let mut offset = 0;
        assert_eq!(read_string(&encoded, &mut offset).as_deref(), Some("Steve"));
        assert_eq!(offset, encoded.len());
    }

    #[test]
    fn identifier_same_wire_as_string() {
        assert_eq!(identifier("minecraft:water"), string("minecraft:water"));
        let encoded = identifier("minecraft:water");
        let mut offset = 0;
        assert_eq!(
            read_string(&encoded, &mut offset).as_deref(),
            Some("minecraft:water")
        );
    }

    #[test]
    fn var_int_array_roundtrip_and_incomplete() {
        for values in [&[][..], &[1], &[1, 2], &[0, 40, 482, 1167]] {
            let encoded = var_int_array(values);
            let mut offset = 0;
            assert_eq!(read_var_int_array(&encoded, &mut offset).unwrap(), values);
        }
        let buffer = [0x02u8, 0x01];
        let mut offset = 0;
        assert_eq!(read_var_int_array(&buffer, &mut offset), None);
        assert_eq!(offset, 0);
    }

    #[test]
    fn frame_layout() {
        let frame = frame(0x00, &string("hello"));
        let mut offset = 0;
        let length = read_var_int(&frame, &mut offset).unwrap();
        assert_eq!(length as usize, frame.len() - offset);
        let body = &frame[offset..offset + length as usize];
        let mut body_offset = 0;
        assert_eq!(read_var_int(body, &mut body_offset), Some(0x00));
        assert_eq!(&body[body_offset..], string("hello").as_slice());
    }

    #[test]
    fn position_vectors_and_roundtrip() {
        assert_eq!(hex(&position_legacy(0, 4, 0)), "0000000010000000");
        assert_eq!(hex(&position_legacy(-1, 64, 200)), "ffffffc1000000c8");
        assert_eq!(
            hex(&position_legacy(1_000_000, 255, -1_000_000)),
            "03d09003fff0bdc0"
        );
        assert_eq!(hex(&position_legacy(0, -64, 0)), "0000003f00000000");
        for pos in [
            (0, 4, 0),
            (-1, 64, 200),
            (1_000_000, 255, -1_000_000),
            (0, -64, 0),
        ] {
            let encoded_legacy = position_legacy(pos.0, pos.1, pos.2);
            let mut offset = 0;
            assert_eq!(read_position_legacy(&encoded_legacy, &mut offset), Some(pos));

            let encoded_modern = position_modern(pos.0, pos.1, pos.2);
            let mut offset = 0;
            assert_eq!(read_position_modern(&encoded_modern, &mut offset), Some(pos));
            let mut offset = 0;
            assert_eq!(read_position(&encoded_modern, &mut offset), Some(pos));
        }
    }

    #[test]
    fn offline_uuid_vectors() {
        assert_eq!(
            hex(&offline_uuid("Steve").to_vec()),
            "5627dd98e6be3c21b8a8e92344183641"
        );
        assert_eq!(
            hex(&offline_uuid("Alex").to_vec()),
            "36532b5ec4423dbba24cc7e55d0f979a"
        );
        let uuid = offline_uuid("Steve");
        assert_eq!(uuid, offline_uuid("Steve"));
        assert_ne!(uuid, offline_uuid("Alex"));
        assert_eq!(uuid_string(&uuid).len(), 36);
        assert_eq!(uuid_string(&uuid), "5627dd98-e6be-3c21-b8a8-e92344183641");
        assert_eq!(uuid[6] & 0xF0, 0x30);
    }

    #[test]
    fn chat_json_vectors() {
        assert_eq!(chat_json("a\"b"), "{\"text\":\"a\\\"b\"}");
        assert_eq!(chat_json("a/b"), "{\"text\":\"a/b\"}");
        assert_eq!(chat_json(""), "{\"text\":\"\"}");
        assert_eq!(chat_json("a\nb"), "{\"text\":\"a\\nb\"}");
        assert_eq!(
            hex(chat_json("\u{0001}\u{001f}\u{007f}").as_bytes()),
            "7b2274657874223a225c75303030315c75303031667f227d"
        );
    }

    #[test]
    fn lp_vec3_vectors() {
        assert_eq!(hex(&lp_vec3(0.0, 0.0, 0.0)), "00");
        assert_eq!(hex(&lp_vec3(1.0, 0.0, 0.0)), "f1ff7ffeffff");
        assert_eq!(hex(&lp_vec3(0.5, 0.5, 0.5)), "f9ffbfff7ffe");
        assert_eq!(hex(&lp_vec3(-1.0, 2.0, -3.5)), "fc7f10017ffd01");
        assert_eq!(hex(&lp_vec3(10.0, 0.0, 0.0)), "f6ff7ffeffff02");
        assert_eq!(hex(&lp_vec3(100.0, 200.0, 300.0)), "a4aafffdaaa64b");
        assert_eq!(hex(&lp_vec3(f64::NAN, 0.0, 0.0)), "00");
    }

    #[test]
    fn angle_matches_php_formula() {
        // Textbook i8 angle (0 -> 0, 90 -> 64, -90 -> -64).
        assert_eq!(angle(0.0), 0);
        assert_eq!(angle(90.0), 64);
        assert_eq!(angle(180.0), -128);
        assert_eq!(angle(-90.0), -64);
        assert_eq!(angle(10.0), 7);
    }

    #[test]
    fn yaw_pitch_vectors() {
        // Same compass in both editions (0 = south, 90 = west).
        assert!(bedrock_yaw_to_java(0.0).abs() < 0.001);
        assert!((bedrock_yaw_to_java(180.0) - 180.0).abs() < 0.001);
        assert!((bedrock_yaw_to_java(90.0) - 90.0).abs() < 0.001);
        assert!((bedrock_yaw_to_java(270.0) - (-90.0)).abs() < 0.001);
        assert!(java_yaw_to_bedrock(0.0).abs() < 0.001);
        assert!((java_yaw_to_bedrock(-90.0) - 270.0).abs() < 0.001);
        assert!((bedrock_pitch_to_java(315.0) + 45.0).abs() < 0.001);
    }

    #[test]
    fn rel_delta_clamps() {
        assert_eq!(rel_delta(0.0), 0);
        assert_eq!(rel_delta(1.0), 4096);
        assert_eq!(rel_delta(8.0), 32767);
        assert_eq!(rel_delta(-8.0), -32768);
        assert_eq!(rel_delta(100.0), 32767);
    }
}
