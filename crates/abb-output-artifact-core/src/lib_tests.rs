use super::*;
use abb_metadata_core::AudiobookMetadata;

fn sample_naming_metadata() -> NamingMetadata {
    NamingMetadata::from_metadata(&AudiobookMetadata {
        title: Some("Dune".to_string()),
        artist: Some("Frank Herbert".to_string()),
        date: Some("1965".to_string()),
        series: Some("Dune Saga".to_string()),
        series_part: Some("1".to_string()),
        subseries: Some("Discovery".to_string()),
        subseries_part: Some("1".to_string()),
        ..Default::default()
    })
}

#[test]
fn a_name_is_measured_the_way_the_output_disk_counts_it() {
    let cjk = "\u{672c}".repeat(100);
    assert_eq!(
        name_fits(std::ffi::OsStr::new(&cjk)),
        !cfg!(target_os = "linux"),
        "100 CJK characters are 100 UTF-16 units and 300 bytes"
    );
    assert!(name_fits(std::ffi::OsStr::new(&"a".repeat(255))));
    assert!(!name_fits(std::ffi::OsStr::new(&"\u{672c}".repeat(256))));
}

#[test]
fn derive_output_artifact_path_appends_preview_suffix() {
    let path = Path::new("/tmp/Book 1.m4b");
    let preview = derive_output_artifact_path(path, OutputKind::Preview).expect("preview");
    assert_eq!(preview, Path::new("/tmp/Book 1.preview.m4b"));
}

#[test]
fn derive_output_artifact_path_keeps_final_path_unchanged() {
    let path = Path::new("/tmp/Book 1.m4b");
    let final_path = derive_output_artifact_path(path, OutputKind::Final).expect("final");
    assert_eq!(final_path, path);
}

#[test]
fn normalize_subseries_folder_prefixes_when_needed() {
    assert_eq!(
        normalize_subseries_folder("Discovery", Some("1")),
        "Part 1 - Discovery"
    );
}

#[test]
fn normalize_subseries_folder_keeps_existing_prefix() {
    assert_eq!(
        normalize_subseries_folder("Part 2 - Rogue Castes", Some("2")),
        "Part 2 - Rogue Castes"
    );
}

#[test]
fn abs_default_builds_author_series_book_path() {
    let metadata = AudiobookMetadata {
        title: Some("The Motion Picture".to_string()),
        artist: Some("Gene Roddenberry".to_string()),
        series: Some("Star Trek".to_string()),
        series_part: Some("1".to_string()),
        date: Some("1979-12".to_string()),
        ..Default::default()
    };
    let naming = NamingMetadata::from_metadata(&metadata);
    let path = build_output_path_preview(
        Path::new("/out"),
        Some(&naming),
        OutputNamingConfig {
            include_year: true,
            ..Default::default()
        },
        None,
    )
    .expect("path");

    assert_eq!(
        path,
        Path::new("/out/Gene Roddenberry/Star Trek/Book 1 - 1979 - The Motion Picture/Book 1 - 1979 - The Motion Picture.m4b")
    );
}

#[test]
fn abs_default_builds_subseries_book_path() {
    let path = build_output_path_preview(
        Path::new("/out"),
        Some(&sample_naming_metadata()),
        OutputNamingConfig::default(),
        None,
    )
    .expect("path");

    assert_eq!(
        path,
        Path::new(
            "/out/Frank Herbert/Dune Saga/Part 1 - Discovery/Book 1 - Dune/Book 1 - Dune.m4b"
        )
    );
}

#[test]
fn default_naming_never_turns_a_dots_only_tag_into_a_folder_step() {
    let metadata = NamingMetadata::from_metadata(&AudiobookMetadata {
        title: Some(".".to_string()),
        artist: Some("..".to_string()),
        series: Some("..".to_string()),
        ..Default::default()
    });
    let path = build_output_path_preview(
        Path::new("/out"),
        Some(&metadata),
        OutputNamingConfig::default(),
        None,
    )
    .expect("path");
    assert!(path.starts_with("/out"));
    assert!(
        path.components().all(|part| !matches!(
            part,
            std::path::Component::ParentDir | std::path::Component::CurDir
        )),
        "{path:?}"
    );
}

