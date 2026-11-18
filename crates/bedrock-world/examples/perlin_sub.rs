//! Prints one raw perlin octave sample per line, for bit-exact validation
//! against a Java generator implementation.
//!
//!   cargo run -p bedrock-world --example perlin_sub -- <seed> <x> <y> <z>

use bedrock_world::generator::java_biomes::JavaBiomes;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        panic!("usage: perlin_sub <seed> <x> <y> <z>");
    }
    let seed: u64 = args[1].parse().expect("seed must be a number");
    let x: f64 = args[2].parse().expect("x must be a number");
    let y: f64 = args[3].parse().expect("y must be a number");
    let z: f64 = args[4].parse().expect("z must be a number");
    let biomes = JavaBiomes::new(seed);
    for index in 0..6 {
        let octaves = biomes.debug_climate(index);
        for (octave, (amplitude, lacunarity, _, _, _, _)) in octaves.iter().enumerate() {
            let value = biomes.debug_perlin(
                index,
                octave,
                x * lacunarity,
                y * lacunarity,
                z * lacunarity,
            );
            println!("{amplitude:.17} {value:.17}");
        }
    }
}
