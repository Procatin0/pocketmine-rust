// Prints the Xoroshiro state and the first two next_longs for a seed, to
// compare with the TS xSetSeed/xNextLong derivation.
use bedrock_world::generator::Xoroshiro128Plus;

fn main() {
    let mut xr = Xoroshiro128Plus::from_seed(12345);
    println!("state after from_seed: {}", xr);
    let x_low = xr.next_long();
    let x_high = xr.next_long();
    println!("x_low: {x_low:#x}  x_high: {x_high:#x}");
}
