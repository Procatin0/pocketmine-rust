//! Server tick sampling for `/tps`.
//!
//! PaperMC reports TPS over 1, 5 and 15 minutes; the same three windows are
//! kept here. Every server tick records an [`Instant`], and a window's TPS is
//! the number of ticks inside it divided by its length in seconds, capped at
//! 20 like Spigot/Paper do.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// Longest window kept: 15 minutes.
const MAX_WINDOW: Duration = Duration::from_secs(900);
/// 20 ticks per second for 15 minutes, plus slack for catch-up ticks.
const CAPACITY: usize = 20 * 900 + 64;

/// Rolling tick sampler shared by the tick loop and the command layer.
pub struct TickMetrics {
    ticks: Mutex<VecDeque<Instant>>,
    /// Instant when the first tick was recorded, used to avoid diluting TPS
    /// at startup when fewer than `seconds` of samples exist.
    start_time: Mutex<Option<Instant>>,
}

impl Default for TickMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl TickMetrics {
    pub fn new() -> Self {
        Self {
            ticks: Mutex::new(VecDeque::with_capacity(CAPACITY)),
            start_time: Mutex::new(None),
        }
    }

    /// Records a tick at the current instant.
    pub fn record_tick(&self) {
        self.record_at(Instant::now());
    }

    /// Records a tick at `instant`, dropping samples older than 15 minutes.
    pub fn record_at(&self, instant: Instant) {
        let mut ticks = self.ticks.lock();
        let mut start = self.start_time.lock();
        if start.is_none() {
            *start = Some(instant);
        }
        drop(start);
        ticks.push_back(instant);
        let cutoff = instant.checked_sub(MAX_WINDOW);
        if let Some(cutoff) = cutoff {
            while ticks.front().is_some_and(|tick| *tick < cutoff) {
                ticks.pop_front();
            }
        }
        while ticks.len() > CAPACITY {
            ticks.pop_front();
        }
    }

    /// Average TPS over the last `seconds`, capped at 20.
    ///
    /// Uses the actual elapsed time since the first tick as the denominator
    /// when fewer than `seconds` have passed, so TPS starts at 20 instead of
    /// being diluted by the full window at startup.
    pub fn tps(&self, seconds: u64, now: Instant) -> f64 {
        let ticks = self.ticks.lock();
        let start = self.start_time.lock();
        let start = start.unwrap_or(now);
        let elapsed = now.duration_since(start).as_secs_f64();
        let window = elapsed.min(seconds as f64);
        if window <= 0.0 {
            return 20.0;
        }
        let Some(cutoff) = now.checked_sub(Duration::from_secs(seconds)) else {
            return 20.0;
        };
        let count = ticks.iter().filter(|tick| **tick >= cutoff).count();
        (count as f64 / window).min(20.0)
    }

    /// `(1m, 5m, 15m)` at `now`.
    pub fn snapshot(&self, now: Instant) -> (f64, f64, f64) {
        (
            self.tps(60, now),
            self.tps(300, now),
            self.tps(900, now),
        )
    }

    /// `(1m, 5m, 15m)` right now.
    pub fn current(&self) -> (f64, f64, f64) {
        self.snapshot(Instant::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_over_the_three_windows_and_caps_at_twenty() {
        let metrics = TickMetrics::new();
        let start = Instant::now();

        // 20 ticks per second for a full minute.
        for i in 0..1_200u64 {
            metrics.record_at(start + Duration::from_millis(i * 50));
        }

        let now = start + Duration::from_secs(60);
        let (one, five, fifteen) = metrics.snapshot(now);
        assert!((one - 20.0).abs() < 0.01, "1m was {one}");
        // Elapsed time is 60s, so all windows use 60s as denominator.
        assert!((five - 20.0).abs() < 0.01, "5m was {five}");
        assert!((fifteen - 20.0).abs() < 0.01, "15m was {fifteen}");

        // More than 20 ticks per second still reports 20.
        let busy = TickMetrics::new();
        for i in 0..2_400u64 {
            busy.record_at(start + Duration::from_millis(i * 25));
        }
        assert_eq!(busy.tps(60, now), 20.0);
    }

    #[test]
    fn tps_at_startup_is_not_diluted() {
        let metrics = TickMetrics::new();
        let start = Instant::now();

        // Only 10 ticks in the first second (10 TPS).
        for i in 0..10u64 {
            metrics.record_at(start + Duration::from_millis(i * 100));
        }

        let now = start + Duration::from_secs(1);
        let (one, _, _) = metrics.snapshot(now);
        // Should report ~10 TPS, not 10/60 = 0.17.
        assert!((one - 10.0).abs() < 0.1, "1m at startup was {one}");
    }

    #[test]
    fn a_stalled_server_reports_near_zero() {
        let metrics = TickMetrics::new();
        let start = Instant::now();
        // One tick, then 15 minutes of silence: the 1m and 5m windows hold no
        // tick, and the tick only survives in the 15m window because it sits on
        // its inclusive lower boundary (1/900 TPS).
        metrics.record_at(start);
        let (one, five, fifteen) = metrics.snapshot(start + Duration::from_secs(900));
        assert_eq!(one, 0.0);
        assert_eq!(five, 0.0);
        assert!((fifteen - 1.0 / 900.0).abs() < 1e-9, "15m was {fifteen}");
    }

    #[test]
    fn windows_longer_than_uptime_use_the_uptime_as_divisor() {
        let metrics = TickMetrics::new();
        let start = Instant::now();
        metrics.record_at(start);
        let (one, five, fifteen) = metrics.snapshot(start + Duration::from_secs(60));
        assert!((one - 1.0 / 60.0).abs() < 1e-9, "1m was {one}");
        // Paper does not dilute windows longer than the uptime: 1 tick in 60s
        // is 1 TPS in all three windows, not 1/300 or 1/900.
        assert!((five - 1.0 / 60.0).abs() < 1e-9, "5m was {five}");
        assert!((fifteen - 1.0 / 60.0).abs() < 1e-9, "15m was {fifteen}");
    }
}
