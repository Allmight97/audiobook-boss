use super::*;
use crate::audio::{
    validate_encoder_settings, validate_sample_rate_config, EncoderSettings, SampleRateConfig,
};

#[test]
fn exposed_encoder_capabilities_match_validators() {
    let capabilities = encoder_settings_capabilities();

    for configuration in &capabilities.encoder_configurations {
        for bitrate in [
            configuration.bitrate_kbps_min,
            configuration.bitrate_kbps_max,
        ] {
            let settings = EncoderSettings {
                encoder_type: configuration.encoder_type,
                bitrate_kbps: bitrate,
                bitrate_mode: configuration.default_mode,
                channels: ChannelConfig::Auto,
                native_aac_speed: capabilities.native_speed_max,
                faac_profile: crate::audio::FaacProfile::Auto,
            };
            validate_encoder_settings(&settings)
                .expect("exposed target and speed bounds should pass request validation");
        }
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
    for quality in &capabilities.faac_quality_presets {
        let mut settings = crate::app_settings::EncoderDefaults::default().settings;
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
            native_aac_speed: 0,
            faac_profile: crate::audio::FaacProfile::Auto,
        };

        validate_encoder_settings(&settings)
            .expect("default mode exposed for an encoder should validate");
    }
}
