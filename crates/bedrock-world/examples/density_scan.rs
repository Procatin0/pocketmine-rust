//! Scans the coarse noise fields that shape the terrain.
//!
//!   cargo run -p bedrock-world --example density_scan -- <seed>

use bedrock_world::generator::java_density::TerrainDensity;

fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .expect("usage: density_scan <seed>")
        .parse()
        .expect("seed must be a number");
    let terrain = TerrainDensity::new(seed);
    for z in (-256..=256).step_by(256) {
        for x in (-512..=512).step_by(128) {
            println!(
                "x={x:5} z={z:5}: continents {:+.3} erosion {:+.3} ridges {:+.3} folded {:+.3} offset {:+.3} factor {:+.3} jagged {:+.3} height {:?}",
                terrain.named("continents", x as f64, 0.0, z as f64),
                terrain.named("erosion", x as f64, 0.0, z as f64),
                terrain.named("ridges", x as f64, 0.0, z as f64),
                terrain.named("ridges_folded", x as f64, 0.0, z as f64),
                terrain.named("offset", x as f64, 0.0, z as f64),
                terrain.named("factor", x as f64, 0.0, z as f64),
                terrain.named("jaggedness", x as f64, 0.0, z as f64),
                terrain.surface_height(x, z),
            );
        }
    }
}
