use crate::metadata::{
    resolve_effective_processing_metadata, resolve_naming_metadata, AlbumSortPatchOp,
    AudiobookMetadata, MetadataIntentPatch, PatchOp,
};
use std::path::Path;

#[test]
fn metadata_intent_plan_contract_preserves_set_clear_noop_semantics() {
    let patch = MetadataIntentPatch {
        title: PatchOp::Set("Contract Title".to_string()),
        artist: PatchOp::Clear,
        series: PatchOp::Noop,
        album_sort: AlbumSortPatchOp::Recompute,
        cover_art: PatchOp::Clear,
        ..Default::default()
    };

    let resolved =
        resolve_effective_processing_metadata(None, Some(&patch)).expect("metadata resolves");
    let metadata = resolved.expect("overlay metadata");

    assert_eq!(metadata.title.as_deref(), Some("Contract Title"));
    assert_eq!(metadata.artist, None);
    assert_eq!(metadata.series, None);
    assert_eq!(metadata.cover_art, None);

    let write_plan = patch.to_write_metadata().expect("write metadata");
    assert_eq!(write_plan.title.as_deref(), Some("Contract Title"));
    assert_eq!(write_plan.artist.as_deref(), Some(""));
    assert_eq!(write_plan.cover_art, Some(Vec::new()));
}

#[test]
fn metadata_intent_plan_contract_rejects_invalid_publication_dates() {
    let patch = MetadataIntentPatch {
        date: PatchOp::Set("2026-99".to_string()),
        ..Default::default()
    };

    let err = resolve_effective_processing_metadata(None, Some(&patch))
        .expect_err("invalid date should fail");

    assert!(err.to_string().contains("Publication date"));
}

#[test]
fn metadata_intent_plan_contract_uses_resolved_metadata_for_naming() {
    let resolved = AudiobookMetadata {
        title: Some("Resolved Title".to_string()),
        series: Some("Series".to_string()),
        series_part: Some("7/8".to_string()),
        ..Default::default()
    };
    let untouched_source_naming =
        resolve_naming_metadata(Some(&resolved), Some(Path::new("/tmp/source.m4b")), None)
            .expect("naming metadata");

    assert_eq!(
        untouched_source_naming.title.as_deref(),
        Some("Resolved Title")
    );
    assert_eq!(untouched_source_naming.series.as_deref(), Some("Series"));
    assert_eq!(
        untouched_source_naming.series_part, None,
        "untouched source naming should scrub legacy slash series parts"
    );

    let patch = MetadataIntentPatch {
        title: PatchOp::Set("Patched Title".to_string()),
        ..Default::default()
    };
    let patched_naming = resolve_naming_metadata(
        Some(&resolved),
        Some(Path::new("/tmp/source.m4b")),
        Some(&patch),
    )
    .expect("patched naming metadata");

    assert_eq!(patched_naming.series_part.as_deref(), Some("7/8"));
}
