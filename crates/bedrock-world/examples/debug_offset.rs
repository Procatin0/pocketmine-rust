//! Debug terrain offset values

use bedrock_world::generator::BDSTerrain;

fn main() {
    let bds = BDSTerrain::new(12345);
    
    println!("=== Terrain Offset & Density Components ===\n");
    
    for x in [0, 1000, 2000, 5000, 10000] {
        let c = bds.sample_climate(2, x as f64, 0.0);
        let e = bds.sample_climate(3, x as f64, 0.0);
        let w = bds.sample_climate(5, x as f64, 0.0);
        let peaks = -3.0 * ((w.abs() - 0.6666667).abs() - 0.33333334);
        
        let offset = bds.terrain_offset(x, 0);
        
        println!("X={:>6}: c={:>7.4}, e={:>7.4}, w={:>7.4}, peaks={:>7.4}, offset={:>7.4}",
                 x, c, e, w, peaks, offset);
        
        // Density formula breakdown
        let y = 62.0;
        let base = 1.0 - (y * 4.0) / 128.0 - 83.0 / 160.0;
        let density = base + offset;
        println!("         base@62={:.4}, density={:.4}", base, density);
    }
    
    println!("\n=== What offset is needed for land at sea level? ===");
    let y = 62.0;
    let base = 1.0 - (y * 4.0) / 128.0 - 83.0 / 160.0;
    println!("Base density at Y=62: {:.4}", base);
    println!("Need offset > {:.4} for density > 0", -base);
    
    // Scan for max offset
    println!("\n=== Scan for max offset in range ===");
    let mut max_offset = f64::MIN;
    let mut min_offset = f64::MAX;
    let mut max_x = 0;
    
    for x in -5000..=5000 {
        if x % 100 == 0 {
            let offset = bds.terrain_offset(x, 0);
            if offset > max_offset {
                max_offset = offset;
                max_x = x;
            }
            if offset < min_offset {
                min_offset = offset;
            }
        }
    }
    println!("Max offset: {:.4} at X={}", max_offset, max_x);
    println!("Min offset: {:.4}", min_offset);
}