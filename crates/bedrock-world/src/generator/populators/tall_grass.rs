//! PocketMine-MP TallGrass Populator port.
//!
//! Matches `pocketmine\world\generator\populator\TallGrass`.

use crate::block_id;
use crate::generator::manager::ChunkManager;
use crate::generator::random::Random;

#[derive(Debug, Clone)]
pub struct TallGrassPopulator {
    pub random_amount: i32,
    pub base_amount: i32,
}

impl TallGrassPopulator {
    pub fn new(base_amount: i32) -> Self {
        Self {
            random_amount: 1,
            base_amount,
        }
    }

    pub fn set_random_amount(&mut self, amount: i32) {
        self.random_amount = amount;
    }

    pub fn set_base_amount(&mut self, amount: i32) {
        self.base_amount = amount;
    }

    pub fn populate<M: ChunkManager>(&self, world: &mut M, chunk_x: i32, chunk_z: i32, random: &mut Random) {
        let amount = random.next_range(0, self.random_amount) + self.base_amount;
        let base_x = chunk_x * 16;
        let base_z = chunk_z * 16;

        for _ in 0..amount {
            let x = random.next_range(base_x, base_x + 15);
            let z = random.next_range(base_z, base_z + 15);
            let y = self.get_highest_workable_block(world, x, z);

            if y != -1 && self.can_tall_grass_stay(world, x, y, z) {
                world.set_block_at(x, y, z, block_id::SHORT_GRASS);
            }
        }
    }

    fn can_tall_grass_stay<M: ChunkManager>(&self, world: &M, x: i32, y: i32, z: i32) -> bool {
        let b = world.get_block_at(x, y, z);
        let below = world.get_block_at(x, y - 1, z);
        (b == block_id::AIR || b == block_id::SNOW_LAYER) && below == block_id::GRASS_BLOCK
    }

    fn get_highest_workable_block<M: ChunkManager>(&self, world: &M, x: i32, z: i32) -> i32 {
        let highest = match world.get_highest_block_at(x, z) {
            Some(h) => h,
            None => return -1,
        };

        for y in (0..=highest).rev() {
            let b = world.get_block_at(x, y, z);
            if b != block_id::AIR && !is_leaves(b) && b != block_id::SNOW_LAYER {
                return y + 1;
            }
        }

        -1
    }
}

#[inline(always)]
fn is_leaves(id: u32) -> bool {
    matches!(
        id,
        block_id::OAK_LEAVES | block_id::SPRUCE_LEAVES | block_id::BIRCH_LEAVES
    )
}
