//! Prints statistics of `base_3d_noise` over a small grid.

use bedrock_world::generator::java_blended_noise::BlendedNoise;

fn main() {
    let noise = BlendedNoise::base_3d_noise();
    let mut values = Vec::new();
    for x in (-64..64).step_by(8) {
        for z in (-64..64).step_by(8) {
            for y in (-64..320).step_by(16) {
                values.push(noise.sample(x as f64, y as f64, z as f64));
            }
        }
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "n={} mean {:+.3} sd {:.3} min {:+.3} p10 {:+.3} med {:+.3} p90 {:+.3} max {:+.3}",
        n,
        mean,
        var.sqrt(),
        values[0],
        values[values.len() / 10],
        values[values.len() / 2],
        values[values.len() * 9 / 10],
        values[values.len() - 1]
    );
}
