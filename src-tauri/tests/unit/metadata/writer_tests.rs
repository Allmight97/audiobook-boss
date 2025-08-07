use audiobook_boss_lib::metadata::{write_metadata, writer::write_cover_art};
use audiobook_boss_lib::metadata::AudiobookMetadata;
use audiobook_boss_lib::errors::AppError;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_write_to_nonexistent_file() {
    let metadata = AudiobookMetadata::new();
    let result = write_metadata("nonexistent.m4b", &metadata);
    assert!(matches!(result, Err(AppError::FileValidation(_))));
}

#[test]
fn test_write_cover_to_nonexistent_file() {
    let cover_data = vec![0u8; 100];
    let result = write_cover_art("nonexistent.m4b", &cover_data);
    assert!(matches!(result, Err(AppError::FileValidation(_))));
}

#[test]
fn test_write_metadata_invalid_file() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("invalid.txt");
    fs::write(&file_path, b"not audio").expect("write temp file");
    let metadata = AudiobookMetadata::new();
    let result = write_metadata(&file_path, &metadata);
    assert!(matches!(result, Err(AppError::Metadata(_))));
}


