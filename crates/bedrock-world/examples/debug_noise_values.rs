// Prints unshifted climate noise values (same points as the TS side).
use bedrock_world::generator::BDSTerrain;

fn main() {
    let bds = BDSTerrain::new(12345);
    let pts = [
        (0.0, 0.0),
        (100.0, 0.0),
        (500.0, 500.0),
        (-1000.0, -1000.0),
        (12345.0, -6789.0),
        (-2048.0, 0.0),
    ];
    for (x, z) in pts {
        let t = bds.sample_climate(0, x, z);
        let h = bds.sample_climate(1, x, z);
        let c = bds.sample_climate(2, x, z);
        let e = bds.sample_climate(3, x, z);
        let p = bds.sample_climate(4, x, z);
        let w = bds.sample_climate(5, x, z);
        println!("({x},{z}) t={t:.6} h={h:.6} c={c:.6} e={e:.6} p={p:.6} w={w:.6}");
    }

    // Surface-biome check points, same as the TS compare script. `biome_at`
    // is the exact per-point niduy sampler; note the chunk fill expands
    // quart cells per column, so it can differ at off-grid columns.
    let biome_pts = [
        (0, 0),
        (100, 0),
        (500, 500),
        (-1000, -1000),
        (3000, -2000),
        (-4520, 8350),
        (12345, -6789),
        (100000, 100000),
        (-100000, 50000),
    ];
    for (x, z) in biome_pts {
        let b = bds.biome_at(x, z);
        println!("PT {x} {z} -> {b}");
    }
}
