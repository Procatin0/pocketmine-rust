use crate::binary::{CodecError, Reader, Writer};
use crate::packets::inventory_transaction::{ChangedSlots, NetworkInventoryAction, UseItemData};
use crate::types::Vector3f;
use crate::Packet;

/// Input flag bits in the `input_flags` field (see `PlayerAuthInputFlags`
/// in BedrockProtocol). On the wire the flags are a count-prefixed list of
/// signed varints (`CommonTypes::readList($in, VarInt::readSignedInt)`),
/// NOT a bitset: `flags` on the wire is `VarUInt(count)` followed by
/// `count` signed varints, bit `n` carried as the flag index `n`.
pub const INPUT_FLAG_PERFORM_ITEM_INTERACTION: u32 = 34;
pub const INPUT_FLAG_PERFORM_BLOCK_ACTIONS: u32 = 35;
pub const INPUT_FLAG_PERFORM_ITEM_STACK_REQUEST: u32 = 36;
/// `IN_CLIENT_PREDICTED_VEHICLE` (bit 45 at protocol 2225).
pub const INPUT_FLAG_VEHICLE: u32 = 45;
/// Bit 40 is `START_CRAWLING` at protocol 2225 (not vehicle). Kept under its
/// historical name for compatibility; it must NOT be treated as a vehicle
/// indicator.
pub const INPUT_FLAG_VEHICLE_OLD: u32 = 40;
pub const INPUT_FLAG_START_CRAWLING: u32 = 40;

/// Movement flags used by PocketMine's InGamePacketHandler to drive the
/// sneak/sprint/swim/glide/fly/jump toggles (see PlayerAuthInputFlags.php).
pub const INPUT_FLAG_SNEAKING: u32 = 8;
pub const INPUT_FLAG_START_SPRINTING: u32 = 25;
pub const INPUT_FLAG_STOP_SPRINTING: u32 = 26;
pub const INPUT_FLAG_START_SNEAKING: u32 = 27;
pub const INPUT_FLAG_STOP_SNEAKING: u32 = 28;
pub const INPUT_FLAG_START_SWIMMING: u32 = 29;
pub const INPUT_FLAG_STOP_SWIMMING: u32 = 30;
pub const INPUT_FLAG_START_JUMPING: u32 = 31;
pub const INPUT_FLAG_START_GLIDING: u32 = 32;
pub const INPUT_FLAG_STOP_GLIDING: u32 = 33;
pub const INPUT_FLAG_START_FLYING: u32 = 42;
pub const INPUT_FLAG_STOP_FLYING: u32 = 43;
pub const INPUT_FLAG_MISSED_SWING: u32 = 39;

/// Block action types (see `PlayerAction` in BedrockProtocol).
pub const BLOCK_ACTION_START_BREAK: i32 = 0;
pub const BLOCK_ACTION_ABORT_BREAK: i32 = 1;
pub const BLOCK_ACTION_STOP_BREAK: i32 = 2;
pub const BLOCK_ACTION_CRACK_BREAK: i32 = 18;
pub const BLOCK_ACTION_PREDICT_DESTROY: i32 = 26;
pub const BLOCK_ACTION_CONTINUE_DESTROY: i32 = 27;

/// Number of input flags (`PlayerAuthInputFlags::NUMBER_OF_FLAGS`).
pub const NUMBER_OF_INPUT_FLAGS: u32 = 66;

/// Maximum block actions accepted from a single packet (PM throws above
/// 100 in `InGamePacketHandler::handlePlayerAuthInput`).
pub const MAX_BLOCK_ACTIONS: u32 = 100;

/// A block action carried by `PlayerAuthInputPacket` when the
/// `PERFORM_BLOCK_ACTIONS` input flag is set. `PlayerBlockAction::read`
/// always carries position + face (also for `STOP_BREAK`/`ABORT_BREAK`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerBlockAction {
    pub action_type: i32,
    pub position: Option<(i32, i32, i32)>,
    pub face: Option<i32>,
}

/// The `itemInteractionData` optional carried by `PlayerAuthInputPacket`
/// (`BedrockProtocol/src/types/ItemInteractionData.php`). When the
/// `PERFORM_ITEM_INTERACTION` input flag is set, a protocol 2225 client
/// delivers its `UseItemTransactionData` inside the movement packet instead of
/// (or alongside) the legacy `InventoryTransactionPacket` 0x1E.
///
/// Wire layout (reference decode order, `ItemInteractionData::read`):
/// 1. `requestId` — VarInt (zigzag, `VarInt::readSignedInt`).
/// 2. `requestChangedSlots` — `CommonTypes::readOptional`: presence byte,
///    then `readList` (`VarUInt` count + entries). Each
///    `InventoryTransactionChangedSlotsHack` is a `u8` container id, a
///    `VarUInt` slot count and that many `u8` slot indexes.
/// 3. `transactionData` — `UseItemTransactionData::decode`: first the
///    `NetworkInventoryAction` list (`VarUInt` count + entries, each
///    `sourceType VarUInt`, nullable `windowId i8`, nullable
///    `sourceFlags VarUInt`, `inventorySlot VarUInt`, two
///    `getItemStackWrapper` item stacks), then the type payload read by
///    [`UseItemData::decode`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemInteractionData {
    pub request_id: i32,
    pub request_changed_slots: Option<Vec<ChangedSlots>>,
    pub actions: Vec<NetworkInventoryAction>,
    pub transaction: UseItemData,
}

