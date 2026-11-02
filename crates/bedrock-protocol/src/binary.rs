use bytes::{BufMut, Bytes, BytesMut};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodecError {
    #[error("Unexpected end of buffer: needed {needed} bytes, got {available}")]
    UnexpectedEof { needed: usize, available: usize },
    #[error("Invalid VarInt format")]
    InvalidVarInt,
    #[error("Invalid UTF-8 string: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
    #[error("Packet error: {0}")]
    Custom(String),
}

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn read_byte(&mut self) -> Result<u8, CodecError> {
        if self.pos >= self.data.len() {
            return Err(CodecError::UnexpectedEof { needed: 1, available: 0 });
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    pub fn read_bool(&mut self) -> Result<bool, CodecError> {
        Ok(self.read_byte()? != 0)
    }

    pub fn read_i16_le(&mut self) -> Result<i16, CodecError> {
        let b = self.read_bytes(2)?;
        Ok(i16::from_le_bytes([b[0], b[1]]))
    }

    pub fn read_u16_le(&mut self) -> Result<u16, CodecError> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn read_i32_le(&mut self) -> Result<i32, CodecError> {
        let b = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_f32_le(&mut self) -> Result<f32, CodecError> {
        let b = self.read_bytes(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_var_u32(&mut self) -> Result<u32, CodecError> {
        let mut value: u32 = 0;
        let mut shift = 0;
        loop {
            let byte = self.read_byte()?;
            value |= ((byte & 0x7F) as u32) << shift;
            if (byte & 0x80) == 0 {
                return Ok(value);
            }
            shift += 7;
            if shift >= 35 {
                return Err(CodecError::InvalidVarInt);
            }
        }
    }

    pub fn read_var_i32(&mut self) -> Result<i32, CodecError> {
        let raw = self.read_var_u32()?;
        Ok(((raw >> 1) as i32) ^ (-((raw & 1) as i32)))
    }

    pub fn read_string(&mut self) -> Result<String, CodecError> {
        let len = self.read_var_u32()? as usize;
        let bytes = self.read_bytes(len)?;
        Ok(String::from_utf8(bytes.to_vec())?)
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], CodecError> {
        if self.remaining() < len {
            return Err(CodecError::UnexpectedEof {
                needed: len,
                available: self.remaining(),
            });
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    pub fn read_remaining(&mut self) -> &'a [u8] {
        let slice = &self.data[self.pos..];
        self.pos = self.data.len();
        slice
    }
}

pub struct Writer {
    buf: BytesMut,
}

impl Default for Writer {
    fn default() -> Self {
        Self::with_capacity(512)
    }
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: BytesMut::with_capacity(capacity),
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.buf.put_u8(byte);
    }

    pub fn write_bool(&mut self, val: bool) {
        self.buf.put_u8(if val { 1 } else { 0 });
    }

    pub fn write_i16_le(&mut self, val: i16) {
        self.buf.put_i16_le(val);
    }

    pub fn write_u16_le(&mut self, val: u16) {
        self.buf.put_u16_le(val);
    }

    pub fn write_i32_le(&mut self, val: i32) {
        self.buf.put_i32_le(val);
    }

    pub fn write_f32_le(&mut self, val: f32) {
        self.buf.put_f32_le(val);
    }

    pub fn write_var_u32(&mut self, mut value: u32) {
        while (value & !0x7F) != 0 {
            self.buf.put_u8(((value & 0x7F) | 0x80) as u8);
            value >>= 7;
        }
        self.buf.put_u8(value as u8);
    }

    pub fn write_var_i32(&mut self, value: i32) {
        let raw = ((value << 1) ^ (value >> 31)) as u32;
        self.write_var_u32(raw);
    }

    pub fn write_string(&mut self, s: &str) {
        self.write_var_u32(s.len() as u32);
        self.buf.put_slice(s.as_bytes());
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.put_slice(bytes);
    }

    pub fn into_bytes(self) -> Bytes {
        self.buf.freeze()
    }
}
