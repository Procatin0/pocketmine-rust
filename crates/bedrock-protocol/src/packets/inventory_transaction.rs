use crate::binary::{CodecError, Reader, Writer};
use crate::packets::item_stack::ItemStack;

/// Packet ID 0x1E (30): InventoryTransactionPacket.
pub const PACKET_ID: u32 = 0x1E;

pub const TYPE_NORMAL: u32 = 0;
pub const TYPE_MISMATCH: u32 = 1;
pub const TYPE_USE_ITEM: u32 = 2;
pub const TYPE_USE_ITEM_ON_ENTITY: u32 = 3;
pub const TYPE_RELEASE_ITEM: u32 = 4;

pub const SOURCE_CONTAINER: u32 = 0;
pub const SOURCE_GLOBAL: u32 = 1;
pub const SOURCE_WORLD: u32 = 2;
pub const SOURCE_CREATIVE: u32 = 3;
pub const SOURCE_TODO: u32 = 99999;

pub const ACTION_MAGIC_SLOT_CREATIVE_DELETE_ITEM: u32 = 0;
pub const ACTION_MAGIC_SLOT_CREATIVE_CREATE_ITEM: u32 = 1;
pub const ACTION_MAGIC_SLOT_DROP_ITEM: u32 = 0;
pub const ACTION_MAGIC_SLOT_PICKUP_ITEM: u32 = 1;

pub const ACTION_CLICK_BLOCK: i32 = 0;
pub const ACTION_CLICK_AIR: i32 = 1;
pub const ACTION_BREAK_BLOCK: i32 = 2;
pub const ACTION_USE_AS_ATTACK: i32 = 3;

pub const ACTION_INTERACT: i32 = 0;
pub const ACTION_ATTACK: i32 = 1;
pub const ACTION_ITEM_INTERACT: i32 = 2;

pub const ACTION_RELEASE: i32 = 0;
pub const ACTION_CONSUME: i32 = 1;

/// One `NetworkInventoryAction`: the source slot, the item the client thinks it
/// holds there and the item it wants to put in its place.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkInventoryAction {
    pub source_type: u32,
    pub window_id: Option<i8>,
    pub source_flags: Option<u32>,
    pub inventory_slot: u32,
    pub old_item: ItemStack,
    pub new_item: ItemStack,
}

impl NetworkInventoryAction {
    pub fn encode(&self, writer: &mut Writer) {
        writer.write_var_u32(self.source_type);

        writer.write_bool(self.window_id.is_some());
        if let Some(window_id) = self.window_id {
            writer.write_byte(window_id as u8);
        }

        writer.write_bool(self.source_flags.is_some());
        if let Some(flags) = self.source_flags {
            writer.write_var_u32(flags);
        }

        writer.write_var_u32(self.inventory_slot);
        self.old_item.encode(writer);
        self.new_item.encode(writer);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let source_type = reader.read_var_u32()?;

        let window_id = if reader.read_bool()? {
            Some(reader.read_i8()?)
        } else {
            None
        };

        let source_flags = if reader.read_bool()? {
            Some(reader.read_var_u32()?)
        } else {
            None
        };

        let inventory_slot = reader.read_var_u32()?;
        let old_item = ItemStack::decode(reader)?;
        let new_item = ItemStack::decode(reader)?;

        Ok(Self {
            source_type,
            window_id,
            source_flags,
            inventory_slot,
            old_item,
            new_item,
        })
    }
}

/// Entry of `requestChangedSlots`: slots the client wants resent unconditionally.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangedSlots {
    pub container_id: u8,
    pub slot_indexes: Vec<u8>,
}

/// Extra data of a `TYPE_USE_ITEM` transaction.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UseItemData {
    pub action_type: i32,
    pub trigger_type: u8,
    pub block_position: (i32, i32, i32),
    pub face: u8,
    pub hotbar_slot: i32,
    pub hand: u8,
    pub item_in_hand: ItemStack,
    pub player_position: (f32, f32, f32),
    pub click_position: (f32, f32, f32),
    pub block_runtime_id: u32,
    pub client_interact_prediction: u8,
    pub client_cooldown_state: u8,
}

