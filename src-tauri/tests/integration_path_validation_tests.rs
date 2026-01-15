//! Integration tests for path validation across all entry points.
//!
//! Verifies that invalid inputs are rejected at all API boundaries and
//! tests symlink handling, output directory validation, and special characters.

use audiobook_boss_lib::audio::{self, SampleRateConfig};
use audiobook_boss_lib::commands::{analyze_audio_files, validate_files};
use std::fs::{File, Permissions};
use tempfile::TempDir;

// ============================================================================
// Command-level validation tests
// ============================================================================

#[test]
fn test_validate_files_rejects_invalid_inputs() {
    // Test nonexistent files
    let nonexistent_files = vec![
        "nonexistent1.mp3".to_string(),
        "nonexistent2.mp3".to_string(),
    ];
    let result = validate_files(nonexistent_files);
    assert!(result.is_err(), "Should reject nonexistent files");
    let error_msg = result.expect_err("expected error").to_string();
    assert!(
        error_msg.contains("Cannot read file metadata"),
        "Expected file validation error, got: {}",
        error_msg
    );

    // Test directory instead of file
    let temp_dir = TempDir::new().expect("create temp dir");
    let dir_files = vec![temp_dir.path().to_string_lossy().to_string()];
    let result = validate_files(dir_files);
    assert!(result.is_err(), "Should reject directories");
    let error_msg = result.expect_err("expected error").to_string();
    assert!(
        error_msg.contains("not a regular file"),
        "Expected directory validation error, got: {}",
        error_msg
    );

    // Test unsupported file type
    let unsupported_file = temp_dir.path().join("document.txt");
    File::create(&unsupported_file).expect("create test file");
    let unsupported_files = vec![unsupported_file.to_string_lossy().to_string()];
    let result = validate_files(unsupported_files);
    assert!(result.is_err(), "Should reject unsupported file types");
    let error_msg = result.expect_err("expected error").to_string();
    assert!(
        error_msg.contains("Unsupported audio format"),
        "Expected format validation error, got: {}",
        error_msg
    );
}

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
    assert!(
        result.is_ok(),
        "Analysis should succeed but mark invalid files"
    );

    let file_info = result.expect("analysis ok");
    assert_eq!(
        file_info.files.len(),
        3,
        "Should analyze all provided paths"
    );

    // Check that invalid files are properly marked
    let invalid_count = file_info.files.iter().filter(|f| !f.is_valid).count();
    eprintln!("Invalid file count: {} (expected 2 or 3)", invalid_count);
    for (i, file) in file_info.files.iter().enumerate() {
        eprintln!(
            "File {}: valid={}, error={:?}",
            i, file.is_valid, file.error
        );
    }

    // Expecting at least 2 invalid files (nonexistent + directory), valid.mp3 may also be invalid due to content
    assert!(
        invalid_count >= 2,
        "At least two files should be marked invalid, got {}",
        invalid_count
    );

    // Verify error messages contain path validation failures
    for file in &file_info.files {
        if !file.is_valid {
            let error = file.error.as_ref().expect("invalid file should have error");
            assert!(
                error.contains("Cannot read file metadata")
                    || error.contains("Path is not a regular file")
                    || error.contains("Unsupported audio format")
                    || error.contains("FFmpeg error"),
                "Error should indicate validation failure: {}",
                error
            );
        }
    }
}

// ============================================================================
// Audio settings validation
// ============================================================================

#[test]
fn test_audio_settings_validation() {
    let temp_dir = TempDir::new().expect("create temp dir");

    // Test valid sample rate + output path
    let result = audio::validate_sample_rate_config(&SampleRateConfig::Auto);
    assert!(result.is_ok(), "Auto sample rate should pass validation");

    let valid_output = temp_dir.path().join("output.m4b");
    let output_check = audio::validate_output_path(&valid_output);
    assert!(
        output_check.is_ok(),
        "Valid output path should pass validation"
    );

    // Test invalid output directory (nonexistent parent)
    let invalid_output: std::path::PathBuf = "/nonexistent/directory/output.m4b".into();
    let output_err = audio::validate_output_path(&invalid_output);
    assert!(
        output_err.is_err(),
        "Should reject nonexistent output directory"
    );
    assert!(output_err
        .expect_err("expected error")
        .to_string()
        .contains("does not exist"));
}

#[test]
fn test_output_path_validation_integration() {
    let temp_dir = TempDir::new().expect("create temp dir");

    // Test valid output path
    let result = audio::validate_output_path(temp_dir.path().join("valid_output.m4b"));
    assert!(result.is_ok(), "Valid output path should pass validation");

    // Test invalid extension
    let result = audio::validate_output_path(temp_dir.path().join("invalid_output.mp3"));
    assert!(result.is_err(), "Should reject non-.m4b extension");
    assert!(result
        .expect_err("expected extension error")
        .to_string()
        .contains(".m4b"));

    // Test nonexistent parent directory
    let result = audio::validate_output_path("/nonexistent/directory/output.m4b");
    assert!(
        result.is_err(),
        "Should reject nonexistent parent directory"
    );
    assert!(result
        .expect_err("expected directory error")
        .to_string()
        .contains("does not exist"));
}

#[cfg(unix)]
#[test]
fn test_read_only_output_directory_integration() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new().expect("create temp dir");
    let readonly_dir = temp_dir.path().join("readonly");
    std::fs::create_dir(&readonly_dir).expect("create readonly dir");

    // Make directory read-only
    let readonly_perms = Permissions::from_mode(0o444);
    std::fs::set_permissions(&readonly_dir, readonly_perms).expect("set readonly permissions");

    let result = audio::validate_output_path(readonly_dir.join("output.m4b"));
    assert!(result.is_err(), "Should reject read-only output directory");
    assert!(result
        .expect_err("expected write permission error")
        .to_string()
        .contains("not writable"));

    // Restore permissions for cleanup
    let normal_perms = Permissions::from_mode(0o755);
    std::fs::set_permissions(&readonly_dir, normal_perms).expect("restore permissions");
}

// ============================================================================
// Special character and Unicode handling
// ============================================================================

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
        assert!(
            !error.contains("invalid characters"),
            "Should not fail on unicode characters"
        );
    }
}

// ============================================================================
// Symlink handling
// ============================================================================

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
    assert!(
        file.path.is_absolute(),
        "Should use canonical absolute path"
    );
}
