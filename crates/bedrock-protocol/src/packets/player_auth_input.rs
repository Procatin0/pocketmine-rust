use crate::binary::{CodecError, Reader, Writer};
use crate::types::Vector3f;
use crate::Packet;

/// Input flag bits in the `input_data` tail (see `PlayerAuthInputFlags` in
/// PocketMine-MP's BedrockProtocol: the flags are a 7-bits-per-byte varlen
/// bitset, bit `n` set via `flags |= 1 << n`).
pub const INPUT_FLAG_PERFORM_ITEM_INTERACTION: u32 = 34;
pub const INPUT_FLAG_PERFORM_BLOCK_ACTIONS: u32 = 35;
pub const INPUT_FLAG_PERFORM_ITEM_STACK_REQUEST: u32 = 36;
/// `IN_CLIENT_PREDICTED_VEHICLE` sits at bit 45 in current BedrockProtocol
/// and at bit 40 in older revisions; both are accepted when validating the
/// packet tail.
pub const INPUT_FLAG_VEHICLE: u32 = 45;
pub const INPUT_FLAG_VEHICLE_OLD: u32 = 40;

/// Block action types (see `PlayerAction` in BedrockProtocol).
pub const BLOCK_ACTION_START_BREAK: i32 = 0;
pub const BLOCK_ACTION_ABORT_BREAK: i32 = 1;
pub const BLOCK_ACTION_STOP_BREAK: i32 = 2;
pub const BLOCK_ACTION_CRACK_BREAK: i32 = 18;
pub const BLOCK_ACTION_PREDICT_DESTROY: i32 = 26;
pub const BLOCK_ACTION_CONTINUE_DESTROY: i32 = 27;

/// Maximum block actions accepted from a single packet (vanilla clients send
/// one or two; the bound only guards against corrupt count fields).
pub const MAX_BLOCK_ACTIONS: i32 = 64;

/// A block action carried by `PlayerAuthInputPacket` when the
/// `PERFORM_BLOCK_ACTIONS` input flag is set. `STOP_BREAK` carries no
/// position; every other known type carries a block position and a face
/// (0-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerBlockAction {
    pub action_type: i32,
    pub position: Option<(i32, i32, i32)>,
    pub face: Option<i32>,
}

/// Best-effort extraction of block actions from the raw `input_data` tail of
/// a `PlayerAuthInputPacket` (everything after head yaw).
///
/// The tail layout after the flags is `inputMode, playMode, [interactionMode
/// + interactRotation on newer clients], tick, delta, [item payloads],
/// [block actions], [vehicle info], [analog/camera/rawMove on newer
/// clients]`. Both the modern and the legacy (pre-interactionMode) header
/// shapes are attempted; the first attempt yielding validated, non-empty
/// actions wins.
///
/// Returns an empty vec when the flag is absent, when item payloads are
/// present (they sit between the movement fields and the block actions and
/// require full item-stack parsing to skip), or when nothing parses cleanly.
/// Movement handling must never depend on this succeeding.
pub fn parse_block_actions(input_data: &[u8]) -> Vec<PlayerBlockAction> {
    let Some((flags, _)) = split_auth_input_flags(input_data) else {
        return Vec::new();
    };
    let has = |bit: u32| (flags >> bit) & 1 == 1;
    if has(INPUT_FLAG_PERFORM_ITEM_INTERACTION)
        || has(INPUT_FLAG_PERFORM_ITEM_STACK_REQUEST)
    {
        return Vec::new();
    }
    if !has(INPUT_FLAG_PERFORM_BLOCK_ACTIONS) {
        return Vec::new();
    }
    let vehicle = has(INPUT_FLAG_VEHICLE) || has(INPUT_FLAG_VEHICLE_OLD);

    let modern = parse_block_actions_with(input_data, true, vehicle).unwrap_or_default();
    if !modern.is_empty() {
        return modern;
    }
    parse_block_actions_with(input_data, false, vehicle).unwrap_or_default()
}

/// Reads the varlen 7-bits-per-byte flag bitset, returning the assembled
/// bits plus the number of bytes consumed.
fn split_auth_input_flags(input_data: &[u8]) -> Option<(u128, usize)> {
    let mut value: u128 = 0;
    // 65 bits fit in 10 groups of 7 bits.
    for group in 0..10usize {
        let byte = *input_data.get(group)?;
        value |= ((byte & 0x7F) as u128) << (7 * group);
        if byte & 0x80 == 0 {
            return Some((value, group + 1));
        }
    }
    None
}

