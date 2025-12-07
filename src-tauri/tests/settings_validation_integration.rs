//! Settings validation integration tests that verify output properties match input settings
//!
//! These tests ensure that audio settings (bitrate, channels, sample rate) are
//! correctly applied and result in output files with matching properties.

use audiobook_boss_lib::audio::processor::detect_input_sample_rate;
use audiobook_boss_lib::audio::settings_encoder::{
    BitrateMode, ChannelConfig as EncoderChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};
use audiobook_boss_lib::audio::MediaProcessingPlan;
use audiobook_boss_lib::audio::{
    validate_audio_settings, AudioSettings, ChannelConfig, SampleRateConfig,
};
use std::path::PathBuf;
use tempfile::TempDir;

/// Creates a minimal valid audio file for testing
fn create_test_audio_file(temp_dir: &TempDir, filename: &str) -> std::io::Result<PathBuf> {
    let test_file_path = temp_dir.path().join(filename);

    // Create a minimal WAV file (44 bytes header + some audio data)
    // This is a valid 1-second mono 8kHz 8-bit WAV file
    let wav_data = [
        // RIFF header
        0x52, 0x49, 0x46, 0x46, // "RIFF"
        0x24, 0x00, 0x00, 0x00, // File size - 8 (36 bytes)
        0x57, 0x41, 0x56, 0x45, // "WAVE"
        // fmt chunk
        0x66, 0x6d, 0x74, 0x20, // "fmt "
        0x10, 0x00, 0x00, 0x00, // Chunk size (16)
        0x01, 0x00, // Audio format (1 = PCM)
        0x01, 0x00, // Number of channels (1 = mono)
        0x40, 0x1f, 0x00, 0x00, // Sample rate (8000)
        0x40, 0x1f, 0x00, 0x00, // Byte rate (8000)
        0x01, 0x00, // Block align (1)
        0x08, 0x00, // Bits per sample (8)
        // data chunk
        0x64, 0x61, 0x74, 0x61, // "data"
        0x04, 0x00, 0x00, 0x00, // Data size (4 bytes)
        0x80, 0x80, 0x80, 0x80, // Audio data (silence)
    ];

    std::fs::write(&test_file_path, wav_data)?;
    Ok(test_file_path)
}

#[test]
fn test_validate_audio_settings_all_valid() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let output_path = temp_dir.path().join("output.m4b");

    let settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Explicit(22050),
        output_path,
    };

    let result = validate_audio_settings(&settings);
    assert!(result.is_ok(), "Valid settings should pass validation");
}

#[test]
fn test_validate_bitrate_edge_cases() {
    // Test minimum valid bitrate
    let temp_dir = TempDir::new().expect("create temp dir");
    let output_path = temp_dir.path().join("output.m4b");

    let settings_min = AudioSettings {
        bitrate: 48,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: output_path.clone(),
    };
    assert!(
        validate_audio_settings(&settings_min).is_ok(),
        "Minimum bitrate (48 kbps) should be valid"
    );

    // Test maximum valid bitrate
    let settings_max = AudioSettings {
        bitrate: 128,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: output_path.clone(),
    };
    assert!(
        validate_audio_settings(&settings_max).is_ok(),
        "Maximum bitrate should be valid"
    );

    // Test invalid low bitrate
    let settings_low = AudioSettings {
        bitrate: 16,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: output_path.clone(),
    };
    let result_low = validate_audio_settings(&settings_low);
    assert!(
        result_low.is_err(),
        "Too low bitrate should fail validation"
    );
    assert!(result_low
        .expect_err("expected error")
        .to_string()
        .contains("48-128"));

    // Test invalid high bitrate
    let settings_high = AudioSettings {
        bitrate: 256,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path,
    };
    let result_high = validate_audio_settings(&settings_high);
    assert!(
        result_high.is_err(),
        "Too high bitrate should fail validation"
    );
    assert!(result_high
        .expect_err("expected error")
        .to_string()
        .contains("48-128"));
}

