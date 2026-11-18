//! Spark-style profiler for the server tick loop.
//!
//! Spark (the Paper/Spigot profiling plugin) answers "what is eating my
//! ticks?" with `/spark profiler`, a ranked table of hot spots. AmberPM has no
//! JVM to sample, so the same question is answered with instrumented sections:
//! every hot subsystem opens a [`scope`], the scope times itself, and while a
//! profiling session is active the profiler aggregates, per section:
//!
//! - **self time** (exclusive: the section minus its instrumented children),
//! - **inclusive time** (the section including its children),
//! - call count and the worst single call.
//!
//! It also keeps every tick duration (for avg/p50/p95/p99/max and the slowest
//! ticks) so a report shows both "the tick is slow" and "this is what was
//! running when it was slow".
//!
//! # Usage
//!
//! ```text
//! /profiler start [seconds]   begin a session (optional auto-stop)
//! /profiler stop              stop and print the report
//! /profiler report            print the report without stopping
//! /profiler status            one line: running, elapsed, ticks
//! /profiler reset             stop and discard samples
//! ```
//!
//! Cost when idle is one relaxed atomic load per scope. When a session is
//! active, each scope pays two `Instant::now()` calls plus a thread-local push;
//! only when the buffer fills does a thread take the global lock, so the hot
//! path stays cheap even with per-entity scopes.
//!
//! Sections are per-thread and nest strictly (LIFO). A guard must not be held
//! across an `.await`: the push and the pop happen on the thread the guard was
//! created on, so a suspended task could resume on another thread and unbalance
//! the stack. All current call sites are synchronous by construction.
//!
//! Samples are split by origin: work timed while the central tick is running
//! (`tick`), and work on every other thread (`net.dispatch` on a network task,
//! async side work, ...). Report percentages and "unattributed" refer to the
//! tick only; off-tick time gets its own labelled table, because mixing it into
//! the denominator would hide tick time that no section measured.
//!
//! The tick loop is async and may resume on a different Tokio worker after an
//! `.await`, so "tick thread" is not a thread property: the synchronous entry
//! points that belong to the tick call [`tick_thread`] for their duration.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// Longest session kept: one hour at 20 TPS.
const MAX_TICK_SAMPLES: usize = 72_000;
/// Cap on distinct section names: a plugin cannot grow the map forever by
/// putting ids in an event name; anything past the cap folds into `other`.
const MAX_SECTIONS: usize = 512;
/// Samples buffered per thread before the global lock is taken.
const FLUSH_EVERY: usize = 64;
/// Slowest ticks listed in the report.
const SLOWEST_TICKS: usize = 5;

/// Accumulated time of one origin (tick thread or off-tick threads).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Bucket {
    /// Times the section was entered.
    pub calls: u64,
    /// Exclusive time: the section minus its instrumented children.
    pub self_ns: u64,
    /// Inclusive time: the section including its children.
    pub incl_ns: u64,
    /// Worst single exclusive sample.
    pub max_self_ns: u64,
    /// Worst single inclusive sample.
    pub max_incl_ns: u64,
}

impl Bucket {
    fn add(&mut self, self_ns: u64, incl_ns: u64) {
        self.calls += 1;
        self.self_ns += self_ns;
        self.incl_ns += incl_ns;
        self.max_self_ns = self.max_self_ns.max(self_ns);
        self.max_incl_ns = self.max_incl_ns.max(incl_ns);
    }

    pub fn avg_self_ms(&self) -> f64 {
        if self.calls == 0 {
            return 0.0;
        }
        self.self_ns as f64 / self.calls as f64 / 1_000_000.0
    }

    pub fn self_ms(&self) -> f64 {
        self.self_ns as f64 / 1_000_000.0
    }

    pub fn max_self_ms(&self) -> f64 {
        self.max_self_ns as f64 / 1_000_000.0
    }
}

/// One section, split by where its samples ran.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Samples taken while the central tick was running.
    pub tick: Bucket,
    /// Samples taken on other threads (packet dispatch, async tasks).
    pub other: Bucket,
}

impl Stats {
    fn add(&mut self, self_ns: u64, incl_ns: u64, on_tick: bool) {
        if on_tick {
            self.tick.add(self_ns, incl_ns);
        } else {
            self.other.add(self_ns, incl_ns);
        }
    }
}

