//! Prints Java biome ids for `x z` pairs read from stdin, for validation
//! against a reference Java generator implementation.
//!
//!   cargo run -p bedrock-world --example biome_grid -- <seed> <y> [block]
//!
//! With `block` the climate is sampled at block resolution (only Y is reduced
//! to quarts), which is the convention Bedrock's stored biomes follow.

use std::io::{self, BufRead};

use bedrock_world::generator::java_biomes::JavaBiomes;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        panic!("usage: biome_grid <seed> <y> [block]");
    }
    let seed: u64 = args[1].parse().expect("seed must be a number");
    let y: i32 = args[2].parse().expect("y must be a number");
    let block_scale = args.get(3).map(String::as_str) == Some("block");
    let biomes = JavaBiomes::new(seed);
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.expect("stdin");
        let mut parts = line.split_whitespace();
        let (Some(x), Some(z)) = (parts.next(), parts.next()) else {
            continue;
        };
        let x: i32 = x.parse().expect("x must be a number");
        let z: i32 = z.parse().expect("z must be a number");
        let id = if block_scale {
            biomes.java_biome_at_quart(x, y >> 2, z)
        } else {
            biomes.java_biome_at(x, y, z)
        };
        println!("{id}");
    }
}
