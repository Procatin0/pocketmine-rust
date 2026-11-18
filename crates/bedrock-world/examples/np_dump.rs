//! Prints Java's six quantised climate parameters for `x z` pairs on stdin.
//!
//! Used to validate [`JavaBiomes`] bit by bit against an independent Java
//! generator implementation; `bds-probe compare` is the end-to-end check.
//!
//!   cargo run -p bedrock-world --example np_dump -- <seed> <y> < points.txt

use bedrock_world::generator::java_biomes::JavaBiomes;
use std::io::{self, BufRead, Write};

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: u64 = args
        .next()
        .expect("usage: np_dump <seed> <y>")
        .parse()
        .expect("seed must be a number");
    let y: i32 = args
        .next()
        .expect("usage: np_dump <seed> <y>")
        .parse()
        .expect("y must be a number");

    let biomes = JavaBiomes::new(seed);
    let stdin = io::stdin();
    let mut out = io::BufWriter::new(io::stdout());
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let mut parts = line.split_whitespace();
        let (Some(x), Some(z)) = (parts.next(), parts.next()) else {
            continue;
        };
        let (Ok(x), Ok(z)) = (x.parse::<i32>(), z.parse::<i32>()) else {
            continue;
        };
        let np = biomes.climate_at(x, y, z);
        writeln!(
            out,
            "{} {} {} {} {} {}",
            np[0], np[1], np[2], np[3], np[4], np[5]
        )
        .ok();
    }
}
