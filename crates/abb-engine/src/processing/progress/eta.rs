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

    pub fn update(&mut self, now: Instant, units_done: f64, units_total: f64) -> Option<f64> {
        if let Some((last_at, last_units)) = self.last_sample {
            if units_done < last_units {
                return self.eta_seconds(last_units, units_total);
            }
            let elapsed = now.saturating_duration_since(last_at).as_secs_f64();
            if elapsed > 0.0 {
                let delta_units = units_done - last_units;
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
#[path = "eta_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "eta_regression_tests.rs"]
mod regression_tests;
