//! Integration tests for path validation across all entry points.
//!
//! Verifies that invalid inputs are rejected at all API boundaries and
//! tests symlink handling, output directory validation, and special characters.

use audiobook_boss_lib::audio::path_validation::{
    validate_input_audio_path, validate_input_image_path,
};
use audiobook_boss_lib::audio::{self, SampleRateConfig};
use audiobook_boss_lib::commands::{analyze_audio_files, validate_files};
use std::fs::{self, File, Permissions};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn sample_mp3_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest dir parent")
        .join("media")
        .join("media_20sec.mp3")
}

fn copy_sample_mp3(target: &Path) {
    fs::copy(sample_mp3_path(), target).expect("copy mp3 fixture");
}

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
    copy_sample_mp3(&valid_file);

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

    let invalid_count = file_info.files.iter().filter(|f| !f.is_valid).count();
    assert_eq!(
        invalid_count, 2,
        "Nonexistent path and directory should be invalid"
    );

    let valid_path = valid_file
        .canonicalize()
        .expect("canonicalize valid file");
    let valid_entry = file_info
        .files
        .iter()
        .find(|f| f.path == valid_path)
        .expect("valid file entry");
    assert!(valid_entry.is_valid, "Valid mp3 should be accepted");
    assert!(valid_entry.error.is_none(), "Valid mp3 should have no error");

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
    copy_sample_mp3(&unicode_file);
    let files = vec![unicode_file.to_string_lossy().to_string()];
    let result = analyze_audio_files(files);
    assert!(result.is_ok(), "Unicode filenames should be accepted");
    let file_info = result.expect("analysis ok");
    let file = &file_info.files[0];
    assert!(file.is_valid, "Unicode filename should be valid with real media");
    assert!(file.error.is_none(), "Unicode filename should not error");
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
    copy_sample_mp3(&target);
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
    assert!(file.is_valid, "Symlinked media should be valid");
}

// ============================================================================
// Audio path validation edge cases
// ============================================================================

#[test]
fn test_rejects_cr_lf_nul_in_path() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let path = temp_dir.path().join("bad\nname.mp3");
    let _ = File::create(&path);
    let err = validate_input_audio_path(&path)
        .expect_err("paths with invalid characters should fail validation");
    assert!(err.to_string().contains("invalid characters"));
}

#[test]
fn test_case_insensitive_extension() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let path = temp_dir.path().join("test.MP3");
    File::create(&path).expect("create uppercase extension file");
    let result = validate_input_audio_path(&path);
    assert!(result.is_ok(), "Uppercase extension should be accepted");
}

#[test]
fn test_rejects_empty_path() {
    let empty_path = std::path::PathBuf::new();
    let err =
        validate_input_audio_path(&empty_path).expect_err("empty path should fail validation");
    let msg = err.to_string();
    assert!(msg.contains("Cannot read file metadata") || msg.contains("no extension"));
}

#[cfg(unix)]
#[test]
fn test_broken_symlink_rejected() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().expect("create temp dir");
    let target = dir.path().join("nonexistent.mp3");
    let link = dir.path().join("broken_link.mp3");
    symlink(&target, &link).expect("create symlink to nonexistent file");

    let err = validate_input_audio_path(&link).expect_err("broken symlink should fail validation");
    let msg = err.to_string();
    assert!(msg.contains("Cannot read file metadata") || msg.contains("canonicalize"));
}

// ============================================================================
// Image path validation
// ============================================================================

#[test]
fn test_image_rejects_nonexistent_path() {
    let bogus = std::path::PathBuf::from("/this/path/does/not/exist.jpg");
    let err = validate_input_image_path(&bogus)
        .expect_err("non-existent image path should fail validation");
    let msg = err.to_string();
    assert!(msg.contains("Cannot read file metadata"));
}

#[test]
fn test_image_rejects_invalid_extension() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("file.txt");
    File::create(&path).expect("create temp file");
    let err = validate_input_image_path(&path)
        .expect_err("invalid image extension should fail validation");
    let msg = err.to_string();
    assert!(msg.contains("Unsupported image format"));
}

#[test]
fn test_image_accepts_supported_extensions() {
    let dir = TempDir::new().expect("create temp dir");
    let extensions = ["jpg", "jpeg", "png", "webp"];

    for ext in extensions {
        let path = dir.path().join(format!("cover.{}", ext));
        File::create(&path).expect("create temp image file");
        let result = validate_input_image_path(&path);
        assert!(
            result.is_ok(),
            "Image extension {} should be supported",
            ext
        );
    }
}

#[test]
fn test_image_rejects_audio_extension() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("audio.mp3");
    File::create(&path).expect("create temp file");
    let err =
        validate_input_image_path(&path).expect_err("audio extension should fail image validation");
    let msg = err.to_string();
    assert!(msg.contains("Unsupported image format"));
}

#[test]
fn test_image_canonicalizes_path() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("cover.png");
    File::create(&path).expect("create temp image file");
    let canon = validate_input_image_path(&path).expect("valid image path should be canonicalized");
    assert!(canon.is_absolute());
}

#[test]
fn test_image_rejects_traversal_attempt() {
    let dir = TempDir::new().expect("create temp dir");
    let traversal = dir.path().join("..").join("..").join("etc").join("passwd");
    let err =
        validate_input_image_path(&traversal).expect_err("traversal path should fail validation");
    let msg = err.to_string();
    assert!(
        msg.contains("Unsupported image format")
            || msg.contains("Cannot read file metadata")
            || msg.contains("no extension")
    );
}
