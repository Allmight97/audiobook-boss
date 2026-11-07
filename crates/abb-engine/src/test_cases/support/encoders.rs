use crate::audio::{BitrateMode, ChannelConfig, EncoderSettings, EncoderType, FaacProfile};

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

pub(crate) fn stereo_native_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        channels: ChannelConfig::Stereo,
        bitrate_kbps: 320,
        ..native_encoder_settings()
    }
}

pub(crate) fn faac_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::Faac,
        faac_profile: FaacProfile::HeAacV1,
        bitrate_mode: BitrateMode::Abr,
        ..native_encoder_settings()
    }
}

pub(crate) fn opus_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::Opus,
        bitrate_mode: BitrateMode::VbrTarget,
        channels: ChannelConfig::Auto,
        ..native_encoder_settings()
    }
}

pub(crate) fn auto_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::Auto,
        channels: ChannelConfig::Auto,
        ..native_encoder_settings()
    }
}
