use crate::metadata::{
    plan_metadata_outcome, plan_metadata_write, AlbumSortPatchOp, CoverArtPassthroughPolicy,
    MetadataIntentPatch, MetadataOutcomeRequest, PatchOp,
};

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

    let outcome = plan_metadata_outcome(MetadataOutcomeRequest {
        input_path: None,
        intent_patch: Some(&patch),
    })
    .expect("metadata resolves");
    let metadata = outcome.effective_metadata.expect("overlay metadata");

    assert_eq!(metadata.title.as_deref(), Some("Contract Title"));
    assert_eq!(metadata.artist, None);
    assert_eq!(metadata.series, None);
    assert_eq!(metadata.cover_art, None);
    assert!(
        matches!(
            outcome.cover_art_passthrough,
            CoverArtPassthroughPolicy::SuppressAfterExplicitClear
        ),
        "explicit cover clear should suppress passthrough cover art"
    );

    let write_plan = plan_metadata_write(&patch)
        .expect("write metadata")
        .metadata;
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

    let err = plan_metadata_outcome(MetadataOutcomeRequest {
        input_path: None,
        intent_patch: Some(&patch),
    })
    .expect_err("invalid date should fail");

    assert!(err.to_string().contains("Publication date"));
}

#[test]
fn metadata_intent_plan_contract_uses_resolved_metadata_for_naming() {
    let patch = MetadataIntentPatch {
        title: PatchOp::Set("Patched Title".to_string()),
        series: PatchOp::Set("Series".to_string()),
        series_part: PatchOp::Set("7".to_string()),
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
