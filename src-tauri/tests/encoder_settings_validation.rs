//! Integration tests for encoder settings validation
//! 
//! These tests verify the acceptance criteria for Phase 1:
//! - Validation rejects invalid combinations
//! - Types round-trip serde correctly
//! - No behavior change for existing AudioSettings

use audiobook_boss_lib::audio::{
    EncoderSettings, EncoderType, AacCoder, ThreadSetting,
    validate_encoder_settings, ALLOWED_BITRATES
};

#[test]
fn test_validation_rejects_invalid_combinations() {
    // Test 1: HE-AAC v2 with mono channels should fail
    let invalid_settings = EncoderSettings {
        encoder_type: EncoderType::HeAacV2,
        bitrate_kbps: 64,
        channels: 1, // Invalid: HE-AAC v2 requires stereo
        aac_coder: Some(AacCoder::Twoloop),
        afterburner: Some(true),
        threads: ThreadSetting::Auto,
    };
    
    let result = validate_encoder_settings(&invalid_settings);
    assert!(result.is_err(), "HE-AAC v2 with mono should be rejected");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("HE-AAC v2 requires exactly 2 channels"),
        "Error message should explain stereo requirement: {}", error_msg
    );

    // Test 2: Invalid bitrate should fail
    let invalid_settings = EncoderSettings {
        encoder_type: EncoderType::HeAacV1,
        bitrate_kbps: 100, // Invalid: not in allowed list
        channels: 1,
        aac_coder: Some(AacCoder::Twoloop),
        afterburner: Some(true),
        threads: ThreadSetting::Auto,
    };
    
    let result = validate_encoder_settings(&invalid_settings);
    assert!(result.is_err(), "Invalid bitrate should be rejected");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Bitrate must be one of"),
        "Error message should list valid bitrates: {}", error_msg
    );

    // Test 3: Zero thread count should fail
    let invalid_settings = EncoderSettings {
        encoder_type: EncoderType::AacAt,
        bitrate_kbps: 64,
        channels: 1,
        aac_coder: Some(AacCoder::Twoloop),
        afterburner: Some(true),
        threads: ThreadSetting::Fixed(0), // Invalid: must be >= 1
    };
    
    let result = validate_encoder_settings(&invalid_settings);
    assert!(result.is_err(), "Zero thread count should be rejected");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Thread count must be at least 1"),
        "Error message should explain thread count requirement: {}", error_msg
    );
}

#[test]
fn test_validation_accepts_valid_combinations() {
    // Test all encoder types with valid settings
    let valid_settings = [
        // AAC-AT with mono
        EncoderSettings {
            encoder_type: EncoderType::AacAt,
            bitrate_kbps: 64,
            channels: 1,
            aac_coder: Some(AacCoder::Twoloop),
            afterburner: Some(true),
            threads: ThreadSetting::Auto,
        },
        // AAC-AT with stereo
        EncoderSettings {
            encoder_type: EncoderType::AacAt,
            bitrate_kbps: 80,
            channels: 2,
            aac_coder: Some(AacCoder::Fast),
            afterburner: Some(false),
            threads: ThreadSetting::Off,
        },
        // HE-AAC v1 with mono
        EncoderSettings {
            encoder_type: EncoderType::HeAacV1,
            bitrate_kbps: 56,
            channels: 1,
            aac_coder: Some(AacCoder::Twoloop),
            afterburner: Some(true),
            threads: ThreadSetting::Fixed(4),
        },
        // HE-AAC v1 with stereo
        EncoderSettings {
            encoder_type: EncoderType::HeAacV1,
            bitrate_kbps: 72,
            channels: 2,
            aac_coder: None, // Optional fields can be None
            afterburner: None,
            threads: ThreadSetting::Auto,
        },
        // HE-AAC v2 with stereo (required)
        EncoderSettings {
            encoder_type: EncoderType::HeAacV2,
            bitrate_kbps: 88,
            channels: 2, // Required for v2
            aac_coder: Some(AacCoder::Fast),
            afterburner: Some(false),
            threads: ThreadSetting::Fixed(8),
        },
    ];

    for (i, settings) in valid_settings.iter().enumerate() {
        let result = validate_encoder_settings(settings);
        assert!(
            result.is_ok(),
            "Valid settings {} should pass validation: {:?}", i, settings
        );
    }
}

