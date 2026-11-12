//! Packet ID 0x91 (145): CreativeContentPacket.
//! Creative-mode item catalog: category groups + the full item list.
//!
//! The structs below mirror the legacy `CreativeGroup { name, icon }` +
//! bare `ItemStack` shape used by the disabled `generated` path; they do
//! NOT reproduce the BDS 1.26 wire and must not be enabled for real
//! clients (`PM_CORE_CREATIVE_MODE=generated`). Reference wire, verified
//! byte-for-byte against `data/bds-capture/creative_content/1.bin` and
//! `1.json` (the `target/BedrockProtocol` PHP snapshot is stale here):
//! `var_u32` group count; per group: LE-u32 category id (1 = construction,
//! 2 = nature, 3 = equipment, 4 = items), name string, and
//! `ItemStackWithoutStackId` icon (`var_i32` id, `u16` count, `var_u32`
//! meta, `var_i32` block runtime hash, string `ItemStackExtraData` — 10
//! bytes minimum when there is no NBT; a null icon `id == 0` is a single
//! `var_i32` 0 and nothing more, 36 of the 123 groups) — then `var_u32`
//! item count; per item: `var_u32` entry id (sequential 1..N),
//! `ItemStackWithoutStackId`, and `var_u32` group index (0-based). Creative
//! stacks therefore differ from the `ItemStackWrapper` encoding used by
//! `ItemStack` (id `i16` LE, stack-id bool, unsigned runtime, NBT only).
//! The capture in `StaticPackets::CREATIVE_CONTENT` remains the only wire
//! sent by default; see its doc for why generation from `data/creative/`
//! cannot replace it (FNV block runtime hashes, negative data-driven item
//! ids, NBT extras).
//! The file `data/bds-capture/creative_content/1.bin` is the PHP-spec form
//! (`CreativeGroupEntry` byte u8 + `ItemStackWithoutStackId` complete),
//! generated from `1.json` in `patch_creative.py`. The original builder
//! emitted LE-u32 category + short-form null icon and was discarded.
//!

use crate::binary::{CodecError, Reader, Writer};
use crate::packets::item_stack::ItemStack;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreativeGroup {
    pub name: String,
    pub icon: ItemStack,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CreativeContentPacket {
    pub groups: Vec<CreativeGroup>,
    pub items: Vec<ItemStack>,
}

impl CreativeContentPacket {
    pub const PACKET_ID: u32 = 0x91;

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u32(self.groups.len() as u32);
        for group in &self.groups {
            writer.write_string(&group.name);
            group.icon.encode(writer);
        }
        writer.write_var_u32(self.items.len() as u32);
        for item in &self.items {
            item.encode(writer);
        }
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let group_count = reader.read_var_u32()?;
        let mut groups = Vec::with_capacity(group_count as usize);
        for _ in 0..group_count {
            let name = reader.read_string()?;
            let icon = ItemStack::decode(reader)?;
            groups.push(CreativeGroup { name, icon });
        }
        let item_count = reader.read_var_u32()?;
        let mut items = Vec::with_capacity(item_count as usize);
        for _ in 0..item_count {
            items.push(ItemStack::decode(reader)?);
        }
        Ok(Self { groups, items })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips() {
        let packet = CreativeContentPacket {
            groups: vec![CreativeGroup { name: "construction".into(), icon: ItemStack::new(1, 1) }],
            items: vec![ItemStack::new(1, 64), ItemStack::new(3, 64)],
        };
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(CreativeContentPacket::decode_payload(&mut reader).unwrap(), packet);
        assert_eq!(reader.remaining(), 0);
    }
}
