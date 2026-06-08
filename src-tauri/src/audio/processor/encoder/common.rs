//! Common encoder helpers and utilities.

use crate::audio::settings_encoder::{self, EncoderSettings, EncoderType, ThreadSetting};
use crate::audio::toolchain::EncoderAvailability;
use crate::errors::{AppError, Result};
use ffmpeg_next as ff;
use std::borrow::Cow;
use std::sync::{Once, OnceLock};

pub(super) fn encoder_log(message: &str) {
    static LOG_TARGET: OnceLock<Option<EncoderLogTarget>> = OnceLock::new();
    static TRUNCATE: Once = Once::new();

    let target = LOG_TARGET.get_or_init(|| {
        encoder_log_target_from_env(
            std::env::var("ABB_ENCODING_LOG").ok(),
            std::env::var("ABB_LOG_FILE").ok(),
        )
    });
    if let Some(target) = target {
        if matches!(target, EncoderLogTarget::Legacy(_)) {
            TRUNCATE.call_once(|| {
                let _ = std::fs::remove_file(target.path());
            });
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(target.path())
        {
            use std::io::Write;
            let _ = writeln!(file, "{}", message);
        }
    }

    log::debug!("{}", message);
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
            EncoderType::FdkHeAac | EncoderType::AacAt | EncoderType::NativeAac => Ok(Self {
                samples_per_frame: AAC_FRAME_QUANTUM_SAMPLES,
            }),
            EncoderType::Auto => Err(AppError::General(
                "Encoder frame plan requires a resolved encoder type.".to_string(),
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

pub(super) fn configure_threads(ctx: &mut ff::codec::context::Context, threads: ThreadSetting) {
    let threads_value = match threads {
        ThreadSetting::Auto => 0,
        ThreadSetting::Off => 1,
        ThreadSetting::Fixed(n) => n as i32,
    };
    if threads_value > 0 {
        unsafe {
            use std::ffi::CString;
            let av_ctx = ctx.as_mut_ptr();
            let key = CString::new("threads").expect("threads key should be valid");
            let _ = ffmpeg_next::sys::av_opt_set_int(
                av_ctx as *mut std::ffi::c_void,
                key.as_ptr(),
                threads_value as i64,
                0,
            );
        }
    }
}

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
        for encoder in [
            EncoderType::NativeAac,
            EncoderType::AacAt,
            EncoderType::FdkHeAac,
        ] {
            let plan = EncoderFramePlan::from_raw_frame_size(0, encoder)
                .expect("resolved AAC encoder should have an explicit frame quantum");

            assert_eq!(plan.samples_per_frame(), AAC_FRAME_QUANTUM_SAMPLES);
        }
    }

    #[test]
    fn frame_plan_rejects_unresolved_auto_encoder() {
        let err = EncoderFramePlan::from_raw_frame_size(0, EncoderType::Auto)
            .expect_err("auto must be resolved before frame planning");

        assert!(err.to_string().contains("resolved encoder type"));
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
}
