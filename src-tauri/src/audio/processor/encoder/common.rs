//! Common encoder helpers and utilities.

use crate::audio::settings_encoder::{self, EncoderSettings, EncoderType};
use crate::audio::toolchain::EncoderAvailability;
use crate::audio::SampleRateConfig;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use ffmpeg_next as ff;
use std::borrow::Cow;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::Path;
use std::sync::{Mutex, Once, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static LOG_TARGET: OnceLock<Option<EncoderLogTarget>> = OnceLock::new();
static TRUNCATE: Once = Once::new();
static ENCODING_LOG_WRITE: Mutex<()> = Mutex::new(());

pub(super) fn encoder_log(message: &str) {
    let _ = with_encoding_log_file(|file| writeln!(file, "{message}"));
    log::debug!("{}", message);
}

pub(crate) struct InProcessEncoderRunLog<'a> {
    pub status: &'a str,
    pub status_detail: Option<&'a str>,
    pub elapsed: Duration,
    pub resolved_encoder: EncoderType,
    pub encoder_settings: &'a EncoderSettings,
    pub sample_rate: &'a SampleRateConfig,
    pub session_id: String,
    pub job_id: Option<&'a str>,
    pub input_index: Option<usize>,
    pub operation_kind: String,
    pub preview: bool,
    pub temp_output: &'a Path,
    pub input_paths: &'a [std::path::PathBuf],
    pub target_duration_seconds: f64,
}

