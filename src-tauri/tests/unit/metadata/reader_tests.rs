use audiobook_boss_lib::metadata::read_metadata;
use audiobook_boss_lib::errors::AppError;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_read_nonexistent_file() {
    let result = read_metadata("nonexistent.m4b");
    assert!(matches!(result, Err(AppError::FileValidation(_))));
}

#[test]
fn test_read_metadata_empty_file() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("empty.txt");
    fs::write(&file_path, b"").expect("write empty file");
    let result = read_metadata(&file_path);
    assert!(matches!(result, Err(AppError::Metadata(_))));
}


