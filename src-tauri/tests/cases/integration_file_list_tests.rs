//! Integration tests for file list validation and metadata extraction.

use audiobook_boss_lib::audio::get_file_list_info;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn validate_empty_file_list_returns_error() {
    let result = get_file_list_info::<&str>(&[]);
    assert!(result.is_err());
    let err = result.expect_err("expected error for empty file list");
    assert!(err.to_string().contains("No files provided"));
}

#[test]
fn validate_nonexistent_file_marks_invalid() {
    let files = vec!["nonexistent.mp3"];
    let result = get_file_list_info(&files).expect("validation should mark file invalid");
    assert_eq!(result.files.len(), 1);
    assert_eq!(result.invalid_count, 1);
    assert!(!result.files[0].is_valid);
    let msg = result.files[0]
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
    let result = get_file_list_info(&files).expect("validation should mark file invalid");
    assert_eq!(result.files.len(), 1);
    assert_eq!(result.invalid_count, 1);
    assert!(!result.files[0].is_valid);
    assert!(result.files[0].error.is_some());
}

fn create_test_wav_file(temp_dir: &TempDir, filename: &str) -> PathBuf {
    let test_file_path = temp_dir.path().join(filename);

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

    fs::write(&test_file_path, wav_data).expect("write wav fixture");
    test_file_path
}

fn feedback_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root should be the manifest parent")
        .join("media")
        .join("Feedback.m4b")
}

#[test]
fn validate_wav_file_is_supported() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let file_path = create_test_wav_file(&temp_dir, "valid.wav");

    let files = vec![file_path.clone()];
    let file_list_info = get_file_list_info(&files).expect("file list info should be available");
    assert_eq!(file_list_info.files.len(), 1);
    assert_eq!(file_list_info.valid_count, 1);
    assert!(file_list_info.files[0].is_valid);
    assert_eq!(file_list_info.files[0].format.as_deref(), Some("WAV"));
    assert_eq!(
        file_list_info.files[0].selected_decoder.as_deref(),
        Some("Native AAC (FFmpeg)")
    );
    assert_eq!(file_list_info.selected_decoders.len(), 1);
    let selected_decoder = file_list_info.selected_decoders[0]
        .as_ref()
        .expect("decoder identity should be present");
    assert_eq!(selected_decoder.decoder_id, "default");
    assert_eq!(selected_decoder.decoder_label, "Native AAC (FFmpeg)");
}

#[test]
fn validate_uppercase_m4b_file_is_supported() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("Feedback.M4B");
    fs::copy(feedback_fixture_path(), &file_path).expect("copy m4b fixture");

    let file_list_info =
        get_file_list_info(&[file_path]).expect("uppercase m4b should be analyzable");

    assert_eq!(file_list_info.valid_count, 1);
    assert_eq!(file_list_info.invalid_count, 0);
    assert_eq!(file_list_info.files[0].format.as_deref(), Some("M4A/M4B"));
}

#[test]
fn get_file_list_info_empty_returns_error() {
    let result = get_file_list_info::<&str>(&[]);
    assert!(result.is_err());
}
