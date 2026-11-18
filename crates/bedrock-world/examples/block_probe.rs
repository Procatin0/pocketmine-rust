//! Prints the canonical ids of the underwater plants, derived from the shipped
//! `data/canonical_block_states.nbt`, so `block_id`'s constants can be rechecked
//! against the table instead of trusted.
//!
//!   cargo run -p bedrock-world --example block_probe

use bedrock_world::block_states::{network_id, state_name, with_property};

fn state(name: &str, key: &str, values: &[&str]) {
    let base = network_id(name).unwrap_or_else(|| panic!("{name} is not in the table"));
    for value in values {
        let id = with_property(base, key, value)
            .unwrap_or_else(|| panic!("{name} has no {key}={value}"));
        println!("{name} {key}={value} -> {id}");
    }
}

fn main() {
    // Any numeric argument is looked up as a network id.
    for arg in std::env::args().skip(1) {
        match arg.parse::<u32>() {
            Ok(id) => println!("{id} -> {:?}", state_name(id)),
            Err(_) => println!("{arg}: no es un id"),
        }
    }
    state("seagrass", "sea_grass_type", &["default", "double_bot", "double_top"]);

    let kelp = network_id("kelp").expect("kelp");
    let ages: Vec<u32> = (0..=25)
        .map(|age| with_property(kelp, "kelp_age", &age.to_string()).expect("kelp_age"))
        .collect();
    println!("kelp ages 0..=25 -> {ages:?}");

    state("sea_pickle", "cluster_count", &["1", "2", "3"]);
}
