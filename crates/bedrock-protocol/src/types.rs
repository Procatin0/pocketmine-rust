use crate::binary::{CodecError, Reader, Writer};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3f {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn write(&self, writer: &mut Writer) {
        writer.write_f32_le(self.x);
        writer.write_f32_le(self.y);
        writer.write_f32_le(self.z);
    }

    pub fn read(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            x: reader.read_f32_le()?,
            y: reader.read_f32_le()?,
            z: reader.read_f32_le()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn write(&self, writer: &mut Writer) {
        writer.write_var_i32(self.x);
        writer.write_var_u32(self.y as u32);
        writer.write_var_i32(self.z);
    }

    pub fn read(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            x: reader.read_var_i32()?,
            y: reader.read_var_u32()? as i32,
            z: reader.read_var_i32()?,
        })
    }
}
