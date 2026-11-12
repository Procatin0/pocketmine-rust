use bedrock_world::generator::normal::NormalGenerator;
use bedrock_world::generator::WorldGenerator;

#[test]
fn block_at_does_not_regenerate_the_column() {
    let generator = NormalGenerator::new(1234);

    // Warm the cache for the column under test.
    let _ = generator.block_at(100, 64, 100);

    // `block_at` used to call `generate_chunk_internal` (a 3x3 neighbourhood
    // plus every populator, ~12 ms) on *every* query, so `propagate_connections`
    // turned one block break into hundreds of milliseconds. 200 cached queries
    // must stay far below a single generation.
    let started = std::time::Instant::now();
    for y in 60..70 {
        for x in 96..112 {
            let _ = generator.block_at(x, y, 100);
        }
    }
    let cached = started.elapsed();
    println!("200 cached block_at (one column): {:?}", cached);
    assert!(
        cached < std::time::Duration::from_millis(20),
        "200 cached block_at took {cached:?}; the column cache is not being used"
    );

    // Results still match a fresh generation.
    let fresh = generator.generate_chunk_internal(6, 6);
    assert_eq!(generator.block_at(100, 64, 100), fresh.get_block(4, 64, 4));
    assert_eq!(generator.highest_block_at(100, 100), fresh.get_highest_block_at(4, 4).unwrap_or(-64));
}
