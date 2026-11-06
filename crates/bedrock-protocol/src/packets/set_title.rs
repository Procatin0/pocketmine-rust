use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x58 (88): SetTitlePacket
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SetTitlePacket {
    pub title_type: i32,
    pub text: String,
    pub fade_in_time: i32,
    pub stay_time: i32,
    pub fade_out_time: i32,
    pub xuid: String,
    pub platform_online_id: String,
    pub filtered_title_text: String,
}

impl SetTitlePacket {
    pub const TYPE_CLEAR_TITLE: i32 = 0;
    pub const TYPE_RESET_TITLE: i32 = 1;
    pub const TYPE_SET_TITLE: i32 = 2;
    pub const TYPE_SET_SUBTITLE: i32 = 3;
    pub const TYPE_SET_ACTIONBAR_MESSAGE: i32 = 4;
    pub const TYPE_SET_ANIMATION_TIMES: i32 = 5;
    pub const TYPE_SET_TITLE_JSON: i32 = 6;
    pub const TYPE_SET_SUBTITLE_JSON: i32 = 7;
    pub const TYPE_SET_ACTIONBAR_MESSAGE_JSON: i32 = 8;

    fn new(title_type: i32) -> Self {
        Self {
            title_type,
            ..Self::default()
        }
    }

    fn with_text(title_type: i32, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::new(title_type)
        }
    }

    pub fn clear_title() -> Self {
        Self::new(Self::TYPE_CLEAR_TITLE)
    }

    pub fn reset_title_options() -> Self {
        Self::new(Self::TYPE_RESET_TITLE)
    }

    pub fn title(text: impl Into<String>) -> Self {
        Self::with_text(Self::TYPE_SET_TITLE, text)
    }

    pub fn subtitle(text: impl Into<String>) -> Self {
        Self::with_text(Self::TYPE_SET_SUBTITLE, text)
    }

    pub fn action_bar_message(text: impl Into<String>) -> Self {
        Self::with_text(Self::TYPE_SET_ACTIONBAR_MESSAGE, text)
    }

    pub fn set_animation_times(fade_in: i32, stay: i32, fade_out: i32) -> Self {
        Self {
            title_type: Self::TYPE_SET_ANIMATION_TIMES,
            fade_in_time: fade_in,
            stay_time: stay,
            fade_out_time: fade_out,
            ..Self::default()
        }
    }
}

impl Packet for SetTitlePacket {
    fn packet_id(&self) -> u32 {
        0x58
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i32(self.title_type);
        writer.write_string(&self.text);
        writer.write_var_i32(self.fade_in_time);
        writer.write_var_i32(self.stay_time);
        writer.write_var_i32(self.fade_out_time);
        writer.write_string(&self.xuid);
        writer.write_string(&self.platform_online_id);
        writer.write_string(&self.filtered_title_text);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            title_type: reader.read_var_i32()?,
            text: reader.read_string()?,
            fade_in_time: reader.read_var_i32()?,
            stay_time: reader.read_var_i32()?,
            fade_out_time: reader.read_var_i32()?,
            xuid: reader.read_string()?,
            platform_online_id: reader.read_string()?,
            filtered_title_text: reader.read_string()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_the_type_and_times_as_signed_varints() {
        let mut writer = Writer::new();
        SetTitlePacket::title("hola")
            .encode(&mut writer)
            .expect("encode");
        let bytes = writer.into_bytes();
        // TYPE_SET_TITLE (2) zigzags to 4; an unsigned varint would write 2.
        assert_eq!(bytes[0], 0x04);
        assert_eq!(bytes[1], 4, "string length prefix");
        assert_eq!(&bytes[2..6], b"hola");
    }

    #[test]
    fn roundtrips_every_constructor() {
        let packets = [
            SetTitlePacket::clear_title(),
            SetTitlePacket::reset_title_options(),
            SetTitlePacket::title("Título"),
            SetTitlePacket::subtitle("Subtítulo"),
            SetTitlePacket::action_bar_message("Barra"),
            SetTitlePacket::set_animation_times(10, 20, 30),
        ];

        for packet in packets {
            let mut writer = Writer::new();
            packet.encode(&mut writer).expect("encode");
            let bytes = writer.into_bytes();
            let mut reader = Reader::new(&bytes);
            let decoded = SetTitlePacket::decode(&mut reader).expect("decode");
            assert_eq!(decoded, packet);
            assert_eq!(reader.remaining(), 0);
        }
    }
}
