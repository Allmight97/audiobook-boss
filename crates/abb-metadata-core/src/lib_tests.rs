#[test]
fn merge_keeps_earlier_fields_and_lets_later_requests_win() {
    let mut pending = MetadataIntentPatch {
        title: Some(PatchOp::Set("First".into())),
        artist: Some(PatchOp::Set("Author".into())),
        ..Default::default()
    };
    pending.merge(&MetadataIntentPatch {
        title: Some(PatchOp::Clear),
        cover_art: Some(PatchOp::Set(vec![1, 2])),
        ..Default::default()
    });

    assert_eq!(pending.title, Some(PatchOp::Clear));
    assert_eq!(pending.artist, Some(PatchOp::Set("Author".into())));
    assert_eq!(pending.cover_art, Some(PatchOp::Set(vec![1, 2])));
    assert!(pending.is_actionable());
    assert!(!MetadataIntentPatch::default().is_actionable());
}

#[test]
fn between_sets_changed_fields_clears_removed_ones_and_reapplies_to_the_target() {
    let from = AudiobookMetadata {
        title: Some("Old".into()),
        genre: Some("Fantasy".into()),
        album_sort: Some("Series 1 - Old".into()),
        cover_art: Some(vec![1]),
        track: Some((1, None)),
        ..Default::default()
    };
    let to = AudiobookMetadata {
        title: Some("New".into()),
        series: Some("Series".into()),
        album_sort: Some("Series 1 - New".into()),
        cover_art: Some(vec![2]),
        track: Some((1, None)),
        ..Default::default()
    };

    let patch = MetadataIntentPatch::between(&from, &to);
    assert_eq!(patch.title, Some(PatchOp::Set("New".into())));
    assert_eq!(patch.genre, Some(PatchOp::Clear));
    assert_eq!(patch.series, Some(PatchOp::Set("Series".into())));
    assert_eq!(
        patch.album_sort,
        Some(AlbumSortPatchOp::Set("Series 1 - New".into()))
    );
    assert_eq!(patch.cover_art, Some(PatchOp::Set(vec![2])));
    assert_eq!(patch.track, None, "an unchanged field is left alone");
    assert_eq!(patch.overlay(&from), to);
    assert!(!MetadataIntentPatch::between(&to, &to).is_actionable());
}

#[test]
fn overlay_sets_and_clears_without_validating() {
    let base = AudiobookMetadata {
        title: Some("Old".into()),
        genre: Some("Fantasy".into()),
        album_sort: Some("Old Sort".into()),
        series_part: Some("1".into()),
        ..Default::default()
    };
    let shown = MetadataIntentPatch {
        title: Some(PatchOp::Set("New".into())),
        genre: Some(PatchOp::Clear),
        series_part: Some(PatchOp::Set("1/2".into())),
        ..Default::default()
    }
    .overlay(&base);

    assert_eq!(shown.title.as_deref(), Some("New"));
    assert_eq!(shown.genre, None);
    assert_eq!(shown.series_part.as_deref(), Some("1/2"));
    assert_eq!(shown.album_sort.as_deref(), Some("Old Sort"));
}

#[test]
fn a_read_fills_known_tags_and_only_text_tags_make_it_a_baseline() {
    let mut known = AudiobookMetadata {
        title: Some("Saved".into()),
        artist: Some("Saved Author".into()),
        ..Default::default()
    };
    known.fill_from(AudiobookMetadata {
        title: Some("From File".into()),
        cover_art: Some(vec![9]),
        ..Default::default()
    });

    assert_eq!(known.title.as_deref(), Some("From File"));
    assert_eq!(known.artist.as_deref(), Some("Saved Author"));
    assert!(known.has_text_tags());
    assert!(!AudiobookMetadata::default().has_text_tags());
    assert!(!AudiobookMetadata {
        cover_art: Some(vec![9]),
        ..Default::default()
    }
    .has_text_tags());
}

use super::*;

#[test]
fn metadata_intent_patch_applies_set_and_clear_ops() {
    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Project Hail Mary".to_string())),
        artist: Some(PatchOp::Clear),
        date: Some(PatchOp::Clear),
        album_sort: Some(AlbumSortPatchOp::Clear),
        cover_art: Some(PatchOp::Clear),
        ..Default::default()
    };

    let metadata = patch
        .to_write_plan_with_source(AudiobookMetadata::default())
        .expect("patch conversion should succeed")
        .metadata;

    assert_eq!(metadata.title.as_deref(), Some("Project Hail Mary"));
    assert_eq!(metadata.artist.as_deref(), Some(""));
    assert_eq!(metadata.date.as_deref(), Some(""));
    assert_eq!(metadata.album_sort.as_deref(), Some(""));
    assert_eq!(metadata.cover_art, Some(Vec::new()));
}

