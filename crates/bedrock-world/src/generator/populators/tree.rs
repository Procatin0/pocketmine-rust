//! PocketMine-MP Tree Populator port.
//!
//! Matches `pocketmine\world\generator\populator\Tree`, `TreeFactory`,
//! `OakTree`, `BirchTree`, and `SpruceTree`.

use std::collections::HashMap;
use crate::block_id;
use crate::generator::manager::ChunkManager;
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
    pub random_amount: i32,
    pub base_amount: i32,
}

impl TreePopulator {
    pub fn new(tree_type: TreeType, base_amount: i32, random_amount: i32) -> Self {
        Self {
            tree_type,
            random_amount,
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
        let base_x = chunk_x << 4;
        let base_z = chunk_z << 4;

        for _ in 0..amount {
            let x = random.next_range(base_x, base_x + 16);
            let z = random.next_range(base_z, base_z + 16);
            let y = self.get_highest_workable_block(world, x, z);
            if y == -1 {
                continue;
            }

            match self.tree_type {
                TreeType::Oak => self.grow_oak(world, x, y, z, random),
                TreeType::Birch => self.grow_birch(world, x, y, z, random),
                TreeType::Spruce => self.grow_spruce(world, x, y, z, random),
            }
        }
    }

    fn get_highest_workable_block<M: ChunkManager>(&self, world: &M, x: i32, z: i32) -> i32 {
        let highest = match world.get_highest_block_at(x, z) {
            Some(h) => h,
            None => return -1,
        };

        for y in (0..=highest).rev() {
            let b = world.get_block_at(x, y, z);
            if b == block_id::DIRT || b == block_id::GRASS_BLOCK {
                return y + 1;
            } else if b != block_id::AIR && b != block_id::SNOW_LAYER {
                return -1;
            }
        }

        -1
    }

    fn can_place_tree<M: ChunkManager>(world: &M, x: i32, y: i32, z: i32, tree_height: i32) -> bool {
        let mut radius_to_check = 0;
        for yy in 0..(tree_height + 3) {
            if yy == 1 || yy == tree_height {
                radius_to_check += 1;
            }
            for xx in -radius_to_check..=radius_to_check {
                for zz in -radius_to_check..=radius_to_check {
                    let b = world.get_block_at(x + xx, y + yy, z + zz);
                    if !block_id::can_override_for_tree(b) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn grow_oak<M: ChunkManager>(&self, world: &mut M, x: i32, y: i32, z: i32, random: &mut Random) {
        let tree_height = random.next_bounded_int(3) + 4;
        self.grow_standard_tree(world, x, y, z, random, tree_height, block_id::OAK_LOG, block_id::OAK_LEAVES);
    }

    fn grow_birch<M: ChunkManager>(&self, world: &mut M, x: i32, y: i32, z: i32, random: &mut Random) {
        // TreeFactory::get: BirchTree($random->nextBoundedInt(39) === 0)
        let super_birch = random.next_bounded_int(39) == 0;
        let mut tree_height = random.next_bounded_int(3) + 5;
        if super_birch {
            tree_height += 5;
        }
        self.grow_standard_tree(world, x, y, z, random, tree_height, block_id::BIRCH_LOG, block_id::BIRCH_LEAVES);
    }

    fn grow_standard_tree<M: ChunkManager>(
        &self,
        world: &mut M,
        x: i32,
        y: i32,
        z: i32,
        random: &mut Random,
        tree_height: i32,
        trunk_block: u32,
        leaf_block: u32,
    ) {
        if !Self::can_place_tree(world, x, y, z, tree_height) {
            return;
        }

        let mut transaction = HashMap::new();

        // 1. Base dirt block
        transaction.insert((x, y - 1, z), block_id::DIRT);

        // 2. Trunk
        let trunk_height = tree_height - 1;
        for yy in 0..trunk_height {
            let pos = (x, y + yy, z);
            let cur = transaction.get(&pos).copied().unwrap_or_else(|| world.get_block_at(x, y + yy, z));
            if block_id::can_override_for_tree(cur) {
                transaction.insert(pos, trunk_block);
            }
        }

        // 3. Canopy
        for yy in (y - 3 + tree_height)..=(y + tree_height) {
            let y_off = yy - (y + tree_height);
            let mid = 1 - y_off / 2;
            for xx in (x - mid)..=(x + mid) {
                let x_off = (xx - x).abs();
                for zz in (z - mid)..=(z + mid) {
                    let z_off = (zz - z).abs();
                    if x_off == mid && z_off == mid && (y_off == 0 || random.next_bounded_int(2) == 0) {
                        continue;
                    }
                    let pos = (xx, yy, zz);
                    let cur = transaction.get(&pos).copied().unwrap_or_else(|| world.get_block_at(xx, yy, zz));
                    if !block_id::is_solid(cur) {
                        transaction.insert(pos, leaf_block);
                    }
                }
            }
        }

        // Apply transaction
        for ((bx, by, bz), block) in transaction {
            world.set_block_at(bx, by, bz, block);
        }
    }

    fn grow_spruce<M: ChunkManager>(&self, world: &mut M, x: i32, y: i32, z: i32, random: &mut Random) {
        let tree_height = random.next_bounded_int(4) + 6;
        if !Self::can_place_tree(world, x, y, z, tree_height) {
            return;
        }

        let mut transaction = HashMap::new();

        // Base dirt
        transaction.insert((x, y - 1, z), block_id::DIRT);

        // Trunk
        let trunk_height = tree_height - random.next_bounded_int(3);
        for yy in 0..trunk_height {
            let pos = (x, y + yy, z);
            let cur = transaction.get(&pos).copied().unwrap_or_else(|| world.get_block_at(x, y + yy, z));
            if block_id::can_override_for_tree(cur) {
                transaction.insert(pos, block_id::SPRUCE_LOG);
            }
        }

        // Canopy
        let top_size = tree_height - (1 + random.next_bounded_int(2));
        let l_radius = 2 + random.next_bounded_int(2);
        let mut radius = random.next_bounded_int(2);
        let mut max_r = 1;
        let mut min_r = 0;

        for yy in 0..=top_size {
            let yyy = y + tree_height - yy;
            for xx in (x - radius)..=(x + radius) {
                let x_off = (xx - x).abs();
                for zz in (z - radius)..=(z + radius) {
                    let z_off = (zz - z).abs();
                    if x_off == radius && z_off == radius && radius > 0 {
                        continue;
                    }
                    let pos = (xx, yyy, zz);
                    let cur = transaction.get(&pos).copied().unwrap_or_else(|| world.get_block_at(xx, yyy, zz));
                    if !block_id::is_solid(cur) {
                        transaction.insert(pos, block_id::SPRUCE_LEAVES);
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

        // Apply transaction
        for ((bx, by, bz), block) in transaction {
            world.set_block_at(bx, by, bz, block);
        }
    }
}
