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
fn derive_output_artifact_path_suffixes_only_previews() {
    let path = Path::new("/tmp/Book 1.m4b");
    let preview = derive_output_artifact_path(path, OutputKind::Preview).expect("preview");
    assert_eq!(preview, Path::new("/tmp/Book 1.preview.m4b"));
    assert_eq!(
        derive_output_artifact_path(path, OutputKind::Final).expect("final"),
        path
    );
}

#[test]
fn a_preview_keeps_the_extension_of_the_file_it_previews() {
    for extension in ["m4b", "m4a", "mka"] {
        let title = Path::new("/tmp").join(format!("Vol. 2.{extension}"));
        assert_eq!(
            derive_output_artifact_path(&title, OutputKind::Preview).expect("preview"),
            Path::new("/tmp").join(format!("Vol. 2.preview.{extension}"))
        );
    }
    assert!(derive_output_artifact_path(Path::new("/tmp/Book"), OutputKind::Preview).is_err());
}

#[test]
fn the_requested_extension_ends_every_naming_preset() {
    let naming = |template: Option<&str>| OutputNamingConfig {
        preset: if template.is_some() {
            NamingPreset::CustomTemplate
        } else {
            NamingPreset::AbsDefault
        },
        include_year: false,
        custom_template: template.map(str::to_string),
    };
    let name = |template: Option<&str>, extension: &str| {
        build_output_path_preview(
            Path::new("/out"),
            Some(&sample_naming_metadata()),
            naming(template),
            None,
            extension,
        )
        .expect("path")
        .file_name()
        .and_then(|value| value.to_str().map(str::to_string))
        .expect("file name")
    };
    assert_eq!(name(None, "mka"), "Book 1 - Dune.mka");
    assert_eq!(name(Some("{title}"), "m4a"), "Dune.m4a");
    assert_eq!(name(Some("{title}.m4b"), "m4a"), "Dune.m4a");
    assert_eq!(name(Some("{title}.M4B"), "mka"), "Dune.mka");
    assert_eq!(name(Some("{title}.m4a"), "m4a"), "Dune.m4a.m4a");
    assert_eq!(name(Some("{title}.m4b.m4b"), "m4b"), "Dune.m4b.m4b");
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
        "m4b",
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
        "m4b",
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
        "m4b",
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
        "m4b",
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
        "m4b",
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
        "m4b",
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
        "m4b",
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
        "m4b",
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
        "m4b",
    )
    .expect("custom template");

    assert_eq!(path, Path::new("/out/Dune.m4b"));
}

#[test]
fn an_output_that_resolves_to_its_own_source_is_blocked() {
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
    assert_eq!(plan.verdict(), OutputVerdict::Blocked);
}

const ALL_KINDS: [OutputCollisionKind; 5] = [
    OutputCollisionKind::ExistingFile,
    OutputCollisionKind::BatchDuplicate,
    OutputCollisionKind::SourceDestinationOverlap,
    OutputCollisionKind::CanonicalPathOverlap,
    OutputCollisionKind::CaseInsensitiveMatch,
];

#[test]
fn every_collision_kind_under_every_policy_resolves_to_its_pinned_action() {
    use CollisionPolicy::{Fail, RenameNew, ReplaceExisting, SkipExisting};
    use PlannedOutputAction::{
        RenameNew as Rename, ReplaceExisting as Replace, ReviewRequired as Review,
        SkipExisting as Skip,
    };
    let policies = [Fail, ReplaceExisting, RenameNew, SkipExisting];
    let table = [
        (
            OutputCollisionKind::ExistingFile,
            false,
            [Review, Replace, Rename, Skip],
        ),
        (
            OutputCollisionKind::ExistingFile,
            true,
            [Review, Replace, Rename, Skip],
        ),
        (
            OutputCollisionKind::BatchDuplicate,
            false,
            [Review, Rename, Rename, Skip],
        ),
        (
            OutputCollisionKind::BatchDuplicate,
            true,
            [Review, Rename, Rename, Skip],
        ),
        (
            OutputCollisionKind::CaseInsensitiveMatch,
            false,
            [Review, Replace, Rename, Skip],
        ),
        (
            OutputCollisionKind::CaseInsensitiveMatch,
            true,
            [Review, Rename, Rename, Skip],
        ),
        (
            OutputCollisionKind::SourceDestinationOverlap,
            false,
            [Review; 4],
        ),
        (
            OutputCollisionKind::SourceDestinationOverlap,
            true,
            [Review; 4],
        ),
        (
            OutputCollisionKind::CanonicalPathOverlap,
            false,
            [Review; 4],
        ),
        (OutputCollisionKind::CanonicalPathOverlap, true, [Review; 4]),
    ];
    assert_eq!(table.len(), ALL_KINDS.len() * 2);
    for (kind, claimed, expected) in table {
        for (policy, action) in policies.into_iter().zip(expected) {
            assert_eq!(
                resolve_action(Some(kind), claimed, policy),
                action,
                "{kind:?} claimed={claimed} under {policy:?}"
            );
        }
    }
    for policy in policies {
        assert_eq!(
            resolve_action(None, false, policy),
            PlannedOutputAction::Write
        );
    }
}

#[test]
fn only_a_review_required_output_needs_a_decision_and_a_source_overlap_can_never_proceed() {
    for kind in ALL_KINDS {
        let expected = if kind.is_hard_block() {
            OutputVerdict::Blocked
        } else {
            OutputVerdict::Review
        };
        assert_eq!(
            output_verdict(PlannedOutputAction::ReviewRequired, Some(kind)),
            expected,
            "{kind:?}"
        );
        for action in [
            PlannedOutputAction::Write,
            PlannedOutputAction::ReplaceExisting,
            PlannedOutputAction::RenameNew,
            PlannedOutputAction::SkipExisting,
        ] {
            assert_eq!(output_verdict(action, Some(kind)), OutputVerdict::Proceed);
        }
    }
    assert_eq!(
        output_verdict(PlannedOutputAction::Write, None),
        OutputVerdict::Proceed
    );
}