#[test]
fn metadata_intent_patch_preserves_album_sort_when_absent() {
    let patch = MetadataIntentPatch {
        genre: Some(PatchOp::Set("Sci-Fi".to_string())),
        ..Default::default()
    };

    let plan = patch
        .to_write_plan_with_source(AudiobookMetadata::default())
        .expect("write plan should preserve album sort");

    assert_eq!(plan.metadata.genre.as_deref(), Some("Sci-Fi"));
    assert_eq!(plan.metadata.album_sort, None);
    assert_eq!(plan.album_sort, AlbumSortWriteAction::Preserve);
}

#[test]
fn metadata_intent_patch_supports_album_sort_set_and_clear() {
    let set_plan = MetadataIntentPatch {
        album_sort: Some(AlbumSortPatchOp::Set("Custom Sort".to_string())),
        ..Default::default()
    }
    .to_write_plan_with_source(AudiobookMetadata::default())
    .expect("album sort set should compile");
    assert_eq!(set_plan.metadata.album_sort.as_deref(), Some("Custom Sort"));
    assert_eq!(
        set_plan.album_sort,
        AlbumSortWriteAction::Set("Custom Sort".to_string())
    );

    let clear_plan = MetadataIntentPatch {
        album_sort: Some(AlbumSortPatchOp::Clear),
        ..Default::default()
    }
    .to_write_plan_with_source(AudiobookMetadata::default())
    .expect("album sort clear should compile");
    assert_eq!(clear_plan.metadata.album_sort.as_deref(), Some(""));
    assert_eq!(clear_plan.album_sort, AlbumSortWriteAction::Clear);
}

#[test]
fn metadata_intent_patch_rejects_invalid_publication_date() {
    let patch = MetadataIntentPatch {
        date: Some(PatchOp::Set("2024-13".to_string())),
        ..Default::default()
    };

    let err = patch
        .to_write_plan_with_source(AudiobookMetadata::default())
        .expect_err("invalid year should be rejected");

    assert!(err.to_string().contains("YYYY"), "unexpected error: {err}");
}

#[test]
fn metadata_intent_patch_rejects_series_part_with_slash() {
    let patch = MetadataIntentPatch {
        series_part: Some(PatchOp::Set("7/8".to_string())),
        ..Default::default()
    };

    let err = patch
        .to_write_plan_with_source(AudiobookMetadata::default())
        .expect_err("series part with slash should be rejected");

    assert!(
        err.to_string()
            .contains("Series sequence (#) cannot include '/'"),
        "unexpected error: {err}"
    );
}

#[test]
fn metadata_intent_patch_write_contract_carries_explicit_artifact_intent_only() {
    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Read Compatible".to_string())),
        track: Some(PatchOp::Set((3, Some(12)))),
        disk: Some(PatchOp::Set((1, Some(2)))),
        comment: Some(PatchOp::Set("Reader note".to_string())),
        ..Default::default()
    };

    let resolved = patch
        .to_write_plan_with_source(AudiobookMetadata::default())
        .expect("write metadata compiles with explicit artifact intent")
        .metadata;

    assert_eq!(resolved.title.as_deref(), Some("Read Compatible"));
    assert_eq!(resolved.track, Some((3, Some(12))));
    assert_eq!(resolved.disk, Some((1, Some(2))));
    assert_eq!(resolved.comment.as_deref(), Some("Reader note"));
}

