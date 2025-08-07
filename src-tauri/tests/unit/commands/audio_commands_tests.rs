use audiobook_boss_lib::commands::{validate_files, analyze_audio_files, validate_audio_settings};
use audiobook_boss_lib::audio::AudioSettings;
use tempfile::TempDir;

#[test]
fn test_analyze_audio_files_empty() {
    let result = analyze_audio_files(vec![]);
    assert!(result.is_err());
    let err = result.expect_err("expected error for empty file list");
    assert!(err.to_string().contains("No files provided"));
}

#[test]
fn test_analyze_audio_files_nonexistent() {
    let files = vec!["nonexistent.mp3".to_string()];
    let result = analyze_audio_files(files).expect("analysis should succeed even for nonexistent file");
    assert_eq!(result.files.len(), 1);
    assert!(!result.files[0].is_valid);
    assert_eq!(result.valid_count, 0);
    assert_eq!(result.invalid_count, 1);
}

#[test]
fn test_validate_audio_settings_valid() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut settings = AudioSettings::audiobook_preset();
    settings.output_path = temp_dir.path().join("test.m4b");
    let result = validate_audio_settings(settings);
    assert!(result.is_ok());
    let value = result.expect("settings should be valid");
    assert_eq!(value, "Settings are valid");
}

#[test]
fn test_validate_audio_settings_invalid_bitrate() {
    let mut settings = AudioSettings::audiobook_preset();
    settings.bitrate = 256; // Invalid - too high
    let result = validate_audio_settings(settings);
    assert!(result.is_err());
    let err = result.expect_err("expected bitrate validation error");
    assert!(err.to_string().contains("Bitrate must be"));
}

#[test]
fn test_validate_files_empty() {
    let result = validate_files(vec![]);
    assert!(result.is_err());
    let err = result.expect_err("expected validation to error on empty input");
    assert!(err.to_string().contains("No files provided for validation"));
}

#[test]
fn test_validate_files_nonexistent() {
    let files = vec!["nonexistent_file.txt".to_string()];
    let result = validate_files(files);
    assert!(result.is_err());
    let err = result.expect_err("expected validation to fail for nonexistent file");
    assert!(err.to_string().contains("File not found"));
}