/// PM drops a `PlayerAuthInputPacket` whose item use transaction carries more
/// than 100 actions (`InGamePacketHandler::handlePlayerAuthInput`); above that
/// cap the tail cannot be valid, so the whole attempt fails instead of
/// pretending to skip a huge bogus count.
const MAX_EMBEDDED_TRANSACTION_ACTIONS: u32 = 100;

impl ItemInteractionData {
    /// Reference decode (`ItemInteractionData::read` + `UseItemTransactionData::decode`).
    pub fn read(reader: &mut Reader) -> Result<Self, CodecError> {
        let request_id = reader.read_var_i32()?;

        let request_changed_slots = if reader.read_bool()? {
            let count = reader.read_var_u32()?;
            let mut groups = Vec::new();
            for _ in 0..count {
                let container_id = reader.read_byte()?;
                let slot_count = reader.read_var_u32()?;
                let mut slot_indexes = Vec::new();
                for _ in 0..slot_count {
                    slot_indexes.push(reader.read_byte()?);
                }
                groups.push(ChangedSlots {
                    container_id,
                    slot_indexes,
                });
            }
            Some(groups)
        } else {
            None
        };

        // `TransactionData::decode` reads the actions list first.
        let count = reader.read_var_u32()?;
        if count > MAX_EMBEDDED_TRANSACTION_ACTIONS {
            return Err(CodecError::Custom(format!(
                "Too many actions in item use transaction: {count}"
            )));
        }
        let mut actions = Vec::new();
        for _ in 0..count {
            actions.push(NetworkInventoryAction::decode(reader)?);
        }
        let transaction = UseItemData::decode(reader)?;

        Ok(Self {
            request_id,
            request_changed_slots,
            actions,
            transaction,
        })
    }
}

/// Best-effort extraction of block actions from the raw `input_data` tail of
/// a `PlayerAuthInputPacket` (everything after head yaw).
///
/// The tail layout mirrors `PlayerAuthInputPacket::decodePayload`:
/// `inputFlags` (VarUInt count + signed-varint flag indexes), `inputMode`
/// (VarUInt), `playMode` (VarUInt), `interactionMode` (VarInt),
/// `interactRotation` (2x f32), `tick` (VarUIntLong), `delta` (3x f32),
/// `itemInteractionData` optional, `itemStackRequest` optional,
/// `blockActions` optional (bool + VarUInt count + actions),
/// `vehicleRotation` optional, `vehicleActorUniqueId` optional,
/// `analogMoveVec` (2x f32), `cameraOrientation` (3x f32), `rawMove`
/// (2x f32). Both the modern and the legacy (pre-interactionMode) header
/// shapes are attempted; the first attempt yielding validated, non-empty
/// actions wins.
///
/// When the client embeds item payloads (`PERFORM_ITEM_INTERACTION` /
/// `PERFORM_ITEM_STACK_REQUEST`) they are decoded/skipped byte-exactly per
/// the reference instead of dropping every block action (a real client sets
/// `PERFORM_ITEM_INTERACTION` when interacting with a held item and still
/// sends `blockActions` after it). Movement handling never depends on this
/// succeeding.
pub fn parse_block_actions(input_data: &[u8]) -> Vec<PlayerBlockAction> {
    parse_block_actions_result(input_data).0
}

/// Like [`parse_block_actions`] but also returns the embedded
/// [`ItemInteractionData`] when the tail carries it. The server uses the
/// transaction exactly like the legacy `InventoryTransactionPacket::TYPE_USE_ITEM`
/// (`handleUseItemTransaction` runs before `blockActions`).
pub fn parse_block_actions_result(
    input_data: &[u8],
) -> (Vec<PlayerBlockAction>, Option<ItemInteractionData>) {
    let Some((flags, _)) = split_auth_input_flags(input_data) else {
        return (Vec::new(), None);
    };
    let has = |bit: u32| (flags >> bit) & 1 == 1;
    // Movement-only tails never carry any of the three payload flags; skip
    // the (successfully parsing) decode attempt entirely for them.
    if !has(INPUT_FLAG_PERFORM_BLOCK_ACTIONS)
        && !has(INPUT_FLAG_PERFORM_ITEM_INTERACTION)
        && !has(INPUT_FLAG_PERFORM_ITEM_STACK_REQUEST)
    {
        return (Vec::new(), None);
    }
    for modern in [true, false] {
        if let Some(parsed) = parse_block_actions_with(input_data, modern) {
            return parsed;
        }
    }
    (Vec::new(), None)
}

/// Decodes the `PlayerAuthInputPacket` `inputFlags` field as a 128-bit
/// mask, or 0 when the tail is empty/malformed. The flags are a varlen
/// list (`VarUInt(count)` + signed-varint flag indexes), not a bitset:
/// bit n corresponds to index n in `PlayerAuthInputFlags`.
pub fn parse_input_flags(input_data: &[u8]) -> u128 {
    split_auth_input_flags(input_data).map(|(flags, _)| flags).unwrap_or(0)
}

