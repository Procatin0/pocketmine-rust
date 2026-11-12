use bedrock_world::generator::biome;
use bedrock_world::generator::normal::NormalGenerator;
use bedrock_world::generator::WorldGenerator;

fn main() {
    let gen = NormalGenerator::new(12345678);
    println!("Spawn pos: {:?}", gen.spawn_position());
    println!("     cx: -10       0        10       20");
    for cz in -5..25 {
        let mut row = String::new();
        for cx in -10..20 {
            let chunk = gen.generate_chunk_internal(cx, cz);
            let b = chunk.biomes[7 * 16 + 7];
            let ch = match b {
                biome::PLAINS => ".",
                biome::FOREST => "F",
                biome::BIRCH_FOREST => "B",
                biome::TAIGA => "T",
                biome::EXTREME_HILLS | biome::EXTREME_HILLS_EDGE => "M",
                biome::RIVER => "~",
                biome::OCEAN => "O",
                biome::ICE_PLAINS => "I",
                biome::DESERT => "D",
                _ => "?",
            };
            row.push_str(ch);
        }
        println!("{:3}: {}", cz, row);
    }
}
