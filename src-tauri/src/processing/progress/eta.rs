//! Wall-clock rate smoothing for backend-authored ETA.
//!
//! Producers feed monotonic progress samples (`units_done` toward
//! `units_total`, any unit) and get back an ETA in seconds once the smoothed
//! rate stabilizes. The UI never estimates; it only formats what the backend
//! authored.

use std::time::Instant;

const EWMA_ALPHA: f64 = 0.3;
const MIN_SAMPLES_FOR_ETA: u32 = 2;
const MIN_RATE_UNITS_PER_SEC: f64 = 1e-9;

#[derive(Debug)]
pub struct EtaEstimator {
    last_sample: Option<(Instant, f64)>,
    smoothed_rate: Option<f64>,
    rate_samples: u32,
}

impl EtaEstimator {
    pub fn new() -> Self {
        Self {
            last_sample: None,
            smoothed_rate: None,
            rate_samples: 0,
        }
    }

    /// Records monotonic progress at `now` and returns the current ETA in
    /// seconds toward `units_total`, or `None` until the rate stabilizes
    /// (fewer than two rate samples, zero elapsed time, or a stalled rate).
    pub fn update(&mut self, now: Instant, units_done: f64, units_total: f64) -> Option<f64> {
        if let Some((last_at, last_units)) = self.last_sample {
            let elapsed = now.saturating_duration_since(last_at).as_secs_f64();
            if elapsed > 0.0 {
                let delta_units = (units_done - last_units).max(0.0);
                let instant_rate = delta_units / elapsed;
                self.smoothed_rate = Some(match self.smoothed_rate {
                    Some(previous) => EWMA_ALPHA * instant_rate + (1.0 - EWMA_ALPHA) * previous,
                    None => instant_rate,
                });
                self.rate_samples += 1;
                self.last_sample = Some((now, units_done));
            }
        } else {
            self.last_sample = Some((now, units_done));
        }

        self.eta_seconds(units_done, units_total)
    }

    fn eta_seconds(&self, units_done: f64, units_total: f64) -> Option<f64> {
        if self.rate_samples < MIN_SAMPLES_FOR_ETA {
            return None;
        }
        let rate = self.smoothed_rate?;
        if rate < MIN_RATE_UNITS_PER_SEC {
            return None;
        }
        Some(((units_total - units_done).max(0.0) / rate).max(0.0))
    }
}

impl Default for EtaEstimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(start: Instant, seconds: f64) -> Instant {
        start + Duration::from_secs_f64(seconds)
    }

    #[test]
    fn suppresses_eta_until_rate_stabilizes() {
        let start = Instant::now();
        let mut eta = EtaEstimator::new();

        assert_eq!(eta.update(at(start, 0.0), 0.0, 100.0), None);
        assert_eq!(eta.update(at(start, 1.0), 10.0, 100.0), None);
        assert!(eta.update(at(start, 2.0), 20.0, 100.0).is_some());
    }

    #[test]
    fn steady_rate_yields_remaining_over_rate() {
        let start = Instant::now();
        let mut eta = EtaEstimator::new();
        eta.update(at(start, 0.0), 0.0, 100.0);
        eta.update(at(start, 1.0), 10.0, 100.0);
        let value = eta
            .update(at(start, 2.0), 20.0, 100.0)
            .expect("steady rate produces an ETA");

        // 80 units left at 10 units/sec.
        assert!((value - 8.0).abs() < 0.25, "eta was {value}");
    }

    #[test]
    fn stall_grows_then_suppresses_eta() {
        let start = Instant::now();
        let mut eta = EtaEstimator::new();
        eta.update(at(start, 0.0), 0.0, 100.0);
        eta.update(at(start, 1.0), 10.0, 100.0);
        let moving = eta.update(at(start, 2.0), 20.0, 100.0).expect("moving eta");

        let stalled = eta
            .update(at(start, 3.0), 20.0, 100.0)
            .expect("stalled eta");
        assert!(stalled > moving, "stall must grow the ETA");

        // A long enough stall decays the smoothed rate to ~zero and suppresses.
        let mut suppressed = Some(stalled);
        for step in 4..80 {
            suppressed = eta.update(at(start, f64::from(step)), 20.0, 100.0);
        }
        match suppressed {
            None => {}
            Some(value) => assert!(
                value > 400.0,
                "deep stall should suppress or blow up: {value}"
            ),
        }
    }

    #[test]
    fn zero_elapsed_samples_do_not_poison_the_rate() {
        let start = Instant::now();
        let mut eta = EtaEstimator::new();
        eta.update(at(start, 0.0), 0.0, 100.0);
        eta.update(at(start, 0.0), 5.0, 100.0);
        eta.update(at(start, 1.0), 10.0, 100.0);
        let value = eta.update(at(start, 2.0), 20.0, 100.0);
        assert!(value.is_some());
    }

    #[test]
    fn overshoot_clamps_to_zero() {
        let start = Instant::now();
        let mut eta = EtaEstimator::new();
        eta.update(at(start, 0.0), 0.0, 100.0);
        eta.update(at(start, 1.0), 60.0, 100.0);
        let value = eta
            .update(at(start, 2.0), 120.0, 100.0)
            .expect("rate is live");
        assert_eq!(value, 0.0);
    }
}