/// Whether flag bit is set in a mask returned by parse_input_flags.
pub fn has_input_flag(flags: u128, bit: u32) -> bool {
    (flags >> bit) & 1 == 1
}

/// Reads the `inputFlags` list field, returning the assembled bits plus the
/// number of bytes consumed.
///
/// Wire format (`PlayerAuthInputPacket::decodePayload`):
/// `VarUInt(count)` followed by `count` signed-varint flag indexes. A
/// duplicate index or an index outside `[0, 66)` (`NUMBER_OF_INPUT_FLAGS`)
/// is malformed, mirroring the duplicate check in the reference decoder
/// and the `BitSet` bounds check.
fn split_auth_input_flags(input_data: &[u8]) -> Option<(u128, usize)> {
    let mut reader = Reader::new(input_data);
    let count = reader.read_var_u32().ok()?;
    if count > NUMBER_OF_INPUT_FLAGS {
        return None;
    }
    let mut value: u128 = 0;
    for _ in 0..count {
        let flag = reader.read_var_i32().ok()?;
        if !(0..NUMBER_OF_INPUT_FLAGS as i32).contains(&flag) {
            return None;
        }
        let bit = 1u128 << flag;
        if value & bit != 0 {
            return None;
        }
        value |= bit;
    }
    Some((value, reader.position()))
}

/// Attempts one header shape (`modern`: with interactionMode +
/// interactRotation). Returns `None` on any structural mismatch so the
/// caller can try the other shape.
fn parse_block_actions_with(
    input_data: &[u8],
    modern: bool,
) -> Option<(Vec<PlayerBlockAction>, Option<ItemInteractionData>)> {
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

    if modern {
        // `itemInteractionData` optional
        // (`CommonTypes::readOptional($in, ItemInteractionData::read(...))`):
        // a `0x01` presence byte followed by the full transaction; the item
        // interaction sits BEFORE the block actions in the same packet.
        // `PERFORM_ITEM_INTERACTION` flag presence matches the payload for
        // real clients; the decode follows the presence byte (PM never
        // gates the optional on the flag).
        let interaction_present = reader.read_bool().ok()?;
        let item_interaction = if interaction_present {
            Some(ItemInteractionData::read(&mut reader).ok()?)
        } else {
            None
        };

        // `itemStackRequest` optional — skipped byte-exactly
        // (`CommonTypes::readOptional($in, ItemStackRequest::read(...))`):
        // nothing on this server consumes the embedded request (GUI moves
        // arrive as the standalone 0x93 packet), but skipping it correctly
        // is what keeps the `blockActions` offset aligned.
        let request_present = reader.read_bool().ok()?;
        if request_present {
            skip_item_stack_request(&mut reader).ok()?;
        }

        // `blockActions` optional — `CommonTypes::readOptional($in,
        // fn => readList(PlayerBlockAction::read))`: one `0x01` byte then
        // the VarUInt count.
        let present = reader.read_byte().ok()?;
        if present != 0 && present != 1 {
            return None;
        }
        let actions = if present == 1 {
            read_block_actions(&mut reader)?
        } else {
            Vec::new()
        };
        skip_modern_trailing(&mut reader)?;
        return Some((actions, item_interaction));
    }

    // Legacy (pre-interactionMode) shape: no item-payload optionals and the
    // same tolerance as before the item payloads were ported.
    let present = reader.read_byte().ok()?;
    if present == 0 {
        return Some((Vec::new(), None));
    }
    if present != 1 {
        return None;
    }
    let actions = read_block_actions(&mut reader)?;
    // Sanity-check the tail: legacy packets end here (or carry vehicle
    // info), modern packets still carry analogMoveVec (8B) +
    // cameraOrientation (12B) + rawMove (8B).
    let remaining = reader.remaining();
    if remaining == 0 || remaining >= 28 {
        Some((actions, None))
    } else {
        None
    }
}

/// Reads the `blockActions` optional list (VarUInt count + actions), keeping
/// the validation of `PlayerBlockAction::read` + the handler face check that
/// this port used before the item payloads were decoded.
fn read_block_actions(reader: &mut Reader) -> Option<Vec<PlayerBlockAction>> {
    let count = reader.read_var_u32().ok()?;
    if count > MAX_BLOCK_ACTIONS {
        return None;
    }
    let mut actions = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let action_type = reader.read_var_i32().ok()?;
        // `PlayerBlockAction::isValidActionType` (and `::read`): every known
        // type carries position + face. Only the action type is validated at
        // decode time; facing is validated later by the handler for the
        // break-like actions, not here.
        let valid = matches!(
            action_type,
            BLOCK_ACTION_STOP_BREAK
                | BLOCK_ACTION_ABORT_BREAK
                | BLOCK_ACTION_START_BREAK
                | BLOCK_ACTION_CRACK_BREAK
                | BLOCK_ACTION_PREDICT_DESTROY
                | BLOCK_ACTION_CONTINUE_DESTROY
        );
        if !valid {
            return None;
        }
        let x = reader.read_var_i32().ok()?;
        let y = reader.read_var_i32().ok()?;
        let z = reader.read_var_i32().ok()?;
        let face = reader.read_var_i32().ok()?;
        // `validateFacing` in the reference handler (START/CONTINUE/CRACK/
        // PREDICT only). `STOP`/`ABORT` never validate the face.
        let face_checked = matches!(
            action_type,
            BLOCK_ACTION_START_BREAK
                | BLOCK_ACTION_CRACK_BREAK
                | BLOCK_ACTION_PREDICT_DESTROY
                | BLOCK_ACTION_CONTINUE_DESTROY
        );
        if face_checked && !(0..=5).contains(&face) {
            return None;
        }
        actions.push(PlayerBlockAction {
            action_type,
            position: Some((x, y, z)),
            face: Some(face),
        });
    }
    Some(actions)
}