#[test]
fn processing_patch_apply_to_metadata_handles_set_clear_and_absent() {
    let base = AudiobookMetadata {
        title: Some("Old Title".to_string()),
        artist: Some("Old Artist".to_string()),
        series: Some("Series".to_string()),
        series_part: Some("1".to_string()),
        album_sort: Some("Old Sort".to_string()),
        cover_art: Some(vec![1, 2, 3]),
        date: Some("2020".to_string()),
        ..Default::default()
    };
    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("New Title".to_string())),
        artist: Some(PatchOp::Clear),
        series_part: Some(PatchOp::Set("2".to_string())),
        date: Some(PatchOp::Set("2024-09-01".to_string())),
        cover_art: Some(PatchOp::Clear),
        ..Default::default()
    };

    let resolved = patch
        .apply_to_metadata(base)
        .expect("set and clear patch should apply");

    assert_eq!(resolved.title.as_deref(), Some("New Title"));
    assert_eq!(resolved.artist, None);
    assert_eq!(resolved.album_sort.as_deref(), Some("Old Sort"));
    assert_eq!(resolved.date.as_deref(), Some("2024-09"));
    assert_eq!(resolved.cover_art, None);
}

#[test]
fn computes_album_sort_with_numeric_part() {
    let result = compute_album_sort("Series", Some("3"), "Title");
    assert_eq!(result.as_deref(), Some("Series 03 - Title"));
}

#[test]
fn skips_album_sort_when_part_missing_or_invalid() {
    for part in [
        None,
        Some(""),
        Some("abc"),
        Some("0"),
        Some("0.0"),
        Some("1/5"),
    ] {
        assert!(
            compute_album_sort("Series", part, "Title").is_none(),
            "{part:?}"
        );
    }
    for part in ["1.", ".5", "1.5.2", "1.a"] {
        assert!(
            compute_album_sort("Series", Some(part), "Title").is_none(),
            "{part}"
        );
    }
}

#[test]
fn computes_album_sort_with_fractional_part_between_whole_books() {
    assert_eq!(
        compute_album_sort("Skyward", Some("2.5"), "Sunreach").as_deref(),
        Some("Skyward 02.5 - Sunreach")
    );
    assert_eq!(
        compute_album_sort("Skyward", Some("0.5"), "Prequel").as_deref(),
        Some("Skyward 00.5 - Prequel")
    );
}

#[test]
fn processing_album_sort_replaces_stale_value_and_keeps_uncomputable_one() {
    let stale = AudiobookMetadata {
        title: Some("Skyward".to_string()),
        series: Some("The Skyward Series".to_string()),
        series_part: Some("1".to_string()),
        album_sort: Some("Skyward Flight 01 - Skyward Flight Book 1: Skyward".to_string()),
        ..AudiobookMetadata::new()
    };
    assert_eq!(
        processing_album_sort(&stale).as_deref(),
        Some("The Skyward Series 01 - Skyward")
    );

    let no_part = AudiobookMetadata {
        series_part: None,
        ..stale
    };
    assert_eq!(
        processing_album_sort(&no_part).as_deref(),
        Some("Skyward Flight 01 - Skyward Flight Book 1: Skyward")
    );
}

#[test]
fn build_series_list_folds_representable_partial_subseries() {
    assert_eq!(
        build_series_list(Some("Primary"), None, Some("Sub"), None),
        (Some("Primary; Sub".to_string()), None)
    );
    assert_eq!(
        build_series_list(Some("Primary"), Some("1"), Some("Sub"), None),
        (Some("Primary; Sub".to_string()), Some("1".to_string()))
    );
    assert_eq!(
        build_series_list(Some("Primary"), Some("1"), Some("Sub"), Some("2")),
        (Some("Primary; Sub".to_string()), Some("1; 2".to_string()))
    );
}

#[test]
fn series_family_validation_rejects_touched_orphan_shapes() {
    let subseries_only = MetadataIntentPatch {
        subseries: Some(PatchOp::Set("Sub".to_string())),
        ..Default::default()
    };
    assert!(subseries_only
        .to_processing_overlay()
        .expect_err("orphan subseries should fail")
        .to_string()
        .contains("Sub-series requires a Series"));

    let part_only = MetadataIntentPatch {
        series_part: Some(PatchOp::Set("1".to_string())),
        ..Default::default()
    };
    assert!(part_only
        .to_processing_overlay()
        .expect_err("orphan series part should fail")
        .to_string()
        .contains("requires a Series"));

    let subseries_part_without_primary_part = MetadataIntentPatch {
        series: Some(PatchOp::Set("Primary".to_string())),
        subseries: Some(PatchOp::Set("Sub".to_string())),
        subseries_part: Some(PatchOp::Set("2".to_string())),
        ..Default::default()
    };
    assert!(subseries_part_without_primary_part
        .to_processing_overlay()
        .expect_err("subseries part without primary part should fail")
        .to_string()
        .contains("requires Series, Series sequence"));
}

