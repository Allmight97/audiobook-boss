//! Backend-owned capability facts for runtime encoder settings.

use super::settings::{encoder_sample_rates, supported_sample_rates};
use super::settings_encoder::{
    all_encoder_types, allowed_bitrate_mode_kinds_for, default_bitrate_mode_for, BitrateMode,
    BitrateModeKind, ChannelConfig, EncoderType, FaacProfile, VALID_VBR_LEVEL_RANGE,
};
use super::toolchain::{detect_encoder_availability, EncoderAvailability};
use serde::{Deserialize, Serialize};

/// Encoder-specific settings facts that cannot be represented by the global
/// controls below. In particular, FAAC's explicit HE-AAC sample-rate support
/// is narrower than the rates accepted by the other encoders.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EncoderConfigurationCapability {
    pub encoder_type: EncoderType,
    pub bitrate_kbps_min: u16,
    pub bitrate_kbps_max: u16,
    pub allowed_modes: Vec<BitrateModeKind>,
    pub default_mode: BitrateMode,
    pub explicit_sample_rates: Vec<u32>,
    pub faac_profiles: Vec<FaacProfileCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FaacProfileCapability {
    pub profile: FaacProfile,
    pub explicit_sample_rates: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EncoderSettingsCapabilities {
    pub availability: EncoderAvailability,
    pub encoder_types: Vec<EncoderType>,
    pub encoder_configurations: Vec<EncoderConfigurationCapability>,
    pub native_speed_max: u8,
    pub faac_quality_presets: Vec<u16>,
    pub faac_quality_default: u16,
    pub vbr_level_min: u16,
    pub vbr_level_max: u16,
    pub vbr_level_default: u16,
    pub sample_rate_auto: bool,
    pub explicit_sample_rates: Vec<u32>,
    pub channel_options: Vec<ChannelConfig>,
}

pub fn encoder_settings_capabilities() -> EncoderSettingsCapabilities {
    EncoderSettingsCapabilities {
        availability: detect_encoder_availability(),
        encoder_types: all_encoder_types().to_vec(),
        encoder_configurations: all_encoder_types()
            .into_iter()
            .map(|encoder_type| EncoderConfigurationCapability {
                encoder_type,
                bitrate_kbps_min: *super::settings_encoder::encoder_bitrate_range(encoder_type)
                    .start(),
                bitrate_kbps_max: *super::settings_encoder::encoder_bitrate_range(encoder_type)
                    .end(),
                allowed_modes: allowed_bitrate_mode_kinds_for(encoder_type).to_vec(),
                default_mode: default_bitrate_mode_for(encoder_type),
                explicit_sample_rates: encoder_sample_rates(encoder_type, FaacProfile::Auto)
                    .to_vec(),
                faac_profiles: if encoder_type == EncoderType::Faac {
                    [FaacProfile::Auto, FaacProfile::AacLc, FaacProfile::HeAacV1]
                        .into_iter()
                        .map(|profile| FaacProfileCapability {
                            profile,
                            explicit_sample_rates: encoder_sample_rates(encoder_type, profile)
                                .to_vec(),
                        })
                        .collect()
                } else {
                    Vec::new()
                },
            })
            .collect(),
        faac_quality_presets: super::settings_encoder::FAAC_QUALITY_PRESETS.to_vec(),
        faac_quality_default: super::settings_encoder::DEFAULT_FAAC_QUALITY,
        native_speed_max: super::settings_encoder::NATIVE_SPEED_MAX,
        vbr_level_min: *VALID_VBR_LEVEL_RANGE.start(),
        vbr_level_max: *VALID_VBR_LEVEL_RANGE.end(),
        vbr_level_default: super::settings_encoder::DEFAULT_VBR_LEVEL,
        sample_rate_auto: true,
        explicit_sample_rates: supported_sample_rates().to_vec(),
        channel_options: vec![
            ChannelConfig::Auto,
            ChannelConfig::Mono,
            ChannelConfig::Stereo,
        ],
    }
}
