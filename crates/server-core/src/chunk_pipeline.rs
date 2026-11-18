//! Background chunk generation pipeline.
//!
//! The central tick must never pay for terrain generation: a cold column costs
//! tens of milliseconds, and doing that inside [`SessionManager::tick`] turns a
//! 50 ms budget into 100-500 ms spikes (measured: `chunk.generate` was 47% of
//! tick time with one player). This module owns that work:
//!
//! - the tick only *requests* the columns a session will need (nearest-first),
//! - rayon workers call [`WorldState::chunk_payload`], which is also the
//!   payload cache the streaming path reads,
//! - `SessionManager::drive_chunk_requests` builds `LevelChunk` packets only
//!   from columns already in that cache and leaves the rest for a later tick.
//!
//! One queue and one cache: warming and sending cannot disagree about which
//! columns exist, generation can never run on the tick thread, and the tick
//! never waits on a worker.
//!
//! Jobs are deduplicated while in flight and capped by [`MAX_IN_FLIGHT`], so a
//! teleport that orders a whole view area cannot flood the rayon pool or pin
//! an unbounded number of payloads in memory.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, Weak};

use parking_lot::Mutex;

use crate::world_state::WorldState;

/// Columns allowed to be generating at once. A payload is tens of kilobytes,
/// and generation is CPU-bound, so this is a memory/backlog bound rather than
/// a throughput knob.
const MAX_IN_FLIGHT: usize = 64;

/// Threads terrain generation may use: every core but one.
///
/// Generation and the tick share the machine, and a cold column costs 20-150 ms.
/// When generation could take every core, the central tick was descheduled for
/// 200-470 ms at a time: the profiler showed a 3.2 ms median tick with p99 over
/// 200 ms, and sections that only move bytes (`chunk.commit`, `chunk.packet`)
/// reporting 40-170 ms maxima. Reserving a core for the tick (and the packet
/// threads) keeps the 20 TPS clock out of the generator's way; on a single-core
/// machine generation keeps its one core.
fn generation_pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let cores = std::thread::available_parallelism()
            .map(|cores| cores.get())
            .unwrap_or(2);
        rayon::ThreadPoolBuilder::new()
            .num_threads(cores.saturating_sub(1).max(1))
            .thread_name(|index| format!("chunk-gen-{index}"))
            .build()
            .expect("chunk generation pool")
    })
}

/// The shared terrain-generation pool. Both consumers of the generator — the
/// chunk pipeline (Bedrock streaming) and the Java chunk prefetch — run here,
/// so neither can occupy the whole machine.
pub(crate) fn generation_pool_ref() -> &'static rayon::ThreadPool {
    generation_pool()
}

/// The shared generation queue. Cheap to clone through the [`Arc`] the
/// `SessionManager` holds; workers only keep a [`Weak`] reference so a dropped
/// server does not keep the pipeline alive.
pub struct ChunkPipeline {
    world: Arc<WorldState>,
    in_flight: Mutex<HashSet<(i32, i32)>>,
    in_flight_count: AtomicUsize,
}

impl ChunkPipeline {
    pub fn new(world: Arc<WorldState>) -> Arc<Self> {
        Arc::new(Self {
            world,
            in_flight: Mutex::new(HashSet::new()),
            in_flight_count: AtomicUsize::new(0),
        })
    }

    /// Requests generation for `columns` in order. Columns already cached or
    /// already being generated are skipped, so callers can pass the same
    /// lookahead every tick. Returns how many jobs were accepted.
    pub fn request(self: &Arc<Self>, columns: &[(i32, i32)]) -> usize {
        let mut accepted = 0;
        for &column in columns {
            if self.in_flight_count.load(Ordering::Relaxed) >= MAX_IN_FLIGHT {
                break;
            }
            if self.world.has_chunk_payload(column.0, column.1) {
                continue;
            }
            {
                let mut in_flight = self.in_flight.lock();
                if !in_flight.insert(column) {
                    continue;
                }
            }
            self.in_flight_count.fetch_add(1, Ordering::Relaxed);
            accepted += 1;

            let world = Arc::clone(&self.world);
            let pipeline: Weak<Self> = Arc::downgrade(self);
            generation_pool().spawn(move || {
                // Off-tick by construction: this runs on a rayon worker, and
                // `/profiler` reports it under the off-tick table.
                let _scope = crate::profiler::scope("chunk.generate");
                world.chunk_payload(column.0, column.1);
                if let Some(pipeline) = pipeline.upgrade() {
                    pipeline.finish(column);
                }
            });
        }
        accepted
    }

    fn finish(&self, column: (i32, i32)) {
        self.in_flight.lock().remove(&column);
        self.in_flight_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// Columns currently queued or generating.
    pub fn in_flight(&self) -> usize {
        self.in_flight_count.load(Ordering::Relaxed)
    }
}