/// One row of the hotspot table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Section name, e.g. `tick.entities`, `entity.zombie`, `plugin.PlayerMoveEvent`.
    pub name: String,
    pub stats: Stats,
}

impl Row {
    /// Exclusive time on the tick thread, in milliseconds.
    pub fn self_ms(&self) -> f64 {
        self.stats.tick.self_ms()
    }

    /// Average exclusive tick time per call, in milliseconds.
    pub fn avg_self_ms(&self) -> f64 {
        self.stats.tick.avg_self_ms()
    }

    pub fn max_self_ms(&self) -> f64 {
        self.stats.tick.max_self_ms()
    }
}

/// One recorded tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickSample {
    /// Central tick number (the same one `SessionManager::tick` returns).
    pub tick: u64,
    pub ns: u64,
}

impl TickSample {
    pub fn ms(&self) -> f64 {
        self.ns as f64 / 1_000_000.0
    }
}

/// Everything the `/profiler` report renders, taken as one consistent copy.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub active: bool,
    /// Wall time since `/profiler start` (frozen at stop).
    pub elapsed: Duration,
    pub ticks: usize,
    /// Sum of every recorded tick duration.
    pub tick_total_ns: u64,
    pub avg_ns: u64,
    pub p50_ns: u64,
    pub p95_ns: u64,
    pub p99_ns: u64,
    pub max_ns: u64,
    /// Wall time measured by top-level scopes on the tick thread. Only this
    /// time counts toward the tick percentages.
    pub tick_profiled_ns: u64,
    /// Wall time measured by top-level scopes on other threads (packet
    /// dispatch, async tasks). Reported separately: it is not tick time.
    pub other_profiled_ns: u64,
    /// Hotspots sorted by exclusive time, descending.
    pub rows: Vec<Row>,
    /// Slowest ticks, descending.
    pub slowest: Vec<TickSample>,
}

impl Snapshot {
    /// Denominator for tick percentages: the larger of the recorded tick time
    /// and the measured tick-thread section time (a session started mid-tick
    /// has section time before its first tick sample).
    ///
    /// Off-tick threads are deliberately excluded: their time is not part of a
    /// tick, so letting it in would hide unattributed tick time.
    pub fn base_ns(&self) -> u64 {
        self.tick_total_ns.max(self.tick_profiled_ns)
    }

    /// Tick time not covered by any tick-thread section (send/await, the Java
    /// tick, uninstrumented paths).
    pub fn unattributed_ns(&self) -> u64 {
        self.tick_total_ns.saturating_sub(self.tick_profiled_ns)
    }
}

/// The profiler state. A session is either running (collecting), stopped
/// (report available) or empty. Data survives `stop` until `reset`/`start`.
pub struct Profiler {
    active: AtomicBool,
    started_at: Mutex<Option<Instant>>,
    stopped_at: Mutex<Option<Instant>>,
    /// Set by `start(Some(limit))`; `record_tick` stops the session when it
    /// passes, so no timer thread is needed.
    deadline: Mutex<Option<Instant>>,
    ticks: Mutex<VecDeque<TickSample>>,
    sections: Mutex<HashMap<String, Stats>>,
    /// Wall time of top-level (parentless) sections on the tick thread.
    tick_roots_ns: AtomicU64,
    /// Wall time of top-level sections on every other thread.
    other_roots_ns: AtomicU64,
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            started_at: Mutex::new(None),
            stopped_at: Mutex::new(None),
            deadline: Mutex::new(None),
            ticks: Mutex::new(VecDeque::with_capacity(256)),
            sections: Mutex::new(HashMap::new()),
            tick_roots_ns: AtomicU64::new(0),
            other_roots_ns: AtomicU64::new(0),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Starts a fresh session. `false` when one is already running.
    pub fn start(&self, limit: Option<Duration>) -> bool {
        if self.active.load(Ordering::SeqCst) {
            return false;
        }
        self.clear();
        let now = Instant::now();
        *self.started_at.lock() = Some(now);
        *self.stopped_at.lock() = None;
        *self.deadline.lock() = limit.map(|limit| now + limit);
        self.active.store(true, Ordering::SeqCst);
        true
    }

    /// Stops collecting and freezes the elapsed time. Keeps the samples.
    pub fn stop(&self) {
        if self.active.swap(false, Ordering::SeqCst) {
            *self.stopped_at.lock() = Some(Instant::now());
            *self.deadline.lock() = None;
        }
    }

