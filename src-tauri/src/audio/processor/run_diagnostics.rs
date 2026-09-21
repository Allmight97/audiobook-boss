use crate::audio::settings_encoder::EncoderSettings;
use crate::audio::SampleRateConfig;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::Path;
use std::sync::{Mutex, Once, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

static LOG_TARGET: OnceLock<Option<EncoderLogTarget>> = OnceLock::new();
static TRUNCATE: Once = Once::new();
static ENCODING_LOG_WRITE: Mutex<()> = Mutex::new(());

pub(super) fn encoding_log_enabled() -> bool {
    encoding_log_target().is_some()
}

pub(super) fn append_run_record(format: impl FnOnce() -> String) {
    if !encoding_log_enabled() {
        return;
    }
    let record = format();
    if let Err(error) = with_encoding_log_file(|file| file.write_all(record.as_bytes())) {
        log::warn!("Failed to append encoding diagnostics: {error}");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EncoderLogTarget {
    Shared(std::ffi::OsString),
    Legacy(std::ffi::OsString),
}

impl EncoderLogTarget {
    fn path(&self) -> &Path {
        match self {
            Self::Shared(path) | Self::Legacy(path) => Path::new(path),
        }
    }
}

fn encoding_log_target() -> Option<&'static EncoderLogTarget> {
    LOG_TARGET
        .get_or_init(|| {
            encoder_log_target_from_env(
                std::env::var_os("ABB_ENCODING_LOG"),
                std::env::var_os("ABB_LOG_FILE"),
            )
        })
        .as_ref()
}

pub(super) fn with_encoding_log_file(
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let Some(target) = encoding_log_target() else {
        return Ok(());
    };
    if matches!(target, EncoderLogTarget::Legacy(_)) {
        TRUNCATE.call_once(|| {
            let _ = std::fs::remove_file(target.path());
        });
    }
    if let Some(parent) = target
        .path()
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let _guard = ENCODING_LOG_WRITE.lock().ok();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(target.path())?;
    write(&mut file)
}

pub(super) fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn encoder_log_target_from_env(
    shared_encoding_log: Option<std::ffi::OsString>,
    legacy_log: Option<std::ffi::OsString>,
) -> Option<EncoderLogTarget> {
    shared_encoding_log
        .filter(|path| !path.is_empty())
        .map(EncoderLogTarget::Shared)
        .or_else(|| {
            legacy_log
                .filter(|path| !path.is_empty())
                .map(EncoderLogTarget::Legacy)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_encoding_log_takes_precedence_without_legacy_truncate() {
        assert_eq!(
            encoder_log_target_from_env(
                Some("/tmp/encoding.log".into()),
                Some("/tmp/legacy.log".into()),
            ),
            Some(EncoderLogTarget::Shared("/tmp/encoding.log".into()))
        );
        assert_eq!(
            encoder_log_target_from_env(None, Some("/tmp/legacy.log".into())),
            Some(EncoderLogTarget::Legacy("/tmp/legacy.log".into()))
        );
        assert_eq!(
            encoder_log_target_from_env(Some(std::ffi::OsString::new()), None),
            None
        );
    }
}