/// Consumes the fields behind `blockActions` for the modern shape:
/// `vehicleRotation` + `vehicleActorUniqueId` optionals (PM requires both
/// present or both absent), then `analogMoveVec` (8B), `cameraOrientation`
/// (12B) and `rawMove` (8B). The tail must end exactly there, which is what
/// a real protocol 2225 client sends (`decodePayload` ends at `rawMove`).
fn skip_modern_trailing(reader: &mut Reader) -> Option<()> {
    let has_rotation = reader.read_bool().ok()?;
    let has_unique_id = reader.read_bool().ok()?;
    if has_rotation != has_unique_id {
        // "Vehicle rotation and actor unique ID must both be present or
        // both be absent" (`PlayerAuthInputPacket::decodePayload`).
        return None;
    }
    if has_rotation {
        reader.read_bytes(8).ok()?; // vehicleRotation (2x f32)
        reader.read_var_i64().ok()?; // vehicleActorUniqueId (signed VarLong)
    }
    reader.read_bytes(28).ok()?; // analogMoveVec 8 + cameraOrientation 12 + rawMove 8
    if reader.remaining() != 0 {
        return None;
    }
    Some(())
}

/// Reads an `ItemStackRequest` (`BedrockProtocol/src/types/inventory/stackrequest/ItemStackRequest.php`)
/// from the stream and discards the decoded values; used purely to keep the
/// byte offset protocol-correct after the `itemStackRequest` optional
/// presence byte. The decoded decision is all-or-nothing: an unknown or
/// malformed action aborts, mirroring the reference constructor throwing
/// `Unhandled item stack request action type ...`.
///
/// `ItemStackRequest::read` layout:
/// `requestId` VarInt (zigzag, `readItemStackRequestId`), `actions`
/// `readList` (each: outer `VarUInt` type id, inner `u8` byte that must
/// equal the type's `INNER_TYPES` entry, then the per-action payload),
/// `filterStrings` `readList` of strings, `filterStringCause` `i32` LE.
fn skip_item_stack_request(reader: &mut Reader) -> Result<(), CodecError> {
    reader.read_var_i32()?; // requestId

    let action_count = reader.read_var_u32()?;
    if action_count > MAX_EMBEDDED_TRANSACTION_ACTIONS {
        return Err(CodecError::Custom(format!(
            "Too many actions in item stack request: {action_count}"
        )));
    }
    for _ in 0..action_count {
        let type_id = reader.read_var_u32()?;
        let inner_type_id = reader.read_byte()?;
        let Some(expected_inner) = stack_request_inner_type(type_id) else {
            return Err(CodecError::Custom(format!(
                "Unhandled item stack request action type {type_id}"
            )));
        };
        if expected_inner != inner_type_id {
            return Err(CodecError::Custom(format!(
                "ItemStackRequestAction type mismatch: outer type {type_id}, expected inner type {expected_inner}, actual inner type {inner_type_id}"
            )));
        }
        skip_stack_request_action(reader, type_id)?;
    }

    let filter_count = reader.read_var_u32()?;
    for _ in 0..filter_count {
        skip_varuint_string(reader)?;
    }
    reader.read_i32_le()?; // filterStringCause
    Ok(())
}

/// `ItemStackRequestActionType::INNER_TYPES`: outer type id -> inner byte.
fn stack_request_inner_type(type_id: u32) -> Option<u8> {
    Some(match type_id {
        0..=6 => type_id as u8, // TAKE..CRAFTING_CREATE_SPECIFIC_RESULT
        7 => 9,                 // LAB_TABLE_COMBINE
        8 => 10,                // BEACON_PAYMENT
        9 => 11,                // MINE_BLOCK
        10 => 12,               // CRAFTING_RECIPE
        11 => 13,               // CRAFTING_RECIPE_AUTO
        12 => 14,               // CREATIVE_CREATE
        13 => 15,               // CRAFTING_RECIPE_OPTIONAL
        14 => 16,               // CRAFTING_GRINDSTONE
        15 => 17,               // CRAFTING_LOOM
        16 => 18,               // CRAFTING_RESERVED
        17 => 19,               // CRAFTING_NON_IMPLEMENTED_DEPRECATED
        18 => 20,               // CRAFTING_RESULTS_DEPRECATED
        _ => return None,
    })
}

