//! Integration tests for file list validation and metadata extraction.

use audiobook_boss_lib::audio::file_list::{get_file_list_info, validate_audio_files};
use std::fs;
use tempfile::TempDir;

#[test]
fn validate_empty_file_list_returns_error() {
    let result = validate_audio_files::<&str>(&[]);
    assert!(result.is_err());
    let err = result.expect_err("expected error for empty file list");
    assert!(err.to_string().contains("No files provided"));
}

#[test]
fn validate_nonexistent_file_marks_invalid() {
    let files = vec!["nonexistent.mp3"];
    let result =
        validate_audio_files(&files).expect("validation should succeed, marking file invalid");
    assert_eq!(result.len(), 1);
    assert!(!result[0].is_valid);
    let msg = result[0]
        .error
        .as_ref()
        .expect("error message for invalid file");
    assert!(msg.contains("Cannot read file metadata") || msg.contains("File not found"));
}

#[test]
fn validate_invalid_audio_file_marks_error() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("invalid.mp3");
    fs::write(&file_path, b"not audio data").expect("write temp file");

    let files = vec![file_path];
    let result =
        validate_audio_files(&files).expect("validation should succeed, marking file invalid");
    assert_eq!(result.len(), 1);
    assert!(!result[0].is_valid);
    assert!(result[0].error.is_some());
}

#[test]
fn get_file_list_info_empty_returns_error() {
    let result = get_file_list_info::<&str>(&[]);
    assert!(result.is_err());
}
