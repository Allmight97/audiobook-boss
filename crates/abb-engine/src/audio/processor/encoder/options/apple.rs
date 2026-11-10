use crate::audio::settings_encoder::EncoderSettings;

use super::EncoderOptions;

/// CVBR needs a target bitrate on the context.
///
/// `aac_at_quality` is an inverted 0..=2 scale: FFmpeg maps it onto Apple's
/// codec-quality property as `96 - value * 32` (FFmpeg 9.0
/// libavcodec/audiotoolboxenc.c), so 0 is the highest quality and 2 the
/// lowest.
pub(in crate::audio::processor::encoder) fn build_apple_options(
    settings: &EncoderSettings,
) -> EncoderOptions {
    log::info!(
        "Apple AAC encoder: mode=cvbr bitrate={}k quality=0 (best; scale inverted)",
        settings.bitrate_kbps
    );
    EncoderOptions {
        bit_rate: i64::from(settings.bitrate_kbps) * 1000,
        compression_level: None,
        private: vec![
            ("aac_at_mode", "cvbr".into()),
            ("aac_at_quality", "0".into()),
        ],
    }
}
