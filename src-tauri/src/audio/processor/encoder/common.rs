//! Common encoder helpers and utilities.

use super::super::run_diagnostics::{
    append_run_record, unix_timestamp_seconds, with_encoding_log_file,
};
use crate::audio::settings_encoder::{EncoderSettings, EncoderType};
use crate::audio::SampleRateConfig;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use ffmpeg_next as ff;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::time::Duration;

pub(super) fn encoder_log(message: &str) {
    let _ = with_encoding_log_file(|file| writeln!(file, "{message}"));
    if message.starts_with("encoder_config ") || message.starts_with("encoder_effective ") {
        log::info!("{message}");
    } else {
        log::debug!("{message}");
    }
}

pub(crate) struct InProcessEncoderRunLog<'a> {
    pub status: &'a str,
    pub status_detail: Option<&'a str>,
    pub elapsed: Duration,
    pub wallclock_elapsed: Option<Duration>,
    pub encoder_details: Option<&'a str>,
    pub opened_encoder: Option<&'a str>,
    pub opened_rate: Option<u32>,
    pub opened_channels: Option<u32>,
    pub input_facts: &'a [String],
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
    append_run_record(|| format_in_process_encoding_log_entry(entry));
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
    output.push_str("stage=encode_mux\n");
    if let Some(detail) = entry.status_detail {
        let _ = writeln!(output, "status_detail={detail}");
    }
    super::super::run_diagnostics::write_common_run_fields(
        &mut output,
        (entry.elapsed, entry.wallclock_elapsed),
        entry.encoder_settings,
        entry.sample_rate,
        entry.opened_encoder,
        entry.opened_rate,
        entry.opened_channels,
    );
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
    let _ = writeln!(output, "preview={}", entry.preview);
    let _ = writeln!(
        output,
        "temp_output={}",
        sanitize_path_for_display(entry.temp_output)
    );
    let _ = writeln!(output, "inputs={}", entry.input_paths.len());
    for (index, path) in entry.input_paths.iter().enumerate() {
        if let Some(fact) = entry.input_facts.get(index) {
            let _ = writeln!(output, "input[{index}] {fact}");
        } else {
            let _ = writeln!(
                output,
                "input[{index}] file={} codec=unknown rate=unknown channels=unknown",
                sanitize_path_for_display(path)
            );
        }
    }
    if let Some(details) = entry.encoder_details {
        output.push_str(details);
    }
    output.push_str("--- end in-process-encoder run ---\n\n");
    output
}

const AAC_FRAME_QUANTUM_SAMPLES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EncoderFramePlan {
    samples_per_frame: usize,
}

impl EncoderFramePlan {
    pub(crate) fn samples_per_frame(self) -> usize {
        self.samples_per_frame
    }

    pub(super) fn from_raw_frame_size(
        frame_size: usize,
        resolved_encoder: EncoderType,
    ) -> Result<Self> {
        if frame_size > 0 {
            return Ok(Self {
                samples_per_frame: frame_size,
            });
        }

        match resolved_encoder {
            EncoderType::AacAt | EncoderType::NativeAac => Ok(Self {
                samples_per_frame: AAC_FRAME_QUANTUM_SAMPLES,
            }),
            EncoderType::Faac | EncoderType::FdkHeAac | EncoderType::Auto => {
                Err(AppError::General(
                    "Encoder frame plan requires a resolved in-process encoder type.".to_string(),
                ))
            }
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
            native_aac_speed: 0,
            faac_profile: crate::audio::FaacProfile::Auto,
        };
        let sample_rate = SampleRateConfig::Explicit(44_100);
        let inputs = [PathBuf::from("/private/input/Book One.m4b")];
        let mut entry = InProcessEncoderRunLog {
            status: "success",
            status_detail: None,
            elapsed: Duration::from_millis(1200),
            wallclock_elapsed: Some(Duration::from_millis(1250)),
            encoder_details: None,
            opened_encoder: Some("aac_at"),
            opened_rate: Some(44_100),
            opened_channels: Some(2),
            input_facts: &[],
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
        };
        let formatted = format_in_process_encoding_log_entry(&entry);

        assert!(formatted.starts_with("--- in-process-encoder run "));
        assert!(formatted.contains("status=success"));
        assert!(formatted.contains("elapsed_monotonic_ms=1200"));
        assert!(formatted.contains("elapsed_wallclock_ms=1250"));
        assert!(formatted.contains("native_aac_speed=0"));
        assert!(formatted.contains("opened_settings encoder=aac_at rate=44100 channels=2"));
        assert!(formatted.contains("job_id=job-1"));
        assert!(formatted.contains("input[0] file=Book One.m4b"));
        assert!(formatted.contains("temp_output=worker-output.m4b"));
        assert!(formatted.contains("--- end in-process-encoder run ---"));
        assert!(!formatted.contains("/private/input"));
        assert!(!formatted.contains("/private/tmp"));

        entry.status = "failed";
        entry.opened_encoder = None;
        let formatted = format_in_process_encoding_log_entry(&entry);
        assert!(formatted.contains("opened_settings encoder=unknown rate=44100 channels=2"));
    }
}