/// Payload of one `ItemStackRequest` action (each action's `::read`).
fn skip_stack_request_action(reader: &mut Reader, type_id: u32) -> Result<(), CodecError> {
    match type_id {
        // TAKE / PLACE (TakeOrPlaceStackRequestActionTrait) and
        // CRAFTING_CONSUME_INPUT: `u8` count + two slots.
        0 | 1 | 5 => {
            reader.read_byte()?; // count
            skip_stack_request_slot(reader)?;
            skip_stack_request_slot(reader)?;
        }
        // SWAP: two slots.
        2 => {
            skip_stack_request_slot(reader)?;
            skip_stack_request_slot(reader)?;
        }
        // DROP: `u8` count + slot + random bool.
        3 => {
            reader.read_byte()?;
            skip_stack_request_slot(reader)?;
            reader.read_byte()?;
        }
        // DESTROY: `u8` count + slot.
        4 => {
            reader.read_byte()?;
            skip_stack_request_slot(reader)?;
        }
        // CRAFTING_CREATE_SPECIFIC_RESULT: `u8` result index.
        6 => {
            reader.read_byte()?;
        }
        // LAB_TABLE_COMBINE: no payload.
        7 => {}
        // BEACON_PAYMENT: two zigzag VarInt effect ids.
        8 => {
            reader.read_var_i32()?;
            reader.read_var_i32()?;
        }
        // MINE_BLOCK: VarInt hotbar slot, VarInt predicted durability,
        // `readItemStackNetIdVariant` (i32 LE) stack id.
        9 => {
            reader.read_var_i32()?;
            reader.read_var_i32()?;
            reader.read_i32_le()?;
        }
        // CRAFTING_RECIPE: VarUInt recipe net id + `u8` repetitions.
        10 => {
            reader.read_var_u32()?;
            reader.read_byte()?;
        }
        // CRAFTING_RECIPE_AUTO: VarUInt recipe id + `u8` repetitions +
        // ingredient list (`readItemDescriptorNormal` + u16 LE count each).
        11 => {
            reader.read_var_u32()?;
            reader.read_byte()?;
            let ingredient_count = reader.read_var_u32()?;
            for _ in 0..ingredient_count {
                skip_item_descriptor_normal(reader)?;
                reader.read_u16_le()?;
            }
        }
        // CREATIVE_CREATE: VarUInt creative item id + `u8` repetitions.
        12 => {
            reader.read_var_u32()?;
            reader.read_byte()?;
        }
        // CRAFTING_RECIPE_OPTIONAL: VarUInt recipe id + i32 LE filter
        // string index.
        13 => {
            reader.read_var_u32()?;
            reader.read_i32_le()?;
        }
        // CRAFTING_GRINDSTONE: u32 LE recipe id (!), `u8` repetitions,
        // zigzag VarInt repair cost.
        14 => {
            reader.read_u32_le()?;
            reader.read_byte()?;
            reader.read_var_i32()?;
        }
        // CRAFTING_LOOM: pattern id string + `u8` repetitions.
        15 => {
            skip_varuint_string(reader)?;
            reader.read_byte()?;
        }
        // CRAFTING_RESERVED: reserved id string + `u8` num crafts.
        16 => {
            skip_varuint_string(reader)?;
            reader.read_byte()?;
        }
        // CRAFTING_NON_IMPLEMENTED_DEPRECATED: no payload.
        17 => {}
        // CRAFTING_RESULTS_DEPRECATED: entry list + `u8` iterations. Each
        // entry: `readItemDescriptorNormal` + u16 LE count + VarUInt block
        // runtime id + raw extra data string.
        18 => {
            let result_count = reader.read_var_u32()?;
            for _ in 0..result_count {
                skip_item_descriptor_normal(reader)?;
                reader.read_u16_le()?;
                reader.read_var_u32()?;
                skip_varuint_string(reader)?;
            }
            reader.read_byte()?;
        }
        _ => {
            return Err(CodecError::Custom(format!(
                "Unhandled item stack request action type {type_id}"
            )));
        }
    }
    Ok(())
}

/// `ItemStackRequestSlotInfo::read`: `FullContainerName` (u8 container id +
/// optional u32 LE dynamic id) + `u8` slot id + `readItemStackNetIdVariant`
/// (i32 LE) stack id.
fn skip_stack_request_slot(reader: &mut Reader) -> Result<(), CodecError> {
    reader.read_byte()?; // container id
    if reader.read_bool()? {
        reader.read_u32_le()?; // dynamic id
    }
    reader.read_byte()?; // slot id
    reader.read_i32_le()?; // stack id
    Ok(())
}

/// `CommonTypes::readItemDescriptorNormal`: `VarUInt` descriptor ordinal +
/// `u8` inner ordinal (must match) + per-type payload.
/// Ordinals (`ItemDescriptorType`, declaration order): 0 empty, 1
/// string-id+meta, 2 molang, 3 item tag (read as tag-only here).
fn skip_item_descriptor_normal(reader: &mut Reader) -> Result<(), CodecError> {
    let descriptor_type_ord = reader.read_var_u32()?;
    let inner_type_ord = reader.read_byte()?;
    if descriptor_type_ord > 3 || descriptor_type_ord != inner_type_ord as u32 {
        return Err(CodecError::Custom(format!(
            "Item descriptor type mismatch: outer type {descriptor_type_ord}, inner type {inner_type_ord}"
        )));
    }
    match descriptor_type_ord {
        0 => {} // EMPTY
        // STRING_ID_META: id string + zigzag VarInt meta.
        1 => {
            skip_varuint_string(reader)?;
            reader.read_var_i32()?;
        }
        // MOLANG: expression string + u16 LE version.
        2 => {
            skip_varuint_string(reader)?;
            reader.read_u16_le()?;
        }
        // TAG (`readTagOnly`): tag string.
        3 => {
            skip_varuint_string(reader)?;
        }
        _ => unreachable!("ordinals > 3 are rejected above"),
    }
    Ok(())
}