#[test]
fn source_aware_write_plan_preserves_valid_partial_subseries() {
    let source = AudiobookMetadata {
        series: Some("Primary".to_string()),
        ..Default::default()
    };
    let patch = MetadataIntentPatch {
        subseries: Some(PatchOp::Set("Sub".to_string())),
        ..Default::default()
    };

    let plan = patch
        .to_write_plan_with_source(source)
        .expect("primary series allows subseries name");

    assert_eq!(plan.metadata.series.as_deref(), Some("Primary"));
    assert_eq!(plan.metadata.subseries.as_deref(), Some("Sub"));
    assert_eq!(plan.metadata.series_part, None);
    assert_eq!(plan.metadata.subseries_part, None);
}

#[test]
fn unrelated_edits_do_not_reject_inherited_orphan_series_tags() {
    let source = AudiobookMetadata {
        series_part: Some("7".to_string()),
        ..Default::default()
    };
    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Retitled".to_string())),
        ..Default::default()
    };

    let merged = patch
        .apply_to_metadata(source.clone())
        .expect("non-series intent preserves inherited orphan");
    assert_eq!(merged.series_part.as_deref(), Some("7"));

    let plan = patch
        .to_write_plan_with_source(source)
        .expect("non-series write intent should not validate inherited orphan");
    assert_eq!(plan.metadata.title.as_deref(), Some("Retitled"));
    assert_eq!(plan.metadata.series_part, None);
}

#[test]
fn processing_patch_into_overlay_applies_without_source_metadata() {
    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Overlay Title".to_string())),
        series: Some(PatchOp::Set("Series Name".to_string())),
        ..Default::default()
    };

    let resolved = patch
        .to_processing_overlay()
        .expect("overlay-only patch should resolve");

    assert_eq!(resolved.title.as_deref(), Some("Overlay Title"));
    assert_eq!(resolved.series.as_deref(), Some("Series Name"));
}

#[test]
fn normalize_publication_date_accepts_year_month_and_full_date_prefix() {
    assert_eq!(normalize_publication_date("2024"), Some("2024".to_string()));
    assert_eq!(
        normalize_publication_date("2024-07"),
        Some("2024-07".to_string())
    );
    assert_eq!(
        normalize_publication_date("2024-07-15"),
        Some("2024-07".to_string())
    );
    assert_eq!(
        normalize_publication_date("2024-07-15T10:00:00Z"),
        Some("2024-07".to_string())
    );
    assert_eq!(normalize_publication_date("2024-13"), None);
    assert_eq!(normalize_publication_date("2024-00"), None);
    assert_eq!(normalize_publication_date("abcd"), None);
}

#[test]
fn metadata_intent_validation_reports_field_errors_as_data() {
    let patch = MetadataIntentPatch {
        date: Some(PatchOp::Set("not a date".to_string())),
        series_part: Some(PatchOp::Set("1/2".to_string())),
        ..Default::default()
    };

    let result = validate_metadata_intent_patch(&patch);

    assert!(!result.is_valid);
    assert_eq!(result.field_errors.len(), 2);
    assert!(result
        .field_errors
        .iter()
        .any(|error| error.message.contains("Publication date")));
    assert!(result
        .field_errors
        .iter()
        .any(|error| error.message.contains("Series sequence")));
}

#[test]
fn validation_reply_carries_only_requested_fields() {
    let result = validate_metadata_intent_patch(&MetadataIntentPatch {
        title: Some(PatchOp::Set("NMR 64k".to_string())),
        date: Some(PatchOp::Set("2024-07-15".to_string())),
        ..Default::default()
    });

    assert_eq!(
        serde_json::to_value(&result).expect("serializes")["metadataPatch"],
        serde_json::json!({
            "title": { "op": "set", "value": "NMR 64k" },
            "date": { "op": "set", "value": "2024-07" },
        })
    );
}

#[test]
fn metadata_intent_validation_normalizes_valid_publication_date() {
    let patch = MetadataIntentPatch {
        date: Some(PatchOp::Set("2024-07-15T12:00:00Z".to_string())),
        ..Default::default()
    };

    let result = validate_metadata_intent_patch(&patch);

    assert!(result.is_valid);
    assert!(result.field_errors.is_empty());
    assert_eq!(
        result.metadata_patch.date,
        Some(PatchOp::Set("2024-07".to_string()))
    );
}

