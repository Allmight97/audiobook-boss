use super::{plan_metadata_outcome, plan_metadata_outcome_from, MetadataOutcomeRequest};
use crate::metadata::{
    AlbumSortPatchOp, AudiobookMetadata, CoverArtPassthroughPolicy, MetadataIntentPatch, PatchOp,
};
use crate::output_artifact::build_output_path_preview;
use crate::output_artifact::OutputNamingConfig;
use std::path::Path;

#[test]
fn effective_processing_metadata_no_patch_reads_source_or_errors() {
    let missing_source = Path::new("/path/that/does/not/exist/input.m4b");
    let outcome = plan_metadata_outcome(MetadataOutcomeRequest {
        input_path: Some(missing_source),
        intent_patch: None,
    });
    assert!(
        outcome.is_err(),
        "missing input should fail read, not return empty metadata"
    );
}

#[test]
fn effective_processing_metadata_uses_overlay_without_source_file() {
    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Overlay Only".to_string())),
        ..Default::default()
    };

    let resolved = plan_metadata_outcome(MetadataOutcomeRequest {
        input_path: None,
        intent_patch: Some(&patch),
    })
    .expect("metadata resolves")
    .effective_metadata;

    assert_eq!(
        resolved.and_then(|value| value.title),
        Some("Overlay Only".to_string())
    );
}

#[test]
fn processing_outputs_carry_series_album_sort_unless_intent_states_one() {
    let album_sort = |album_sort| {
        let patch = MetadataIntentPatch {
            title: Some(PatchOp::Set("Sunreach".to_string())),
            series: Some(PatchOp::Set("Skyward".to_string())),
            series_part: Some(PatchOp::Set("2.5".to_string())),
            album_sort,
            ..Default::default()
        };
        plan_metadata_outcome(MetadataOutcomeRequest {
            input_path: None,
            intent_patch: Some(&patch),
        })
        .expect("metadata resolves")
        .effective_metadata
        .and_then(|metadata| metadata.album_sort)
    };

    assert_eq!(album_sort(None).as_deref(), Some("Skyward 02.5 - Sunreach"));
    assert_eq!(album_sort(Some(AlbumSortPatchOp::Clear)), None);
    assert_eq!(
        album_sort(Some(AlbumSortPatchOp::Set("Custom".to_string()))).as_deref(),
        Some("Custom")
    );
}

#[test]
fn write_intent_carries_the_processing_album_sort_for_source_tag_routes() {
    let plan = |patch: Option<&MetadataIntentPatch>| {
        plan_metadata_outcome(MetadataOutcomeRequest {
            input_path: None,
            intent_patch: patch,
        })
        .expect("metadata resolves")
        .write_intent
    };
    let series = MetadataIntentPatch {
        title: Some(PatchOp::Set("Sunreach".to_string())),
        series: Some(PatchOp::Set("Skyward".to_string())),
        series_part: Some(PatchOp::Set("2.5".to_string())),
        ..Default::default()
    };
    assert_eq!(
        plan(Some(&series)).and_then(|intent| intent.album_sort),
        Some(AlbumSortPatchOp::Set("Skyward 02.5 - Sunreach".to_string()))
    );

    let cleared = MetadataIntentPatch {
        album_sort: Some(AlbumSortPatchOp::Clear),
        ..series
    };
    assert_eq!(
        plan(Some(&cleared)).and_then(|intent| intent.album_sort),
        Some(AlbumSortPatchOp::Clear)
    );
    assert_eq!(plan(None), None, "nothing to write without intent or a key");

    let no_series = MetadataIntentPatch {
        title: Some(PatchOp::Set("Standalone".to_string())),
        ..Default::default()
    };
    assert_eq!(
        plan(Some(&no_series)).and_then(|intent| intent.album_sort),
        None,
        "an underivable key leaves the source's album sort untouched"
    );
}

#[test]
fn effective_processing_metadata_rejects_invalid_patch_values() {
    let patch = MetadataIntentPatch {
        date: Some(PatchOp::Set("2024-99".to_string())),
        ..Default::default()
    };

    let err = plan_metadata_outcome(MetadataOutcomeRequest {
        input_path: None,
        intent_patch: Some(&patch),
    })
    .expect_err("invalid patch should fail");
    assert!(
        err.to_string().contains("Publication date"),
        "unexpected error: {err}"
    );
}

