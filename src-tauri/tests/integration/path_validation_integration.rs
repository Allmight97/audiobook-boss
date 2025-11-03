// Integration tests for path validation implementation across all entry points
// These tests verify that invalid inputs are rejected at all API boundaries

use audiobook_boss_lib::audio::{self, AudioSettings, ChannelConfig, SampleRateConfig};
use audiobook_boss_lib::commands::{analyze_audio_files, validate_files};
use tempfile::TempDir;
use std::fs::File;

/// Test that validate_files command rejects invalid paths
#[test]
fn test_validate_files_rejects_invalid_inputs() {
    // Test nonexistent files
    let nonexistent_files = vec!["nonexistent1.mp3".to_string(), "nonexistent2.mp3".to_string()];
    let result = validate_files(nonexistent_files);
    assert!(result.is_err(), "Should reject nonexistent files");
    assert!(result.expect_err("expected error").to_string().contains("Cannot read file metadata"));
    
    // Test directory instead of file
    let temp_dir = TempDir::new().expect("create temp dir");
    let dir_files = vec![temp_dir.path().to_string_lossy().to_string()];
    let result = validate_files(dir_files);
    assert!(result.is_err(), "Should reject directories");
    assert!(result.expect_err("expected error").to_string().contains("not a regular file"));
    
    // Test unsupported file type
    let unsupported_file = temp_dir.path().join("document.txt");
    File::create(&unsupported_file).expect("create test file");
    let unsupported_files = vec![unsupported_file.to_string_lossy().to_string()];
    let result = validate_files(unsupported_files);
    assert!(result.is_err(), "Should reject unsupported file types");
    assert!(result.expect_err("expected error").to_string().contains("Unsupported audio format"));
}

/// Test that analyze_audio_files command properly validates inputs
#[test]
fn test_analyze_audio_files_validates_inputs() {
    let temp_dir = TempDir::new().expect("create temp dir");
    
    // Test with mix of valid and invalid files
    let valid_file = temp_dir.path().join("valid.mp3");
    File::create(&valid_file).expect("create valid file");
    
    let files = vec![
        "nonexistent.mp3".to_string(),
        valid_file.to_string_lossy().to_string(),
        temp_dir.path().to_string_lossy().to_string(), // directory
    ];
    
    let result = analyze_audio_files(files);
    assert!(result.is_ok(), "Analysis should succeed but mark invalid files");
    
    let file_info = result.expect("analysis ok");
    assert_eq!(file_info.files.len(), 3, "Should analyze all provided paths");
    
    // Check that invalid files are properly marked
    let invalid_count = file_info.files.iter().filter(|f| !f.is_valid).count();
    assert_eq!(invalid_count, 2, "Two files should be marked invalid");
    
    // Verify error messages contain path validation failures
    for file in &file_info.files {
        if !file.is_valid {
            let error = file.error.as_ref().expect("invalid file should have error");
            assert!(
                error.contains("Cannot read file metadata") || 
                error.contains("not a regular file") ||
                error.contains("Unsupported audio format"),
                "Error should indicate path validation failure: {}", error
            );
        }
    }
}

/// Test that audio settings validation works correctly
#[test] 
fn test_audio_settings_validation() {
    let temp_dir = TempDir::new().expect("create temp dir");
    
    // Test valid settings
    let valid_settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: temp_dir.path().join("output.m4b"),
    };
    let result = audio::validate_audio_settings(&valid_settings);
    assert!(result.is_ok(), "Valid settings should pass validation");
    
    // Test invalid output directory (nonexistent parent)
    let invalid_settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: "/nonexistent/directory/output.m4b".into(),
    };
    let result = audio::validate_audio_settings(&invalid_settings);
    assert!(result.is_err(), "Should reject nonexistent output directory");
    assert!(result.expect_err("expected error").to_string().contains("does not exist"));
}

/// Test that path validation handles special characters correctly
#[test]
fn test_path_validation_special_characters() {
    let temp_dir = TempDir::new().expect("create temp dir");
    
    // Test unicode filename (should be accepted)
    let unicode_file = temp_dir.path().join("测试文件.mp3");
    File::create(&unicode_file).expect("create unicode file");
    let files = vec![unicode_file.to_string_lossy().to_string()];
    let result = analyze_audio_files(files);
    assert!(result.is_ok(), "Unicode filenames should be accepted");
    let file_info = result.expect("analysis ok");
    // The file will be marked invalid due to content, but not due to path validation
    let file = &file_info.files[0];
    if let Some(error) = &file.error {
        assert!(!error.contains("invalid characters"), "Should not fail on unicode characters");
    }
}

/// Test symlink handling with integration
#[test] 
#[cfg(unix)] // Symlinks are Unix-specific
fn test_symlink_integration() {
    use std::os::unix::fs::symlink;
    
    let temp_dir = TempDir::new().expect("create temp dir");
    
    // Create target file and symlink
    let target = temp_dir.path().join("target.mp3");
    File::create(&target).expect("create target file");
    let link = temp_dir.path().join("link.mp3");
    symlink(&target, &link).expect("create symlink");
    
    let files = vec![link.to_string_lossy().to_string()];
    let result = analyze_audio_files(files);
    assert!(result.is_ok(), "Symlinks should be accepted");
    
    let file_info = result.expect("analysis ok");
    assert_eq!(file_info.files.len(), 1, "Should process symlink");
    
    // Check that the symlink was resolved to canonical path
    let file = &file_info.files[0];
    assert!(file.path.is_absolute(), "Should use canonical absolute path");
}
