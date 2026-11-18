//! Samples the climate at block resolution (`block / 4` in quart space) and
//! prints the resulting Bedrock biome id, to test whether Bedrock resolves
//! biome borders per block instead of per 4-block cell.
//!
//!   cargo run -p bedrock-world --example biome_fine -- <seed> <y> [ox] [oz] [oy]
//!
//! `ox/oz/oy` shift the sample point inside the 4-block cell (0..3), which
//! probes where Bedrock actually samples the climate for a block.

use std::io::{self, BufRead};

use bedrock_world::generator::bds_biome_tree::java_to_bedrock;
use bedrock_world::generator::java_biome_tree::climate_to_biome;
use bedrock_world::generator::java_biomes::JavaBiomes;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        panic!("usage: biome_fine <seed> <y>");
    }
    let seed: u64 = args[1].parse().expect("seed must be a number");
    let y: i32 = args[2].parse().expect("y must be a number");
    let offset = |index: usize| -> f64 {
        args.get(index)
            .map(|v| v.parse().expect("offset must be a number"))
            .unwrap_or(0.0)
    };
    let (ox, oz, oy) = (offset(3), offset(4), offset(5));
    let biomes = JavaBiomes::new(seed);
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.expect("stdin");
        let mut parts = line.split_whitespace();
        let (Some(x), Some(z)) = (parts.next(), parts.next()) else {
            continue;
        };
        let x: f64 = x.parse().expect("x must be a number");
        let z: f64 = z.parse().expect("z must be a number");
        let np = biomes.climate_at_f64((x + ox) / 4.0, (y as f64 + oy) / 4.0, (z + oz) / 4.0);
        println!("{}", java_to_bedrock(climate_to_biome(&np)));
    }
}
