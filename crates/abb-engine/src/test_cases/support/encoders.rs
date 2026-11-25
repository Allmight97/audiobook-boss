use abb_media_pipeline::audio::{
    BitrateMode, ChannelConfig, EncoderSettings, EncoderType, FaacProfile,
};

pub(crate) fn native_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: ChannelConfig::Mono,
        native_aac_speed: 0,
        faac_profile: FaacProfile::Auto,
    }
}
