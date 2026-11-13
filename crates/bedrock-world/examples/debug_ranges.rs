//! Debug climate noise ranges

use bedrock_world::generator::BDSTerrain;

fn main() {
    let bds = BDSTerrain::new(12345);
    
    println!("=== Climate Noise Ranges (10k samples) ===\n");
    
    let mut ranges = vec![
        ("Temperature", f64::MAX, f64::MIN),
        ("Humidity", f64::MAX, f64::MIN),
        ("Continentalness", f64::MAX, f64::MIN),
        ("Erosion", f64::MAX, f64::MIN),
        ("Peaks", f64::MAX, f64::MIN),
        ("Weirdness", f64::MAX, f64::MIN),
    ];
    
    for x in -5000..=5000 {
        if x % 50 == 0 {
            for z in -5000..=5000 {
                if z % 50 == 0 {
                    let vals = [
                        bds.sample_climate(0, x as f64, z as f64),
                        bds.sample_climate(1, x as f64, z as f64),
                        bds.sample_climate(2, x as f64, z as f64),
                        bds.sample_climate(3, x as f64, z as f64),
                        bds.sample_climate(4, x as f64, z as f64),
                        bds.sample_climate(5, x as f64, z as f64),
                    ];
                    for (i, v) in vals.iter().enumerate() {
                        if *v < ranges[i].1 { ranges[i].1 = *v; }
                        if *v > ranges[i].2 { ranges[i].2 = *v; }
                    }
                }
            }
        }
    }
    
    for (name, min, max) in ranges {
        println!("{:>14}: min={:>8.4}, max={:>8.4}, range={:.4}", name, min, max, max - min);
    }
    
    println!("\n=== Compare with Java reference (from niduy) ===");
    println!("Java climate noises typically produce values in [-1.5, 1.5] or wider");
    println!("Our values should be similar if using correct seeds/params");
}