#[test]
fn a_source_with_legacy_series_parts_names_its_output_without_them() {
    let source = AudiobookMetadata {
        title: Some("Legacy Source".to_string()),
        series: Some("Series".to_string()),
        series_part: Some("7/8".to_string()),
        subseries: Some("Subseries".to_string()),
        subseries_part: Some("2/3".to_string()),
        ..Default::default()
    };
    let input_path = Path::new("/tmp/source.m4b");

    let outcome = plan_metadata_outcome_from(
        Some(source),
        MetadataOutcomeRequest {
            input_path: Some(input_path),
            intent_patch: None,
        },
    )
    .expect("source resolves");
    let naming = outcome.naming_metadata.expect("naming metadata");

    assert_eq!(naming.title(), Some("Legacy Source"));
    assert_eq!(naming.series(), Some("Series"));
    assert_eq!(naming.series_part(), None);
    assert_eq!(naming.subseries(), Some("Subseries"));
    assert_eq!(naming.subseries_part(), None);
    let output_path = build_output_path_preview(
        Path::new("/tmp"),
        Some(&naming),
        OutputNamingConfig::default(),
        Some(input_path),
        "m4b",
    )
    .expect("legacy source naming should no longer fail");
    assert!(output_path.to_string_lossy().contains("Legacy Source"));

    let empty = plan_metadata_outcome(MetadataOutcomeRequest {
        input_path: None,
        intent_patch: None,
    })
    .expect("empty request resolves");
    assert!(
        empty.naming_metadata.is_none(),
        "empty request should not invent naming metadata"
    );
}

#[test]
fn a_title_edit_names_the_output_by_the_new_title_without_legacy_parts() {
    let source = AudiobookMetadata {
        title: Some("Patched Source".to_string()),
        series: Some("Series".to_string()),
        series_part: Some("7/8".to_string()),
        ..Default::default()
    };
    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Renamed".to_string())),
        ..Default::default()
    };
    let input_path = Path::new("/tmp/source.m4b");

    let outcome = plan_metadata_outcome_from(
        Some(source),
        MetadataOutcomeRequest {
            input_path: Some(input_path),
            intent_patch: Some(&patch),
        },
    )
    .expect("title edit resolves");
    let naming = outcome.naming_metadata.expect("naming metadata");

    assert_eq!(naming.title(), Some("Renamed"));
    assert_eq!(naming.series_part(), None);
    assert_eq!(
        outcome.effective_metadata.and_then(|value| value.title),
        Some("Renamed".to_string())
    );
    let output_path = build_output_path_preview(
        Path::new("/tmp"),
        Some(&naming),
        OutputNamingConfig::default(),
        Some(input_path),
        "m4b",
    )
    .expect("a title edit should not fail on an inherited legacy part");
    assert!(output_path.to_string_lossy().contains("Renamed"));
}

#[test]
fn a_series_edit_keeps_an_inherited_invalid_series_part_visible() {
    let metadata = AudiobookMetadata {
        title: Some("Patched Source".to_string()),
        series: Some("Series".to_string()),
        series_part: Some("7/8".to_string()),
        ..Default::default()
    };
    let patch = MetadataIntentPatch {
        series: Some(PatchOp::Set("Renamed Series".to_string())),
        ..Default::default()
    };

    let err = plan_metadata_outcome_from(
        Some(metadata),
        MetadataOutcomeRequest {
            input_path: Some(Path::new("/tmp/source.m4b")),
            intent_patch: Some(&patch),
        },
    )
    .expect_err("series patch should keep inherited invalid part visible");

    assert!(
        err.to_string().contains("Series sequence"),
        "unexpected error: {err}"
    );
}

#[test]
fn outcome_plan_populates_naming_from_resolved_metadata() {
    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Patched Title".to_string())),
        series: Some(PatchOp::Set("Series".to_string())),
        series_part: Some(PatchOp::Set("7".to_string())),
        ..Default::default()
    };
    let outcome = plan_metadata_outcome(MetadataOutcomeRequest {
        input_path: None,
        intent_patch: Some(&patch),
    })
    .expect("metadata outcome should resolve");
    let naming = outcome.naming_metadata.expect("naming metadata");

    assert_eq!(naming.title(), Some("Patched Title"));
    assert_eq!(naming.series(), Some("Series"));
    assert_eq!(naming.series_part(), Some("7"));
}

#[test]
fn outcome_plan_reports_cover_art_clear_policy() {
    let patch = MetadataIntentPatch {
        cover_art: Some(PatchOp::Clear),
        ..Default::default()
    };

    let outcome = plan_metadata_outcome(MetadataOutcomeRequest {
        input_path: None,
        intent_patch: Some(&patch),
    })
    .expect("metadata outcome resolves");

    assert!(
        matches!(
            outcome.cover_art_passthrough,
            CoverArtPassthroughPolicy::SuppressAfterExplicitClear
        ),
        "explicit cover clear must suppress source cover art passthrough"
    );
}