impl UseItemData {
    /// Field order of `UseItemTransactionData::decodeData`
    /// (`BedrockProtocol/src/types/inventory/UseItemTransactionData.php`):
    /// `actionType` VarInt, `triggerType` u8, `blockPosition` 3x VarInt,
    /// `face` u8, `hotbarSlot` VarInt, `hand` u8, `itemInHand`
    /// `CommonTypes::getItemStackWrapper` (i16 LE id, u16 LE count,
    /// VarUInt meta, optional VarInt stack net id, VarUInt block runtime
    /// id, raw extra data string), `playerPosition` + `clickPosition`
    /// (3x f32 each), `blockRuntimeId` VarUInt, `clientInteractPrediction`
    /// u8, `clientCooldownState` u8. Shared by `InventoryTransactionPacket`
    /// (TYPE_USE_ITEM) and the `itemInteractionData` embedded in
    /// `PlayerAuthInputPacket`.
    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            action_type: reader.read_var_i32()?,
            trigger_type: reader.read_byte()?,
            block_position: (
                reader.read_var_i32()?,
                reader.read_var_i32()?,
                reader.read_var_i32()?,
            ),
            face: reader.read_byte()?,
            hotbar_slot: reader.read_var_i32()?,
            hand: reader.read_byte()?,
            item_in_hand: ItemStack::decode(reader)?,
            player_position: (
                reader.read_f32_le()?,
                reader.read_f32_le()?,
                reader.read_f32_le()?,
            ),
            click_position: (
                reader.read_f32_le()?,
                reader.read_f32_le()?,
                reader.read_f32_le()?,
            ),
            block_runtime_id: reader.read_var_u32()?,
            client_interact_prediction: reader.read_byte()?,
            client_cooldown_state: reader.read_byte()?,
        })
    }
}

/// Extra data of a `TYPE_USE_ITEM_ON_ENTITY` transaction.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UseItemOnEntityData {
    pub actor_runtime_id: u64,
    pub action_type: i32,
    pub hotbar_slot: i32,
    pub hand: u8,
    pub item_in_hand: ItemStack,
    pub player_position: (f32, f32, f32),
    pub click_position: (f32, f32, f32),
}

/// Extra data of a `TYPE_RELEASE_ITEM` transaction.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReleaseItemData {
    pub action_type: i32,
    pub hotbar_slot: i32,
    pub item_in_hand: ItemStack,
    pub head_position: (f32, f32, f32),
    pub hand: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransactionData {
    Normal,
    Mismatch,
    UseItem(UseItemData),
    UseItemOnEntity(UseItemOnEntityData),
    ReleaseItem(ReleaseItemData),
}

impl Default for TransactionData {
    fn default() -> Self {
        Self::Normal
    }
}

impl TransactionData {
    pub const fn type_id(&self) -> u32 {
        match self {
            Self::Normal => TYPE_NORMAL,
            Self::Mismatch => TYPE_MISMATCH,
            Self::UseItem(_) => TYPE_USE_ITEM,
            Self::UseItemOnEntity(_) => TYPE_USE_ITEM_ON_ENTITY,
            Self::ReleaseItem(_) => TYPE_RELEASE_ITEM,
        }
    }
}

/// InventoryTransactionPacket (0x1E), ported from Axolotl's
/// `InventoryTransactionPacket`: the transaction type comes first, then the
/// action list, then the type specific fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InventoryTransactionPacket {
    pub request_id: i32,
    pub request_changed_slots: Option<Vec<ChangedSlots>>,
    pub actions: Vec<NetworkInventoryAction>,
    pub data: TransactionData,
}

impl InventoryTransactionPacket {
    pub const PACKET_ID: u32 = 0x1E;

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_i32(self.request_id);

