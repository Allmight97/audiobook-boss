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
        .apply_to_metadata(AudiobookMetadata::default())
        .expect_err("orphan subseries should fail")
        .to_string()
        .contains("Sub-series requires a Series"));

    let part_only = MetadataIntentPatch {
        series_part: Some(PatchOp::Set("1".to_string())),
        ..Default::default()
    };
    assert!(part_only
        .apply_to_metadata(AudiobookMetadata::default())
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
        .apply_to_metadata(AudiobookMetadata::default())
        .expect_err("subseries part without primary part should fail")
        .to_string()
        .contains("requires Series, Series sequence"));
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
fn validation_reply_carries_only_requested_fields() {
    let result = validate_metadata_intent_patch(&MetadataIntentPatch {
        title: Some(PatchOp::Set("NMR 64k".to_string())),
        date: Some(PatchOp::Set("2024-07-15".to_string())),
        ..Default::default()
    });

    assert!(result.is_valid);
    assert!(result.field_errors.is_empty());
    assert_eq!(
        serde_json::to_value(&result).expect("serializes")["metadataPatch"],
        serde_json::json!({
            "title": { "op": "set", "value": "NMR 64k" },
            "date": { "op": "set", "value": "2024-07" },
        })
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
            && error.message.contains("Publication date")
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
fn publication_year_from_date_handles_multibyte_prefix_without_panicking() {
    assert_eq!(publication_year_from_date(Some("2024-07")), Some(2024));
    assert_eq!(publication_year_from_date(Some("💥024-07")), None);
    assert_eq!(publication_year_from_date(Some("20💥4-07")), None);
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
}
