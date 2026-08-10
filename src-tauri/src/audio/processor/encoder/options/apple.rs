//! Apple AAC (AudioToolbox) encoder options.

use crate::audio::settings_encoder::EncoderSettings;
use ff::Dictionary;
use ffmpeg_next as ff;

/// Apple AAC (AudioToolbox) encoder options.
///
/// Sets bit_rate on context (required for CVBR) and returns Dictionary with aac_at_mode.
pub(in crate::audio::processor::encoder) fn build_apple_options(
    ctx: &mut ff::codec::context::Context,
    settings: &EncoderSettings,
) -> Dictionary<'static> {
    // CVBR requires a target bitrate on the context
    let target_bit_rate = settings.bitrate_kbps as i64 * 1000;
    unsafe {
        (*ctx.as_mut_ptr()).bit_rate = target_bit_rate;
    }

    let mut opts = Dictionary::new();
    opts.set("aac_at_mode", "cvbr");
    // aac_at_quality is an INVERTED 0..=2 scale: FFmpeg maps it onto Apple's
    // codec-quality property as `96 - value * 32` (FFmpeg 9.0
    // libavcodec/audiotoolboxenc.c), so 0 is the highest quality reachable
    // through the wrapper and 2 the lowest. Pin the best value explicitly so
    // it cannot be "raised" to 2 under the intuitive higher-is-better reading.
    opts.set("aac_at_quality", "0");
    log::info!(
        "Apple AAC encoder: mode=cvbr bitrate={}k quality=0 (best; scale inverted)",
        settings.bitrate_kbps
    );

    opts
}