/// Attempts one header shape (`modern`: with interactionMode +
/// interactRotation). Returns `None` on any structural mismatch so the
/// caller can try the other shape.
fn parse_block_actions_with(
    input_data: &[u8],
    modern: bool,
    vehicle: bool,
) -> Option<Vec<PlayerBlockAction>> {
    let mut reader = Reader::new(input_data);
    let (_, flags_len) = split_auth_input_flags(input_data)?;
    reader.read_bytes(flags_len).ok()?;
    reader.read_var_u32().ok()?; // inputMode
    reader.read_var_u32().ok()?; // playMode
    if modern {
        reader.read_var_u32().ok()?; // interactionMode
        reader.read_bytes(8).ok()?; // interactRotation (2x f32)
    }
    reader.read_var_u64().ok()?; // tick
    reader.read_bytes(12).ok()?; // delta (3x f32)
    let count = reader.read_var_i32().ok()?;
    if count < 0 || count > MAX_BLOCK_ACTIONS {
        return None;
    }
    let mut actions = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let action_type = reader.read_var_i32().ok()?;
        match action_type {
            BLOCK_ACTION_STOP_BREAK => actions.push(PlayerBlockAction {
                action_type,
                position: None,
                face: None,
            }),
            BLOCK_ACTION_START_BREAK
            | BLOCK_ACTION_ABORT_BREAK
            | BLOCK_ACTION_CRACK_BREAK
            | BLOCK_ACTION_PREDICT_DESTROY
            | BLOCK_ACTION_CONTINUE_DESTROY => {
                let x = reader.read_var_i32().ok()?;
                let y = reader.read_var_i32().ok()?;
                let z = reader.read_var_i32().ok()?;
                let face = reader.read_var_i32().ok()?;
                if !(0..=5).contains(&face) {
                    return None;
                }
                if x.unsigned_abs() > 30_000_000 || z.unsigned_abs() > 30_000_000 {
                    return None;
                }
                if !(-1024..=2048).contains(&y) {
                    return None;
                }
                actions.push(PlayerBlockAction {
                    action_type,
                    position: Some((x, y, z)),
                    face: Some(face),
                });
            }
            _ => return None,
        }
    }
    // Sanity-check the tail: legacy packets end here (or carry vehicle
    // info), modern packets still carry analogMoveVec (8B) +
    // cameraOrientation (12B) + rawMove (8B).
    let remaining = reader.remaining();
    if remaining == 0 || remaining >= 28 || (vehicle && remaining > 0) {
        Some(actions)
    } else {
        None
    }
}

/// Packet ID 0x90 (144): PlayerAuthInputPacket
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerAuthInputPacket {
    pub pitch: f32,
    pub yaw: f32,
    pub position: Vector3f,
    pub move_vec_x: f32,
    pub move_vec_z: f32,
    pub head_yaw: f32,
    pub input_data: Vec<u8>,
}

impl PlayerAuthInputPacket {
    pub fn new(
        pitch: f32,
        yaw: f32,
        position: Vector3f,
        move_vec_x: f32,
        move_vec_z: f32,
        head_yaw: f32,
    ) -> Self {
        Self {
            pitch,
            yaw,
            position,
            move_vec_x,
            move_vec_z,
            head_yaw,
            input_data: Vec::new(),
        }
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Self::decode(reader)
    }
}

impl Packet for PlayerAuthInputPacket {
    fn packet_id(&self) -> u32 {
        0x90
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_f32_le(self.pitch);
        writer.write_f32_le(self.yaw);
        self.position.write(writer);
        writer.write_f32_le(self.move_vec_x);
        writer.write_f32_le(self.move_vec_z);
        writer.write_f32_le(self.head_yaw);
        if !self.input_data.is_empty() {
            writer.write_bytes(&self.input_data);
        }
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let pitch = reader.read_f32_le()?;
        let yaw = reader.read_f32_le()?;
        let position = Vector3f::read(reader)?;
        let move_vec_x = reader.read_f32_le()?;
        let move_vec_z = reader.read_f32_le()?;
        let head_yaw = reader.read_f32_le()?;
        let input_data = reader.read_remaining().to_vec();

        Ok(Self {
            pitch,
            yaw,
            position,
            move_vec_x,
            move_vec_z,
            head_yaw,
            input_data,
        })
    }
}

