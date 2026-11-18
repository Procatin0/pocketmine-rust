//! Compares the spread of the terrain climate noises against the biome climate
//! noises they mirror.
//!
//!   cargo run -p bedrock-world --example field_stats -- <seed>

use bedrock_world::generator::java_biomes::JavaBiomes;
use bedrock_world::generator::java_density::TerrainDensity;

fn stats(name: &str, mut values: Vec<f64>) {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let sd = (values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n).sqrt();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "{name:24} mean {mean:+.3} sd {sd:.3} min {:+.3} max {:+.3}",
        values[0],
        values[values.len() - 1]
    );
}

fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .expect("usage: field_stats <seed>")
        .parse()
        .expect("seed must be a number");
    let terrain = TerrainDensity::new(seed);
    let biomes = JavaBiomes::new(seed);

    for (name, index) in [("continents", 2usize), ("erosion", 3), ("offset", 4)] {
        let mut values = Vec::new();
        for x in (-512..=512).step_by(16) {
            for z in (-512..=512).step_by(16) {
                values.push(terrain.named(name, x as f64, 0.0, z as f64));
            }
        }
        stats(&format!("terreno/{name}"), values);

        let mut values = Vec::new();
        for x in (-128..=128).step_by(4) {
            for z in (-128..=128).step_by(4) {
                values.push(biomes.noise_at(index, x as f64, z as f64));
            }
        }
        stats(&format!("clima/noise[{index}]"), values);
    }
}