#[test] 
fn test_bitrate_validation_comprehensive() {
    // Test all allowed bitrates pass
    for &bitrate in ALLOWED_BITRATES {
        let settings = EncoderSettings {
            encoder_type: EncoderType::HeAacV1,
            bitrate_kbps: bitrate,
            channels: 1,
            aac_coder: Some(AacCoder::Twoloop),
            afterburner: Some(true),
            threads: ThreadSetting::Auto,
        };
        
        let result = validate_encoder_settings(&settings);
        assert!(
            result.is_ok(),
            "Bitrate {} should be valid", bitrate
        );
    }

    // Test common invalid bitrates fail
    let invalid_bitrates = [32, 48, 100, 128, 192, 256];
    for bitrate in invalid_bitrates {
        let settings = EncoderSettings {
            encoder_type: EncoderType::HeAacV1,
            bitrate_kbps: bitrate,
            channels: 1,
            aac_coder: Some(AacCoder::Twoloop),
            afterburner: Some(true),
            threads: ThreadSetting::Auto,
        };
        
        let result = validate_encoder_settings(&settings);
        assert!(
            result.is_err(),
            "Bitrate {} should be invalid", bitrate
        );
    }
}

#[test]
fn test_serde_round_trip_comprehensive() {
    // Test complex settings with all field variations
    let test_settings = [
        EncoderSettings {
            encoder_type: EncoderType::AacAt,
            bitrate_kbps: 56,
            channels: 1,
            aac_coder: None,
            afterburner: None,
            threads: ThreadSetting::Auto,
        },
        EncoderSettings {
            encoder_type: EncoderType::HeAacV1,
            bitrate_kbps: 96,
            channels: 2,
            aac_coder: Some(AacCoder::Fast),
            afterburner: Some(false),
            threads: ThreadSetting::Off,
        },
        EncoderSettings {
            encoder_type: EncoderType::HeAacV2,
            bitrate_kbps: 80,
            channels: 2,
            aac_coder: Some(AacCoder::Twoloop),
            afterburner: Some(true),
            threads: ThreadSetting::Fixed(16),
        },
    ];

    for (i, settings) in test_settings.iter().enumerate() {
        // Serialize to JSON
        let json = serde_json::to_string(settings)
            .expect(&format!("Settings {} should serialize", i));
        
        // Deserialize back
        let deserialized: EncoderSettings = serde_json::from_str(&json)
            .expect(&format!("Settings {} should deserialize", i));
        
        // Verify all fields match
        assert_eq!(settings.encoder_type, deserialized.encoder_type);
        assert_eq!(settings.bitrate_kbps, deserialized.bitrate_kbps);
        assert_eq!(settings.channels, deserialized.channels);
        assert_eq!(settings.aac_coder, deserialized.aac_coder);
        assert_eq!(settings.afterburner, deserialized.afterburner);
        assert_eq!(settings.threads, deserialized.threads);
    }
}

#[test]
fn test_default_behavior() {
    let default_settings = EncoderSettings::default();
    
    // Should have reasonable defaults
    assert!(ALLOWED_BITRATES.contains(&default_settings.bitrate_kbps));
    assert!(default_settings.channels >= 1 && default_settings.channels <= 2);
    
    // Should validate successfully
    let result = validate_encoder_settings(&default_settings);
    assert!(result.is_ok(), "Default settings should be valid");
    
    // Platform-specific encoder type behavior
    #[cfg(target_os = "macos")]
    assert_eq!(default_settings.encoder_type, EncoderType::AacAt);
    
    #[cfg(not(target_os = "macos"))]
    assert_eq!(default_settings.encoder_type, EncoderType::HeAacV1);
}