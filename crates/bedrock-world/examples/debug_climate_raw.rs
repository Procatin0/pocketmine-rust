//! Debug raw climate noise values (before normalization)

use bedrock_world::generator::BDSTerrain;

fn main() {
    let bds = BDSTerrain::new(12345);
    
    println!("=== Raw Climate Noise Analysis (before normalization) ===\n");
    
    // Sample at various distances
    let positions = [
        (0, 0),
        (100, 0),
        (500, 0),
        (1000, 0),
        (2000, 0),
        (5000, 0),
        (10000, 0),
        (20000, 0),
        (50000, 0),
        (100000, 0),
    ];
    
    println!("{:>12} {:>12} | {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
             "X", "Z", "Temp", "Humid", "Cont", "Eros", "Peaks", "Weird");
    println!("{}", "-".repeat(100));
    
    for (x, z) in positions {
        let temp = bds.sample_climate(0, x as f64, z as f64);
        let humidity = bds.sample_climate(1, x as f64, z as f64);
        let continentalness = bds.sample_climate(2, x as f64, z as f64);
        let erosion = bds.sample_climate(3, x as f64, z as f64);
        let peaks = bds.sample_climate(4, x as f64, z as f64);
        let weirdness = bds.sample_climate(5, x as f64, z as f64);
        
        println!("{:>12} {:>12} | {:>10.4} {:>10.4} {:>10.4} {:>10.4} {:>10.4} {:>10.4}",
                 x, z, temp, humidity, continentalness, erosion, peaks, weirdness);
    }
    
    println!("\n=== Large Scale Grid (every 5000 blocks) ===");
    
    let mut ocean_at_distance = std::collections::HashMap::new();
    
    for dist in [1000, 5000, 10000, 20000, 50000, 100000] {
        let mut ocean = 0;
        let mut land = 0;
        for angle in [0.0f64, 0.78, 1.57, 2.35, 3.14, 3.92, 4.71, 5.49] {
            let x = (dist as f64 * angle.cos()) as i32;
            let z = (dist as f64 * angle.sin()) as i32;
            let continentalness = bds.sample_climate(2, x as f64, z as f64);
            let c = (continentalness + 1.0) * 0.5;
            if c < 0.3 {
                ocean += 1;
            } else {
                land += 1;
            }
        }
        ocean_at_distance.insert(dist, (ocean, land));
        println!("Dist {:>6}: Ocean={}, Land={}", dist, ocean, land);
    }
}