    /// Stops collecting and discards every sample.
    pub fn reset(&self) {
        self.active.store(false, Ordering::SeqCst);
        *self.deadline.lock() = None;
        self.clear();
    }

    fn clear(&self) {
        self.ticks.lock().clear();
        self.sections.lock().clear();
        self.tick_roots_ns.store(0, Ordering::Relaxed);
        self.other_roots_ns.store(0, Ordering::Relaxed);
        *self.started_at.lock() = None;
        *self.stopped_at.lock() = None;
    }

    /// Wall time of the session, frozen at `stop`.
    pub fn elapsed(&self) -> Duration {
        let Some(started) = *self.started_at.lock() else {
            return Duration::ZERO;
        };
        match *self.stopped_at.lock() {
            Some(stopped) => stopped.saturating_duration_since(started),
            None => started.elapsed(),
        }
    }

    /// True when a report has anything to show.
    pub fn has_data(&self) -> bool {
        !self.ticks.lock().is_empty() || !self.sections.lock().is_empty()
    }

    /// Records one server tick. Called once per tick from the tick loop, after
    /// the body (so every scope of that tick has already closed).
    pub fn record_tick(&self, tick: u64, elapsed: Duration) {
        // Land whatever this thread still has buffered in the current window.
        flush_thread();
        if !self.is_active() {
            return;
        }
        // Copy the deadline out before calling `stop()`: a `MutexGuard` in an
        // `if let` scrutinee lives for the whole statement, and `stop()` locks
        // the same mutex (parking_lot guards are not reentrant).
        let deadline = *self.deadline.lock();
        if let Some(deadline) = deadline {
            if Instant::now() >= deadline {
                self.stop();
                return;
            }
        }
        let ns = nanos(elapsed);
        let mut ticks = self.ticks.lock();
        ticks.push_back(TickSample { tick, ns });
        while ticks.len() > MAX_TICK_SAMPLES {
            ticks.pop_front();
        }
    }

    /// Merges a thread's buffered samples into the session.
    fn record_sections(&self, samples: Vec<(Cow<'static, str>, u64, u64, bool)>) {
        let mut sections = self.sections.lock();
        for (name, self_ns, incl_ns, on_tick) in samples {
            if let Some(stats) = sections.get_mut(&*name) {
                stats.add(self_ns, incl_ns, on_tick);
                continue;
            }
            if sections.len() >= MAX_SECTIONS {
                sections
                    .entry("other".to_string())
                    .or_default()
                    .add(self_ns, incl_ns, on_tick);
                continue;
            }
            let mut stats = Stats::default();
            stats.add(self_ns, incl_ns, on_tick);
            sections.insert(name.into_owned(), stats);
        }
    }

    fn add_root(&self, ns: u64, on_tick: bool) {
        let counter = if on_tick {
            &self.tick_roots_ns
        } else {
            &self.other_roots_ns
        };
        counter.fetch_add(ns, Ordering::Relaxed);
    }

    /// Copies the current session for rendering.
    pub fn snapshot(&self) -> Snapshot {
        let ticks: Vec<TickSample> = self.ticks.lock().iter().copied().collect();
        let tick_total_ns: u64 = ticks.iter().map(|tick| tick.ns).sum();

        let mut sorted: Vec<u64> = ticks.iter().map(|tick| tick.ns).collect();
        sorted.sort_unstable();
        let avg_ns = if sorted.is_empty() {
            0
        } else {
            tick_total_ns / sorted.len() as u64
        };

        let mut slowest = ticks.clone();
        slowest.sort_by(|left, right| right.ns.cmp(&left.ns));
        slowest.truncate(SLOWEST_TICKS);

        let mut rows: Vec<Row> = self
            .sections
            .lock()
            .iter()
            .map(|(name, stats)| Row {
                name: name.clone(),
                stats: *stats,
            })
            .collect();
        rows.sort_by(|left, right| {
            right
                .stats
                .tick
                .self_ns
                .cmp(&left.stats.tick.self_ns)
                .then_with(|| right.stats.other.self_ns.cmp(&left.stats.other.self_ns))
                .then_with(|| right.stats.tick.incl_ns.cmp(&left.stats.tick.incl_ns))
                .then_with(|| left.name.cmp(&right.name))
        });

        Snapshot {
            active: self.is_active(),
            elapsed: self.elapsed(),
            ticks: ticks.len(),
            tick_total_ns,
            avg_ns,
            p50_ns: percentile(&sorted, 0.50),
            p95_ns: percentile(&sorted, 0.95),
            p99_ns: percentile(&sorted, 0.99),
            max_ns: sorted.last().copied().unwrap_or(0),
            tick_profiled_ns: self.tick_roots_ns.load(Ordering::Relaxed),
            other_profiled_ns: self.other_roots_ns.load(Ordering::Relaxed),
            rows,
            slowest,
        }
    }
}

/// Nearest-rank percentile of an ascending slice.
fn percentile(sorted: &[u64], fraction: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let last = sorted.len() - 1;
    let index = ((last as f64) * fraction).round() as usize;
    sorted[index.min(last)]
}

fn nanos(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}

static PROFILER: OnceLock<Profiler> = OnceLock::new();

/// The process-wide profiler the `/profiler` command and every scope share.
pub fn global() -> &'static Profiler {
    PROFILER.get_or_init(Profiler::new)
}

