use audiobook_boss_lib::commands::{read_audio_metadata, write_audio_metadata, write_cover_art, load_cover_art_file};
use audiobook_boss_lib::metadata::AudiobookMetadata;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_read_metadata_nonexistent() {
    let result = read_audio_metadata("nonexistent.m4b".to_string());
    assert!(result.is_err());
    let err = result.expect_err("expected read_audio_metadata to fail for nonexistent file");
    assert!(err.to_string().contains("File not found"));
}

#[test]
fn test_write_metadata_nonexistent() {
    let metadata = AudiobookMetadata::new();
    let result = write_audio_metadata("nonexistent.m4b".to_string(), metadata);
    assert!(result.is_err());
    let err = result.expect_err("expected write_audio_metadata to fail for nonexistent file");
    assert!(err.to_string().contains("File not found"));
}

#[test]
fn test_write_cover_art_nonexistent() {
    let cover_data = vec![0u8; 100];
    let result = write_cover_art("nonexistent.m4b".to_string(), cover_data);
    assert!(result.is_err());
    let err = result.expect_err("expected write_cover_art to fail for nonexistent file");
    assert!(err.to_string().contains("File not found"));
}

#[tokio::test]
async fn test_load_cover_art_file_nonexistent() {
    let result = load_cover_art_file("nonexistent.jpg".to_string()).await;
    assert!(result.is_err());
    let err = result.expect_err("expected load_cover_art_file to fail for nonexistent image");
    assert!(err.to_string().contains("Image file not found"));
}

#[tokio::test]
async fn test_load_cover_art_file_invalid_extension() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("test.txt");
    fs::write(&file_path, b"not an image").expect("write temp file");
    let result = load_cover_art_file(file_path.to_string_lossy().to_string()).await;
    assert!(result.is_err());
    let err = result.expect_err("expected unsupported image format");
    assert!(err.to_string().contains("Unsupported image format"));
}

#[test]
fn test_read_metadata_invalid_file() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("invalid.txt");
    fs::write(&file_path, b"not audio").expect("write temp file");
    let result = read_audio_metadata(file_path.to_string_lossy().to_string());
    assert!(result.is_err());
    let err = result.expect_err("expected metadata error");
    assert!(err.to_string().contains("metadata error"));
}


