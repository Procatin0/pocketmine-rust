//! ChunkManager abstractions for world generation populators.

use crate::block_id;
use crate::chunk::Chunk;

pub trait ChunkManager {
    fn get_block_at(&self, x: i32, y: i32, z: i32) -> u32;
    fn set_block_at(&mut self, x: i32, y: i32, z: i32, block: u32);
    fn get_highest_block_at(&self, x: i32, z: i32) -> Option<i32>;
}

/// 3x3 Chunk Grid Manager for populator operations.
///
/// Holds the center chunk `(center_cx, center_cz)` and all 8 adjacent chunks:
/// `[center_cx - 1 ..= center_cx + 1][center_cz - 1 ..= center_cz + 1]`.
pub struct ChunkManager3x3 {
    min_cx: i32,
    min_cz: i32,
    chunks: [Option<Chunk>; 9],
}

impl ChunkManager3x3 {
    pub fn new(center_cx: i32, center_cz: i32) -> Self {
        Self {
            min_cx: center_cx - 1,
            min_cz: center_cz - 1,
            chunks: Default::default(),
        }
    }

    #[inline(always)]
    fn chunk_index(&self, cx: i32, cz: i32) -> Option<usize> {
        let rx = cx - self.min_cx;
        let rz = cz - self.min_cz;
        if rx >= 0 && rx < 3 && rz >= 0 && rz < 3 {
            Some((rx * 3 + rz) as usize)
        } else {
            None
        }
    }

    pub fn set_chunk(&mut self, cx: i32, cz: i32, chunk: Chunk) {
        if let Some(idx) = self.chunk_index(cx, cz) {
            self.chunks[idx] = Some(chunk);
        }
    }

    pub fn get_chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        let idx = self.chunk_index(cx, cz)?;
        self.chunks[idx].as_ref()
    }

    pub fn take_chunk(&mut self, cx: i32, cz: i32) -> Option<Chunk> {
        let idx = self.chunk_index(cx, cz)?;
        self.chunks[idx].take()
    }
}

impl ChunkManager for ChunkManager3x3 {
    fn get_block_at(&self, x: i32, y: i32, z: i32) -> u32 {
        if y < 0 || y >= 320 {
            return block_id::AIR;
        }
        let cx = x >> 4;
        let cz = z >> 4;
        if let Some(idx) = self.chunk_index(cx, cz) {
            if let Some(chunk) = &self.chunks[idx] {
                let lx = (x & 15) as usize;
                let lz = (z & 15) as usize;
                return chunk.get_block(lx, y, lz);
            }
        }
        block_id::AIR
    }

    fn set_block_at(&mut self, x: i32, y: i32, z: i32, block: u32) {
        if y < 0 || y >= 320 {
            return;
        }
        let cx = x >> 4;
        let cz = z >> 4;
        if let Some(idx) = self.chunk_index(cx, cz) {
            if let Some(chunk) = &mut self.chunks[idx] {
                let lx = (x & 15) as usize;
                let lz = (z & 15) as usize;
                chunk.set_block(lx, y, lz, block);
            }
        }
    }

    fn get_highest_block_at(&self, x: i32, z: i32) -> Option<i32> {
        let cx = x >> 4;
        let cz = z >> 4;
        let idx = self.chunk_index(cx, cz)?;
        let chunk = self.chunks[idx].as_ref()?;
        let lx = (x & 15) as usize;
        let lz = (z & 15) as usize;
        chunk.get_highest_block_at(lx, lz)
    }
}

impl ChunkManager for Chunk {
    fn get_block_at(&self, x: i32, y: i32, z: i32) -> u32 {
        if (0..16).contains(&x) && (0..16).contains(&z) && (0..320).contains(&y) {
            self.get_block(x as usize, y, z as usize)
        } else {
            block_id::AIR
        }
    }

    fn set_block_at(&mut self, x: i32, y: i32, z: i32, block: u32) {
        if (0..16).contains(&x) && (0..16).contains(&z) && (0..320).contains(&y) {
            self.set_block(x as usize, y, z as usize, block);
        }
    }

    fn get_highest_block_at(&self, x: i32, z: i32) -> Option<i32> {
        if (0..16).contains(&x) && (0..16).contains(&z) {
            self.get_highest_block_at(x as usize, z as usize)
        } else {
            None
        }
    }
}
