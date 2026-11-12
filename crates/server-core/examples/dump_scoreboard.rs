use server_core::session::SessionManager;

fn main() {
    let manager = SessionManager::new();
    let count = manager.load_scoreboards();
    println!("Loaded scoreboards: {}", count);
    let board = manager.get_scoreboard("procatin0");
    println!("Board for procatin0: {:?}", board);

    let frames = manager.take_scoreboard_java_frames("procatin0", 775);
    println!("Frames count: {}", frames.len());
    for (i, frame) in frames.iter().enumerate() {
        let (id, payload, _) = java_bridge::wire::try_split_frame(frame).expect("valid frame");
        println!("Frame {}: id=0x{:02x} ({}) len={}", i, id, id, payload.len());
        println!("  Payload hex: {}", payload.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "));
    }
}
