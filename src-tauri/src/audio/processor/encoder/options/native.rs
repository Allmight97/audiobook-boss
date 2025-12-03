//! Native FFmpeg AAC encoder options.

use crate::audio::settings_encoder::EncoderSettings;
use ff::Dictionary;
use ffmpeg_next as ff;

/// Native FFmpeg AAC encoder options.
///
/// Sets bit_rate on context (required for CBR) and returns Dictionary with optional twoloop coder.
pub(crate) fn build_native_options(
    ctx: &mut ff::codec::context::Context,
    settings: &EncoderSettings,
) -> Dictionary<'static> {
    // CBR requires a target bitrate on the context
    let target_bit_rate = settings.bitrate_kbps as i64 * 1000;
    unsafe {
        (*ctx.as_mut_ptr()).bit_rate = target_bit_rate;
    }

    let mut opts = Dictionary::new();

    let disable_twoloop = std::env::var("ABB_DISABLE_TWOOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if !disable_twoloop {
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