        match &self.request_changed_slots {
            Some(groups) => {
                writer.write_bool(true);
                writer.write_var_u32(groups.len() as u32);
                for group in groups {
                    writer.write_byte(group.container_id);
                    writer.write_var_u32(group.slot_indexes.len() as u32);
                    for slot in &group.slot_indexes {
                        writer.write_byte(*slot);
                    }
                }
            }
            None => writer.write_bool(false),
        }

        writer.write_var_u32(self.data.type_id());

        writer.write_var_u32(self.actions.len() as u32);
        for action in &self.actions {
            action.encode(writer);
        }

        match &self.data {
            TransactionData::Normal | TransactionData::Mismatch => {}
            TransactionData::UseItem(data) => {
                writer.write_var_i32(data.action_type);
                writer.write_byte(data.trigger_type);
                writer.write_var_i32(data.block_position.0);
                writer.write_var_i32(data.block_position.1);
                writer.write_var_i32(data.block_position.2);
                writer.write_byte(data.face);
                writer.write_var_i32(data.hotbar_slot);
                writer.write_byte(data.hand);
                data.item_in_hand.encode(writer);
                writer.write_f32_le(data.player_position.0);
                writer.write_f32_le(data.player_position.1);
                writer.write_f32_le(data.player_position.2);
                writer.write_f32_le(data.click_position.0);
                writer.write_f32_le(data.click_position.1);
                writer.write_f32_le(data.click_position.2);
                writer.write_var_u32(data.block_runtime_id);
                writer.write_byte(data.client_interact_prediction);
                writer.write_byte(data.client_cooldown_state);
            }
            TransactionData::UseItemOnEntity(data) => {
                writer.write_var_u64(data.actor_runtime_id);
                writer.write_var_i32(data.action_type);
                writer.write_var_i32(data.hotbar_slot);
                writer.write_byte(data.hand);
                data.item_in_hand.encode(writer);
                writer.write_f32_le(data.player_position.0);
                writer.write_f32_le(data.player_position.1);
                writer.write_f32_le(data.player_position.2);
                writer.write_f32_le(data.click_position.0);
                writer.write_f32_le(data.click_position.1);
                writer.write_f32_le(data.click_position.2);
            }
            TransactionData::ReleaseItem(data) => {
                writer.write_var_i32(data.action_type);
                writer.write_var_i32(data.hotbar_slot);
                data.item_in_hand.encode(writer);
                writer.write_f32_le(data.head_position.0);
                writer.write_f32_le(data.head_position.1);
                writer.write_f32_le(data.head_position.2);
                writer.write_byte(data.hand);
            }
        }
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let request_id = reader.read_var_i32()?;