thread_local! {
    static ON_TICK: Cell<bool> = const { Cell::new(false) };
}

/// True while this thread runs synchronous work of the central tick.
pub fn on_tick_thread() -> bool {
    ON_TICK.with(Cell::get)
}

/// Marks synchronous work as part of the central tick; restores on drop.
///
/// Only for regions without `.await`: the marker is thread-local.
#[must_use = "the marker only applies while the guard is alive"]
pub struct TickThread {
    previous: bool,
}

impl Drop for TickThread {
    fn drop(&mut self) {
        ON_TICK.with(|flag| flag.set(self.previous));
    }
}

/// Marks the calling region as tick work until the returned guard is dropped.
pub fn tick_thread() -> TickThread {
    let previous = ON_TICK.with(|flag| flag.replace(true));
    TickThread { previous }
}

struct Frame {
    name: Cow<'static, str>,
    started: Instant,
    /// Whether the frame was opened while the tick was running.
    on_tick: bool,
    /// Inclusive time of the children already closed under this frame.
    children_ns: u64,
}

thread_local! {
    static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
    static PENDING: RefCell<Vec<(Cow<'static, str>, u64, u64, bool)>> =
        const { RefCell::new(Vec::new()) };
}

/// A timed section. Measures on drop; created by [`scope`]/[`scope_dyn`].
#[must_use = "the section is only measured when the guard is dropped"]
pub struct Scope {
    active: bool,
}

impl Scope {
    const INACTIVE: Self = Self { active: false };
}

impl Drop for Scope {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let Some(frame) = FRAMES.with(|frames| frames.borrow_mut().pop()) else {
            return;
        };
        let ns = nanos(frame.started.elapsed());
        let self_ns = ns.saturating_sub(frame.children_ns);
        let has_parent = FRAMES.with(|frames| {
            let mut stack = frames.borrow_mut();
            match stack.last_mut() {
                Some(parent) => {
                    parent.children_ns = parent.children_ns.saturating_add(ns);
                    true
                }
                None => false,
            }
        });
        if !has_parent {
            global().add_root(ns, frame.on_tick);
        }
        let full = PENDING.with(|pending| {
            let mut pending = pending.borrow_mut();
            pending.push((frame.name, self_ns, ns, frame.on_tick));
            pending.len() >= FLUSH_EVERY
        });
        if full {
            flush_thread();
        }
    }
}

/// Starts a section with a static name. No-op (one atomic load) while no
/// session is running.
pub fn scope(name: &'static str) -> Scope {
    if !global().is_active() {
        return Scope::INACTIVE;
    }
    FRAMES.with(|frames| {
        frames.borrow_mut().push(Frame {
            name: Cow::Borrowed(name),
            started: Instant::now(),
            on_tick: on_tick_thread(),
            children_ns: 0,
        })
    });
    Scope { active: true }
}

/// Starts a section with a name built at runtime (`entity.zombie`,
/// `plugin.PlayerMoveEvent`). The closure only runs while a session is
/// running, so naming a hot loop allocates nothing when idle.
pub fn scope_dyn<F>(name: F) -> Scope
where
    F: FnOnce() -> String,
{
    if !global().is_active() {
        return Scope::INACTIVE;
    }
    let name = name();
    FRAMES.with(|frames| {
        frames.borrow_mut().push(Frame {
            name: Cow::Owned(name),
            started: Instant::now(),
            on_tick: on_tick_thread(),
            children_ns: 0,
        })
    });
    Scope { active: true }
}