/// `CommonTypes::getString`: VarUInt length + bytes (consumed and dropped).
fn skip_varuint_string(reader: &mut Reader) -> Result<(), CodecError> {
    let len = reader.read_var_u32()? as usize;
    reader.read_bytes(len)?;
    Ok(())
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

    /// Builds an `input_data` tail: flags list with [35], zeroed movement
    /// fields, then the given block actions. Flag index 35 zigzags to 0x46
    /// as a signed varint; the list is `VarUInt(1)` + `VarInt(35)`.
    fn build_tail(modern: bool, actions: &[(i32, Option<(i32, i32, i32, i32)>)]) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(1); // flag count
        writer.write_var_i32(35); // PERFORM_BLOCK_ACTIONS
        writer.write_var_u32(0); // inputMode
        writer.write_var_u32(0); // playMode
        if modern {
            writer.write_var_i32(0); // interactionMode (signed)
            writer.write_bytes(&[0u8; 8]); // interactRotation
        }
        writer.write_var_u64(1234); // tick
        writer.write_bytes(&[0u8; 12]); // delta
        if modern {
            // `itemInteractionData` + `itemStackRequest` optionals, both
            // absent (`CommonTypes::readOptional` presence bytes).
            writer.write_byte(0);
            writer.write_byte(0);
        }
        // `CommonTypes::readOptional` for `blockActions`: present bool (`0x01`)
        // then the VarUInt count. The old fixture wrote only the count, which
        // real clients never do (see `parse_block_actions_with`).
        writer.write_byte(1); // blockActions present
        writer.write_var_u32(actions.len() as u32);
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
            // vehicleRotation + vehicleActorUniqueId optionals (both absent),
            // then analogMoveVec (8) + cameraOrientation (12) + rawMove (8).
            writer.write_bytes(&[0u8; 2 + 28]);
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
    fn stop_break_carries_position_like_reference() {
        // `PlayerBlockAction::read` always carries position + face, even for
        // STOP_BREAK (old Rust treated it as positionless and misparsed the
        // stream).
        let tail = build_tail(false, &[(BLOCK_ACTION_STOP_BREAK, Some((1, 64, 1, 0)))]);
        let actions = parse_block_actions(&tail);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].position, Some((1, 64, 1)));
        assert_eq!(actions[0].face, Some(0));
    }

    #[test]
    fn flags_decode_as_count_prefixed_list_not_bitset() {
        // Flags [8, 27] (SNEAKING + START_SNEAKING) on the wire:
        // VarUInt(2) + VarInt(8) + VarInt(27).
        let mut writer = Writer::new();
        writer.write_var_u32(2);
        writer.write_var_i32(8);
        writer.write_var_i32(27);
        writer.write_bytes(&[0u8; 8]); // minimal rest is not needed for flags
        let tail = writer.into_bytes().to_vec();
        let flags = parse_input_flags(&tail);
        assert!(has_input_flag(flags, 8));
        assert!(has_input_flag(flags, 27));
        assert!(!has_input_flag(flags, 28));
        // Duplicate index is malformed (reference throws).
        let mut dup = Writer::new();
        dup.write_var_u32(2);
        dup.write_var_i32(8);
        dup.write_var_i32(8);
        assert_eq!(parse_input_flags(&dup.into_bytes().to_vec()), 0);
        // Out-of-range index is malformed (`BitSet` bounds check).
        let mut oob = Writer::new();
        oob.write_var_u32(1);
        oob.write_var_i32(66);
        assert_eq!(parse_input_flags(&oob.into_bytes().to_vec()), 0);
    }

    #[test]
    fn garbage_tail_is_ignored() {
        assert!(parse_block_actions(&[0xFF, 0x00, 0x13, 0x77, 0x42]).is_empty());
        // Flag count 0x80 0x80 ... without terminator within the slice.
        assert!(parse_block_actions(&[0x80; 10]).is_empty());
        // Flag set but truncated before the action count.
        let mut truncated = Writer::new();
        truncated.write_var_u32(1);
        truncated.write_var_i32(35);
        truncated.write_bytes(&[0x00]);
        assert!(parse_block_actions(&truncated.into_bytes().to_vec()).is_empty());
    }

    #[test]
    fn item_payload_flags_without_payload_bytes_fail_decode() {
        // Flags [34 (item interaction), 35 (block actions)] with nothing but
        // zero bytes after them: the payload presence bytes read as absent
        // and the modern trailing-length check fails, so the tail is
        // rejected on both header shapes instead of being silently
        // misparsed. (Previously the item payload flags unconditionally
        // bailed the whole parse.)
        let mut writer = Writer::new();
        writer.write_var_u32(2);
        writer.write_var_i32(34);
        writer.write_var_i32(35);
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

    // ------------------------------------------------------------------
    // Item payloads embedded in the tail (reference: BedrockProtocol
    // PlayerAuthInputPacket.php + types/ItemInteractionData.php +
    // types/inventory/UseItemTransactionData.php + stackrequest/*).
    // ------------------------------------------------------------------

    use crate::packets::inventory_transaction::ACTION_CLICK_BLOCK;
    use crate::ItemStack;

    const HELD_STACK: ItemStack = ItemStack {
        network_id: 745,
        count: 64,
        meta: 0,
        stack_id: 1,
        block_runtime_id: 5,
        nbt: Vec::new(),
    };

    fn write_modern_head(writer: &mut Writer, flags: &[i32]) {
        // `inputFlags`: VarUInt count + signed-varint flag indexes.
        writer.write_var_u32(flags.len() as u32);
        for flag in flags {
            writer.write_var_i32(*flag);
        }
        writer.write_var_u32(0); // inputMode
        writer.write_var_u32(0); // playMode
        writer.write_var_i32(0); // interactionMode
        writer.write_bytes(&[0u8; 8]); // interactRotation (2x f32)
        writer.write_var_u64(1234); // tick
        writer.write_bytes(&[0u8; 12]); // delta (3x f32)
    }

    fn write_single_block_action(
        writer: &mut Writer,
        action_type: i32,
        x: i32,
        y: i32,
        z: i32,
        face: i32,
    ) {
        writer.write_byte(1); // blockActions optional present
        writer.write_var_u32(1); // one action
        writer.write_var_i32(action_type);
        writer.write_var_i32(x);
        writer.write_var_i32(y);
        writer.write_var_i32(z);
        writer.write_var_i32(face);
    }

    fn write_modern_trailer(writer: &mut Writer) {
        // vehicleRotation + vehicleActorUniqueId optionals (both absent),
        // then analogMoveVec (8) + cameraOrientation (12) + rawMove (8).
        writer.write_bytes(&[0u8; 2 + 28]);
    }

    /// Writes the reference `ItemInteractionData`: request id, one predicted
    /// `NetworkInventoryAction` and the `UseItemTransactionData`
    /// `ACTION_CLICK_BLOCK` payload for the held stack.
    fn write_item_interaction(writer: &mut Writer, held: &ItemStack) {
        writer.write_bool(true); // itemInteractionData optional present
        writer.write_var_i32(-2); // requestId (legacy request ids are negative)
        writer.write_bool(false); // requestChangedSlots absent
        // `TransactionData::decode` reads the actions list first: one
        // SOURCE_CONTAINER prediction for hotbar slot 4.
        writer.write_var_u32(1);
        writer.write_var_u32(0); // SOURCE_CONTAINER
        writer.write_bool(true);
        writer.write_byte(0); // windowId
        writer.write_bool(false); // sourceFlags absent
        writer.write_var_u32(4); // inventorySlot
        ItemStack::empty().encode(writer); // old item: air
        held.encode(writer); // new item: the held stack
        // `UseItemTransactionData::decodeData` payload.
        writer.write_var_i32(0); // ACTION_CLICK_BLOCK
        writer.write_byte(1); // triggerType PLAYER_INPUT
        writer.write_var_i32(10);
        writer.write_var_i32(64);
        writer.write_var_i32(-5); // blockPosition
        writer.write_byte(1); // face
        writer.write_var_i32(4); // hotbarSlot
        writer.write_byte(0); // hand MAINHAND
        held.encode(writer); // itemInHand
        writer.write_f32_le(1.5);
        writer.write_f32_le(66.62);
        writer.write_f32_le(-4.5); // playerPosition
        writer.write_f32_le(10.5);
        writer.write_f32_le(65.0);
        writer.write_f32_le(-4.5); // clickPosition
        writer.write_var_u32(0); // blockRuntimeId of the clicked block
        writer.write_byte(1); // clientInteractPrediction SUCCESS
        writer.write_byte(0); // clientCooldownState
    }

    /// Writes a reference `ItemStackRequest`: one `Take` action (u8 count +
    /// two slot infos) and one filter string — the exact bytes
    /// `skip_item_stack_request` must consume to stay aligned.
    fn write_item_stack_request(writer: &mut Writer) {
        writer.write_bool(true); // itemStackRequest optional present
        writer.write_var_i32(-1); // requestId
        writer.write_var_u32(1); // one action
        writer.write_var_u32(0); // TAKE (outer type id)
        writer.write_byte(0); // INNER_TYPES[TAKE] = 0
        writer.write_byte(1); // count (u8)
        // Source slot: FullContainerName(hotbar 36, no dynamic id), slot 0,
        // stack id 1 (readItemStackNetIdVariant: i32 LE).
        writer.write_byte(36);
        writer.write_bool(false);
        writer.write_byte(0);
        writer.write_i32_le(1);
        // Destination slot: container 12 carrying a dynamic id.
        writer.write_byte(12);
        writer.write_bool(true);
        writer.write_u32_le(7);
        writer.write_byte(1);
        writer.write_i32_le(2);
        // filterStrings + filterStringCause.
        writer.write_var_u32(1);
        writer.write_string("some_filter_string");
        writer.write_i32_le(0);
    }

    #[test]
    fn item_interaction_and_block_actions_decode_together() {
        // Real-client order for a place/interact on 2225: flags carry both
        // PERFORM_ITEM_INTERACTION (34) and PERFORM_BLOCK_ACTIONS (35); the
        // tail holds the transaction first, then the block actions.
        let mut writer = Writer::new();
        write_modern_head(&mut writer, &[34, 35]);
        write_item_interaction(&mut writer, &HELD_STACK);
        writer.write_bool(false); // itemStackRequest absent
        write_single_block_action(&mut writer, BLOCK_ACTION_PREDICT_DESTROY, 0, 65, 0, 0);
        write_modern_trailer(&mut writer);
        let tail = writer.into_bytes().to_vec();

        let (actions, interaction) = parse_block_actions_result(&tail);
        assert_eq!(actions.len(), 1, "block actions must survive the payload");
        assert_eq!(actions[0].action_type, BLOCK_ACTION_PREDICT_DESTROY);
        assert_eq!(actions[0].position, Some((0, 65, 0)));
        assert_eq!(actions[0].face, Some(0));
        // parse_block_actions keeps its single-return behavior.
        assert_eq!(parse_block_actions(&tail), actions);

        let interaction = interaction.expect("item interaction decoded");
        assert_eq!(interaction.request_id, -2);
        assert!(interaction.request_changed_slots.is_none());
        assert_eq!(interaction.actions.len(), 1);
        assert_eq!(interaction.actions[0].inventory_slot, 4);
        assert_eq!(interaction.transaction.action_type, ACTION_CLICK_BLOCK);
        assert_eq!(interaction.transaction.block_position, (10, 64, -5));
        assert_eq!(interaction.transaction.face, 1);
        assert_eq!(interaction.transaction.hotbar_slot, 4);
        assert_eq!(interaction.transaction.item_in_hand, HELD_STACK);
        assert_eq!(
            interaction.transaction.player_position,
            (1.5, 66.62, -4.5)
        );
        assert_eq!(interaction.transaction.click_position, (10.5, 65.0, -4.5));
        assert_eq!(interaction.transaction.client_interact_prediction, 1);
    }

    #[test]
    fn item_stack_request_optional_is_skipped_byte_exact() {
        // Flags [36 (PERFORM_ITEM_STACK_REQUEST), 35 (block actions)].
        let mut writer = Writer::new();
        write_modern_head(&mut writer, &[36, 35]);
        writer.write_bool(false); // itemInteractionData absent
        write_item_stack_request(&mut writer);
        write_single_block_action(&mut writer, BLOCK_ACTION_START_BREAK, 10, 64, -20, 1);
        write_modern_trailer(&mut writer);
        let tail = writer.into_bytes().to_vec();

        let (actions, interaction) = parse_block_actions_result(&tail);
        assert!(interaction.is_none());
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type, BLOCK_ACTION_START_BREAK);
        assert_eq!(actions[0].position, Some((10, 64, -20)));
    }

    #[test]
    fn interaction_stack_request_and_block_actions_in_one_tail() {
        let mut writer = Writer::new();
        write_modern_head(&mut writer, &[34, 36, 35]);
        write_item_interaction(&mut writer, &HELD_STACK);
        write_item_stack_request(&mut writer);
        write_single_block_action(&mut writer, BLOCK_ACTION_STOP_BREAK, 1, 64, 1, 0);
        write_modern_trailer(&mut writer);
        let tail = writer.into_bytes().to_vec();

        let (actions, interaction) = parse_block_actions_result(&tail);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type, BLOCK_ACTION_STOP_BREAK);
        assert_eq!(actions[0].position, Some((1, 64, 1)));
        let interaction = interaction.expect("item interaction decoded");
        assert_eq!(interaction.request_id, -2);
        assert_eq!(interaction.transaction.action_type, ACTION_CLICK_BLOCK);
        assert_eq!(interaction.transaction.item_in_hand, HELD_STACK);
    }

    #[test]
    fn unhandled_stack_request_action_type_rejects_decode() {
        // The reference decoder throws for unknown outer type ids, so the
        // tail must not decode instead of silently losing its alignment.
        let mut writer = Writer::new();
        write_modern_head(&mut writer, &[36, 35]);
        writer.write_bool(false); // itemInteractionData absent
        writer.write_bool(true); // itemStackRequest present
        writer.write_var_i32(-1); // requestId
        writer.write_var_u32(1); // one action
        writer.write_var_u32(99); // unknown outer type id
        writer.write_byte(0); // inner byte (mismatch)
        write_single_block_action(&mut writer, BLOCK_ACTION_PREDICT_DESTROY, 0, 65, 0, 0);
        write_modern_trailer(&mut writer);
        let tail = writer.into_bytes().to_vec();

        let (actions, interaction) = parse_block_actions_result(&tail);
        assert!(interaction.is_none());
        assert!(actions.is_empty());
    }
}
