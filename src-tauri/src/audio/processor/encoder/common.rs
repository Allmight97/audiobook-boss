//! Common encoder helpers and utilities.

use crate::audio::settings_encoder::{self, EncoderSettings, EncoderType, ThreadSetting};
use crate::audio::toolchain::EncoderAvailability;
use crate::errors::Result;
use ffmpeg_next as ff;
use std::borrow::Cow;
use std::sync::{Once, OnceLock};

pub(super) fn encoder_log(message: &str) {
    static LOG_PATH: OnceLock<Option<String>> = OnceLock::new();
    static TRUNCATE: Once = Once::new();

    let path = LOG_PATH.get_or_init(|| std::env::var("ABB_LOG_FILE").ok());
    if let Some(p) = path {
        TRUNCATE.call_once(|| {
            let _ = std::fs::remove_file(p);
        });
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
        {
            use std::io::Write;
            let _ = writeln!(file, "{}", message);
        }
    }

    log::debug!("{}", message);
}

pub(super) fn resolve_plan_encoder_settings<'a>(
    plan: &'a crate::audio::processor::MediaProcessingPlan,
    availability: &EncoderAvailability,
) -> (Cow<'a, EncoderSettings>, EncoderType) {
    let resolved = settings_encoder::resolve_encoder_type(&plan.encoder_settings, availability);
    (Cow::Borrowed(&plan.encoder_settings), resolved)
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
