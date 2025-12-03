//! Common encoder helpers and utilities.

use crate::audio::settings_encoder::{
    self, BitrateMode, ChannelConfig as EncoderChannelConfig, EncoderAvailability, EncoderSettings,
    EncoderType, ThreadSetting,
};
use crate::audio::ChannelConfig as LegacyChannelConfig;
use crate::errors::Result;
use ffmpeg_next as ff;
use std::borrow::Cow;
use std::sync::{Once, OnceLock};

pub(super) const DEFAULT_FDK_VBR_LEVEL: u8 = 3;

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

pub(super) fn default_bitrate_mode_for(encoder_type: EncoderType) -> BitrateMode {
    match encoder_type {
        EncoderType::FdkHeAac | EncoderType::Auto => BitrateMode::Vbr(DEFAULT_FDK_VBR_LEVEL),
        EncoderType::AacAt => BitrateMode::Cvbr,
        EncoderType::NativeAac => BitrateMode::Cbr,
    }
}

pub(super) fn legacy_channel_to_encoder(ch: LegacyChannelConfig) -> EncoderChannelConfig {
    match ch {
        LegacyChannelConfig::Mono => EncoderChannelConfig::Mono,
        LegacyChannelConfig::Stereo => EncoderChannelConfig::Stereo,
    }
}

pub(super) fn resolve_plan_encoder_settings<'a>(
    plan: &'a crate::audio::media_pipeline::MediaProcessingPlan,
    availability: &EncoderAvailability,
) -> (Cow<'a, EncoderSettings>, EncoderType) {
    if let Some(settings) = &plan.encoder_settings_v2 {
        let resolved = settings_encoder::resolve_encoder_type(settings, availability);
        (Cow::Borrowed(settings), resolved)
    } else {
        let default_encoder_type = if availability.fdk_available {
            EncoderType::FdkHeAac
        } else if availability.aac_at_available {
            EncoderType::AacAt
        } else {
            EncoderType::NativeAac
        };
        let synthesized = EncoderSettings {
            encoder_type: default_encoder_type,
            bitrate_kbps: plan.settings.bitrate as u16,
            bitrate_mode: default_bitrate_mode_for(default_encoder_type),
            channels: legacy_channel_to_encoder(plan.settings.channels.clone()),
            afterburner: matches!(default_encoder_type, EncoderType::FdkHeAac),
            threads: ThreadSetting::Auto,
        };
        if let Err(err) = settings_encoder::validate_encoder_settings(&synthesized) {
            log::warn!("synthesized encoder settings failed validation: {}", err);
        }
        (Cow::Owned(synthesized), default_encoder_type)
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
            log::debug!(
                "Could not set strict=experimental: FFmpeg error code {}",
                result
            );
        } else {
            log::debug!("Set strict=experimental on encoder context");
        }

        Ok(())
    }
}

/// Resolves target sample rate and channels from plan settings or first input file.
pub(super) fn resolve_target_audio_params(
    plan: &crate::audio::media_pipeline::MediaProcessingPlan,
) -> Result<(u32, i32)> {
    use crate::audio::SampleRateConfig;

    let needs_probe_for_rate = matches!(plan.settings.sample_rate, SampleRateConfig::Auto);
    let needs_probe_for_channels = plan
        .encoder_settings_v2
        .as_ref()
        .map(|enc| matches!(enc.channels, EncoderChannelConfig::Auto))
        .unwrap_or(false);

    let probe = if needs_probe_for_rate || needs_probe_for_channels {
        Some(probe_first_input(plan)?)
    } else {
        None
    };

    let sampled_rate = probe.map(|(rate, _)| rate);
    let sampled_channels = probe.map(|(_, ch)| ch);

    let target_sample_rate = match plan.settings.sample_rate {
        SampleRateConfig::Explicit(rate) => rate,
        SampleRateConfig::Auto => sampled_rate.expect("input probe to provide sample rate"),
    };

    let plan_channel_fallback = plan.settings.channels.channel_count() as i32;
    let target_channels = plan
        .encoder_settings_v2
        .as_ref()
        .and_then(|enc| enc.channels.forced_channels().map(|c| c as i32))
        .or(sampled_channels)
        .unwrap_or(plan_channel_fallback);

    Ok((target_sample_rate, target_channels))
}

pub(super) fn probe_first_input(
    plan: &crate::audio::media_pipeline::MediaProcessingPlan,
) -> Result<(u32, i32)> {
    use crate::errors::AppError;

    let first = plan
        .input_file_paths
        .first()
        .ok_or_else(|| AppError::InvalidInput("No input files provided".to_string()))?;
    let ictx = ff::format::input(&first)
        .map_err(|e| AppError::General(format!("Open input failed: {e}")))?;
    let stream = ictx
        .streams()
        .best(ff::media::Type::Audio)
        .ok_or_else(|| AppError::InvalidInput("No audio stream in first input".to_string()))?;
    let codec_ctx = ff::codec::context::Context::from_parameters(stream.parameters())
        .map_err(|e| AppError::General(format!("Decoder ctx from params failed: {e}")))?;
    let decoder = codec_ctx
        .decoder()
        .audio()
        .map_err(|e| AppError::General(format!("Open audio decoder failed: {e}")))?;
    let channels = decoder.channel_layout().channels() as i32;
    Ok((decoder.rate(), channels.max(1)))
}

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
