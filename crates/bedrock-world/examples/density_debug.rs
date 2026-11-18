//! Prints the named density sub-functions for a column.
//!
//!   cargo run -p bedrock-world --example density_debug -- <seed> <x> <z>

use bedrock_world::generator::java_density::TerrainDensity;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args[1].parse().expect("seed");
    let x: f64 = args[2].parse().expect("x");
    let z: f64 = args[3].parse().expect("z");
    let terrain = TerrainDensity::new(seed);
    for y in [-64.0, 0.0, 64.0, 128.0, 200.0, 319.0] {
        println!(
            "y={y:6}: continents {:+.4} erosion {:+.4} ridges {:+.4} offset {:+.4} factor {:+.4} jaggedness {:+.4} depth {:+.4} density {:+.4}",
            terrain.named("continents", x, y, z),
            terrain.named("erosion", x, y, z),
            terrain.named("ridges", x, y, z),
            terrain.named("offset", x, y, z),
            terrain.named("factor", x, y, z),
            terrain.named("jaggedness", x, y, z),
            terrain.named("depth", x, y, z),
            terrain.density(x, y, z),
        );
    }
}