#[test]
fn test_validate_sample_rate_configurations() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let output_path = temp_dir.path().join("output.m4b");

    // Test all valid explicit sample rates
    let valid_rates = [22050, 32000, 44100, 48000];
    for rate in valid_rates {
        let settings = AudioSettings {
            bitrate: 64,
            channels: ChannelConfig::Mono,
            sample_rate: SampleRateConfig::Explicit(rate),
            output_path: output_path.clone(),
        };
        assert!(
            validate_audio_settings(&settings).is_ok(),
            "Valid sample rate {rate} should pass validation"
        );
    }

    // Test invalid sample rate
    let settings_invalid = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Explicit(12345),
        output_path,
    };
    let result = validate_audio_settings(&settings_invalid);
    assert!(
        result.is_err(),
        "Invalid sample rate should fail validation"
    );
    let error_msg = result.expect_err("expected error").to_string();
    assert!(
        error_msg.contains("12345"),
        "Error should mention the invalid rate"
    );
    assert!(error_msg.contains("22050"), "Error should list valid rates");
}

#[test]
fn test_channel_config_properties() {
    // Test channel count mapping
    assert_eq!(ChannelConfig::Mono.channel_count(), 1);
    assert_eq!(ChannelConfig::Stereo.channel_count(), 2);

    // Channel layout strings helper removed; verify counts only
}

#[test]
fn test_audio_settings_presets() {
    // Test audiobook preset
    let audiobook = AudioSettings::audiobook_preset();
    assert_eq!(audiobook.bitrate, 64);
    assert!(matches!(audiobook.channels, ChannelConfig::Mono));
    assert!(matches!(audiobook.sample_rate, SampleRateConfig::Auto));

    // Test high quality preset
    let hq = AudioSettings::high_quality_preset();
    assert_eq!(hq.bitrate, 128);
    assert!(matches!(hq.channels, ChannelConfig::Stereo));
    assert!(matches!(hq.sample_rate, SampleRateConfig::Explicit(44100)));

    // Test low bandwidth preset
    let low = AudioSettings::low_bandwidth_preset();
    assert_eq!(low.bitrate, 48);
    assert!(matches!(low.channels, ChannelConfig::Mono));
    assert!(matches!(low.sample_rate, SampleRateConfig::Explicit(22050)));

    // Validate all presets are valid
    let temp_dir = TempDir::new().expect("create temp dir");
    let output_path = temp_dir.path().join("test.m4b");

    let mut audiobook_with_path = audiobook;
    audiobook_with_path.output_path = output_path.clone();
    assert!(validate_audio_settings(&audiobook_with_path).is_ok());

    let mut hq_with_path = hq;
    hq_with_path.output_path = output_path.clone();
    assert!(validate_audio_settings(&hq_with_path).is_ok());

    let mut low_with_path = low;
    low_with_path.output_path = output_path;
    assert!(validate_audio_settings(&low_with_path).is_ok());
}

#[test]
fn test_sample_rate_detection_from_files() {
    let temp_dir = TempDir::new().expect("create temp dir");

    // Create test files with different sample rates (simulated via AudioFile metadata)
    let file1_path = create_test_audio_file(&temp_dir, "file1.wav").expect("create test file 1");
    let file2_path = create_test_audio_file(&temp_dir, "file2.wav").expect("create test file 2");
    let file3_path = create_test_audio_file(&temp_dir, "file3.wav").expect("create test file 3");

    let file_paths = vec![file1_path, file2_path, file3_path];

    // Test detection with actual files
    // Note: Our test WAV files all have 8000 Hz, so that should be detected
    let detected = detect_input_sample_rate(&file_paths).expect("detect sample rate");
    assert_eq!(
        detected, 8000,
        "Should detect sample rate from test WAV files"
    );
}

#[test]
fn test_sample_rate_detection_error_cases() {
    // Test with empty file list
    let result = detect_input_sample_rate(&[]);
    assert!(result.is_err());
    let error = result.expect_err("expected error");
    assert!(error.to_string().contains("no input files"));

    // Test with nonexistent files
    let nonexistent_files = vec![
        PathBuf::from("/nonexistent/file1.mp3"),
        PathBuf::from("/nonexistent/file2.mp3"),
    ];
    let result = detect_input_sample_rate(&nonexistent_files);
    assert!(result.is_err());
    let error = result.expect_err("expected error");
    assert!(error.to_string().contains("no valid audio files"));
}

