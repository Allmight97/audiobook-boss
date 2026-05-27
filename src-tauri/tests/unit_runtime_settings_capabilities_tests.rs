//! Unit tests for runtime settings capability facts.
//!
//! These tests live outside the production capability module so the public strip
//! proves the capability contract without inlining test-only code into the
//! Audio Engine implementation file.

use audiobook_boss_lib::audio::{
    encoder_settings_capabilities, validate_encoder_settings, validate_sample_rate_config,
    BitrateMode, ChannelConfig, EncoderSettings, EncoderType, SampleRateConfig, ThreadSetting,
};

#[test]
fn exposed_encoder_capabilities_match_validators() {
    let capabilities = encoder_settings_capabilities(None);

    assert_eq!(
        capabilities.auto_resolution_order,
        vec![
            EncoderType::FdkHeAac,
            EncoderType::AacAt,
            EncoderType::NativeAac,
        ]
    );

    for bitrate in &capabilities.bitrate_kbps_options {
        let settings = EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: *bitrate,
            bitrate_mode: BitrateMode::Vbr(capabilities.vbr_level_default),
            channels: ChannelConfig::Auto,
            afterburner: true,
            threads: ThreadSetting::Auto,
            twoloop: true,
        };
        validate_encoder_settings(&settings).expect("exposed bitrate should validate for FDK VBR");
    }

    for sample_rate in &capabilities.explicit_sample_rates {
        validate_sample_rate_config(&SampleRateConfig::Explicit(*sample_rate))
            .expect("exposed sample rate should validate");
    }
}

#[test]
fn exposed_mode_defaults_validate_for_each_encoder() {
    let capabilities = encoder_settings_capabilities(None);

    for entry in capabilities.bitrate_modes_by_encoder {
        let settings = EncoderSettings {
            encoder_type: entry.encoder_type,
            bitrate_kbps: 64,
            bitrate_mode: entry.default_mode,
            channels: ChannelConfig::Auto,
            afterburner: true,
            threads: ThreadSetting::Fixed(capabilities.thread_fixed_min),
            twoloop: true,
        };

        validate_encoder_settings(&settings)
            .expect("default mode exposed for an encoder should validate");
    }
}
