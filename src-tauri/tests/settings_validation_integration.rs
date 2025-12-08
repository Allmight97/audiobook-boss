//! Settings validation integration tests for the v2 encoder contract.
//!
//! These tests cover sample rate validation, output path validation, encoder
//! settings validation, and MediaProcessingPlan construction without legacy adapters.

use audiobook_boss_lib::audio::processor::detect_input_sample_rate;
use audiobook_boss_lib::audio::settings_encoder::{
    BitrateMode, ChannelConfig as EncoderChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};
use audiobook_boss_lib::audio::{
    validate_output_path, validate_sample_rate_config, MediaProcessingPlan, SampleRateConfig,
};
use std::path::PathBuf;
use tempfile::TempDir;

fn create_test_audio_file(temp_dir: &TempDir, filename: &str) -> std::io::Result<PathBuf> {
    let test_file_path = temp_dir.path().join(filename);

    // Minimal WAV header + 1s of silence
    let wav_data = [
        0x52, 0x49, 0x46, 0x46, // "RIFF"
        0x24, 0x00, 0x00, 0x00, // File size - 8 (36 bytes)
        0x57, 0x41, 0x56, 0x45, // "WAVE"
        0x66, 0x6d, 0x74, 0x20, // "fmt "
        0x10, 0x00, 0x00, 0x00, // Chunk size (16)
        0x01, 0x00, // Audio format (1 = PCM)
        0x01, 0x00, // Number of channels (1 = mono)
        0x40, 0x1f, 0x00, 0x00, // Sample rate (8000)
        0x40, 0x1f, 0x00, 0x00, // Byte rate (8000)
        0x01, 0x00, // Block align (1)
        0x08, 0x00, // Bits per sample (8)
        0x64, 0x61, 0x74, 0x61, // "data"
        0x04, 0x00, 0x00, 0x00, // Data size (4 bytes)
        0x80, 0x80, 0x80, 0x80, // Audio data (silence)
    ];

    std::fs::write(&test_file_path, wav_data)?;
    Ok(test_file_path)
}

fn baseline_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Mono,
        afterburner: false,
        threads: ThreadSetting::Auto,
    }
}

#[test]
fn sample_rate_validation_accepts_expected_values() {
    let valid_rates = [
        SampleRateConfig::Auto,
        SampleRateConfig::Explicit(22050),
        SampleRateConfig::Explicit(32000),
        SampleRateConfig::Explicit(44100),
        SampleRateConfig::Explicit(48000),
    ];
    for config in valid_rates {
        assert!(
            validate_sample_rate_config(&config).is_ok(),
            "Sample rate {:?} should be valid",
            config
        );
    }
}

#[test]
fn sample_rate_validation_rejects_unknown_values() {
    let invalid = SampleRateConfig::Explicit(12345);
    let err = validate_sample_rate_config(&invalid).expect_err("expected invalid sample rate");
    assert!(err.to_string().contains("Unsupported sample rate"));
}

#[test]
fn output_path_validation_covers_extension_and_directory() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let valid_output = temp_dir.path().join("output.m4b");
    assert!(validate_output_path(&valid_output).is_ok());

    let invalid_ext = temp_dir.path().join("output.mp3");
    let err = validate_output_path(&invalid_ext).expect_err("expected extension error");
    assert!(err.to_string().contains(".m4b"));

    let missing_dir = PathBuf::from("/nonexistent/path/output.m4b");
    let err = validate_output_path(&missing_dir).expect_err("expected missing dir error");
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn encoder_settings_validation_accepts_supported_combinations() {
    let mut settings = baseline_encoder_settings();
    settings.bitrate_mode = BitrateMode::Cbr;
    settings.encoder_type = EncoderType::NativeAac;
    audiobook_boss_lib::audio::settings_encoder::validate_encoder_settings(&settings)
        .expect("native AAC + CBR should be valid");

    settings.encoder_type = EncoderType::AacAt;
    settings.bitrate_mode = BitrateMode::Cvbr;
    audiobook_boss_lib::audio::settings_encoder::validate_encoder_settings(&settings)
        .expect("AAC-AT + CVBR should be valid");
}

#[test]
fn encoder_settings_validation_rejects_invalid_combinations() {
    let mut settings = baseline_encoder_settings();
    settings.encoder_type = EncoderType::AacAt;
    settings.bitrate_mode = BitrateMode::Cbr;
    let err = audiobook_boss_lib::audio::settings_encoder::validate_encoder_settings(&settings)
        .expect_err("AAC-AT + CBR should be rejected");
    assert!(err.to_string().contains("not supported"));
}

#[test]
fn sample_rate_detection_reads_from_inputs() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let files = vec![
        create_test_audio_file(&temp_dir, "file1.wav").expect("create file1"),
        create_test_audio_file(&temp_dir, "file2.wav").expect("create file2"),
    ];

    let detected = detect_input_sample_rate(&files).expect("detect sample rate");
    assert_eq!(detected, 8000);
}

#[test]
fn media_processing_plan_preserves_settings() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let input_file = create_test_audio_file(&temp_dir, "input.wav").expect("create input");
    let output_file = temp_dir.path().join("output.m4b");
    let encoder_settings = baseline_encoder_settings();
    let sample_rate = SampleRateConfig::Explicit(22050);
    let total_duration = 1.5;

    let plan = MediaProcessingPlan::new(
        output_file.clone(),
        encoder_settings.clone(),
        sample_rate.clone(),
        vec![input_file.clone()],
        total_duration,
    );

    assert_eq!(plan.output_path, output_file);
    assert_eq!(plan.input_file_paths, vec![input_file]);
    assert_eq!(plan.total_duration, total_duration);
    assert_eq!(
        plan.encoder_settings.bitrate_kbps,
        encoder_settings.bitrate_kbps
    );
    assert_eq!(plan.sample_rate, sample_rate);
}
