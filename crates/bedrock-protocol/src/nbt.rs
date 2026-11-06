//! Network NBT, matching `NetworkNbtSerializer` from BedrockProtocol.
//!
//! The network flavour of NBT differs from the on-disk one: `TAG_Short` is
//! little-endian, `TAG_Int`/`TAG_Long` are zigzag varints, floats and doubles
//! are little-endian, names and strings carry a varuint length, and a tag type
//! is a single unsigned byte. Item stack extra data (the `nbt` field of
//! [`crate::ItemStack`]) is serialized with it.

use crate::binary::Writer;

pub const TAG_END: u8 = 0;
pub const TAG_BYTE: u8 = 1;
pub const TAG_SHORT: u8 = 2;
pub const TAG_INT: u8 = 3;
pub const TAG_LONG: u8 = 4;
pub const TAG_FLOAT: u8 = 5;
pub const TAG_DOUBLE: u8 = 6;
pub const TAG_STRING: u8 = 8;
pub const TAG_LIST: u8 = 9;
pub const TAG_COMPOUND: u8 = 10;

/// One NBT tag.
///
/// Only the tags a JSON NBT document can express are modelled: `TAG_Byte`,
/// `TAG_Short`, `TAG_Int`, `TAG_Long`, `TAG_Float`, `TAG_Double`,
/// `TAG_String`, `TAG_List` and `TAG_Compound`.
#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    List(Vec<Tag>),
    /// Entries keep their order, like `CompoundTag`.
    Compound(Vec<(String, Tag)>),
}

impl Tag {
    pub const fn type_id(&self) -> u8 {
        match self {
            Self::Byte(_) => TAG_BYTE,
            Self::Short(_) => TAG_SHORT,
            Self::Int(_) => TAG_INT,
            Self::Long(_) => TAG_LONG,
            Self::Float(_) => TAG_FLOAT,
            Self::Double(_) => TAG_DOUBLE,
            Self::String(_) => TAG_STRING,
            Self::List(_) => TAG_LIST,
            Self::Compound(_) => TAG_COMPOUND,
        }
    }
}

/// Serializes a root tag the way `TreeRoot` does: type, empty name, payload.
pub fn write_network_nbt(root: &Tag) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_byte(root.type_id());
    writer.write_string(""); // the root tag has no name
    write_payload(&mut writer, root);
    writer.into_bytes().to_vec()
}

fn write_payload(writer: &mut Writer, tag: &Tag) {
    match tag {
        Tag::Byte(value) => writer.write_i8(*value),
        Tag::Short(value) => writer.write_i16_le(*value),
        Tag::Int(value) => writer.write_var_i32(*value),
        Tag::Long(value) => writer.write_var_i64(*value),
        Tag::Float(value) => writer.write_f32_le(*value),
        Tag::Double(value) => writer.write_i64_le(value.to_bits() as i64),
        Tag::String(value) => writer.write_string(value),
        Tag::List(items) => {
            // An empty list has no element type, exactly like `ListTag`.
            writer.write_byte(items.first().map(Tag::type_id).unwrap_or(TAG_END));
            writer.write_var_i32(items.len() as i32);
            for item in items {
                write_payload(writer, item);
            }
        }
        Tag::Compound(entries) => {
            for (name, value) in entries {
                writer.write_byte(value.type_id());
                writer.write_string(name);
                write_payload(writer, value);
            }
            writer.write_byte(TAG_END);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compound_uses_varint_ints_and_little_endian_shorts() {
        let tag = Tag::Compound(vec![
            ("Name".to_string(), Tag::String("Sword".to_string())),
            ("Damage".to_string(), Tag::Short(3)),
        ]);

        assert_eq!(
            write_network_nbt(&tag),
            vec![
                10, 0, // TAG_Compound, empty root name
                8, 4, b'N', b'a', b'm', b'e', 5, b'S', b'w', b'o', b'r', b'd', // Name: "Sword"
                2, 6, b'D', b'a', b'm', b'a', b'g', b'e', 3, 0, // Damage: short 3 (LE)
                0, // end of compound
            ]
        );
    }

    #[test]
    fn lists_carry_their_element_type_and_a_varint_length() {
        let tag = Tag::List(vec![Tag::Int(1), Tag::Int(-1)]);
        // Length 2 zigzags to 4; ints zigzag to 2 and 1.
        assert_eq!(write_network_nbt(&tag), vec![9, 0, 3, 4, 2, 1]);
    }

    #[test]
    fn empty_lists_are_tag_end_typed() {
        assert_eq!(write_network_nbt(&Tag::List(Vec::new())), vec![9, 0, 0, 0]);
    }

    #[test]
    fn doubles_are_little_endian() {
        assert_eq!(
            write_network_nbt(&Tag::Double(1.0)),
            vec![6, 0, 0, 0, 0, 0, 0, 0, 240, 63]
        );
    }
}
