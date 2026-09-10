//! Native FFmpeg NMR AAC configuration and opened-setting verification.

use crate::audio::settings_encoder::{BitrateMode, EncoderSettings};
use crate::errors::{AppError, Result};
use ffmpeg_next as ff;
use std::ffi::CStr;

pub(in crate::audio::processor::encoder) fn build_native_options(
    ctx: &mut ff::codec::context::Context,
    settings: &EncoderSettings,
) -> ff::Dictionary<'static> {
    // SAFETY: this uniquely borrowed live context owns these scalar fields.
    unsafe {
        let raw = &mut *ctx.as_mut_ptr();
        match settings.bitrate_mode {
            BitrateMode::NativeVbr(q) => {
                raw.flags |= ff::ffi::AV_CODEC_FLAG_QSCALE as i32;
                raw.global_quality = (q * f64::from(ff::ffi::FF_QP2LAMBDA)).round() as i32;
                // Retain upstream VBR initialization; sticky target kbps must not affect q.
                raw.bit_rate = 0;
            }
            _ => raw.bit_rate = i64::from(settings.bitrate_kbps) * 1000,
        }
    }
    let mut opts = ff::Dictionary::new();
    opts.set("aac_coder", "nmr");
    opts.set("aac_nmr_speed", &settings.native_aac_speed.to_string());
    opts
}

pub(in crate::audio::processor::encoder) fn validate_native_options(
    encoder: &ff::codec::encoder::audio::Encoder,
    settings: &EncoderSettings,
) -> Result<()> {
    // AAC_CODER_NMR = 2 in the pinned libavcodec/aacenc.h.
    for (name, expected) in [
        (c"aac_coder", 2),
        (c"aac_nmr_speed", i64::from(settings.native_aac_speed)),
    ] {
        let actual = native_option(encoder, name)?;
        if actual != expected {
            return Err(AppError::General(format!(
                "NMR AAC requires {}={expected}; opened encoder reports {actual}",
                name.to_string_lossy()
            )));
        }
    }
    // SAFETY: only copy scalar fields while the opened context remains borrowed.
    let (flags, quality, bitrate) = unsafe {
        let raw = &*encoder.as_ptr();
        (raw.flags, raw.global_quality, raw.bit_rate)
    };
    let uses_quality = flags & ff::ffi::AV_CODEC_FLAG_QSCALE as i32 != 0;
    let matches = match settings.bitrate_mode {
        BitrateMode::NativeVbr(q) => {
            uses_quality && quality == (q * f64::from(ff::ffi::FF_QP2LAMBDA)).round() as i32
        }
        _ => !uses_quality && bitrate == i64::from(settings.bitrate_kbps) * 1000,
    };
    if !matches {
        return Err(AppError::General(format!("Native AAC opened rate control differs from request: qscale={uses_quality} quality={quality} bitrate={bitrate}")));
    }
    log::info!(
        "Native AAC opened: coder=nmr speed={} qscale={uses_quality} native_q={} bitrate={bitrate}",
        settings.native_aac_speed,
        f64::from(quality) / f64::from(ff::ffi::FF_QP2LAMBDA)
    );
    Ok(())
}

fn native_option(encoder: &ff::codec::encoder::audio::Encoder, name: &CStr) -> Result<i64> {
    let mut value = 0;
    // SAFETY: the context and NUL-terminated name outlive this read; value is one writable i64.
    let result = unsafe {
        ff::ffi::av_opt_get_int(
            encoder.as_ptr().cast_mut().cast(),
            name.as_ptr(),
            ff::ffi::AV_OPT_SEARCH_CHILDREN,
            &mut value,
        )
    };
    if result < 0 {
        return Err(AppError::General(format!(
            "NMR AAC cannot verify {}: {}",
            name.to_string_lossy(),
            ff::Error::from(result)
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::processor::encoder::context::create_audio_encoder;
    use crate::audio::settings_encoder::{ChannelConfig, EncoderType};

    #[test]
    fn opened_nmr_honors_requested_speed_and_fractional_quality() {
        ff::init().expect("initialize FFmpeg");
        for (speed, mode) in [(0, BitrateMode::Cbr), (4, BitrateMode::NativeVbr(1.25))] {
            let mut settings = EncoderSettings {
                encoder_type: EncoderType::NativeAac,
                bitrate_kbps: 96,
                bitrate_mode: mode,
                channels: ChannelConfig::Stereo,
                afterburner: false,
                native_aac_speed: speed,
            };
            let encoder = create_audio_encoder(&settings, EncoderType::NativeAac, 44100, 2, true)
                .expect("open requested NMR encoder");
            validate_native_options(&encoder, &settings).expect("verify opened NMR settings");
            assert_eq!(
                native_option(&encoder, c"aac_is").expect("read opened psychoacoustic option"),
                1
            );
            assert_eq!(
                native_option(&encoder, c"aac_pns").expect("read opened psychoacoustic option"),
                1
            );
            settings.native_aac_speed = (speed + 1) % 5;
            assert!(validate_native_options(&encoder, &settings).is_err());
        }
    }

    #[test]
    fn native_rejects_target_that_ffmpeg_would_silently_clamp() {
        let settings = EncoderSettings {
            encoder_type: EncoderType::NativeAac,
            bitrate_kbps: 97,
            bitrate_mode: BitrateMode::Cbr,
            channels: ChannelConfig::Mono,
            afterburner: false,
            native_aac_speed: 0,
        };
        assert!(create_audio_encoder(&settings, EncoderType::NativeAac, 16000, 1, true).is_err());
    }
}
