//! Native FFmpeg AAC encoder options.

use crate::audio::settings_encoder::EncoderSettings;
use ff::Dictionary;
use ffmpeg_next as ff;

/// Native FFmpeg AAC encoder options.
///
/// Sets bit_rate on context (required for CBR) and returns Dictionary with optional twoloop coder.
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

    let disable_twoloop = std::env::var("ABB_DISABLE_TWOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    // FFmpeg's default aac_coder is already twoloop (FFmpeg 8.1
    // libavcodec/aacenc.c), so disabling the toggle must select the `fast`
    // coder explicitly; omitting aac_coder would silently keep twoloop and
    // make the UI toggle a no-op.
    if settings.twoloop && !disable_twoloop {
        // twoloop provides better psychoacoustic analysis
        opts.set("aac_coder", "twoloop");
        log::info!(
            "Native AAC encoder: bitrate={}k coder=twoloop aac_is=0 aac_pns=0",
            settings.bitrate_kbps
        );
    } else {
        opts.set("aac_coder", "fast");
        log::info!(
            "Native AAC encoder: bitrate={}k coder=fast aac_is=0 aac_pns=0 (twoloop disabled)",
            settings.bitrate_kbps
        );
    }

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
            threads: crate::audio::settings_encoder::ThreadSetting::Auto,
            twoloop: true,
        }
    }

    #[test]
    fn respects_twoloop_flag_and_env_override() {
        let mut ctx = ff::codec::context::Context::new();
        let mut s = base_settings();

        // UI on, env off -> twoloop set
        std::env::remove_var("ABB_DISABLE_TWOLOOP");
        let opts = build_native_options(&mut ctx, &s);
        assert_eq!(opts.get("aac_coder"), Some("twoloop"));
        assert_eq!(opts.get("aac_is"), Some("0"));
        assert_eq!(opts.get("aac_pns"), Some("0"));

        // UI off -> fast coder selected explicitly (FFmpeg's default coder is
        // already twoloop, so omitting aac_coder would keep twoloop silently)
        s.twoloop = false;
        let opts = build_native_options(&mut ctx, &s);
        assert_eq!(opts.get("aac_coder"), Some("fast"));
        assert_eq!(opts.get("aac_is"), Some("0"));
        assert_eq!(opts.get("aac_pns"), Some("0"));

        // Env override disables regardless of UI
        s.twoloop = true;
        std::env::set_var("ABB_DISABLE_TWOLOOP", "1");
        let opts = build_native_options(&mut ctx, &s);
        assert_eq!(opts.get("aac_coder"), Some("fast"));
        assert_eq!(opts.get("aac_is"), Some("0"));
        assert_eq!(opts.get("aac_pns"), Some("0"));
        std::env::remove_var("ABB_DISABLE_TWOLOOP");
    }
}
