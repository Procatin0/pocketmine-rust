//! Prints Bedrock biome ids for `x z` pairs read from stdin.
//!
//! This is the same sampling `bds-probe compare --impl java` uses, so the
//! result can be checked against the captured `runs` in `data/bds-reference/`.
//!
//!   cargo run -p bedrock-world --example biome_bedrock -- <seed> <y>

use std::io::{self, BufRead};

use bedrock_world::generator::java_biomes::JavaBiomes;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args[1].parse().expect("seed must be a number");
    let y: i32 = args.get(2).map(|v| v.parse().expect("y must be a number")).unwrap_or(64);
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
        println!("{}", biomes.biome_at(x, y, z));
    }
}
