//! Encoder settings and validation for advanced audio encoding options
//!
//! This module defines the new encoder configuration types and validation
//! logic for Phase 1 of the Encoder v2 implementation. It provides:
//! - Encoder type selection (AAC-AT, HE-AAC v1/v2)
//! - Advanced options (AAC coder, afterburner, threading)  
//! - Validation of invalid combinations (e.g., HE-AAC v2 requires stereo)

use serde::{Deserialize, Serialize};
use crate::errors::{AppError, Result};

/// Audio encoder type selection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncoderType {
    /// Apple AudioToolbox encoder (macOS native, high quality at low bitrates)
    AacAt,
    /// HE-AAC v1 (native FFmpeg, supports mono/stereo)
    HeAacV1,
    /// HE-AAC v2 (native FFmpeg, stereo-only with Parametric Stereo)
    HeAacV2,
}

/// AAC coder algorithm selection (native encoders only)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AacCoder {
    /// Twoloop coder (higher quality, slower encoding)
    #[default]
    Twoloop,
    /// Fast coder (faster encoding, lower quality)
    Fast,
}

/// Threading configuration for encoder
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThreadSetting {
    /// Automatic thread detection
    #[default]
    Auto,
    /// Disable threading (single-threaded)
    Off,
    /// Fixed number of threads
    Fixed(u16),
}

/// Advanced encoder settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderSettings {
    /// Encoder type selection
    pub encoder_type: EncoderType,
    /// Target bitrate in kbps (must be from allowed list)
    pub bitrate_kbps: u32,
    /// Number of audio channels
    pub channels: u16,
    /// AAC coder selection (ignored for AacAt)
    pub aac_coder: Option<AacCoder>,
    /// Enable afterburner processing (ignored for AacAt)
    pub afterburner: Option<bool>,
    /// Threading configuration
    pub threads: ThreadSetting,
}

#[allow(clippy::derivable_impls)]
impl Default for EncoderType {
    fn default() -> Self {
        // Platform-specific default: macOS prefers AAC-AT for quality
        #[cfg(target_os = "macos")]
        {
            EncoderType::AacAt
        }
        #[cfg(not(target_os = "macos"))]
        {
            EncoderType::HeAacV1
        }
    }
}

impl Default for EncoderSettings {
    fn default() -> Self {
        Self {
            encoder_type: EncoderType::default(),
            bitrate_kbps: 64, // Common audiobook quality
            channels: 1,     // Mono default for audiobooks
            aac_coder: Some(AacCoder::default()),
            afterburner: Some(true), // Quality over speed by default
            threads: ThreadSetting::default(),
        }
    }
}

/// Valid bitrate values in kbps for encoder settings
pub const ALLOWED_BITRATES: &[u32] = &[56, 64, 72, 80, 88, 96];

/// Validates encoder settings for invalid combinations and constraints
pub fn validate_encoder_settings(settings: &EncoderSettings) -> Result<()> {
    validate_bitrate(settings.bitrate_kbps)?;
    validate_channels(settings.channels)?;
    validate_encoder_channel_combination(&settings.encoder_type, settings.channels)?;
    validate_thread_setting(&settings.threads)?;
    Ok(())
}

/// Validates bitrate is in the allowed list
fn validate_bitrate(bitrate_kbps: u32) -> Result<()> {
    if !ALLOWED_BITRATES.contains(&bitrate_kbps) {
        return Err(AppError::InvalidInput(
            format!(
                "Bitrate must be one of {:?} kbps, got: {} kbps",
                ALLOWED_BITRATES, bitrate_kbps
            )
        ));
    }
    Ok(())
}

/// Validates channel count is reasonable
fn validate_channels(channels: u16) -> Result<()> {
    if !(1..=2).contains(&channels) {
        return Err(AppError::InvalidInput(
            format!("Channels must be 1 (mono) or 2 (stereo), got: {}", channels)
        ));
    }
    Ok(())
}

/// Validates encoder type and channel combination constraints
fn validate_encoder_channel_combination(encoder_type: &EncoderType, channels: u16) -> Result<()> {
    match encoder_type {
        EncoderType::HeAacV2 => {
            if channels != 2 {
                return Err(AppError::InvalidInput(
                    "HE-AAC v2 requires exactly 2 channels (stereo) due to Parametric Stereo technology".to_string()
                ));
            }
        }
        EncoderType::AacAt | EncoderType::HeAacV1 => {
            // These encoders support both mono and stereo
        }
    }
    Ok(())
}

