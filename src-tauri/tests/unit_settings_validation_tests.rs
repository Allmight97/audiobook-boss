//! Unit tests for settings validation logic.
//!
//! Tests validation functions for sample rate, output paths, and encoder settings
//! without touching real files or FFmpeg infrastructure.

use audiobook_boss_lib::audio::settings_encoder::{
    BitrateMode, ChannelConfig as EncoderChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};
use audiobook_boss_lib::audio::{
    validate_output_path, validate_sample_rate_config, MediaProcessingPlan, SampleRateConfig,
};
use std::path::PathBuf;
use tempfile::TempDir;

fn baseline_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Mono,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
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
fn media_processing_plan_preserves_settings() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let output_file = temp_dir.path().join("output.m4b");
    let input_file = temp_dir.path().join("input.wav");
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
    assert_eq!(plan.encoder_settings, encoder_settings);
    assert_eq!(plan.sample_rate, sample_rate);
}

#[test]
fn output_path_validation_rejects_file_parent() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let file_parent = temp_dir.path().join("not_a_dir.txt");
    std::fs::write(&file_parent, b"test").expect("create temp file");

    let output_path = file_parent.join("child.m4b");
    let err = validate_output_path(&output_path).expect_err("expected not directory error");
    assert!(err.to_string().contains("not a directory"));
}

#[test]
fn sample_rate_config_helper_methods() {
    let auto_config = SampleRateConfig::Auto;
    assert!(auto_config.requires_detection());
    assert_eq!(auto_config.explicit_rate(), None);

    let explicit_config = SampleRateConfig::Explicit(44100);
    assert!(!explicit_config.requires_detection());
    assert_eq!(explicit_config.explicit_rate(), Some(44100));
}
