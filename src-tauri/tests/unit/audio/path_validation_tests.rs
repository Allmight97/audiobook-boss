use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

use tempfile::tempdir;

use audiobook_boss_lib::audio::path_validation::validate_input_audio_path;

#[test]
fn test_rejects_nonexistent_path() {
    let bogus = PathBuf::from("/this/path/does/not/exist.mp3");
    let err = validate_input_audio_path(&bogus).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Cannot read file metadata"));
}

#[test]
fn test_rejects_directory_path() {
    let dir = tempdir().unwrap();
    let err = validate_input_audio_path(dir.path()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("not a regular file"));
}

#[test]
fn test_rejects_invalid_extension() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("file.txt");
    File::create(&path).unwrap();
    let err = validate_input_audio_path(&path).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Unsupported audio format") || msg.contains("no extension"));
}

#[test]
fn test_accepts_supported_extension_and_canonicalizes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("audio.mp3");
    File::create(&path).unwrap();
    let canon = validate_input_audio_path(&path).unwrap();
    assert!(canon.is_absolute());
}

#[test]
fn test_symlink_is_accepted_and_resolved() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("real.m4a");
    File::create(&target).unwrap();
    let link = dir.path().join("link.m4a");
    symlink(&target, &link).unwrap();
    let resolved = validate_input_audio_path(&link).unwrap();
    // Canonical path should point to the target
    assert_eq!(resolved, target.canonicalize().unwrap());
}

#[test]
fn test_rejects_cr_lf_nul_in_path() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad\nname.mp3");
    // Try creating the file; even if OS disallows, our function should reject prior to metadata.
    let _ = File::create(&path);
    let err = validate_input_audio_path(&path).unwrap_err();
    assert!(err.to_string().contains("invalid characters"));
}

#[test]
fn test_accepts_all_supported_extensions() {
    let dir = tempdir().unwrap();
    let extensions = ["mp3", "m4a", "m4b", "aac", "wav", "flac"];
    
    for ext in extensions {
        let path = dir.path().join(format!("test.{}", ext));
        File::create(&path).unwrap();
        let result = validate_input_audio_path(&path);
        assert!(result.is_ok(), "Extension {} should be supported", ext);
    }
}

#[test]
fn test_rejects_empty_path() {
    let empty_path = PathBuf::new();
    let err = validate_input_audio_path(&empty_path).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Cannot read file metadata") || msg.contains("no extension"));
}

#[test]
fn test_accepts_unicode_in_path() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("测试文件.mp3");
    File::create(&path).unwrap();
    let result = validate_input_audio_path(&path);
    assert!(result.is_ok(), "Unicode in path should be accepted");
}

#[test]
fn test_case_insensitive_extension() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.MP3");
    File::create(&path).unwrap();
    let result = validate_input_audio_path(&path);
    assert!(result.is_ok(), "Uppercase extension should be accepted");
}

#[test]
fn test_broken_symlink_rejected() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("nonexistent.mp3");
    let link = dir.path().join("broken_link.mp3");
    symlink(&target, &link).unwrap();
    
    let err = validate_input_audio_path(&link).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Cannot read file metadata") || msg.contains("canonicalize"));
}



