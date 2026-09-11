//! Unit tests for runtime settings capability facts.
//!
//! These tests live outside the production capability module so the Public API
//! Strip proves the capability contract without inlining test-only code into the
//! Audio Engine implementation file.

use audiobook_boss_lib::audio::{
    encoder_settings_capabilities, validate_encoder_sample_rate, validate_encoder_settings,
    validate_sample_rate_config, BitrateMode, ChannelConfig, EncoderSettings, EncoderType,
    SampleRateConfig,
};

#[test]
fn exposed_encoder_capabilities_match_validators() {
    let capabilities = encoder_settings_capabilities();

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
    let capabilities = encoder_settings_capabilities();

    for entry in capabilities.encoder_configurations {
        let settings = EncoderSettings {
            encoder_type: entry.encoder_type,
            bitrate_kbps: 64,
            bitrate_mode: entry.default_mode,
            channels: ChannelConfig::Auto,
            afterburner: true,
        };

        validate_encoder_settings(&settings)
            .expect("default mode exposed for an encoder should validate");
    }
}

#[test]
fn faac_capabilities_and_validation_keep_he_rates_and_abr_explicit() {
    let capabilities = encoder_settings_capabilities();
    let faac = capabilities
        .encoder_configurations
        .iter()
        .find(|entry| entry.encoder_type == EncoderType::FaacHeAac)
        .expect("bundled FAAC option");
    assert_eq!(faac.default_mode, BitrateMode::Abr);
    assert_eq!(faac.explicit_sample_rates, vec![32000, 44100, 48000]);
    for rate in &faac.explicit_sample_rates {
        validate_encoder_sample_rate(EncoderType::FaacHeAac, &SampleRateConfig::Explicit(*rate))
            .unwrap();
    }
    assert!(validate_encoder_sample_rate(
        EncoderType::FaacHeAac,
        &SampleRateConfig::Explicit(22050)
    )
    .is_err());
    validate_encoder_sample_rate(EncoderType::NativeAac, &SampleRateConfig::Explicit(22050))
        .unwrap();
    let settings = EncoderSettings {
        encoder_type: EncoderType::FaacHeAac,
        bitrate_mode: BitrateMode::Vbr(3),
        bitrate_kbps: 64,
        channels: ChannelConfig::Mono,
        afterburner: false,
    };
    assert!(
        validate_encoder_settings(&settings).is_err(),
        "FAAC does not accept FDK's VBR scale"
    );
}
