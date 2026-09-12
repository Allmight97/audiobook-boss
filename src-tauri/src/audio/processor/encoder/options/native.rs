//! Native FFmpeg NMR AAC configuration and opened-setting verification.

use crate::audio::settings_encoder::EncoderSettings;
use crate::errors::{AppError, Result};
use ffmpeg_next as ff;
use std::ffi::CStr;

pub(in crate::audio::processor::encoder) fn build_native_options(
    ctx: &mut ff::codec::context::Context,
    settings: &EncoderSettings,
) -> ff::Dictionary<'static> {
    // SAFETY: this uniquely borrowed live context owns these scalar fields.
    unsafe {
        (*ctx.as_mut_ptr()).bit_rate = i64::from(settings.bitrate_kbps) * 1000;
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
    let (flags, bitrate) = unsafe {
        let raw = &*encoder.as_ptr();
        (raw.flags, raw.bit_rate)
    };
    let uses_target = flags & ff::ffi::AV_CODEC_FLAG_QSCALE as i32 == 0;
    if !uses_target || bitrate != i64::from(settings.bitrate_kbps) * 1000 {
        return Err(AppError::General(format!("Native AAC opened target differs from request: target_mode={uses_target} bitrate={bitrate}")));
    }
    log::info!(
        "Native AAC opened: coder=nmr speed={} bitrate={bitrate}",
        settings.native_aac_speed,
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
    use crate::audio::settings_encoder::{BitrateMode, ChannelConfig, EncoderType};

    #[test]
    fn opened_nmr_honors_requested_speed_and_target() {
        ff::init().expect("initialize FFmpeg");
        for (speed, bitrate_kbps) in [(0, 33), (4, 193)] {
            let mut settings = EncoderSettings {
                encoder_type: EncoderType::NativeAac,
                bitrate_kbps,
                bitrate_mode: BitrateMode::Cbr,
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
            settings.native_aac_speed = speed;
            settings.bitrate_kbps += 1;
            assert!(validate_native_options(&encoder, &settings).is_err());
        }
    }

    #[test]
    fn native_rejects_target_that_ffmpeg_would_silently_clamp() {
        let settings = EncoderSettings {
            encoder_type: EncoderType::NativeAac,
            bitrate_kbps: 133,
            bitrate_mode: BitrateMode::Cbr,
            channels: ChannelConfig::Mono,
            afterburner: false,
            native_aac_speed: 0,
        };
        let error = create_audio_encoder(&settings, EncoderType::NativeAac, 22050, 1, true)
            .err()
            .expect("reject a target above the resolved mono ceiling");
        assert!(
            matches!(error, AppError::InvalidInput(message) if message.contains("Native target bitrate exceeds 132 kbps"))
        );
    }
}
