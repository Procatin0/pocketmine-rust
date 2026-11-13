//! Debug density and terrain at distance

use bedrock_world::generator::BDSTerrain;

fn main() {
    let bds = BDSTerrain::new(12345);
    
    println!("=== Terrain & Density at Distance ===\n");
    
    for x in [0, 1000, 2000, 5000, 8000, 10000, 12000, 15000] {
        let continentalness = bds.sample_climate(2, x as f64, 0.0);
        let c = (continentalness + 1.0) * 0.5;
        let height = bds.height_at(x, 0);
        let density_at_sea = bds.density_at(x, 62, 0);
        let density_at_63 = bds.density_at(x, 63, 0);
        let density_at_70 = bds.density_at(x, 70, 0);
        
        println!("X={:>6}: cont_raw={:>7.4}, cont_norm={:.4}, height={:>4}, density@62={:>7.4}, @63={:>7.4}, @70={:>7.4}",
                 x, continentalness, c, height, density_at_sea, density_at_63, density_at_70);
    }
    
    println!("\n=== Scan for land at 10k ===");
    for x in 9000..=11000 {
        if x % 100 == 0 {
            let continentalness = bds.sample_climate(2, x as f64, 0.0);
            let c = (continentalness + 1.0) * 0.5;
            let height = bds.height_at(x, 0);
            if c >= 0.3 && height > 62 {
                println!("X={}: land! c={:.4}, h={}", x, c, height);
            }
        }
    }
}