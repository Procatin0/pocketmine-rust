//! Packet IDs 0x64 (100) and 0x65 (101): `ModalFormRequestPacket` /
//! `ModalFormResponsePacket`.
//!
//! PocketMine `Player::sendForm()` serializes the form with
//! `json_encode($form)` and sends it in `ModalFormRequestPacket`; the client
//! answers with `ModalFormResponsePacket`, whose `data` field is the JSON the
//! form expects (`null` when the player closed it without submitting).
//!
//! Layout (protocol 2225):
//! - request:  `form_id` var_u32, `data` string
//! - response: `form_id` var_u32, `data` string (`"null"` = closed),
//!   `cancel_reason` u8 (0 = closed by user, 1 = user busy, ...)

use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Bedrock's JSON encoding of "no response" (`PM\Form` receives `null`).
pub const FORM_RESPONSE_NONE: &str = "null";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModalFormRequestPacket {
    /// `Player::sendForm()`'s `$id`; echoed back by the response packet.
    pub form_id: u32,
    /// `json_encode($form)`.
    pub data: String,
}

impl ModalFormRequestPacket {
    pub const PACKET_ID: u32 = 0x64;

    pub fn new(form_id: u32, data: impl Into<String>) -> Self {
        Self {
            form_id,
            data: data.into(),
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u32(self.form_id);
        writer.write_string(&self.data);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            form_id: reader.read_var_u32()?,
            data: reader.read_string()?,
        })
    }
}

impl Packet for ModalFormRequestPacket {
    fn packet_id(&self) -> u32 {
        Self::PACKET_ID
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_u32(Self::PACKET_ID);
        self.encode_payload(writer);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Self::decode_payload(reader)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModalFormResponsePacket {
    pub form_id: u32,
    /// Response JSON; `None` is Bedrock's `"null"` (form closed).
    pub data: Option<String>,
    /// `ModalFormCancelReason`; 0 when the player submitted.
    pub cancel_reason: u8,
}

impl ModalFormResponsePacket {
    pub const PACKET_ID: u32 = 0x65;

    pub fn new(form_id: u32, data: Option<String>, cancel_reason: u8) -> Self {
        Self {
            form_id,
            data,
            cancel_reason,
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u32(self.form_id);
        writer.write_string(self.data.as_deref().unwrap_or(FORM_RESPONSE_NONE));
        writer.write_byte(self.cancel_reason);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let form_id = reader.read_var_u32()?;
        let raw = reader.read_string()?;
        // The cancel reason is absent on older clients; treat a missing byte
        // as "submitted" the way BDS does.
        let cancel_reason = reader.read_byte().unwrap_or(0);
        Ok(Self {
            form_id,
            data: if raw == FORM_RESPONSE_NONE {
                None
            } else {
                Some(raw)
            },
            cancel_reason,
        })
    }
}

impl Packet for ModalFormResponsePacket {
    fn packet_id(&self) -> u32 {
        Self::PACKET_ID
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_u32(Self::PACKET_ID);
        self.encode_payload(writer);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Self::decode_payload(reader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrips() {
        let packet = ModalFormRequestPacket::new(7, r#"{"type":"form","title":"Menú"}"#);
        let mut writer = Writer::new();
        packet.encode(&mut writer).unwrap();
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(reader.read_var_u32().unwrap(), ModalFormRequestPacket::PACKET_ID);
        assert_eq!(
            ModalFormRequestPacket::decode_payload(&mut reader).unwrap(),
            packet
        );
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn response_roundtrips_and_decodes_null() {
        let packet = ModalFormResponsePacket::new(3, Some("true".to_string()), 0);
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            ModalFormResponsePacket::decode_payload(&mut reader).unwrap(),
            packet
        );
        assert_eq!(reader.remaining(), 0);

        let mut writer = Writer::new();
        ModalFormResponsePacket::new(3, None, 0).encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let closed = ModalFormResponsePacket::decode_payload(&mut reader).unwrap();
        assert_eq!(closed.data, None);
    }
}