/// Pushes this thread's buffered samples into the profiler.
///
/// Called automatically every [`FLUSH_EVERY`] samples and at the end of each
/// tick, so a report never misses more than one tick of samples.
pub fn flush_thread() {
    let samples = PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut()));
    if samples.is_empty() {
        return;
    }
    global().record_sections(samples);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    /// The global profiler is shared by the whole test binary; sessions that
    /// touch it run one at a time.
    static GLOBAL_LOCK: StdMutex<()> = StdMutex::new(());

    fn row<'a>(snapshot: &'a Snapshot, name: &str) -> &'a Row {
        snapshot
            .rows
            .iter()
            .find(|row| row.name == name)
            .unwrap_or_else(|| panic!("no row {name} in {:?}", snapshot.rows))
    }

    #[test]
    fn nested_scopes_split_self_and_inclusive_time() {
        let _serial = GLOBAL_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let profiler = global();
        profiler.reset();
        assert!(profiler.start(None));

        // Private names: the rest of the test binary also ticks instrumented
        // code on other threads while this session is live.
        {
            let _tick = tick_thread();
            let _outer = scope("test.outer");
            std::thread::sleep(Duration::from_millis(20));
            {
                let _inner = scope("test.inner");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        flush_thread();
        profiler.stop();

        let snapshot = profiler.snapshot();
        let outer = row(&snapshot, "test.outer");
        let inner = row(&snapshot, "test.inner");
        assert_eq!(outer.stats.tick.calls, 1);
        assert_eq!(inner.stats.tick.calls, 1);
        // ~20 ms each: the parent's self time excludes the child's 20 ms.
        assert!(
            inner.stats.tick.self_ns >= Duration::from_millis(15).as_nanos() as u64,
            "inner self was {} ns",
            inner.stats.tick.self_ns
        );
        // The parent's self time is its inclusive time minus the child, so the
        // two must add up exactly. (Comparing `outer self < inner self` instead
        // assumed the un-instrumented parts of the parent stay shorter than the
        // child's sleep, which stops being true when parallel tests starve
        // this thread.)
        assert_eq!(
            outer.stats.tick.self_ns + inner.stats.tick.incl_ns,
            outer.stats.tick.incl_ns,
            "outer self {} + inner incl {} should be outer incl {}",
            outer.stats.tick.self_ns,
            inner.stats.tick.incl_ns,
            outer.stats.tick.incl_ns
        );
        assert!(
            outer.stats.tick.incl_ns
                >= outer.stats.tick.self_ns + inner.stats.tick.incl_ns
        );
        assert!(
            snapshot.tick_profiled_ns >= outer.stats.tick.incl_ns,
            "the parentless tick scope counts as tick time"
        );
        profiler.reset();
    }

    #[test]
    fn dynamic_scopes_group_under_their_name() {
        let _serial = GLOBAL_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let profiler = global();
        profiler.reset();
        assert!(profiler.start(None));
        {
            let _tick = tick_thread();
            for _ in 0..3 {
                let _entity = scope_dyn(|| "test.entity.zombie".to_string());
            }
        }
        flush_thread();
        profiler.stop();

        let snapshot = profiler.snapshot();
        assert_eq!(row(&snapshot, "test.entity.zombie").stats.tick.calls, 3);
        profiler.reset();
    }

    #[test]
    fn start_refuses_a_second_session_and_reset_clears() {
        let profiler = Profiler::new();
        assert!(profiler.start(None));
        assert!(!profiler.start(None));
        profiler.record_tick(1, Duration::from_millis(10));
        assert!(profiler.has_data());
        profiler.reset();
        assert!(!profiler.is_active());
        assert!(!profiler.has_data());
        let snapshot = profiler.snapshot();
        assert_eq!(snapshot.ticks, 0);
        assert!(snapshot.rows.is_empty());
        assert!(profiler.start(None), "reset frees the session slot");
        profiler.reset();
    }

    #[test]
    fn tick_percentiles_and_slowest_ticks_are_reported() {
        let profiler = Profiler::new();
        assert!(profiler.start(None));
        for (index, ms) in [10u64, 20, 30, 40, 50].into_iter().enumerate() {
            profiler.record_tick(index as u64, Duration::from_millis(ms));
        }
        let snapshot = profiler.snapshot();
        assert_eq!(snapshot.ticks, 5);
        assert_eq!(snapshot.tick_total_ns, 150_000_000);
        assert_eq!(snapshot.avg_ns, 30_000_000);
        assert_eq!(snapshot.p50_ns, 30_000_000);
        assert_eq!(snapshot.p95_ns, 50_000_000);
        assert_eq!(snapshot.max_ns, 50_000_000);
        assert_eq!(snapshot.slowest[0].tick, 4);
        assert_eq!(snapshot.slowest[0].ns, 50_000_000);
        profiler.reset();
    }

    #[test]
    fn a_zero_limit_stops_the_session_on_the_next_tick() {
        let profiler = Profiler::new();
        assert!(profiler.start(Some(Duration::ZERO)));
        profiler.record_tick(1, Duration::from_millis(10));
        assert!(!profiler.is_active(), "deadline should stop the session");
        assert_eq!(profiler.snapshot().ticks, 0);
    }

    #[test]
    fn sections_fold_into_other_past_the_name_cap() {
        let profiler = Profiler::new();
        assert!(profiler.start(None));
        let samples: Vec<(Cow<'static, str>, u64, u64, bool)> = (0..MAX_SECTIONS + 10)
            .map(|index| {
                (
                    Cow::Owned(format!("section.{index}")),
                    1_000_000,
                    1_000_000,
                    true,
                )
            })
            .collect();
        profiler.record_sections(samples);
        let snapshot = profiler.snapshot();
        // The cap plus the `other` overflow bucket.
        assert!(snapshot.rows.len() <= MAX_SECTIONS + 1);
        assert_eq!(row(&snapshot, "other").stats.tick.calls, 10);
        profiler.reset();
    }

    #[test]
    fn idle_scopes_record_nothing() {
        let _serial = GLOBAL_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let profiler = global();
        profiler.reset();
        assert!(!profiler.is_active());
        {
            let _idle = scope("test.idle.work");
        }
        flush_thread();
        assert!(!profiler.has_data());
        let snapshot = profiler.snapshot();
        assert_eq!(snapshot.base_ns(), 0);
        assert_eq!(snapshot.unattributed_ns(), 0);
    }

    #[test]
    fn tick_and_off_tick_time_are_separated() {
        let _serial = GLOBAL_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let profiler = global();
        profiler.reset();
        assert!(profiler.start(None));

        {
            let _tick = tick_thread();
            let _tick_work = scope("test.sep.tick");
            std::thread::sleep(Duration::from_millis(20));
            {
                // Nested sections inherit the origin of their parent.
                let _nested = scope("test.sep.nested");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        // Same thread, marker dropped: this is packet-dispatch style work.
        {
            let _off = scope("test.sep.off");
            std::thread::sleep(Duration::from_millis(20));
        }
        flush_thread();
        profiler.record_tick(1, Duration::from_millis(50));
        profiler.stop();

        let snapshot = profiler.snapshot();
        let tick_work = row(&snapshot, "test.sep.tick");
        let nested = row(&snapshot, "test.sep.nested");
        let off = row(&snapshot, "test.sep.off");
        let millis = Duration::from_millis(15).as_nanos() as u64;

        // Tick work lands in the tick bucket, off-tick work in `other`.
        assert!(tick_work.stats.tick.incl_ns >= millis);
        assert_eq!(tick_work.stats.other.calls, 0);
        assert_eq!(nested.stats.other.calls, 0);
        assert!(off.stats.other.self_ns >= millis);
        assert_eq!(off.stats.tick.calls, 0);

        // Only tick-thread roots count as tick time...
        assert!(snapshot.tick_profiled_ns >= tick_work.stats.tick.incl_ns);
        assert!(snapshot.other_profiled_ns >= off.stats.other.incl_ns);
        // ...and percentages/unattributed are relative to the tick alone.
        assert_eq!(snapshot.tick_total_ns, 50_000_000);
        assert!(snapshot.base_ns() >= 50_000_000);
        assert_eq!(
            snapshot.unattributed_ns(),
            snapshot
                .tick_total_ns
                .saturating_sub(snapshot.tick_profiled_ns)
        );
        profiler.reset();
    }
}
