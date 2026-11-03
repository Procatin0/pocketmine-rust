//! PocketMine-MP Tree Populator port.
//!
//! Matches `pocketmine\world\generator\populator\Tree` and `TreeFactory` (Oak, Spruce, Birch).

use crate::block_id;
use crate::chunk::Chunk;
use crate::generator::random::Random;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeType {
    Oak,
    Spruce,
    Birch,
}

#[derive(Debug, Clone)]
pub struct TreePopulator {
    pub tree_type: TreeType,
    pub base_amount: i32,
    pub random_amount: i32,
}

impl TreePopulator {
    pub fn new(tree_type: TreeType, base_amount: i32, random_amount: i32) -> Self {
        Self {
            tree_type,
            base_amount,
            random_amount,
        }
    }

    pub fn populate(&self, chunk: &mut Chunk, _chunk_x: i32, _chunk_z: i32, random: &mut Random) {
        let amount = random.next_range(0, self.random_amount) + self.base_amount;
        for _ in 0..amount {
            let x = random.next_range(0, 15) as usize;
            let z = random.next_range(0, 15) as usize;

            let highest = match chunk.get_highest_block_at(x, z) {
                Some(y) => y,
                None => continue,
            };

            let mut workable_y = -1;
            for y in (0..=highest).rev() {
                let bid = chunk.get_block(x, y, z);
                if bid == block_id::DIRT || bid == block_id::GRASS_BLOCK {
                    workable_y = y + 1;
                    break;
                } else if bid != block_id::AIR && bid != block_id::SNOW_LAYER {
                    break;
                }
            }

            if workable_y == -1 {
                continue;
            }

            match self.tree_type {
                TreeType::Oak => self.grow_oak(chunk, x, workable_y, z, random),
                TreeType::Spruce => self.grow_spruce(chunk, x, workable_y, z, random),
                TreeType::Birch => self.grow_birch(chunk, x, workable_y, z, random),
            }
        }
    }

    fn grow_oak(&self, chunk: &mut Chunk, x: usize, y: i32, z: usize, random: &mut Random) {
        let height = random.next_bounded_int(3) + 4; // 4..6
        if !self.can_grow(chunk, x, y, z, height) {
            return;
        }

        // Base dirt
        chunk.set_block(x, y - 1, z, block_id::DIRT);

        // Trunk
        for dy in 0..(height - 1) {
            chunk.set_block(x, y + dy, z, block_id::OAK_LOG);
        }

        // Canopy
        for yy in (y - 3 + height)..=(y + height) {
            let y_off = yy - (y + height);
            let mid = 1 - y_off / 2;
            for xx in (x as i32 - mid)..=(x as i32 + mid) {
                let x_off = (xx - x as i32).abs();
                for zz in (z as i32 - mid)..=(z as i32 + mid) {
                    let z_off = (zz - z as i32).abs();
                    if x_off == mid && z_off == mid && (y_off == 0 || random.next_bounded_int(2) == 0) {
                        continue;
                    }
                    if xx >= 0 && xx < 16 && zz >= 0 && zz < 16 {
                        let cur = chunk.get_block(xx as usize, yy, zz as usize);
                        if !block_id::is_solid(cur) {
                            chunk.set_block(xx as usize, yy, zz as usize, block_id::OAK_LEAVES);
                        }
                    }
                }
            }
        }
    }

    fn grow_birch(&self, chunk: &mut Chunk, x: usize, y: i32, z: usize, random: &mut Random) {
        let height = random.next_bounded_int(3) + 5; // 5..7
        if !self.can_grow(chunk, x, y, z, height) {
            return;
        }

        // Base dirt
        chunk.set_block(x, y - 1, z, block_id::DIRT);

        // Trunk
        for dy in 0..(height - 1) {
            chunk.set_block(x, y + dy, z, block_id::BIRCH_LOG);
        }

        // Canopy
        for yy in (y - 3 + height)..=(y + height) {
            let y_off = yy - (y + height);
            let mid = 1 - y_off / 2;
            for xx in (x as i32 - mid)..=(x as i32 + mid) {
                let x_off = (xx - x as i32).abs();
                for zz in (z as i32 - mid)..=(z as i32 + mid) {
                    let z_off = (zz - z as i32).abs();
                    if x_off == mid && z_off == mid && (y_off == 0 || random.next_bounded_int(2) == 0) {
                        continue;
                    }
                    if xx >= 0 && xx < 16 && zz >= 0 && zz < 16 {
                        let cur = chunk.get_block(xx as usize, yy, zz as usize);
                        if !block_id::is_solid(cur) {
                            chunk.set_block(xx as usize, yy, zz as usize, block_id::BIRCH_LEAVES);
                        }
                    }
                }
            }
        }
    }

    fn grow_spruce(&self, chunk: &mut Chunk, x: usize, y: i32, z: usize, random: &mut Random) {
        let height = random.next_bounded_int(4) + 6; // 6..9
        if !self.can_grow(chunk, x, y, z, height) {
            return;
        }

        // Base dirt
        chunk.set_block(x, y - 1, z, block_id::DIRT);

        // Trunk
        let trunk_height = height - random.next_bounded_int(3);
        for dy in 0..trunk_height {
            chunk.set_block(x, y + dy, z, block_id::SPRUCE_LOG);
        }

        // Conical canopy
        let top_size = height - (1 + random.next_bounded_int(2));
        let l_radius = 2 + random.next_bounded_int(2);
        let mut radius = random.next_bounded_int(2);
        let mut max_r = 1;
        let mut min_r = 0;

        for yy in 0..=top_size {
            let yyy = y + height - yy;
            for xx in (x as i32 - radius)..=(x as i32 + radius) {
                let x_off = (xx - x as i32).abs();
                for zz in (z as i32 - radius)..=(z as i32 + radius) {
                    let z_off = (zz - z as i32).abs();
                    if x_off == radius && z_off == radius && radius > 0 {
                        continue;
                    }
                    if xx >= 0 && xx < 16 && zz >= 0 && zz < 16 {
                        let cur = chunk.get_block(xx as usize, yyy, zz as usize);
                        if !block_id::is_solid(cur) {
                            chunk.set_block(xx as usize, yyy, zz as usize, block_id::SPRUCE_LEAVES);
                        }
                    }
                }
            }

            if radius >= max_r {
                radius = min_r;
                min_r = 1;
                max_r += 1;
                if max_r > l_radius {
                    max_r = l_radius;
                }
            } else {
                radius += 1;
            }
        }
    }

    fn can_grow(&self, chunk: &Chunk, x: usize, y: i32, z: usize, height: i32) -> bool {
        let mut radius_to_check = 0;
        for yy in 0..(height + 3) {
            if yy == 1 || yy == height {
                radius_to_check += 1;
            }
            for xx in (x as i32 - radius_to_check)..=(x as i32 + radius_to_check) {
                for zz in (z as i32 - radius_to_check)..=(z as i32 + radius_to_check) {
                    if xx >= 0 && xx < 16 && zz >= 0 && zz < 16 {
                        let block = chunk.get_block(xx as usize, y + yy, zz as usize);
                        if !block_id::can_override_for_tree(block) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }
}