#[cfg(test)]
mod block_action_tests {
    use super::*;

    /// Builds an `input_data` tail: flags with bit 35 set, zeroed movement
    /// fields, then the given block actions.
    fn build_tail(modern: bool, actions: &[(i32, Option<(i32, i32, i32, i32)>)]) -> Vec<u8> {
        let mut writer = Writer::new();
        // Flags bit 35 -> group 5, bit 0: five continuation bytes + 0x01.
        writer.write_bytes(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x01]);
        writer.write_var_u32(0); // inputMode
        writer.write_var_u32(0); // playMode
        if modern {
            writer.write_var_u32(0); // interactionMode
            writer.write_bytes(&[0u8; 8]); // interactRotation
        }
        writer.write_var_u64(1234); // tick
        writer.write_bytes(&[0u8; 12]); // delta
        writer.write_var_i32(actions.len() as i32);
        for (action_type, pos) in actions {
            writer.write_var_i32(*action_type);
            if let Some((x, y, z, face)) = pos {
                writer.write_var_i32(*x);
                writer.write_var_i32(*y);
                writer.write_var_i32(*z);
                writer.write_var_i32(*face);
            }
        }
        if modern {
            writer.write_bytes(&[0u8; 28]); // analog + camera + rawMove
        }
        writer.into_bytes().to_vec()
    }

    #[test]
    fn empty_input_yields_no_actions() {
        assert!(parse_block_actions(&[]).is_empty());
        assert!(parse_block_actions(&[0x00]).is_empty());
    }

    #[test]
    fn legacy_start_break_parses() {
        let tail = build_tail(false, &[(BLOCK_ACTION_START_BREAK, Some((10, 64, -20, 1)))]);
        let actions = parse_block_actions(&tail);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type, BLOCK_ACTION_START_BREAK);
        assert_eq!(actions[0].position, Some((10, 64, -20)));
        assert_eq!(actions[0].face, Some(1));
    }

    #[test]
    fn modern_predict_destroy_parses() {
        let tail = build_tail(true, &[(BLOCK_ACTION_PREDICT_DESTROY, Some((0, 65, 0, 0)))]);
        let actions = parse_block_actions(&tail);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type, BLOCK_ACTION_PREDICT_DESTROY);
        assert_eq!(actions[0].position, Some((0, 65, 0)));
    }

    #[test]
    fn legacy_predict_destroy_parses_too() {
        let tail = build_tail(false, &[(BLOCK_ACTION_PREDICT_DESTROY, Some((3, 70, 4, 2)))]);
        let actions = parse_block_actions(&tail);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].position, Some((3, 70, 4)));
    }

    #[test]
    fn stop_break_has_no_position() {
        let tail = build_tail(false, &[(BLOCK_ACTION_STOP_BREAK, None)]);
        let actions = parse_block_actions(&tail);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].position, None);
    }

    #[test]
    fn garbage_tail_is_ignored() {
        assert!(parse_block_actions(&[0xFF, 0x00, 0x13, 0x77, 0x42]).is_empty());
        // Ten continuation bytes: malformed bitset.
        assert!(parse_block_actions(&[0x80; 10]).is_empty());
        // Flag set but truncated before the action count.
        assert!(parse_block_actions(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x01, 0x00]).is_empty());
    }

    #[test]
    fn item_payload_flag_bails_out() {
        // Bits 34 (item interaction) + 35 (block actions).
        let mut writer = Writer::new();
        writer.write_bytes(&[0x80, 0x80, 0x80, 0x80, 0xC0, 0x01]);
        writer.write_bytes(&[0u8; 30]);
        assert!(parse_block_actions(&writer.into_bytes().to_vec()).is_empty());
    }

    #[test]
    fn negative_y_parses_as_signed() {
        let tail = build_tail(false, &[(BLOCK_ACTION_START_BREAK, Some((5, -60, 5, 1)))]);
        let actions = parse_block_actions(&tail);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].position, Some((5, -60, 5)));
    }
}
