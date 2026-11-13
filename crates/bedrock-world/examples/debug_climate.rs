//! Debug climate noise values

use bedrock_world::generator::BDSTerrain;

fn main() {
    let bds = BDSTerrain::new(12345);
    
    println!("=== Climate Noise Analysis ===\n");
    
    // Sample at various distances
    let positions = [
        (0, 0),
        (100, 0),
        (500, 0),
        (1000, 0),
        (2000, 0),
        (5000, 0),
        (10000, 0),
        (0, 100),
        (0, 500),
        (0, 1000),
        (100, 100),
        (500, 500),
        (1000, 1000),
        (-100, -100),
        (-500, -500),
        (-1000, -1000),
    ];
    
    println!("{:>10} {:>10} | {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} | {:>8} {:>8}",
             "X", "Z", "Temp", "Humid", "Cont", "Eros", "Peaks", "Weird", "Biome", "Height");
    println!("{}", "-".repeat(120));
    
    for (x, z) in positions {
        let temp = bds.sample_climate(0, x as f64, z as f64);
        let humidity = bds.sample_climate(1, x as f64, z as f64);
        let continentalness = bds.sample_climate(2, x as f64, z as f64);
        let erosion = bds.sample_climate(3, x as f64, z as f64);
        let peaks = bds.sample_climate(4, x as f64, z as f64);
        let weirdness = bds.sample_climate(5, x as f64, z as f64);
        
        // Normalize to [0,1] like climate_to_biome does
        let c = (continentalness + 1.0) * 0.5;
        let t = (temp + 1.0) * 0.5;
        let h = (humidity + 1.0) * 0.5;
        let e = (erosion + 1.0) * 0.5;
        let p = (peaks + 1.0) * 0.5;
        
        // Determine biome
        let biome = if c < 0.1 { "DEEP_OCN" }
        else if c < 0.2 { "OCEAN" }
        else if c < 0.3 { "COAST" }
        else if t < 0.15 {
            if h > 0.6 { "ICE" } else if h > 0.3 { "TAIGA" } else { "HILLS" }
        } else if t < 0.5 {
            if h > 0.7 { "FOREST" } else if h > 0.4 { if p > 0.6 { "HILLS" } else { "PLAINS" } }
            else if h > 0.2 { "PLAINS" } else { "DESERT" }
        } else {
            if h > 0.7 { "FOREST" } else if h > 0.4 { if p > 0.7 { "HILLS" } else { "PLAINS" } }
            else if h > 0.2 { "DESERT" } else if c > 0.7 && e < 0.3 { "BADLANDS" } else { "DESERT" }
        };
        
        let height = bds.height_at(x, z);
        
        println!("{:>10} {:>10} | {:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>8.4} | {:>8} {:>8}",
                 x, z, t, h, c, e, p, (weirdness+1.0)*0.5, biome, height);
    }
    
    println!("\n=== Statistics ===");
    
    // Sample a grid to see distribution
    let mut land_count = 0;
    let mut ocean_count = 0;
    let mut cont_sum = 0.0;
    let mut cont_min = f64::MAX;
    let mut cont_max = f64::MIN;
    
    for x in -200..=200 {
        for z in -200..=200 {
            if x % 20 == 0 && z % 20 == 0 {
                let continentalness = bds.sample_climate(2, x as f64, z as f64);
                let c = (continentalness + 1.0) * 0.5;
                cont_sum += c;
                cont_min = cont_min.min(c);
                cont_max = cont_max.max(c);
                if c < 0.3 {
                    ocean_count += 1;
                } else {
                    land_count += 1;
                }
            }
        }
    }
    
    println!("Samples: {}", land_count + ocean_count);
    println!("Land (c>=0.3): {}", land_count);
    println!("Ocean (c<0.3): {}", ocean_count);
    println!("Continentalness avg: {:.4}, min: {:.4}, max: {:.4}", cont_sum / (land_count + ocean_count) as f64, cont_min, cont_max);
}