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

    let disable_twoloop = std::env::var("ABB_DISABLE_TWOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
        // FALLBACK[FB-005]: trigger=legacy env var typo still used in local setups
        // observe=test coverage (`respects_twoloop_flag_and_env_override`) + startup logs
        // sunset=2026-03-31 issue=#200
        || std::env::var("ABB_DISABLE_TWOOLOOP")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

    if settings.twoloop && !disable_twoloop {
        // twoloop provides better psychoacoustic analysis
        opts.set("aac_coder", "twoloop");
        log::info!(
            "Native AAC encoder: bitrate={}k coder=twoloop",
            settings.bitrate_kbps
        );
    } else {
        log::info!(
            "Native AAC encoder: bitrate={}k (twoloop disabled)",
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

        // UI off -> no twoloop even if env allows
        s.twoloop = false;
        let opts = build_native_options(&mut ctx, &s);
        assert!(opts.get("aac_coder").is_none());

        // Env override (new name) disables regardless of UI
        s.twoloop = true;
        std::env::set_var("ABB_DISABLE_TWOLOOP", "1");
        let opts = build_native_options(&mut ctx, &s);
        assert!(opts.get("aac_coder").is_none());

        // Backward compatibility: old typo still disables
        std::env::remove_var("ABB_DISABLE_TWOLOOP");
        std::env::set_var("ABB_DISABLE_TWOOLOOP", "1");
        let opts = build_native_options(&mut ctx, &s);
        assert!(opts.get("aac_coder").is_none());
        std::env::remove_var("ABB_DISABLE_TWOOLOOP");
    }
}