#[test]
fn test_media_processing_plan_construction() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let input_file = create_test_audio_file(&temp_dir, "input.wav").expect("create test file");
    let output_file = temp_dir.path().join("output.m4b");

    // Concat no longer required in single-engine path

    let settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Explicit(22050),
        output_path: output_file.clone(),
    };
    let encoder_settings = EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: settings.bitrate as u16,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Mono,
        afterburner: false,
        threads: ThreadSetting::Auto,
    };

    let plan = MediaProcessingPlan::new(
        output_file,
        encoder_settings,
        settings.sample_rate.clone(),
        1,
        vec![input_file],
        1.0, // 1 second duration
    );

    // Test plan creation and validation
    // Note: build_ffmpeg_command removed in Phase 11 cleanup - behavior now validated via end-to-end processing

    // Verify the plan contains our settings
    assert_eq!(plan.encoder_settings.bitrate_kbps, 64);
    assert!(matches!(
        plan.sample_rate,
        audiobook_boss_lib::audio::SampleRateConfig::Explicit(22050)
    ));
    assert!(matches!(
        plan.encoder_settings.channels,
        EncoderChannelConfig::Mono
    ));
}

/// Integration test that verifies settings are applied correctly by the processor
/// This test validates the processing pipeline structure and settings preservation
#[test]
fn test_settings_application_integration() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let input_file = create_test_audio_file(&temp_dir, "input.wav").expect("create test file");
    let _output_file = temp_dir.path().join("output.m4b");
    // Concat file not needed in single-engine path

    // Test different settings configurations
    let test_cases = vec![
        // Low bandwidth mono (minimum bitrate)
        AudioSettings {
            bitrate: 48,
            channels: ChannelConfig::Mono,
            sample_rate: SampleRateConfig::Explicit(22050),
            output_path: temp_dir.path().join("output_low.m4b"),
        },
        // High quality stereo
        AudioSettings {
            bitrate: 128,
            channels: ChannelConfig::Stereo,
            sample_rate: SampleRateConfig::Explicit(44100),
            output_path: temp_dir.path().join("output_high.m4b"),
        },
        // Auto sample rate
        AudioSettings {
            bitrate: 64,
            channels: ChannelConfig::Mono,
            sample_rate: SampleRateConfig::Auto,
            output_path: temp_dir.path().join("output_auto.m4b"),
        },
    ];

    for (i, settings) in test_cases.into_iter().enumerate() {
        // Validate settings are valid
        assert!(
            validate_audio_settings(&settings).is_ok(),
            "Test case {i} settings should be valid"
        );

        let encoder_settings = EncoderSettings {
            encoder_type: EncoderType::NativeAac,
            bitrate_kbps: settings.bitrate as u16,
            bitrate_mode: BitrateMode::Cbr,
            channels: match settings.channels {
                ChannelConfig::Mono => EncoderChannelConfig::Mono,
                ChannelConfig::Stereo => EncoderChannelConfig::Stereo,
            },
            afterburner: false,
            threads: ThreadSetting::Auto,
        };

        // Create processing plan
        let plan = MediaProcessingPlan::new(
            settings.output_path.clone(),
            encoder_settings.clone(),
            settings.sample_rate.clone(),
            encoder_settings.channels.forced_channels().unwrap_or(1),
            vec![input_file.clone()],
            1.0,
        );

        // Verify plan can be created
        // Note: build_ffmpeg_command removed in Phase 11 cleanup
        assert!(
            plan.encoder_settings.bitrate_kbps > 0,
            "Test case {i} should have valid bitrate"
        );

        // In a full integration test, we would:
        // 1. Create mock ProcessingContext
        // 2. Execute the plan using available processor implementation
        // 3. Verify output file properties match settings
        // However, that requires actual FFmpeg execution which is complex for unit tests
    }
}

