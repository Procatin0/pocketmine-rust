//! Prints the predicted Overworld surface height for each `x z` pair on stdin.
//!
//!   cargo run -p bedrock-world --example surface_height -- <seed> [final]

use std::io::{self, BufRead};

use bedrock_world::generator::java_density::{DensityRoot, TerrainDensity};

fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .expect("usage: surface_height <seed>")
        .parse()
        .expect("seed must be a number");
    let root = match std::env::args().nth(2).as_deref() {
        Some("final") => DensityRoot::FinalDensity,
        _ => DensityRoot::SlopedCheese,
    };
    let terrain = TerrainDensity::with_root(seed, root);
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.expect("stdin");
        let mut parts = line.split_whitespace();
        let (Some(x), Some(z)) = (parts.next(), parts.next()) else {
            continue;
        };
        let x: i32 = x.parse().expect("x must be a number");
        let z: i32 = z.parse().expect("z must be a number");
        match terrain.surface_height(x, z) {
            Some(height) => println!("{height}"),
            None => println!("none"),
        }
    }
}
