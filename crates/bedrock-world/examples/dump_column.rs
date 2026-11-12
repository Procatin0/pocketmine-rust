//! Dumps a real generated column for the JavaBridge cross-check.
//!
//! Writes two files for the chunk `(cx, cz)`:
//! - `<prefix>.bin`: the Bedrock network payload exactly as `world.get_chunk`
//!   returns it to plugins.
//! - `<prefix>.txt`: ground truth, one `x y z id` line per non-air block, in
//!   absolute world coordinates.
//!
//! Usage: `cargo run -p bedrock-world --example dump_column -- <cx> <cz> <prefix>`

use std::fmt::Write as _;

use bedrock_world::block_id;
use bedrock_world::generator::normal::NormalGenerator;

fn main() {
    let mut args = std::env::args().skip(1);
    let cx: i32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let cz: i32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let prefix = args
        .next()
        .unwrap_or_else(|| format!("/tmp/column_{cx}_{cz}"));

    let generator = NormalGenerator::new(12345678);
    let chunk = generator.generate_chunk_internal(cx, cz);
    let (subchunks, payload) = chunk.serialize_network_payload(chunk.biomes[7 * 16 + 7]);

    std::fs::write(format!("{prefix}.bin"), &payload).expect("write payload");

    let mut truth = String::new();
    writeln!(truth, "# subchunks {subchunks}").unwrap();
    let mut non_air = 0u32;
    for x in 0..16usize {
        for z in 0..16usize {
            for y in -64..320i32 {
                let id = chunk.get_block(x, y, z);
                if id != 0 && id != block_id::AIR {
                    non_air += 1;
                    writeln!(
                        truth,
                        "{} {} {} {}",
                        cx * 16 + x as i32,
                        y,
                        cz * 16 + z as i32,
                        id
                    )
                    .unwrap();
                }
            }
        }
    }
    std::fs::write(format!("{prefix}.txt"), &truth).expect("write truth");

    println!(
        "chunk ({cx},{cz}): {subchunks} subchunks, {} payload bytes, {non_air} non-air blocks",
        payload.len()
    );
}
