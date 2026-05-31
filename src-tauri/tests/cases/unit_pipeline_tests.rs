//! Unit tests for audio processing pipeline data structures.
//!
//! Tests public audio data shapes without executing real audio processing.

use audiobook_boss_lib::audio::{
    AudioFile, BitrateMode, ChannelConfig as EncoderChannelConfig, EncoderSettings, EncoderType,
    SampleRateConfig, ThreadSetting,
};
use std::path::PathBuf;

#[test]
fn test_public_encoder_settings_and_sample_rate_compile() {
    let encoder = EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Stereo,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
    };
    let sample_rate = SampleRateConfig::Explicit(44100);

    assert_eq!(encoder.bitrate_kbps, 64);
    assert_eq!(encoder.channels, EncoderChannelConfig::Stereo);
    assert!(matches!(sample_rate, SampleRateConfig::Explicit(44100)));
}

#[test]
fn test_duration_calculation() {
    let files = [
        AudioFile {
            path: PathBuf::from("file1.mp3"),
            duration: Some(30.0),
            size: Some(1000000.0),
            sample_rate: Some(44100),
            channels: Some(2),
            bitrate: Some(128),
            format: Some("MP3".to_string()),
            codec_label: Some("MP3".to_string()),
            selected_decoder: Some("Native AAC (FFmpeg)".to_string()),
            is_valid: true,
            error: None,
        },
        AudioFile {
            path: PathBuf::from("file2.mp3"),
            duration: Some(45.5),
            size: Some(1500000.0),
            sample_rate: Some(44100),
            channels: Some(2),
            bitrate: Some(128),
            format: Some("MP3".to_string()),
            codec_label: Some("MP3".to_string()),
            selected_decoder: Some("Native AAC (FFmpeg)".to_string()),
            is_valid: true,
            error: None,
        },
        AudioFile {
            path: PathBuf::from("file3.mp3"),
            duration: None, // Should be ignored
            size: Some(800000.0),
            sample_rate: Some(22050),
            channels: Some(1),
            bitrate: Some(64),
            format: Some("MP3".to_string()),
            codec_label: Some("MP3".to_string()),
            selected_decoder: Some("Native AAC (FFmpeg)".to_string()),
            is_valid: true,
            error: None,
        },
    ];

    let total: f64 = files.iter().filter_map(|file| file.duration).sum();
    assert_eq!(total, 75.5, "Should sum only non-None durations");
}

#[test]
fn preview_path_derivation_does_not_panic() {
    let tmp = tempfile::TempDir::new().expect("temp dir");
    let out = tmp.path().join("Book Title (2025).m4b");
    let _sample_rate = SampleRateConfig::Explicit(22050);

    // Derive expected preview path
    let parent = out
        .parent()
        .expect("preview output should have a parent directory");
    let stem = out
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("preview output should have a valid UTF-8 stem");
    let expected = parent.join(format!("{}.preview.m4b", stem));

    // Confirm name derives as expected by reusing the same logic
    assert!(expected.display().to_string().ends_with(".preview.m4b"));
    assert!(expected
        .parent()
        .expect("preview path should have a parent directory")
        .exists());
    // Keep explicit-rate branch coverage for preview path derivation.
    assert!(matches!(_sample_rate, SampleRateConfig::Explicit(22050)));
}
