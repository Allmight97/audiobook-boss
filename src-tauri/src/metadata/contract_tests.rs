use crate::metadata::{
    plan_metadata_outcome, plan_metadata_write, validate_metadata_intent_patch, AlbumSortPatchOp,
    AudiobookMetadata, CoverArtPassthroughPolicy, MetadataIntentPatch, MetadataOutcomeRequest,
    PatchOp,
};
use abb_metadata_core::{MetadataIntentValidationCode, MetadataIntentValidationField};

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
fn metadata_intent_validation_contract_reports_field_errors_as_data() {
    let patch = MetadataIntentPatch {
        date: PatchOp::Set("not a date".to_string()),
        series_part: PatchOp::Set("1/2".to_string()),
        ..Default::default()
    };

    let result = validate_metadata_intent_patch(&patch);

    assert!(!result.is_valid);
    assert_eq!(result.field_errors.len(), 2);
    assert!(
        result
            .field_errors
            .iter()
            .any(|error| error.message.contains("Publication date")),
        "expected publication-date field error"
    );
    assert!(
        result
            .field_errors
            .iter()
            .any(|error| error.message.contains("Series sequence")),
        "expected series-part field error"
    );
}

#[test]
fn metadata_intent_validation_contract_normalizes_valid_publication_date() {
    let patch = MetadataIntentPatch {
        date: PatchOp::Set("2024-07-15T12:00:00Z".to_string()),
        ..Default::default()
    };

    let result = validate_metadata_intent_patch(&patch);

    assert!(result.is_valid);
    assert!(result.field_errors.is_empty());
    assert_eq!(
        result.metadata_patch.date,
        PatchOp::Set("2024-07".to_string())
    );
}

#[test]
fn metadata_intent_validation_contract_reports_structured_field_codes() {
    let patch = MetadataIntentPatch {
        date: PatchOp::Set("2024-13".to_string()),
        series_part: PatchOp::Set("7/8".to_string()),
        subseries_part: PatchOp::Set("2/3".to_string()),
        ..Default::default()
    };

    let result = validate_metadata_intent_patch(&patch);

    assert!(!result.is_valid);
    assert_eq!(result.field_errors.len(), 3);
    assert!(result.field_errors.iter().any(|error| {
        error.field == MetadataIntentValidationField::Date
            && error.code == MetadataIntentValidationCode::PublicationDateSyntax
    }));
    assert!(result.field_errors.iter().any(|error| {
        error.field == MetadataIntentValidationField::SeriesPart
            && error.code == MetadataIntentValidationCode::SeriesPartContainsSlash
            && error.message.contains("Series sequence")
    }));
    assert!(result.field_errors.iter().any(|error| {
        error.field == MetadataIntentValidationField::SubseriesPart
            && error.code == MetadataIntentValidationCode::SubseriesPartContainsSlash
            && error.message.contains("Sub-series sequence")
    }));
}

#[test]
fn metadata_intent_validation_contract_preserves_invalid_date_for_validation() {
    let patch = MetadataIntentPatch::from(AudiobookMetadata {
        date: Some("not a date".to_string()),
        ..Default::default()
    });

    assert_eq!(patch.date, PatchOp::Set("not a date".to_string()));
    let result = validate_metadata_intent_patch(&patch);
    assert!(!result.is_valid);
    assert_eq!(
        result.field_errors.first().map(|error| error.field),
        Some(MetadataIntentValidationField::Date)
    );
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
