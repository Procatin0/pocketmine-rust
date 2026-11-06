
use bedrock_world::generator::normal::NormalGenerator;
use bedrock_world::generator::WorldGenerator;
use std::time::Instant;

fn main() {
    let gen = NormalGenerator::new(12345678);
    let start = Instant::now();
    let mut count = 0;
    for dx in -12..=12 {
        for dz in -12..=12 {
            if dx * dx + dz * dz <= 144 {
                let _ = gen.generate_chunk(1 + dx, -1 + dz);
                count += 1;
            }
        }
    }
    let elapsed = start.elapsed();
    println!("Generated {} chunks in {:.2?} ({:.2?} per chunk)", count, elapsed, elapsed / count);
}