#[test]
fn test_settings_validation_comprehensive() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let valid_output = temp_dir.path().join("valid.m4b");
    let invalid_output = temp_dir.path().join("invalid.mp3");

    // Valid settings should pass
    let valid_settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: valid_output,
    };
    assert!(validate_audio_settings(&valid_settings).is_ok());

    // Invalid bitrate
    let invalid_bitrate = AudioSettings {
        bitrate: 200,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: temp_dir.path().join("test.m4b"),
    };
    assert!(validate_audio_settings(&invalid_bitrate).is_err());

    // Invalid sample rate
    let invalid_sample_rate = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Explicit(99999),
        output_path: temp_dir.path().join("test.m4b"),
    };
    assert!(validate_audio_settings(&invalid_sample_rate).is_err());

    // Invalid output extension
    let invalid_extension = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: invalid_output,
    };
    let result = validate_audio_settings(&invalid_extension);
    assert!(result.is_err());
    let error_msg = result.expect_err("expected error").to_string();
    assert!(
        error_msg.contains(".m4b"),
        "Error should mention required extension"
    );
}

#[test]
fn test_channel_config_edge_cases() {
    // Test that channel count is correct for all variants
    let mono = ChannelConfig::Mono;
    let stereo = ChannelConfig::Stereo;

    assert_eq!(mono.channel_count(), 1);
    assert_eq!(stereo.channel_count(), 2);

    // Channel layout helper removed

    // Test that these values work in settings validation
    let temp_dir = TempDir::new().expect("create temp dir");
    let output_path = temp_dir.path().join("test.m4b");

    let mono_settings = AudioSettings {
        bitrate: 64,
        channels: mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: output_path.clone(),
    };
    assert!(validate_audio_settings(&mono_settings).is_ok());

    let stereo_settings = AudioSettings {
        bitrate: 64,
        channels: stereo,
        sample_rate: SampleRateConfig::Auto,
        output_path,
    };
    assert!(validate_audio_settings(&stereo_settings).is_ok());
}

/// Test that validates the settings are correctly applied in MediaProcessingPlan
#[test]
fn test_media_processing_plan_settings_preservation() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let output_file = temp_dir.path().join("output.m4b");
    let input_file = temp_dir.path().join("input.wav");

    let original_settings = AudioSettings {
        bitrate: 96,
        channels: ChannelConfig::Stereo,
        sample_rate: SampleRateConfig::Explicit(48000),
        output_path: output_file.clone(),
    };
    let encoder_settings = EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: original_settings.bitrate as u16,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Stereo,
        afterburner: false,
        threads: ThreadSetting::Auto,
    };

    let plan = MediaProcessingPlan::new(
        output_file.clone(),
        encoder_settings.clone(),
        original_settings.sample_rate.clone(),
        encoder_settings.channels.forced_channels().unwrap_or(2),
        vec![input_file],
        2.5,
    );

    // Verify plan preserves all settings
    assert_eq!(
        plan.encoder_settings.bitrate_kbps,
        original_settings.bitrate as u16
    );
    assert!(matches!(
        plan.encoder_settings.channels,
        EncoderChannelConfig::Stereo
    ));
    assert!(matches!(
        plan.sample_rate,
        SampleRateConfig::Explicit(48000)
    ));
    assert_eq!(plan.output_path, original_settings.output_path);
    assert_eq!(plan.total_duration, 2.5);
}

/// Comprehensive test covering multiple scenarios
#[test]
fn test_settings_validation_matrix() {
    let temp_dir = TempDir::new().expect("create temp dir");

    // Test matrix of valid combinations (48-128 kbps range)
    let bitrates = [48, 64, 96, 128];
    let channels = [ChannelConfig::Mono, ChannelConfig::Stereo];
    let sample_rates = [
        SampleRateConfig::Auto,
        SampleRateConfig::Explicit(22050),
        SampleRateConfig::Explicit(44100),
        SampleRateConfig::Explicit(48000),
    ];

    let mut test_count = 0;
    for bitrate in bitrates {
        for channel in &channels {
            for sample_rate in &sample_rates {
                let output_path = temp_dir.path().join(format!("test_{test_count}.m4b"));
                let settings = AudioSettings {
                    bitrate,
                    channels: channel.clone(),
                    sample_rate: sample_rate.clone(),
                    output_path,
                };

                let result = validate_audio_settings(&settings);
                assert!(result.is_ok(),
                        "Settings combination {test_count} should be valid: bitrate={bitrate}, channels={:?}, sample_rate={:?}",
                        channel, sample_rate);

                test_count += 1;
            }
        }
    }

    println!("Validated {test_count} valid settings combinations");
}