/// Validates thread setting bounds
fn validate_thread_setting(threads: &ThreadSetting) -> Result<()> {
    if let ThreadSetting::Fixed(count) = threads {
        if *count == 0 {
            return Err(AppError::InvalidInput(
                "Thread count must be at least 1 when using Fixed threading".to_string()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_type_default() {
        let default_type = EncoderType::default();
        
        #[cfg(target_os = "macos")]
        assert_eq!(default_type, EncoderType::AacAt);
        
        #[cfg(not(target_os = "macos"))]
        assert_eq!(default_type, EncoderType::HeAacV1);
    }

    #[test]
    fn test_encoder_settings_default() {
        let settings = EncoderSettings::default();
        assert_eq!(settings.bitrate_kbps, 64);
        assert_eq!(settings.channels, 1);
        assert_eq!(settings.aac_coder, Some(AacCoder::Twoloop));
        assert_eq!(settings.afterburner, Some(true));
        assert_eq!(settings.threads, ThreadSetting::Auto);
    }

    #[test]
    fn test_validate_bitrate_valid() {
        for &bitrate in ALLOWED_BITRATES {
            assert!(validate_bitrate(bitrate).is_ok(), "Bitrate {} should be valid", bitrate);
        }
    }

    #[test]
    fn test_validate_bitrate_invalid() {
        let invalid_bitrates = [32, 48, 100, 128, 192];
        for bitrate in invalid_bitrates {
            assert!(validate_bitrate(bitrate).is_err(), "Bitrate {} should be invalid", bitrate);
        }
    }

    #[test]
    fn test_validate_channels_valid() {
        assert!(validate_channels(1).is_ok());
        assert!(validate_channels(2).is_ok());
    }

    #[test]
    fn test_validate_channels_invalid() {
        assert!(validate_channels(0).is_err());
        assert!(validate_channels(3).is_err());
        assert!(validate_channels(8).is_err());
    }

    #[test]
    fn test_validate_he_aac_v2_requires_stereo() {
        // HE-AAC v2 with mono should fail
        let result = validate_encoder_channel_combination(&EncoderType::HeAacV2, 1);
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("HE-AAC v2 requires exactly 2 channels"));

        // HE-AAC v2 with stereo should pass
        assert!(validate_encoder_channel_combination(&EncoderType::HeAacV2, 2).is_ok());
    }

    #[test]
    fn test_validate_other_encoders_allow_mono_and_stereo() {
        for encoder in [EncoderType::AacAt, EncoderType::HeAacV1] {
            assert!(validate_encoder_channel_combination(&encoder, 1).is_ok());
            assert!(validate_encoder_channel_combination(&encoder, 2).is_ok());
        }
    }

    #[test]
    fn test_validate_thread_setting() {
        assert!(validate_thread_setting(&ThreadSetting::Auto).is_ok());
        assert!(validate_thread_setting(&ThreadSetting::Off).is_ok());
        assert!(validate_thread_setting(&ThreadSetting::Fixed(1)).is_ok());
        assert!(validate_thread_setting(&ThreadSetting::Fixed(4)).is_ok());
        assert!(validate_thread_setting(&ThreadSetting::Fixed(16)).is_ok());

        // Zero threads should be invalid
        assert!(validate_thread_setting(&ThreadSetting::Fixed(0)).is_err());
    }

    #[test]
    fn test_validate_encoder_settings_comprehensive() {
        // Valid settings
        let valid_settings = EncoderSettings {
            encoder_type: EncoderType::HeAacV1,
            bitrate_kbps: 64,
            channels: 1,
            aac_coder: Some(AacCoder::Twoloop),
            afterburner: Some(true),
            threads: ThreadSetting::Auto,
        };
        assert!(validate_encoder_settings(&valid_settings).is_ok());

        // Invalid bitrate
        let mut invalid_settings = valid_settings.clone();
        invalid_settings.bitrate_kbps = 100;
        assert!(validate_encoder_settings(&invalid_settings).is_err());

        // Invalid channel count
        let mut invalid_settings = valid_settings.clone();
        invalid_settings.channels = 0;
        assert!(validate_encoder_settings(&invalid_settings).is_err());

        // HE-AAC v2 with mono channels
        let mut invalid_settings = valid_settings.clone();
        invalid_settings.encoder_type = EncoderType::HeAacV2;
        invalid_settings.channels = 1;
        assert!(validate_encoder_settings(&invalid_settings).is_err());

        // Invalid thread setting
        let mut invalid_settings = valid_settings.clone();
        invalid_settings.threads = ThreadSetting::Fixed(0);
        assert!(validate_encoder_settings(&invalid_settings).is_err());
    }

    #[test]
    fn test_serde_round_trip_encoder_type() {
        for encoder_type in [EncoderType::AacAt, EncoderType::HeAacV1, EncoderType::HeAacV2] {
            let json = serde_json::to_string(&encoder_type).expect("serialize");
            let deserialized: EncoderType = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(encoder_type, deserialized);
        }
    }

    #[test]
    fn test_serde_round_trip_aac_coder() {
        for coder in [AacCoder::Twoloop, AacCoder::Fast] {
            let json = serde_json::to_string(&coder).expect("serialize");
            let deserialized: AacCoder = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(coder, deserialized);
        }
    }

    #[test]
    fn test_serde_round_trip_thread_setting() {
        for setting in [ThreadSetting::Auto, ThreadSetting::Off, ThreadSetting::Fixed(4)] {
            let json = serde_json::to_string(&setting).expect("serialize");
            let deserialized: ThreadSetting = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(setting, deserialized);
        }
    }

    #[test]
    fn test_serde_round_trip_encoder_settings() {
        let settings = EncoderSettings {
            encoder_type: EncoderType::HeAacV1,
            bitrate_kbps: 80,
            channels: 2,
            aac_coder: Some(AacCoder::Fast),
            afterburner: Some(false),
            threads: ThreadSetting::Fixed(8),
        };

        let json = serde_json::to_string(&settings).expect("serialize");
        let deserialized: EncoderSettings = serde_json::from_str(&json).expect("deserialize");
        
        assert_eq!(settings.encoder_type, deserialized.encoder_type);
        assert_eq!(settings.bitrate_kbps, deserialized.bitrate_kbps);
        assert_eq!(settings.channels, deserialized.channels);
        assert_eq!(settings.aac_coder, deserialized.aac_coder);
        assert_eq!(settings.afterburner, deserialized.afterburner);
        assert_eq!(settings.threads, deserialized.threads);
    }
}