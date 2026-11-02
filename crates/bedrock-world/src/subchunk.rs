use crate::paletted_block_array::PalettedBlockArray;

pub struct SubChunk {
    pub layers: Vec<PalettedBlockArray>,
}

impl SubChunk {
    pub fn new(fill_entry: u32) -> Self {
        Self {
            layers: vec![PalettedBlockArray::new(fill_entry)],
        }
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize, layer: usize) -> u32 {
        self.layers
            .get(layer)
            .map(|l| l.get(x, y, z))
            .unwrap_or(0)
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, layer: usize, block_id: u32) {
        if layer >= self.layers.len() {
            self.layers.resize_with(layer + 1, || PalettedBlockArray::new(0));
        }
        self.layers[layer].set(x, y, z, block_id);
    }
}
