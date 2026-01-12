use std::path::PathBuf;

use audiobook_boss_lib::commands::audio::{
    build_output_path, resolve_collision, OutputNamingConfig,
};
use audiobook_boss_lib::metadata::AudiobookMetadata;
use tempfile::tempdir;

fn sample_metadata() -> AudiobookMetadata {
    AudiobookMetadata {
        title: Some("Flybot testing".to_string()),
        artist: Some("Dennis E. Taylor".to_string()),
        series: Some("FLY BOT SERIES".to_string()),
        series_part: Some("24".to_string()),
        date: Some(2025),
        ..Default::default()
    }
}

#[test]
fn build_output_path_with_abs_defaults() {
    let dir = tempdir().expect("tempdir");
    let md = sample_metadata();

    let path = build_output_path(
        dir.path(),
        Some(&md),
        OutputNamingConfig::default(),
        None,
    )
    .expect("should build path");

    let expected = dir.path().join(
        "Dennis E. Taylor/FLY BOT SERIES/Book 24 - Flybot testing/Book 24 - Flybot testing.m4b",
    );
    assert_eq!(path, expected);
}

#[test]
fn build_output_path_with_abs_year_places_year_after_series_part() {
    let dir = tempdir().expect("tempdir");
    let md = sample_metadata();

    let path = build_output_path(
        dir.path(),
        Some(&md),
        OutputNamingConfig {
            abs_compatible: true,
            include_year: true,
        },
        None,
    )
    .expect("should build path");

    let expected = dir.path().join(
        "Dennis E. Taylor/FLY BOT SERIES/Book 24 - 2025 - Flybot testing/Book 24 - 2025 - Flybot testing.m4b",
    );
    assert_eq!(path, expected);
}

#[test]
fn build_output_path_with_abs_year_prefixes_when_no_series() {
    let dir = tempdir().expect("tempdir");
    let md = AudiobookMetadata {
        title: Some("Standalone Title".to_string()),
        artist: Some("Ada Palmer".to_string()),
        date: Some(2025),
        ..Default::default()
    };

    let path = build_output_path(
        dir.path(),
        Some(&md),
        OutputNamingConfig {
            abs_compatible: true,
            include_year: true,
        },
        None,
    )
    .expect("should build path");

    let expected = dir
        .path()
        .join("Ada Palmer/2025 - Standalone Title/2025 - Standalone Title.m4b");
    assert_eq!(path, expected);
}

#[test]
fn build_output_path_without_abs_structure_uses_simple_filename() {
    let dir = tempdir().expect("tempdir");
    let source = PathBuf::from("/tmp/source/ExampleBook.m4b");

    let path = build_output_path(
        dir.path(),
        None,
        OutputNamingConfig {
            abs_compatible: false,
            include_year: false,
        },
        Some(&source),
    )
    .expect("should build path");

    let expected = dir.path().join("ExampleBook.m4b");
    assert_eq!(path, expected);
}

#[test]
fn build_output_path_keeps_full_title() {
    let dir = tempdir().expect("tempdir");
    let md = AudiobookMetadata {
        title: Some("Demon World: Undying Mercenaries, Book 24 - Special Edition".to_string()),
        artist: Some("B.V. Larson".to_string()),
        series: Some("Undying Mercenaries".to_string()),
        series_part: Some("24".to_string()),
        ..Default::default()
    };

    let path = build_output_path(dir.path(), Some(&md), OutputNamingConfig::default(), None)
        .expect("should build path");

    let expected = dir.path().join(
        "B.V. Larson/Undying Mercenaries/Book 24 - Demon World - Undying Mercenaries - Book 24 - Special Edition/Book 24 - Demon World - Undying Mercenaries - Book 24 - Special Edition.m4b",
    );
    assert_eq!(path, expected);
}

#[test]
fn build_output_path_rejects_series_part_with_slash() {
    let dir = tempdir().expect("tempdir");
    let md = AudiobookMetadata {
        title: Some("Welcome to Paradise".to_string()),
        artist: Some("Ryk Brown".to_string()),
        series: Some("The Frontiers Saga".to_string()),
        series_part: Some("14/15".to_string()),
        ..Default::default()
    };

    let result = build_output_path(dir.path(), Some(&md), OutputNamingConfig::default(), None);
    assert!(result.is_err(), "should reject series part containing '/' characters");
}

#[test]
fn build_output_path_sanitizes_colons() {
    let dir = tempdir().expect("tempdir");
    let md = AudiobookMetadata {
        title: Some("Rogue: Aftermath".to_string()),
        artist: Some("Ada Palmer".to_string()),
        ..Default::default()
    };

    let path = build_output_path(dir.path(), Some(&md), OutputNamingConfig::default(), None)
        .expect("should build path");

    let expected = dir
        .path()
        .join("Ada Palmer/Rogue - Aftermath/Rogue - Aftermath.m4b");
    assert_eq!(path, expected);
}

#[test]
fn build_output_path_preserves_commas_in_author() {
    let dir = tempdir().expect("tempdir");
    let md = AudiobookMetadata {
        title: Some("Wizard, Tower".to_string()),
        artist: Some("Goodkind, Terry".to_string()),
        ..Default::default()
    };

    let path = build_output_path(dir.path(), Some(&md), OutputNamingConfig::default(), None)
        .expect("should build path");

    let expected = dir
        .path()
        .join("Goodkind, Terry/Wizard - Tower/Wizard - Tower.m4b");
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
