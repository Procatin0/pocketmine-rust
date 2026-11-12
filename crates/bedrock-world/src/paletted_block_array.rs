/// PalettedBlockArray for Bedrock subchunks (4096 blocks = 16x16x16)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalettedBlockArray {
    bits_per_block: u8,
    words: Vec<u32>,
    palette: Vec<u32>,
}

impl PalettedBlockArray {
    pub const SIZE: usize = 4096;

    pub fn new(fill_entry: u32) -> Self {
        Self {
            bits_per_block: 0,
            words: Vec::new(),
            palette: vec![fill_entry],
        }
    }

    #[inline(always)]
    fn get_index(x: usize, y: usize, z: usize) -> usize {
        debug_assert!(x < 16 && y < 16 && z < 16);
        (x << 8) | (z << 4) | y
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> u32 {
        if self.bits_per_block == 0 {
            return self.palette.first().copied().unwrap_or(0);
        }
        let index = Self::get_index(x, y, z);
        let palette_index = self.read_word_bits(index);
        self.palette
            .get(palette_index as usize)
            .copied()
            .unwrap_or(0)
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, val: u32) {
        let palette_index = if let Some(pos) = self.palette.iter().position(|&p| p == val) {
            pos as u32
        } else {
            self.palette.push(val);
            let new_pos = (self.palette.len() - 1) as u32;
            self.check_resize();
            new_pos
        };

        if self.bits_per_block > 0 {
            let index = Self::get_index(x, y, z);
            self.write_word_bits(index, palette_index);
        }
    }

    pub fn words_count_for_bpb(bpb: u8) -> Option<usize> {
        match bpb {
            0 => Some(0),
            1 => Some(128),
            2 => Some(256),
            3 => Some(410),
            4 => Some(512),
            5 => Some(683),
            6 => Some(820),
            8 => Some(1024),
            16 => Some(2048),
            _ => None,
        }
    }

    fn check_resize(&mut self) {
        let palette_len = self.palette.len();
        let needed_bits = match palette_len {
            0..=1 => 0,
            2 => 1,
            3..=4 => 2,
            5..=8 => 3,
            9..=16 => 4,
            17..=32 => 5,
            33..=64 => 6,
            65..=256 => 8,
            _ => 16,
        };

        if needed_bits > self.bits_per_block {
            self.rebuild_with_bits(needed_bits);
        }
    }

    fn rebuild_with_bits(&mut self, new_bits: u8) {
        let old_bits = self.bits_per_block;
        let old_words = std::mem::take(&mut self.words);
        self.bits_per_block = new_bits;

        let words_count = Self::words_count_for_bpb(new_bits).unwrap_or_else(|| {
            let bpw = 32 / new_bits.max(1) as usize;
            (Self::SIZE + bpw - 1) / bpw
        });
        self.words = vec![0u32; words_count];

        if old_bits > 0 {
            let old_bpw = 32 / old_bits as usize;
            let old_mask = (1 << old_bits) - 1;
            for i in 0..Self::SIZE {
                let word_idx = i / old_bpw;
                let sub_idx = i % old_bpw;
                let val = (old_words[word_idx] >> (sub_idx * old_bits as usize)) & old_mask;
                self.write_word_bits(i, val);
            }
        }
    }

    #[inline(always)]
    fn read_word_bits(&self, index: usize) -> u32 {
        let bpb = self.bits_per_block as usize;
        let blocks_per_word = 32 / bpb;
        let word_idx = index / blocks_per_word;
        let sub_idx = index % blocks_per_word;
        (self.words[word_idx] >> (sub_idx * bpb)) & ((1 << bpb) - 1)
    }

    #[inline(always)]
    fn write_word_bits(&mut self, index: usize, val: u32) {
        let bpb = self.bits_per_block as usize;
        let blocks_per_word = 32 / bpb;
        let word_idx = index / blocks_per_word;
        let sub_idx = index % blocks_per_word;
        let mask = ((1 << bpb) - 1) << (sub_idx * bpb);
        self.words[word_idx] =
            (self.words[word_idx] & !mask) | ((val & ((1 << bpb) - 1)) << (sub_idx * bpb));
    }

    pub fn bits_per_block(&self) -> u8 {
        self.bits_per_block
    }

    pub fn words(&self) -> &[u32] {
        &self.words
    }

    pub fn palette(&self) -> &[u32] {
        &self.palette
    }

    pub fn from_raw(bits_per_block: u8, words: Vec<u32>, palette: Vec<u32>) -> Self {
        Self {
            bits_per_block,
            words,
            palette,
        }
    }
}
