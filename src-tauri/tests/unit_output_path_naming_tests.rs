use audiobook_boss_lib::output_artifact::{
    build_output_path_preview, NamingPreset, OutputNamingConfig,
};
use audiobook_boss_lib::AudiobookMetadata;
use tempfile::TempDir;

fn sample_metadata() -> AudiobookMetadata {
    AudiobookMetadata {
        title: Some("Dune".to_string()),
        artist: Some("Frank Herbert".to_string()),
        album: None,
        composer: None,
        genre: None,
        date: Some("1965".to_string()),
        track: None,
        disk: None,
        comment: None,
        description: None,
        series: Some("Dune Saga".to_string()),
        series_part: Some("1".to_string()),
        subseries: Some("Discovery".to_string()),
        subseries_part: Some("1".to_string()),
        album_sort: None,
        cover_art: None,
    }
}

#[test]
fn abs_default_preview_matches_existing_abs_layout() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let metadata = sample_metadata();
    let naming = OutputNamingConfig::default();

    let preview = build_output_path_preview(temp_dir.path(), Some(&metadata), naming, None)
        .expect("build preview path");

    let expected = temp_dir
        .path()
        .join("Frank Herbert")
        .join("Dune Saga")
        .join("Part 1 - Discovery")
        .join("Book 1 - Dune")
        .join("Book 1 - Dune.m4b");
    assert_eq!(preview, expected);
}

#[test]
fn custom_template_substitutes_whitelisted_tokens() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let metadata = sample_metadata();
    let naming = OutputNamingConfig {
        preset: NamingPreset::CustomTemplate,
        include_year: false,
        custom_template: Some("{author}/{series}/{title}-{seriesPart}-{year}".to_string()),
    };

    let preview = build_output_path_preview(temp_dir.path(), Some(&metadata), naming, None)
        .expect("build preview path");

    let expected = temp_dir
        .path()
        .join("Frank Herbert")
        .join("Dune Saga")
        .join("Dune-1-1965.m4b");
    assert_eq!(preview, expected);
}

#[test]
fn custom_template_rejects_unknown_tokens() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let metadata = sample_metadata();
    let naming = OutputNamingConfig {
        preset: NamingPreset::CustomTemplate,
        include_year: false,
        custom_template: Some("{author}/{bogus}/{title}".to_string()),
    };

    let err = build_output_path_preview(temp_dir.path(), Some(&metadata), naming, None)
        .expect_err("expected unknown token error");
    assert!(err.to_string().contains("Unknown template token"));
}

#[test]
fn custom_template_rejects_traversal_and_absolute_paths() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let metadata = sample_metadata();

    let traversal = OutputNamingConfig {
        preset: NamingPreset::CustomTemplate,
        include_year: false,
        custom_template: Some("../{title}".to_string()),
    };
    let traversal_err =
        build_output_path_preview(temp_dir.path(), Some(&metadata), traversal, None)
            .expect_err("expected traversal rejection");
    assert!(traversal_err.to_string().contains("traversal"));

    let absolute = OutputNamingConfig {
        preset: NamingPreset::CustomTemplate,
        include_year: false,
        custom_template: Some("/{author}/{title}".to_string()),
    };
    let absolute_err = build_output_path_preview(temp_dir.path(), Some(&metadata), absolute, None)
        .expect_err("expected absolute path rejection");
    assert!(absolute_err.to_string().contains("relative path"));
}

#[test]
fn custom_template_auto_appends_m4b_extension() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let metadata = sample_metadata();
    let naming = OutputNamingConfig {
        preset: NamingPreset::CustomTemplate,
        include_year: false,
        custom_template: Some("{author}/{title}".to_string()),
    };

    let preview = build_output_path_preview(temp_dir.path(), Some(&metadata), naming, None)
        .expect("build preview path");

    assert_eq!(
        preview.file_name().and_then(|s| s.to_str()),
        Some("Dune.m4b")
    );
}

#[test]
fn custom_template_with_missing_series_skips_empty_segment() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut metadata = sample_metadata();
    metadata.series = None;
    metadata.series_part = None;
    metadata.subseries = None;
    metadata.subseries_part = None;

    let naming = OutputNamingConfig {
        preset: NamingPreset::CustomTemplate,
        include_year: false,
        custom_template: Some("{series}/{title}".to_string()),
    };

    let preview = build_output_path_preview(temp_dir.path(), Some(&metadata), naming, None)
        .expect("build preview path");

    assert_eq!(preview, temp_dir.path().join("Dune.m4b"));
}
