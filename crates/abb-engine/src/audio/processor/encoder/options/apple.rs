use crate::audio::settings_encoder::EncoderSettings;

use super::EncoderOptions;

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
