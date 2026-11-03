use crate::network::{self, SubChunkNetworkError};
use crate::paletted_block_array::PalettedBlockArray;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubChunk {
    pub default_block: u32,
    pub layers: Vec<PalettedBlockArray>,
}

impl SubChunk {
    pub const CURRENT_VERSION: u8 = network::CURRENT_VERSION;

    pub fn new(fill_entry: u32) -> Self {
        Self {
            default_block: fill_entry,
            layers: vec![PalettedBlockArray::new(fill_entry)],
        }
    }

    pub fn empty() -> Self {
        Self {
            default_block: crate::block_id::AIR,
            layers: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
            || (self.layers.len() == 1
                && self.layers[0].bits_per_block() == 0
                && (self.layers[0].palette().first() == Some(&crate::block_id::AIR)
                    || self.layers[0].palette().first() == Some(&0)))
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize, layer: usize) -> u32 {
        self.layers
            .get(layer)
            .map(|l| l.get(x, y, z))
            .unwrap_or(self.default_block)
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, layer: usize, block_id: u32) {
        if layer >= self.layers.len() {
            self.layers.resize_with(layer + 1, || PalettedBlockArray::new(self.default_block));
        }
        self.layers[layer].set(x, y, z, block_id);
    }

    /// Serializes this SubChunk into network format as a byte vector.
    pub fn serialize_network(&self) -> Vec<u8> {
        let mut writer = bedrock_protocol::Writer::new();
        network::serialize_subchunk(self, &mut writer);
        writer.into_bytes().to_vec()
    }

    /// Serializes this SubChunk into the provided `bedrock_protocol::Writer`.
    pub fn serialize_network_to(&self, writer: &mut bedrock_protocol::Writer) {
        network::serialize_subchunk(self, writer);
    }

    /// Deserializes a SubChunk from network format bytes.
    pub fn deserialize_network(data: &[u8]) -> Result<Self, SubChunkNetworkError> {
        let mut reader = bedrock_protocol::Reader::new(data);
        network::deserialize_subchunk(&mut reader)
    }

    /// Deserializes a SubChunk from a `bedrock_protocol::Reader`.
    pub fn deserialize_network_from(reader: &mut bedrock_protocol::Reader) -> Result<Self, SubChunkNetworkError> {
        network::deserialize_subchunk(reader)
    }
}
