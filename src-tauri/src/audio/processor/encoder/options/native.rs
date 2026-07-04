//! Native FFmpeg AAC encoder options.

use crate::audio::settings_encoder::EncoderSettings;
use ff::Dictionary;
use ffmpeg_next as ff;

/// Native FFmpeg AAC encoder options.
///
/// Sets bit_rate on context (required for CBR) and returns a Dictionary with
/// the coder plus speech-tuned psychoacoustic switches.
pub(in crate::audio::processor::encoder) fn build_native_options(
    ctx: &mut ff::codec::context::Context,
    settings: &EncoderSettings,
) -> Dictionary<'static> {
    // CBR requires a target bitrate on the context
    let target_bit_rate = settings.bitrate_kbps as i64 * 1000;
    unsafe {
        (*ctx.as_mut_ptr()).bit_rate = target_bit_rate;
    }

    let mut opts = Dictionary::new();

    // Speech-heavy audiobook content sounds more stable with psychoacoustic
    // substitutions disabled on native AAC (avoids swishy/static artifacts).
    opts.set("aac_is", "0");
    opts.set("aac_pns", "0");

    // twoloop is FFmpeg's default coder and its best psychoacoustic search;
    // pin it explicitly so the choice is visible rather than inherited.
    opts.set("aac_coder", "twoloop");
    log::info!(
        "Native AAC encoder: bitrate={}k coder=twoloop aac_is=0 aac_pns=0",
        settings.bitrate_kbps
    );

    opts
}

// EXCEPTION: tiny helper inline test
#[cfg(test)]
mod tests {
    use super::*;

    fn base_settings() -> EncoderSettings {
        EncoderSettings {
            encoder_type: crate::audio::settings_encoder::EncoderType::NativeAac,
            bitrate_kbps: 64,
            bitrate_mode: crate::audio::settings_encoder::BitrateMode::Cbr,
            channels: crate::audio::settings_encoder::ChannelConfig::Auto,
            afterburner: false,
        }
    }

    #[test]
    fn pins_speech_tuned_options_and_twoloop_coder() {
        let mut ctx = ff::codec::context::Context::new();
        let opts = build_native_options(&mut ctx, &base_settings());

        assert_eq!(opts.get("aac_coder"), Some("twoloop"));
        assert_eq!(opts.get("aac_is"), Some("0"));
        assert_eq!(opts.get("aac_pns"), Some("0"));
    }
}
