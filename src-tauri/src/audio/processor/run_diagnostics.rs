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
        "requested_settings encoder={:?} bitrate_mode={:?} bitrate_kbps={} rate={:?} channels={:?} afterburner={} native_aac_speed={} fdk_profile={:?}",
        settings.encoder_type,
        settings.bitrate_mode,
        settings.bitrate_kbps,
        sample_rate,
        settings.channels,
        settings.afterburner,
        settings.native_aac_speed,
        settings.fdk_profile
    );
    let _ = writeln!(
        output,
        "opened_settings encoder={} rate={} channels={}",
        opened_encoder.unwrap_or("unknown"),
        opened_rate.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
        opened_channels.map_or_else(|| "unknown".to_string(), |value| value.to_string())
    );
}

/// Observe the finalized staged file, without decoding or changing processing success.
pub(super) fn log_output_observation(context: &crate::processing::ProcessingContext, path: &Path) {
    if !encoding_log_enabled() && !log::log_enabled!(log::Level::Info) {
        return;
    }
    let identity = format!(
        "job_id={} output_path={:?}",
        context.job_id.as_deref().unwrap_or("unscoped"),
        context.output.final_path()
    );
    let record = match observe_output(path) {
        Ok(properties) => format!("audio_output {identity} status=observed {properties}"),
        Err(error) => format!("audio_output {identity} status=unavailable error={error:?}"),
    };
    log::info!("{record}");
    append_run_record(|| format!("{record}\n"));
}

fn observe_output(path: &Path) -> Result<String, ffmpeg_next::Error> {
    use ffmpeg_next as ff;
    ff::init()?;
    let input = ff::format::input(path)?;
    let stream = input
        .streams()
        .best(ff::media::Type::Audio)
        .ok_or(ff::Error::StreamNotFound)?;
    let parameters = stream.parameters();
    // SAFETY: parameters owns the live AVCodecParameters. Copy scalar fields only;
    // no pointer escapes and the input is dropped before publication can move the file.
    let (profile, rate, channels, bitrate) = unsafe {
        let parameters = &*parameters.as_ptr();
        (
            parameters.profile,
            parameters.sample_rate,
            parameters.ch_layout.nb_channels,
            parameters.bit_rate,
        )
    };
    let profile = ff::codec::Profile::from((parameters.id(), profile));
    let duration =
        (input.duration() > 0).then(|| input.duration() as f64 / ff::ffi::AV_TIME_BASE as f64);
    Ok(format!("codec={:?} profile={:?} sample_rate={} channels={} duration_seconds={:?} stream_bitrate_bps={:?} container_bitrate_bps={:?}",
        parameters.id(), profile, rate, channels, duration, (bitrate > 0).then_some(bitrate),
        (input.bit_rate() > 0).then_some(input.bit_rate())))
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
    fn output_observation_reads_media_instead_of_requested_settings() {
        let directory = tempfile::tempdir().expect("fixture directory");
        let path = directory.path().join("observed.wav");
        let data_size = 16_000u32;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&8_000u32.to_le_bytes());
        wav.extend_from_slice(&16_000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        wav.resize(wav.len() + data_size as usize, 0);
        std::fs::write(&path, &wav).expect("write wave");
        let record = observe_output(&path).expect("read media properties");
        assert!(record.contains("sample_rate=8000 channels=1 duration_seconds=Some(1.0)"));
        assert!(record.contains("stream_bitrate_bps=Some(128000)"));
        assert_eq!(std::fs::read(&path).expect("read unchanged media"), wav);
        std::fs::write(&path, b"invalid media").expect("replace fixture");
        assert!(observe_output(&path).is_err());
    }

    #[test]
    fn shared_encoding_log_takes_precedence_over_legacy_target() {
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
