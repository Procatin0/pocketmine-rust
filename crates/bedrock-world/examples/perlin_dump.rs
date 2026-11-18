//! Prints the internal octave setup of the climate noises, for bit-exact
//! validation against a Java generator implementation.
//!
//!   cargo run -p bedrock-world --example perlin_dump -- <seed>

use bedrock_world::generator::java_biomes::JavaBiomes;

fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .expect("usage: perlin_dump <seed>")
        .parse()
        .expect("seed must be a number");
    let biomes = JavaBiomes::new(seed);
    println!("-- raw samples at (0,0)/(1000,1000):");
    for index in 0..6 {
        println!(
            "{:.17}\n{:.17}",
            biomes.noise_at(index, 0.0, 0.0),
            biomes.noise_at(index, 1000.0, 1000.0)
        );
    }
    println!("-- octaves:");
    for (index, name) in [
        "temperature",
        "vegetation",
        "continents",
        "erosion",
        "offset",
        "ridge",
    ]
    .iter()
    .enumerate()
    {
        println!("{name}:");
        for (i, (amplitude, lacunarity, a, b, _c, permutation)) in
            biomes.debug_climate(index).iter().enumerate()
        {
            println!(
                "  oct {i} amp {amplitude:.17} lac {lacunarity:.17} a {a:.17} b {b:.17} perm {:?}",
                permutation
            );
        }
    }
}
