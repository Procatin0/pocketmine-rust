//! Big-endian buffer primitives for the Java Edition protocol.
//!
//! Java uses VarInt/VarLong little-endian-in-seven-bit-groups encoding for
//! lengths and ids, and big-endian for every fixed-width integer and float.

use thiserror::Error;

/// Maximum characters a Java protocol string may hold (vanilla limit).
pub const MAX_STRING_CHARS: usize = 32_767;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CodecError {
    #[error("unexpected end of buffer: needed {needed} bytes, {available} available")]
    UnexpectedEof { needed: usize, available: usize },
    #[error("invalid VarInt/VarLong")]
    InvalidVarInt,
    #[error("string too long: {0} characters")]
    StringTooLong(usize),
    #[error("invalid UTF-8 string: {0}")]
    InvalidUtf8(String),
    #[error("protocol error: {0}")]
    Protocol(String),
}

`pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Bytes that have not been read yet.
    pub fn rest(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    pub fn read_bytes(&mut self, count: usize) -> Result<&'a [u8], CodecError> {
        if self.remaining() < count {
            return Err(CodecError::UnexpectedEof {
                needed: count,
                available: self.remaining(),
            });
        }
        let slice = &self.data[self.pos..self.pos + count];
        self.pos += count;
        Ok(slice)
    }

    pub fn read_u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.read_bytes(1)?[0])
    }

    pub fn read_i8(&mut self) -> Result<i8, CodecError> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_bool(&mut self) -> Result<bool, CodecError> {
        Ok(self.read_u8()? != 0)
    }

    pub fn read_i16(&mut self) -> Result<i16, CodecError> {
        let bytes = self.read_bytes(2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_u16(&mut self) -> Result<u16, CodecError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_i32(&mut self) -> Result<i32, CodecError> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_i64(&mut self) -> Result<i64, CodecError> {
        let bytes = self.read_bytes(8)?;
        Ok(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    pub fn read_f32(&mut self) -> Result<f32, CodecError> {
        Ok(f32::from_bits(self.read_i32()? as u32))
    }

    pub fn read_f64(&mut self) -> Result<f64, CodecError> {
        Ok(f64::from_bits(self.read_i64()? as u64))
    }

    /// VarInt, at most five bytes.
    pub fn read_var_i32(&mut self) -> Result<i32, CodecError> {
        let mut value: u32 = 0;
        for shift in [0u32, 7, 14, 21, 28] {
            let byte = self.read_u8()?;
            value |= ((byte & 0x7F) as u32) << shift;
            if byte & 0x80 == 0 {
                return Ok(value as i32);
            }
        }
        Err(CodecError::InvalidVarInt)
    }

    /// VarLong, at most ten bytes.
    pub fn read_var_i64(&mut self) -> Result<i64, CodecError> {
        let mut value: u64 = 0;
        for shift in [0u32, 7, 14, 21, 28, 35, 42, 49, 56, 63] {
            let byte = self.read_u8()?;
            value |= ((byte & 0x7F) as u64) << shift;
            if byte & 0x80 == 0 {
                return Ok(value as i64);
            }
        }
        Err(CodecError::InvalidVarInt)
    }

    /// Length-prefixed UTF-8 string.
    pub fn read_string(&mut self) -> Result<String, CodecError> {
        let length = self.read_var_i32()?;
        if length < 0 {
            return Err(CodecError::Protocol(format!(
                "negative string length: {length}"
            )));
        }
        let length = length as usize;
        if length > MAX_STRING_CHARS * 4 {
            return Err(CodecError::StringTooLong(length));
        }
        let text = std::str::from_utf8(self.read_bytes(length)?)
            .map_err(|error| CodecError::InvalidUtf8(error.to_string()))?
            .to_string();
        if text.chars().count() > MAX_STRING_CHARS {
            return Err(CodecError::StringTooLong(text.chars().count()));
        }
        Ok(text)
    }

    /// Packed block position: x (26 bits) | y (12 bits) | z (26 bits).
    pub fn read_position(&mut self) -> Result<(i32, i32, i32), CodecError> {
        let raw = self.read_i64()?;
        let x = (raw >> 38) as i32;
        let y = ((raw >> 26) & 0xFFF) as i32;
        let z = ((raw << 38) >> 38) as i32;
        Ok((x, y, z))
    }
}

#[derive(Debug, Default, Clone)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub fn write_u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    pub fn write_bool(&mut self, value: bool) {
        self.buf.push(u8::from(value));
    }

    pub fn write_i8(&mut self, value: i8) {
        self.buf.push(value as u8);
    }

    pub fn write_i16(&mut self, value: i16) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_u16(&mut self, value: u16) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_i32(&mut self, value: i32) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_i64(&mut self, value: i64) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_f32(&mut self, value: f32) {
        self.write_i32(value.to_bits() as i32);
    }

    pub fn write_f64(&mut self, value: f64) {
        self.write_i64(value.to_bits() as i64);
    }

    pub fn write_var_i32(&mut self, value: i32) {
        let mut remaining = value as u32;
        loop {
            if remaining & !0x7F == 0 {
                self.buf.push(remaining as u8);
                return;
            }
            self.buf.push(((remaining & 0x7F) | 0x80) as u8);
            remaining >>= 7;
        }
    }

    pub fn write_var_i64(&mut self, value: i64) {
        let mut remaining = value as u64;
        loop {
            if remaining & !0x7F == 0 {
                self.buf.push(remaining as u8);
                return;
            }
            self.buf.push(((remaining & 0x7F) | 0x80) as u8);
            remaining >>= 7;
        }
    }

    pub fn write_string(&mut self, value: &str) {
        self.write_var_i32(value.len() as i32);
        self.write_bytes(value.as_bytes());
    }

    pub fn write_position(&mut self, x: i32, y: i32, z: i32) {
        let packed = (((x as i64) & 0x3FF_FFFF) << 38)
            | (((y as i64) & 0xFFF) << 26)
            | ((z as i64) & 0x3FF_FFFF);
        self.write_i64(packed);
    }
}
