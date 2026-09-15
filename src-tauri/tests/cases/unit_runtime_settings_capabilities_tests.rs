//! Unit tests for runtime settings capability facts.
//!
//! These tests live outside the production capability module so the Public API
//! Strip proves the capability contract without inlining test-only code into the
//! Audio Engine implementation file.

use audiobook_boss_lib::audio::{
    encoder_settings_capabilities, validate_encoder_settings, validate_sample_rate_config,
    BitrateMode, ChannelConfig, EncoderSettings, EncoderType, SampleRateConfig,
};

#[test]
fn exposed_encoder_capabilities_match_validators() {
    let capabilities = encoder_settings_capabilities();

    for bitrate in [capabilities.bitrate_kbps_min, capabilities.bitrate_kbps_max] {
        let settings = EncoderSettings {
            encoder_type: EncoderType::NativeAac,
            bitrate_kbps: bitrate,
            bitrate_mode: BitrateMode::Cbr,
            channels: ChannelConfig::Auto,
            afterburner: false,
            native_aac_speed: capabilities.native_speed_max,
        };
        validate_encoder_settings(&settings)
            .expect("exposed target and speed bounds should pass request validation");
    }

    for sample_rate in &capabilities.explicit_sample_rates {
        validate_sample_rate_config(&SampleRateConfig::Explicit(*sample_rate))
            .expect("exposed sample rate should validate");
    }
}

#[test]
fn exposed_mode_defaults_validate_for_each_encoder() {
    let capabilities = encoder_settings_capabilities();

    for entry in capabilities.bitrate_modes_by_encoder {
        let settings = EncoderSettings {
            encoder_type: entry.encoder_type,
            bitrate_kbps: 64,
            bitrate_mode: entry.default_mode,
            channels: ChannelConfig::Auto,
            afterburner: true,
            native_aac_speed: 0,
        };

        validate_encoder_settings(&settings)
            .expect("default mode exposed for an encoder should validate");
    }
}
