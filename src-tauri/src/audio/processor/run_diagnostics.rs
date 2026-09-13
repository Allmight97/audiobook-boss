use crate::audio::settings_encoder::EncoderSettings;
use crate::audio::SampleRateConfig;
use std::fmt::Write;
use std::time::{Duration, Instant, SystemTime};

/// Captures both clocks because process timing and user-visible elapsed time
/// answer different questions. A wall-clock failure stays unknown.
pub(crate) struct RunTiming {
    monotonic: Instant,
    wallclock: SystemTime,
}

impl RunTiming {
    pub(crate) fn start() -> Self {
        Self {
            monotonic: Instant::now(),
            wallclock: SystemTime::now(),
        }
    }

    pub(crate) fn elapsed(&self) -> (Duration, Option<Duration>) {
        (self.monotonic.elapsed(), self.wallclock.elapsed().ok())
    }
}

pub(crate) fn write_common_run_fields(
    output: &mut String,
    (monotonic_elapsed, wallclock_elapsed): (Duration, Option<Duration>),
    settings: &EncoderSettings,
    sample_rate: &SampleRateConfig,
    opened_encoder: Option<&str>,
    opened_rate: Option<u32>,
    opened_channels: Option<u32>,
) {
    let _ = writeln!(
        output,
        "elapsed_monotonic_ms={}",
        monotonic_elapsed.as_millis()
    );
    let _ = writeln!(
        output,
        "elapsed_wallclock_ms={}",
        wallclock_elapsed.map_or_else(
            || "unknown".to_string(),
            |value| value.as_millis().to_string()
        )
    );
    let _ = writeln!(
        output,
        "requested_settings encoder={:?} bitrate_mode={:?} bitrate_kbps={} rate={:?} channels={:?} afterburner={} native_aac_speed={}",
        settings.encoder_type,
        settings.bitrate_mode,
        settings.bitrate_kbps,
        sample_rate,
        settings.channels,
        settings.afterburner,
        settings.native_aac_speed
    );
    let _ = writeln!(
        output,
        "opened_settings encoder={} rate={} channels={}",
        opened_encoder.unwrap_or("unknown"),
        opened_rate.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
        opened_channels.map_or_else(|| "unknown".to_string(), |value| value.to_string())
    );
}