#[test]
fn metadata_intent_validation_reports_structured_field_codes() {
    let patch = MetadataIntentPatch {
        date: Some(PatchOp::Set("2024-13".to_string())),
        series_part: Some(PatchOp::Set("7/8".to_string())),
        subseries_part: Some(PatchOp::Set("2/3".to_string())),
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
fn metadata_intent_validation_preserves_invalid_date_for_validation() {
    let patch = MetadataIntentPatch {
        date: Some(PatchOp::Set("not a date".to_string())),
        ..Default::default()
    };

    let result = validate_metadata_intent_patch(&patch);
    assert!(!result.is_valid);
    assert_eq!(
        result.field_errors.first().map(|error| error.field),
        Some(MetadataIntentValidationField::Date)
    );
}

#[test]
fn publication_year_from_date_handles_multibyte_prefix_without_panicking() {
    assert_eq!(publication_year_from_date(Some("2024-07")), Some(2024));
    assert_eq!(publication_year_from_date(Some("💥024-07")), None);
    assert_eq!(publication_year_from_date(Some("20💥4-07")), None);
}

#[test]
fn naming_metadata_scrubs_legacy_series_parts() {
    let mut naming = NamingMetadata::from_metadata(&AudiobookMetadata {
        title: Some("Legacy Source".to_string()),
        series: Some("Series".to_string()),
        series_part: Some("7/8".to_string()),
        subseries: Some("Subseries".to_string()),
        subseries_part: Some("2/3".to_string()),
        ..Default::default()
    });

    naming.scrub_legacy_source_series_parts_for_naming();

    assert_eq!(naming.title(), Some("Legacy Source"));
    assert_eq!(naming.series(), Some("Series"));
    assert_eq!(naming.series_part(), None);
    assert_eq!(naming.subseries(), Some("Subseries"));
    assert_eq!(naming.subseries_part(), None);
}

#[test]
fn artifact_clear_intents_reach_write_plan_sentinels() {
    let patch = MetadataIntentPatch {
        comment: Some(PatchOp::Clear),
        track: Some(PatchOp::Clear),
        disk: Some(PatchOp::Clear),
        ..Default::default()
    };

    let plan = patch
        .to_write_plan_with_source(AudiobookMetadata::default())
        .expect("write plan");

    assert_eq!(
        plan.metadata.comment,
        Some(String::new()),
        "comment clear uses the empty-string sentinel the op planner clears on"
    );
    assert_eq!(
        plan.metadata.track,
        Some((0, None)),
        "track clear uses the zero-position sentinel the op planner clears on"
    );
    assert_eq!(plan.metadata.disk, Some((0, None)));
}

#[test]
fn artifact_clear_intents_remove_values_in_processing_semantics() {
    let base = AudiobookMetadata {
        comment: Some("provenance note".to_string()),
        track: Some((3, Some(12))),
        disk: Some((1, Some(2))),
        ..AudiobookMetadata::new()
    };
    let patch = MetadataIntentPatch {
        comment: Some(PatchOp::Clear),
        track: Some(PatchOp::Clear),
        disk: Some(PatchOp::Clear),
        ..Default::default()
    };

    let merged = patch.apply_to_metadata(base).expect("patch applies");

    assert_eq!(merged.comment, None);
    assert_eq!(merged.track, None);
    assert_eq!(merged.disk, None);
}

#[test]
fn absent_artifact_intents_preserve_values() {
    let base = AudiobookMetadata {
        comment: Some("keep me".to_string()),
        track: Some((3, Some(12))),
        disk: Some((1, Some(2))),
        ..AudiobookMetadata::new()
    };

    let merged = MetadataIntentPatch::default()
        .apply_to_metadata(base)
        .expect("empty patch applies");

    assert_eq!(merged.comment.as_deref(), Some("keep me"));
    assert_eq!(merged.track, Some((3, Some(12))));
    assert_eq!(merged.disk, Some((1, Some(2))));

    let plan = MetadataIntentPatch::default()
        .to_write_plan_with_source(AudiobookMetadata::default())
        .expect("write plan");
    assert_eq!(
        plan.metadata.comment, None,
        "an absent field must not clear at write"
    );
    assert_eq!(plan.metadata.track, None);
    assert_eq!(plan.metadata.disk, None);
}
