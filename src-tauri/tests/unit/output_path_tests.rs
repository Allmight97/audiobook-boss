use std::path::PathBuf;

use audiobook_boss_lib::commands::audio::{build_output_path, resolve_collision, FilenamePattern};
use audiobook_boss_lib::metadata::AudiobookMetadata;
use tempfile::tempdir;

fn sample_metadata() -> AudiobookMetadata {
    AudiobookMetadata {
        title: Some("Flybot testing".to_string()),
        artist: Some("Dennis E. Taylor".to_string()),
        series: Some("FLY BOT SERIES".to_string()),
        date: Some(2025),
        ..Default::default()
    }
}

#[test]
fn build_output_path_with_metadata_and_pattern() {
    let dir = tempdir().expect("tempdir");
    let md = sample_metadata();

    let path = build_output_path(
        dir.path(),
        Some(&md),
        true,
        FilenamePattern::TitleYear,
        None,
    )
    .expect("should build path");

    let expected = dir
        .path()
        .join("Dennis E. Taylor/FLY BOT SERIES/Flybot testing/Flybot testing (2025).m4b");
    assert_eq!(path, expected);
}

#[test]
fn build_output_path_without_subdir_or_metadata_falls_back() {
    let dir = tempdir().expect("tempdir");
    let source = PathBuf::from("/tmp/source/ExampleBook.m4b");

    let path = build_output_path(
        dir.path(),
        None,
        false,
        FilenamePattern::AuthorTitle,
        Some(&source),
    )
    .expect("should build path");

    let expected = dir.path().join("Unknown Author - ExampleBook.m4b");
    assert_eq!(path, expected);
}

#[test]
fn resolve_collision_appends_suffix() {
    let dir = tempdir().expect("tempdir");
    let base = dir.path().join("Book (2025).m4b");
    std::fs::write(&base, b"existing").expect("seed file");

    let resolved = resolve_collision(&base).expect("should resolve collision");
    assert_ne!(resolved, base);
    assert!(
        resolved
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .starts_with("Book (2025)-1."),
        "expected suffix on collision"
    );
}
