//! PocketMine-MP GroundCover Populator port.
//!
//! Matches `pocketmine\world\generator\populator\GroundCover`.

use crate::block_id;
use crate::chunk::Chunk;
use crate::generator::biome::get_biome_info;
use crate::generator::random::Random;

#[derive(Debug, Default, Clone)]
pub struct GroundCoverPopulator;

impl GroundCoverPopulator {
    pub fn new() -> Self {
        Self
    }

    pub fn populate(&self, chunk: &mut Chunk, _chunk_x: i32, _chunk_z: i32, _random: &mut Random) {
        for x in 0..16 {
            for z in 0..16 {
                let biome_id = chunk.biomes[x * 16 + z];
                let info = get_biome_info(biome_id);
                let cover = info.ground_cover;
                if cover.is_empty() {
                    continue;
                }

                let diff_y = if !block_id::is_solid(cover[0]) { 1 } else { 0 };

                let start_y = match chunk.get_highest_block_at(x, z) {
                    Some(y) => y,
                    None => continue,
                };

                let mut solid_y = start_y;
                while solid_y > 0 && block_id::is_transparent(chunk.get_block(x, solid_y, z)) {
                    solid_y -= 1;
                }

                let cover_start = (solid_y + diff_y).min(127);
                let cover_end = cover_start - cover.len() as i32;

                for y in ((cover_end + 1).max(0)..=cover_start).rev() {
                    let b = cover[(cover_start - y) as usize];
                    let current = chunk.get_block(x, y, z);

                    if current == block_id::AIR && block_id::is_solid(b) {
                        break;
                    }

                    if block_id::can_be_flowed_into(b) && current == block_id::WATER {
                        continue;
                    }

                    chunk.set_block(x, y, z, b);
                }
            }
        }
    }
}