pub(crate) fn append_in_process_encoding_log_best_effort(entry: &InProcessEncoderRunLog<'_>) {
    if encoding_log_target().is_none() {
        return;
    }
    if let Err(error) = with_encoding_log_file(|file| {
        file.write_all(format_in_process_encoding_log_entry(entry).as_bytes())
    }) {
        log::warn!("Failed to append in-process encoding diagnostics: {error}");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EncoderLogTarget {
    Shared(String),
    Legacy(String),
}

impl EncoderLogTarget {
    fn path(&self) -> &str {
        match self {
            Self::Shared(path) | Self::Legacy(path) => path,
        }
    }
}

fn encoding_log_target() -> Option<&'static EncoderLogTarget> {
    LOG_TARGET
        .get_or_init(|| {
            encoder_log_target_from_env(
                std::env::var("ABB_ENCODING_LOG").ok(),
                std::env::var("ABB_LOG_FILE").ok(),
            )
        })
        .as_ref()
}

fn with_encoding_log_file(
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
    if let Some(parent) = std::path::Path::new(target.path())
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

fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn format_in_process_encoding_log_entry(entry: &InProcessEncoderRunLog<'_>) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "--- in-process-encoder run {} ---",
        unix_timestamp_seconds()
    );
    let _ = writeln!(
        output,
        "run_id={}",
        std::env::var("ABB_RUN_ID").unwrap_or_else(|_| "unscoped".to_string())
    );
    let _ = writeln!(output, "status={}", entry.status);
    if let Some(detail) = entry.status_detail {
        let _ = writeln!(output, "status_detail={detail}");
    }
    let _ = writeln!(output, "elapsed_ms={}", entry.elapsed.as_millis());
    let _ = writeln!(
        output,
        "target_duration_seconds={:.3}",
        entry.target_duration_seconds
    );
    let _ = writeln!(output, "session_id={}", entry.session_id);
    if let Some(job_id) = entry.job_id {
        let _ = writeln!(output, "job_id={job_id}");
    }
    if let Some(input_index) = entry.input_index {
        let _ = writeln!(output, "input_index={input_index}");
    }
    let _ = writeln!(output, "operation_kind={}", entry.operation_kind);
    let _ = writeln!(output, "encoder={}", entry.resolved_encoder);
    let _ = writeln!(
        output,
        "requested_encoder={}",
        entry.encoder_settings.encoder_type
    );
    let _ = writeln!(output, "preview={}", entry.preview);
    let _ = writeln!(
        output,
        "encoder_settings encoder_type={:?} bitrate_mode={:?} bitrate_kbps={} channels={:?} sample_rate={:?} afterburner={}",
        entry.encoder_settings.encoder_type,
        entry.encoder_settings.bitrate_mode,
        entry.encoder_settings.bitrate_kbps,
        entry.encoder_settings.channels,
        entry.sample_rate,
        entry.encoder_settings.afterburner
    );
    let _ = writeln!(
        output,
        "temp_output={}",
        sanitize_path_for_display(entry.temp_output)
    );
    let _ = writeln!(output, "inputs={}", entry.input_paths.len());
    for (index, path) in entry.input_paths.iter().enumerate() {
        let _ = writeln!(
            output,
            "input[{index}] file={}",
            sanitize_path_for_display(path)
        );
    }
    output.push_str("--- end in-process-encoder run ---\n\n");
    output
}

fn encoder_log_target_from_env(
    shared_encoding_log: Option<String>,
    legacy_log: Option<String>,
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

pub(super) fn resolve_plan_encoder_settings<'a>(
    plan: &'a crate::audio::processor::plan::MediaProcessingPlan,
    availability: &EncoderAvailability,
) -> (Cow<'a, EncoderSettings>, EncoderType) {
    let resolved = settings_encoder::resolve_encoder_type(&plan.encoder_settings, availability);
    (Cow::Borrowed(&plan.encoder_settings), resolved)
}

const AAC_FRAME_QUANTUM_SAMPLES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EncoderFramePlan {
    samples_per_frame: usize,
}

impl EncoderFramePlan {
    pub(crate) fn from_opened_encoder(
        encoder: &ff::codec::encoder::audio::Encoder,
        resolved_encoder: EncoderType,
    ) -> Result<Self> {
        Self::from_raw_frame_size(encoder.frame_size() as usize, resolved_encoder)
    }

    pub(crate) fn samples_per_frame(self) -> usize {
        self.samples_per_frame
    }

    fn from_raw_frame_size(frame_size: usize, resolved_encoder: EncoderType) -> Result<Self> {
        if frame_size > 0 {
            return Ok(Self {
                samples_per_frame: frame_size,
            });
        }

        match resolved_encoder {
            EncoderType::AacAt | EncoderType::NativeAac => Ok(Self {
                samples_per_frame: AAC_FRAME_QUANTUM_SAMPLES,
            }),
            EncoderType::FdkHeAac | EncoderType::Auto => Err(AppError::General(
                "Encoder frame plan requires a resolved in-process encoder type.".to_string(),
            )),
        }
    }
}

/// Finds encoder by name using FFmpeg's encoder registry
pub(super) fn find_encoder_by_name(name: &str) -> Result<ff::Codec> {
    use crate::errors::AppError;
    use std::ffi::CString;

    unsafe {
        let c_name = CString::new(name)
            .map_err(|e| AppError::General(format!("Invalid encoder name '{}': {}", name, e)))?;

        let codec_ptr = ffmpeg_next::sys::avcodec_find_encoder_by_name(c_name.as_ptr());
        if codec_ptr.is_null() {
            return Err(AppError::General(format!("Encoder '{}' not found", name)));
        }

        Ok(ff::Codec::wrap(codec_ptr))
    }
}

/// Attempts to configure AAC encoder for variable frame sizes.
pub(super) fn try_configure_variable_frame_size(
    encoder_ctx: &mut ff::codec::context::Context,
) -> Result<()> {
    use crate::errors::AppError;
    use std::ffi::CString;

    unsafe {
        let av_ctx = encoder_ctx.as_mut_ptr();
        if av_ctx.is_null() {
            return Err(AppError::General(
                "Invalid encoder context pointer".to_string(),
            ));
        }

        let strict_key = CString::new("strict")
            .map_err(|e| AppError::General(format!("Failed to create strict key string: {}", e)))?;
        let experimental_value = CString::new("experimental").map_err(|e| {
            AppError::General(format!("Failed to create experimental value string: {}", e))
        })?;

        let result = ffmpeg_next::sys::av_opt_set(
            av_ctx as *mut std::ffi::c_void,
            strict_key.as_ptr(),
            experimental_value.as_ptr(),
            0,
        );

        if result < 0 {
            log::warn!(
                "strict=experimental not applied (FFmpeg error code {}); continuing with encoder defaults",
                result
            );
        } else {
            log::debug!("Set strict=experimental on encoder context");
        }

        Ok(())
    }
}

// Target audio params now resolved via engine::resolve_target_audio_params

#[cfg(test)]
// EXCEPTION: tiny private frame-plan invariant tests; keeping them inline avoids widening the production API for test access.
mod tests {
    use super::*;

    #[test]
    fn frame_plan_uses_reported_encoder_frame_size() {
        let plan = EncoderFramePlan::from_raw_frame_size(2048, EncoderType::NativeAac)
            .expect("reported frame size should be accepted");

        assert_eq!(plan.samples_per_frame(), 2048);
    }

    #[test]
    fn frame_plan_uses_aac_quantum_for_variable_frame_encoders() {
        for encoder in [EncoderType::NativeAac, EncoderType::AacAt] {
            let plan = EncoderFramePlan::from_raw_frame_size(0, encoder)
                .expect("resolved AAC encoder should have an explicit frame quantum");

            assert_eq!(plan.samples_per_frame(), AAC_FRAME_QUANTUM_SAMPLES);
        }
    }

    #[test]
    fn frame_plan_rejects_non_in_process_encoders() {
        for encoder in [EncoderType::Auto, EncoderType::FdkHeAac] {
            let err = EncoderFramePlan::from_raw_frame_size(0, encoder)
                .expect_err("only resolved in-process encoders get a frame plan");

            assert!(err.to_string().contains("in-process encoder type"));
        }
    }

    #[test]
    fn shared_encoding_log_takes_precedence_without_legacy_truncate() {
        assert_eq!(
            encoder_log_target_from_env(
                Some("/tmp/encoding.log".to_string()),
                Some("/tmp/legacy.log".to_string()),
            ),
            Some(EncoderLogTarget::Shared("/tmp/encoding.log".to_string()))
        );
        assert_eq!(
            encoder_log_target_from_env(None, Some("/tmp/legacy.log".to_string())),
            Some(EncoderLogTarget::Legacy("/tmp/legacy.log".to_string()))
        );
        assert_eq!(encoder_log_target_from_env(Some(String::new()), None), None);
    }

    #[test]
    fn in_process_log_entry_sanitizes_paths_and_names_the_encoder() {
        use crate::audio::{BitrateMode, ChannelConfig, EncoderSettings, SampleRateConfig};
        use std::path::PathBuf;
        use std::time::Duration;

        let encoder_settings = EncoderSettings {
            encoder_type: EncoderType::AacAt,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Cbr,
            channels: ChannelConfig::Stereo,
            afterburner: false,
        };
        let sample_rate = SampleRateConfig::Explicit(44_100);
        let inputs = [PathBuf::from("/private/input/Book One.m4b")];
        let formatted = format_in_process_encoding_log_entry(&InProcessEncoderRunLog {
            status: "success",
            status_detail: None,
            elapsed: Duration::from_millis(1200),
            resolved_encoder: EncoderType::AacAt,
            encoder_settings: &encoder_settings,
            sample_rate: &sample_rate,
            session_id: "session-1".to_string(),
            job_id: Some("job-1"),
            input_index: Some(0),
            operation_kind: "ProcessingBatch".to_string(),
            preview: false,
            temp_output: Path::new("/private/tmp/worker-output.m4b"),
            input_paths: &inputs,
            target_duration_seconds: 12.5,
        });

        assert!(formatted.starts_with("--- in-process-encoder run "));
        assert!(formatted.contains("status=success"));
        assert!(formatted.contains("encoder=aac_at"));
        assert!(formatted.contains("requested_encoder=aac_at"));
        assert!(formatted.contains("job_id=job-1"));
        assert!(formatted.contains("input[0] file=Book One.m4b"));
        assert!(formatted.contains("temp_output=worker-output.m4b"));
        assert!(formatted.contains("--- end in-process-encoder run ---"));
        assert!(!formatted.contains("/private/input"));
        assert!(!formatted.contains("/private/tmp"));
    }
}