        let request_changed_slots = if reader.read_bool()? {
            let count = reader.read_var_u32()?;
            let mut groups = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let container_id = reader.read_byte()?;
                let slot_count = reader.read_var_u32()?;
                let mut slot_indexes = Vec::with_capacity(slot_count as usize);
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

        let transaction_type = reader.read_var_u32()?;

        let action_count = reader.read_var_u32()?;
        let mut actions = Vec::with_capacity(action_count as usize);
        for _ in 0..action_count {
            actions.push(NetworkInventoryAction::decode(reader)?);
        }

        let data = match transaction_type {
            TYPE_NORMAL => TransactionData::Normal,
            TYPE_MISMATCH => {
                if !actions.is_empty() {
                    return Err(CodecError::Custom(format!(
                        "Mismatch transaction type should not have any actions, got {}",
                        actions.len()
                    )));
                }
                TransactionData::Mismatch
            }
            TYPE_USE_ITEM => TransactionData::UseItem(UseItemData::decode(reader)?),
            TYPE_USE_ITEM_ON_ENTITY => TransactionData::UseItemOnEntity(UseItemOnEntityData {
                actor_runtime_id: reader.read_var_u64()?,
                action_type: reader.read_var_i32()?,
                hotbar_slot: reader.read_var_i32()?,
                hand: reader.read_byte()?,
                item_in_hand: ItemStack::decode(reader)?,
                player_position: (
                    reader.read_f32_le()?,
                    reader.read_f32_le()?,
                    reader.read_f32_le()?,
                ),
                click_position: (
                    reader.read_f32_le()?,
                    reader.read_f32_le()?,
                    reader.read_f32_le()?,
                ),
            }),
            TYPE_RELEASE_ITEM => TransactionData::ReleaseItem(ReleaseItemData {
                action_type: reader.read_var_i32()?,
                hotbar_slot: reader.read_var_i32()?,
                item_in_hand: ItemStack::decode(reader)?,
                head_position: (
                    reader.read_f32_le()?,
                    reader.read_f32_le()?,
                    reader.read_f32_le()?,
                ),
                hand: reader.read_byte()?,
            }),
            other => {
                return Err(CodecError::Custom(format!(
                    "Unknown inventory transaction type {other}"
                )))
            }
        };

        Ok(Self {
            request_id,
            request_changed_slots,
            actions,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(network_id: i16, count: u16) -> ItemStack {
        ItemStack::new(network_id, count)
    }

    #[test]
    fn roundtrips_a_normal_transaction() {
        let packet = InventoryTransactionPacket {
            request_id: -7,
            request_changed_slots: Some(vec![ChangedSlots {
                container_id: 0,
                slot_indexes: vec![0, 1],
            }]),
            actions: vec![
                NetworkInventoryAction {
                    source_type: SOURCE_CONTAINER,
                    window_id: Some(0),
                    source_flags: None,
                    inventory_slot: 0,
                    old_item: stack(1, 32),
                    new_item: ItemStack::empty(),
                },
                NetworkInventoryAction {
                    source_type: SOURCE_CONTAINER,
                    window_id: Some(0),
                    source_flags: Some(1),
                    inventory_slot: 1,
                    old_item: ItemStack::empty(),
                    new_item: stack(1, 32),
                },
            ],
            data: TransactionData::Normal,
        };

        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();

        let mut reader = Reader::new(&bytes);
        let decoded = InventoryTransactionPacket::decode_payload(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn roundtrips_use_item_and_release_item() {
        let use_item = InventoryTransactionPacket {
            request_id: 3,
            request_changed_slots: None,
            actions: Vec::new(),
            data: TransactionData::UseItem(UseItemData {
                action_type: ACTION_CLICK_BLOCK,
                trigger_type: 2,
                block_position: (10, 64, -5),
                face: 1,
                hotbar_slot: 4,
                hand: 0,
                item_in_hand: stack(1, 1),
                player_position: (10.5, 64.62, -4.5),
                click_position: (10.5, 65.0, -4.5),
                block_runtime_id: 1,
                client_interact_prediction: 0,
                client_cooldown_state: 0,
            }),
        };
        let mut writer = Writer::new();
        use_item.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            InventoryTransactionPacket::decode_payload(&mut reader).expect("decode"),
            use_item
        );
        assert_eq!(reader.remaining(), 0);

        let release = InventoryTransactionPacket {
            request_id: 4,
            request_changed_slots: None,
            actions: Vec::new(),
            data: TransactionData::ReleaseItem(ReleaseItemData {
                action_type: ACTION_CONSUME,
                hotbar_slot: 2,
                item_in_hand: stack(285, 1),
                head_position: (0.0, 70.0, 0.0),
                hand: 0,
            }),
        };
        let mut writer = Writer::new();
        release.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            InventoryTransactionPacket::decode_payload(&mut reader).expect("decode"),
            release
        );
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn rejects_mismatch_transactions_with_actions() {
        let mut writer = Writer::new();
        writer.write_var_i32(1);
        writer.write_bool(false);
        writer.write_var_u32(TYPE_MISMATCH);
        writer.write_var_u32(1);
        NetworkInventoryAction {
            source_type: SOURCE_CONTAINER,
            window_id: Some(0),
            source_flags: None,
            inventory_slot: 0,
            old_item: ItemStack::empty(),
            new_item: ItemStack::empty(),
        }
        .encode(&mut writer);
        let bytes = writer.into_bytes();

        let mut reader = Reader::new(&bytes);
        assert!(InventoryTransactionPacket::decode_payload(&mut reader).is_err());
    }
}
