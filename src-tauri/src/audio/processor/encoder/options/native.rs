//! Native FFmpeg NMR AAC encoder options.

use crate::audio::settings_encoder::EncoderSettings;
use crate::errors::{AppError, Result};
use ff::Dictionary;
use ffmpeg_next as ff;
use std::ffi::CStr;

/// Configure the qualified NMR baseline, retaining upstream psychoacoustic defaults.
pub(in crate::audio::processor::encoder) fn build_native_options(
    ctx: &mut ff::codec::context::Context,
    settings: &EncoderSettings,
) -> Dictionary<'static> {
    let target_bit_rate = settings.bitrate_kbps as i64 * 1000;
    unsafe {
        (*ctx.as_mut_ptr()).bit_rate = target_bit_rate;
    }

    let mut opts = Dictionary::new();
    opts.set("aac_coder", "nmr");
    opts.set("aac_nmr_speed", "0");
    opts
}

/// `open_as_with` discards unconsumed options. Read the opened codec's values
/// so an incompatible linked FFmpeg cannot silently use a different native coder.
pub(in crate::audio::processor::encoder) fn validate_native_options(
    encoder: &ff::codec::encoder::audio::Encoder,
) -> Result<()> {
    // AAC_CODER_NMR = 2 in the pinned FFmpeg source's libavcodec/aacenc.h.
    for (name, expected) in [(c"aac_coder", 2), (c"aac_nmr_speed", 0)] {
        let actual = native_option(encoder, name)?;
        if actual != expected {
            return Err(AppError::General(format!(
                "NMR AAC requires {}={expected}, but the opened encoder reports {actual}",
                name.to_string_lossy()
            )));
        }
    }
    Ok(())
}

fn native_option(encoder: &ff::codec::encoder::audio::Encoder, name: &CStr) -> Result<i64> {
    let mut value = 0;
    // SAFETY: the opened encoder and its private AVOptions outlive this call;
    // name is NUL-terminated and FFmpeg writes one i64 into the local value.
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
            "NMR AAC cannot verify required option {}: {}",
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
    fn opened_native_encoder_uses_nmr_baseline_and_rejects_changed_options() {
        ff::init().expect("initialize FFmpeg");
        let settings = EncoderSettings {
            encoder_type: EncoderType::NativeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Cbr,
            channels: ChannelConfig::Stereo,
            afterburner: false,
        };
        let mut encoder = create_audio_encoder(&settings, EncoderType::NativeAac, 44_100, 2, true)
            .expect("open NMR AAC encoder");
        validate_native_options(&encoder).expect("effective NMR baseline");
        assert_eq!(
            native_option(&encoder, c"aac_is").expect("read intensity stereo"),
            1
        );
        assert_eq!(
            native_option(&encoder, c"aac_pns").expect("read perceptual noise substitution"),
            1
        );
        for (name, value, valid) in [
            (c"aac_nmr_speed", 2, false),
            (c"aac_nmr_speed", 0, true),
            (c"aac_coder", 0, false),
        ] {
            let result = unsafe {
                ff::ffi::av_opt_set_int(
                    encoder.as_mut_ptr().cast(),
                    name.as_ptr(),
                    value,
                    ff::ffi::AV_OPT_SEARCH_CHILDREN,
                )
            };
            assert_eq!(result, 0);
            assert_eq!(validate_native_options(&encoder).is_ok(), valid);
        }
    }
}
