//! Tests Bedrock's biome sampling convention: the climate parameters are
//! sampled on Java's 4-block lattice and trilinearly interpolated per block
//! before the biome lookup.
//!
//!   cargo run -p bedrock-world --example biome_interp -- <seed> <y>

use std::io::{self, BufRead};

use bedrock_world::generator::bds_biome_tree::java_to_bedrock;
use bedrock_world::generator::java_biome_tree::climate_to_biome;
use bedrock_world::generator::java_biomes::JavaBiomes;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        panic!("usage: biome_interp <seed> <y>");
    }
    let seed: u64 = args[1].parse().expect("seed must be a number");
    let y: i32 = args[2].parse().expect("y must be a number");
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

        // Corners of the surrounding 4-block cell, in quart coordinates.
        let (qx0, qx1) = (x >> 2, (x >> 2) + 1);
        let (qz0, qz1) = (z >> 2, (z >> 2) + 1);
        let (qy0, qy1) = (y >> 2, (y >> 2) + 1);
        let fx = (x & 3) as f64 / 4.0;
        let fy = (y & 3) as f64 / 4.0;
        let fz = (z & 3) as f64 / 4.0;

        let mut climate = [0.0f64; 6];
        for (i, slot) in climate.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (dy, wy) in [(qy0, 1.0 - fy), (qy1, fy)] {
                for (dz, wz) in [(qz0, 1.0 - fz), (qz1, fz)] {
                    for (dx, wx) in [(qx0, 1.0 - fx), (qx1, fx)] {
                        let corner = biomes.climate_at(dx, dy, dz);
                        sum += wx * wy * wz * corner[i] as f64;
                    }
                }
            }
            *slot = sum;
        }

        let np: [i32; 6] = std::array::from_fn(|i| climate[i] as i32);
        println!("{}", java_to_bedrock(climate_to_biome(&np)));
    }
}
