//! Backend-owned capability facts for runtime encoder settings.

use super::settings::supported_sample_rates;
use super::settings_encoder::{
    all_encoder_types, allowed_bitrate_mode_kinds_for, auto_encoder_resolution_order,
    default_bitrate_mode_for, BitrateMode, BitrateModeKind, ChannelConfig, EncoderType,
    VALID_VBR_LEVEL_RANGE,
};
use super::toolchain::{detect_encoder_availability, EncoderAvailability};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EncoderBitrateModeCapability {
    pub encoder_type: EncoderType,
    pub allowed_modes: Vec<BitrateModeKind>,
    pub default_mode: BitrateMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EncoderSettingsCapabilities {
    pub availability: EncoderAvailability,
    pub encoder_types: Vec<EncoderType>,
    pub auto_resolution_order: Vec<EncoderType>,
    pub bitrate_kbps_min: u16,
    pub bitrate_modes_by_encoder: Vec<EncoderBitrateModeCapability>,
    pub bitrate_kbps_max: u16,
    #[specta(type = specta_typescript::Number)]
    pub native_quality_min: f64,
    #[specta(type = specta_typescript::Number)]
    pub native_quality_max: f64,
    #[specta(type = specta_typescript::Number)]
    pub native_quality_default: f64,
    pub native_speed_max: u8,
    pub vbr_level_min: u8,
    pub vbr_level_max: u8,
    pub vbr_level_default: u8,
    pub sample_rate_auto: bool,
    pub explicit_sample_rates: Vec<u32>,
    pub channel_options: Vec<ChannelConfig>,
}

pub fn encoder_settings_capabilities() -> EncoderSettingsCapabilities {
    EncoderSettingsCapabilities {
        availability: detect_encoder_availability(),
        encoder_types: all_encoder_types().to_vec(),
        auto_resolution_order: auto_encoder_resolution_order().to_vec(),
        bitrate_kbps_min: 1,
        bitrate_modes_by_encoder: all_encoder_types()
            .into_iter()
            .map(|encoder_type| EncoderBitrateModeCapability {
                encoder_type,
                allowed_modes: allowed_bitrate_mode_kinds_for(encoder_type).to_vec(),
                default_mode: default_bitrate_mode_for(encoder_type),
            })
            .collect(),
        bitrate_kbps_max: super::settings_encoder::MAX_ENCODER_BITRATE,
        native_quality_min: super::settings_encoder::NATIVE_Q_MIN,
        native_quality_max: super::settings_encoder::NATIVE_Q_MAX,
        native_quality_default: super::settings_encoder::NATIVE_Q_DEFAULT,
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
