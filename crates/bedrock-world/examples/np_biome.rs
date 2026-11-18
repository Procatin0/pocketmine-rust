//! Prints the Java biome id selected for each `t h c e d w` line on stdin.

use std::io::{self, BufRead};

use bedrock_world::generator::java_biome_tree::climate_to_biome;

fn main() {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.expect("stdin");
        let values: Vec<i32> = line
            .split_whitespace()
            .map(|v| v.parse().expect("integer"))
            .collect();
        if values.len() != 6 {
            continue;
        }
        let np: [i32; 6] = values.try_into().unwrap();
        println!("{}", climate_to_biome(&np));
    }
}
