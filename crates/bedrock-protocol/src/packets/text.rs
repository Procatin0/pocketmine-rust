use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x09 (9): TextPacket
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPacket {
    pub needs_translation: bool,
    pub text_type: u8,
    pub source_name: String,
    pub message: String,
    pub parameters: Vec<String>,
    pub xbox_user_id: String,
    pub platform_chat_id: String,
}

impl TextPacket {
    pub const TYPE_RAW: u8 = 0;
    pub const TYPE_CHAT: u8 = 1;
    pub const TYPE_TRANSLATION: u8 = 2;
    pub const TYPE_POPUP: u8 = 3;
    pub const TYPE_TIP: u8 = 5;
    pub const TYPE_SYSTEM: u8 = 6;
    pub const TYPE_WHISPER: u8 = 7;

    pub fn raw(message: impl Into<String>) -> Self {
        Self {
            needs_translation: false,
            text_type: Self::TYPE_RAW,
            source_name: String::new(),
            message: message.into(),
            parameters: Vec::new(),
            xbox_user_id: String::new(),
            platform_chat_id: String::new(),
        }
    }

    pub fn chat(source_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            needs_translation: false,
            text_type: Self::TYPE_CHAT,
            source_name: source_name.into(),
            message: message.into(),
            parameters: Vec::new(),
            xbox_user_id: String::new(),
            platform_chat_id: String::new(),
        }
    }

    pub fn tip(message: impl Into<String>) -> Self {
        Self {
            needs_translation: false,
            text_type: Self::TYPE_TIP,
            source_name: String::new(),
            message: message.into(),
            parameters: Vec::new(),
            xbox_user_id: String::new(),
            platform_chat_id: String::new(),
        }
    }
}

impl Packet for TextPacket {
    fn packet_id(&self) -> u32 {
        0x09
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_bool(self.needs_translation);
        let category = match self.text_type {
            Self::TYPE_CHAT | Self::TYPE_WHISPER => 1u8,
            Self::TYPE_TRANSLATION | Self::TYPE_POPUP => 2u8,
            _ => 0u8,
        };
        writer.write_byte(category);
        writer.write_byte(self.text_type);

        match category {
            1 => {
                writer.write_string(&self.source_name);
                writer.write_string(&self.message);
            }
            2 => {
                writer.write_string(&self.message);
                writer.write_var_u32(self.parameters.len() as u32);
                for p in &self.parameters {
                    writer.write_string(p);
                }
            }
            _ => {
                writer.write_string(&self.message);
            }
        }

        writer.write_string(&self.xbox_user_id);
        writer.write_string(&self.platform_chat_id);
        writer.write_bool(false); // filtered_message: None
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let needs_translation = reader.read_bool()?;
        let category = reader.read_byte()?;
        let text_type = reader.read_byte()?;

        let mut source_name = String::new();
        let mut message = String::new();
        let mut parameters = Vec::new();

        match category {
            1 => {
                source_name = reader.read_string()?;
                message = reader.read_string()?;
            }
            2 => {
                message = reader.read_string()?;
                let count = reader.read_var_u32()? as usize;
                for _ in 0..count {
                    parameters.push(reader.read_string()?);
                }
            }
            _ => {
                message = reader.read_string()?;
            }
        }

        let xbox_user_id = reader.read_string()?;
        let platform_chat_id = reader.read_string()?;

        Ok(Self {
            needs_translation,
            text_type,
            source_name,
            message,
            parameters,
            xbox_user_id,
            platform_chat_id,
        })
    }
}
