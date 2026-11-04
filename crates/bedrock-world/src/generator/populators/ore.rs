//! PocketMine-MP Ore Populator port.
//!
//! Matches `pocketmine\world\generator\populator\Ore` and `pocketmine\world\generator\object\Ore`.

use std::collections::HashSet;
use std::f64::consts::PI;
use crate::block_id;
use crate::generator::manager::ChunkManager;
use crate::generator::random::Random;

#[derive(Debug, Clone)]
pub struct OreType {
    pub material: u32,
    pub replaces: u32,
    pub cluster_count: i32,
    pub cluster_size: i32,
    pub min_height: i32,
    pub max_height: i32,
}

#[derive(Debug, Clone)]
pub struct OrePopulator {
    pub ore_types: Vec<OreType>,
}

impl Default for OrePopulator {
    fn default() -> Self {
        Self::new_overworld()
    }
}

impl OrePopulator {
    pub fn new(ore_types: Vec<OreType>) -> Self {
        Self { ore_types }
    }

    pub fn new_overworld() -> Self {
        Self {
            ore_types: vec![
                OreType {
                    material: block_id::COAL_ORE,
                    replaces: block_id::STONE,
                    cluster_count: 20,
                    cluster_size: 16,
                    min_height: 0,
                    max_height: 128,
                },
                OreType {
                    material: block_id::IRON_ORE,
                    replaces: block_id::STONE,
                    cluster_count: 20,
                    cluster_size: 8,
                    min_height: 0,
                    max_height: 64,
                },
                OreType {
                    material: block_id::REDSTONE_ORE,
                    replaces: block_id::STONE,
                    cluster_count: 8,
                    cluster_size: 7,
                    min_height: 0,
                    max_height: 16,
                },
                OreType {
                    material: block_id::LAPIS_ORE,
                    replaces: block_id::STONE,
                    cluster_count: 1,
                    cluster_size: 6,
                    min_height: 0,
                    max_height: 32,
                },
                OreType {
                    material: block_id::GOLD_ORE,
                    replaces: block_id::STONE,
                    cluster_count: 2,
                    cluster_size: 8,
                    min_height: 0,
                    max_height: 32,
                },
                OreType {
                    material: block_id::DIAMOND_ORE,
                    replaces: block_id::STONE,
                    cluster_count: 1,
                    cluster_size: 7,
                    min_height: 0,
                    max_height: 16,
                },
                OreType {
                    material: block_id::DIRT,
                    replaces: block_id::STONE,
                    cluster_count: 20,
                    cluster_size: 32,
                    min_height: 0,
                    max_height: 128,
                },
                OreType {
                    material: block_id::GRAVEL,
                    replaces: block_id::STONE,
                    cluster_count: 10,
                    cluster_size: 16,
                    min_height: 0,
                    max_height: 128,
                },
            ],
        }
    }

    pub fn new_emerald() -> Self {
        Self {
            ore_types: vec![
                OreType {
                    material: block_id::EMERALD_ORE,
                    replaces: block_id::STONE,
                    cluster_count: 11,
                    cluster_size: 1,
                    min_height: 0,
                    max_height: 32,
                },
            ],
        }
    }

    pub fn populate<M: ChunkManager>(&self, world: &mut M, chunk_x: i32, chunk_z: i32, random: &mut Random) {
        let base_x = chunk_x << 4;
        let base_z = chunk_z << 4;

        for ore_type in &self.ore_types {
            for _ in 0..ore_type.cluster_count {
                let x = random.next_range(base_x, base_x + 15);
                let y = random.next_range(ore_type.min_height, ore_type.max_height);
                let z = random.next_range(base_z, base_z + 15);

                if world.get_block_at(x, y, z) == ore_type.replaces {
                    self.place_cluster(world, x, y, z, ore_type, random);
                }
            }
        }
    }

    fn place_cluster<M: ChunkManager>(
        &self,
        world: &mut M,
        x: i32,
        y: i32,
        z: i32,
        ore_type: &OreType,
        random: &mut Random,
    ) {
        let cluster_size = ore_type.cluster_size;
        let angle = random.next_float() * PI;
        let offset_x = angle.cos() * (cluster_size as f64 / 8.0);
        let offset_z = angle.sin() * (cluster_size as f64 / 8.0);

        let x1 = x as f64 + 8.0 + offset_x;
        let x2 = x as f64 + 8.0 - offset_x;
        let z1 = z as f64 + 8.0 + offset_z;
        let z2 = z as f64 + 8.0 - offset_z;
        let y1 = y as f64 + (random.next_bounded_int(3) + 2) as f64;
        let y2 = y as f64 + (random.next_bounded_int(3) + 2) as f64;

        let mut visited = HashSet::new();

        for count in 0..=cluster_size {
            let t = count as f64 / cluster_size as f64;
            let center_x = x1 + (x2 - x1) * t;
            let center_y = y1 + (y2 - y1) * t;
            let center_z = z1 + (z2 - z1) * t;
            let radius = (((count as f64 * (PI / cluster_size as f64)).sin() + 1.0)
                * random.next_float()
                * (cluster_size as f64 / 16.0)
                + 1.0)
                / 2.0;

            let start_x = (center_x - radius) as i32;
            let end_x = (center_x + radius) as i32;
            let start_y = (center_y - radius) as i32;
            let end_y = (center_y + radius) as i32;
            let start_z = (center_z - radius) as i32;
            let end_z = (center_z + radius) as i32;

            for xx in start_x..=end_x {
                let size_x = (xx as f64 + 0.5 - center_x) / radius;
                let size_x_sq = size_x * size_x;
                if size_x_sq >= 1.0 {
                    continue;
                }

                for yy in start_y..=end_y {
                    let size_y = (yy as f64 + 0.5 - center_y) / radius;
                    let size_y_sq = size_y * size_y;
                    if yy <= 0 || (size_x_sq + size_y_sq) >= 1.0 {
                        continue;
                    }

                    for zz in start_z..=end_z {
                        let size_z = (zz as f64 + 0.5 - center_z) / radius;
                        let size_z_sq = size_z * size_z;
                        if (size_x_sq + size_y_sq + size_z_sq) < 1.0 {
                            let hash = ((xx as i64) << 40) ^ ((zz as i64) << 20) ^ ((yy as i64) & 0xfffff);
                            if visited.insert(hash) {
                                if world.get_block_at(xx, yy, zz) == ore_type.replaces {
                                    world.set_block_at(xx, yy, zz, ore_type.material);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