#[test]
fn custom_template_rejects_path_traversal() {
    let err = build_output_path_preview(
        Path::new("/out"),
        None,
        OutputNamingConfig {
            preset: NamingPreset::CustomTemplate,
            include_year: false,
            custom_template: Some("../escape/{title}".to_string()),
        },
        Some(Path::new("/tmp/source.m4b")),
    )
    .expect_err("traversal should fail");

    assert!(err.to_string().contains("path traversal"));

    let absolute_err = build_output_path_preview(
        Path::new("/out"),
        Some(&sample_naming_metadata()),
        OutputNamingConfig {
            preset: NamingPreset::CustomTemplate,
            include_year: false,
            custom_template: Some("/{author}/{title}".to_string()),
        },
        None,
    )
    .expect_err("absolute path should fail");
    assert!(absolute_err.to_string().contains("relative path"));
}

#[test]
fn custom_template_substitutes_whitelisted_tokens() {
    let path = build_output_path_preview(
        Path::new("/out"),
        Some(&sample_naming_metadata()),
        OutputNamingConfig {
            preset: NamingPreset::CustomTemplate,
            include_year: false,
            custom_template: Some("{author}/{series}/{title}-{seriesPart}-{year}".to_string()),
        },
        None,
    )
    .expect("custom template");

    assert_eq!(
        path,
        Path::new("/out/Frank Herbert/Dune Saga/Dune-1-1965.m4b")
    );
}

#[test]
fn custom_template_rejects_unknown_tokens() {
    let err = build_output_path_preview(
        Path::new("/out"),
        Some(&sample_naming_metadata()),
        OutputNamingConfig {
            preset: NamingPreset::CustomTemplate,
            include_year: false,
            custom_template: Some("{author}/{bogus}/{title}".to_string()),
        },
        None,
    )
    .expect_err("unknown token should fail");

    assert!(err.to_string().contains("Unknown template token"));
}

#[test]
fn custom_template_appends_m4b_extension() {
    let path = build_output_path_preview(
        Path::new("/out"),
        Some(&sample_naming_metadata()),
        OutputNamingConfig {
            preset: NamingPreset::CustomTemplate,
            include_year: false,
            custom_template: Some("{author}/{title}".to_string()),
        },
        None,
    )
    .expect("custom template");

    assert_eq!(
        path.file_name().and_then(|value| value.to_str()),
        Some("Dune.m4b")
    );
}

#[test]
fn custom_template_skips_empty_series_segment() {
    let metadata = NamingMetadata::from_metadata(&AudiobookMetadata {
        title: Some("Dune".to_string()),
        artist: Some("Frank Herbert".to_string()),
        ..Default::default()
    });
    let path = build_output_path_preview(
        Path::new("/out"),
        Some(&metadata),
        OutputNamingConfig {
            preset: NamingPreset::CustomTemplate,
            include_year: false,
            custom_template: Some("{series}/{title}".to_string()),
        },
        None,
    )
    .expect("custom template");

    assert_eq!(path, Path::new("/out/Dune.m4b"));
}

#[test]
fn pure_action_helpers_classify_write_and_hard_blocks() {
    assert!(action_requires_output_write(PlannedOutputAction::Write));
    assert!(!action_requires_output_write(
        PlannedOutputAction::SkipExisting
    ));

    let plan = ResolvedOutputPlan {
        kind: OutputKind::Final,
        requested_path: PathBuf::from("/tmp/input.m4b"),
        resolved_path: PathBuf::from("/tmp/input.m4b"),
        rename_candidate: None,
        collision: Some(OutputCollision {
            kind: OutputCollisionKind::SourceDestinationOverlap,
            conflicting_path: Some(PathBuf::from("/tmp/input.m4b")),
            detail: None,
        }),
        action: PlannedOutputAction::ReviewRequired,
    };
    assert!(plan_is_hard_block(&plan));
}
