use crate::binary::{CodecError, Reader, Writer};

/// Network item stack for protocol 2225.
///
/// Layout mirrors Axolotl's `CommonTypes::putItemStackWrapper`: i16 LE numeric
/// item id (0 = empty), u16 LE count, varuint metadata, optional varint stack
/// network id, varuint block runtime id and the raw NBT payload (varuint
/// length).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemStack {
    pub network_id: i16,
    pub count: u16,
    pub meta: u32,
    pub stack_id: i32,
    pub block_runtime_id: u32,
    pub nbt: Vec<u8>,
}

impl ItemStack {
    pub const fn empty() -> Self {
        Self {
            network_id: 0,
            count: 0,
            meta: 0,
            stack_id: 0,
            block_runtime_id: 0,
            nbt: Vec::new(),
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.network_id == 0
    }

    /// Plain stack without metadata or extra data.
    pub fn new(network_id: i16, count: u16) -> Self {
        Self {
            network_id,
            count,
            ..Self::empty()
        }
    }

    pub fn encode(&self, writer: &mut Writer) {
        writer.write_i16_le(self.network_id);
        writer.write_u16_le(self.count);
        writer.write_var_u32(self.meta);

        writer.write_bool(self.stack_id != 0);
        if self.stack_id != 0 {
            writer.write_var_i32(self.stack_id);
        }

        writer.write_var_u32(self.block_runtime_id);
        writer.write_var_u32(self.nbt.len() as u32);
        writer.write_bytes(&self.nbt);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let network_id = reader.read_i16_le()?;
        let count = reader.read_u16_le()?;
        let meta = reader.read_var_u32()?;

        let stack_id = if reader.read_bool()? {
            reader.read_var_i32()?
        } else {
            0
        };

        let block_runtime_id = reader.read_var_u32()?;
        let nbt_len = reader.read_var_u32()? as usize;
        let nbt = reader.read_bytes(nbt_len)?.to_vec();

        Ok(Self {
            network_id,
            count,
            meta,
            stack_id,
            block_runtime_id,
            nbt,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stack_is_eight_bytes() {
        // BDS encodes an empty stack as 8 zero bytes; the inventory captures
        // depend on it.
        let mut writer = Writer::new();
        ItemStack::empty().encode(&mut writer);
        let bytes = writer.into_bytes();
        assert_eq!(bytes.len(), 8);
        assert!(bytes.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn roundtrip_with_metadata_and_nbt() {
        let stack = ItemStack {
            network_id: 1,
            count: 64,
            meta: 3,
            stack_id: 7,
            block_runtime_id: 42,
            nbt: vec![1, 2, 3],
        };
        let mut writer = Writer::new();
        stack.encode(&mut writer);
        let bytes = writer.into_bytes();

        let mut reader = Reader::new(&bytes);
        let decoded = ItemStack::decode(&mut reader).expect("decode");
        assert_eq!(decoded, stack);
        assert_eq!(reader.remaining(), 0);
    }
}
