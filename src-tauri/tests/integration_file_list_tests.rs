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

#[test]
fn debug_m4b_filename_issues() {
    let test_cases = vec![
        "simple.m4b",
        "file with spaces.m4b",
        "file-with-dashes.m4b",
        "file_with_underscores.m4b",
        "David Thomas, Andrew Hunt - The Pragmatic Programmer 20th Anniversary, 2nd Editiony.m4b",
        "file.with.dots.m4b",
        "file'with'quotes.m4b",
    ];

    for filename in test_cases {
        println!("Testing filename: {filename}");
        let path = std::path::Path::new(filename);

        let extension = path.extension().and_then(|s| s.to_str());
        println!("  Extension detected: {extension:?}");

        if let Some(ext) = extension {
            let format_result = match ext {
                "mp3" => Ok("MP3"),
                "m4a" | "m4b" => Ok("M4A/M4B"),
                "aac" => Ok("AAC"),
                "wav" => Ok("WAV"),
                "flac" => Ok("FLAC"),
                _ => Err(format!("Unsupported format: {ext}")),
            };
            println!("  Format mapping: {format_result:?}");
        }

        println!("  Path display: {}", path.display());
        println!("  Path debug: {path:?}");
        println!();
    }
}
