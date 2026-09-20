//! Unit tests for runtime settings capability facts.
//!
//! These tests live outside the production capability module so the Public API
//! Strip proves the capability contract without inlining test-only code into the
//! Audio Engine implementation file.

use audiobook_boss_lib::audio::{
    encoder_settings_capabilities, validate_encoder_settings, validate_sample_rate_config,
    BitrateMode, BitrateModeKind, ChannelConfig, EncoderSettings, EncoderType, SampleRateConfig,
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
            faac_profile: audiobook_boss_lib::audio::FaacProfile::Auto,
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
    let described: Vec<_> = capabilities
        .encoder_configurations
        .iter()
        .map(|entry| entry.encoder_type)
        .collect();
    assert_eq!(described, capabilities.encoder_types);
    let faac = capabilities
        .encoder_configurations
        .iter()
        .find(|entry| entry.encoder_type == EncoderType::Faac)
        .unwrap();
    assert_eq!(
        faac.allowed_modes,
        [BitrateModeKind::Abr, BitrateModeKind::Vbr]
    );
    assert_eq!(faac.default_mode, BitrateMode::Abr);
    assert!(faac.explicit_sample_rates.contains(&22050));
    assert_eq!(faac.faac_profiles.len(), 3);
    let he = faac
        .faac_profiles
        .iter()
        .find(|entry| entry.profile == audiobook_boss_lib::audio::FaacProfile::HeAacV1)
        .unwrap();
    assert_eq!(he.explicit_sample_rates, [32000, 44100, 48000]);
    for quality in &capabilities.faac_quality_presets {
        let mut settings = audiobook_boss_lib::app_settings::EncoderDefaults::default().settings;
        settings.encoder_type = EncoderType::Faac;
        settings.bitrate_mode = BitrateMode::Vbr(*quality);
        validate_encoder_settings(&settings).expect("exposed FAAC quality must be accepted");
    }

    for entry in capabilities.encoder_configurations {
        let settings = EncoderSettings {
            encoder_type: entry.encoder_type,
            bitrate_kbps: 64,
            bitrate_mode: entry.default_mode,
            channels: ChannelConfig::Auto,
            afterburner: true,
            native_aac_speed: 0,
            faac_profile: audiobook_boss_lib::audio::FaacProfile::Auto,
        };

        validate_encoder_settings(&settings)
            .expect("default mode exposed for an encoder should validate");
    }
}
