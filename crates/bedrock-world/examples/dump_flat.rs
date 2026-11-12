use bedrock_world::chunk::Chunk;
use bedrock_world::generator::flat::FlatGenerator;
use bedrock_world::WorldGenerator;
fn main() {
    let gen = FlatGenerator::new();
    let (count, payload) = gen.generate_chunk(1, 0);
    println!("subChunkCount {} len {}", count, payload.len());
    println!("head {}", payload[..40].iter().map(|b| format!("{:02x}",b)).collect::<Vec<_>>().join(" "));
    std::fs::write("C:\\Users\\matic\\AppData\\Local\\Temp\\opencode\\flat_payload.bin", &payload).unwrap();
}
