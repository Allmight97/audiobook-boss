use super::*;

fn text(field: TextField, value: &str) -> FieldOp {
    FieldOp::Text {
        field,
        op: PatchOp::Set(value.to_string()),
    }
}

fn cleared(field: TextField) -> FieldOp {
    FieldOp::Text {
        field,
        op: PatchOp::Clear,
    }
}

fn position(field: PositionField, number: u32, total: Option<u32>) -> FieldOp {
    FieldOp::Position {
        field,
        op: PatchOp::Set((number, total)),
    }
}

fn from_patch(patch: &MetadataIntentPatch) -> MetadataWritePlan {
    MetadataWritePlan::from_patch(patch, AudiobookMetadata::default()).expect("write plan")
}

#[test]
fn an_edit_plans_set_and_clear_for_each_field_it_names_and_nothing_else() {
    let plan = from_patch(&MetadataIntentPatch {
        title: Some(PatchOp::Set("Project Hail Mary".into())),
        artist: Some(PatchOp::Clear),
        date: Some(PatchOp::Clear),
        album_sort: Some(AlbumSortPatchOp::Clear),
        cover_art: Some(PatchOp::Clear),
        ..Default::default()
    });

    assert_eq!(
        plan.ops,
        [
            text(TextField::Title, "Project Hail Mary"),
            cleared(TextField::Artist),
            cleared(TextField::Date),
        ]
    );
    assert_eq!(plan.album_sort, Some(PatchOp::Clear));
    assert_eq!(plan.cover, Some(PatchOp::Clear));
    assert!(!plan.series_touched);
}

#[test]
fn an_edit_that_names_no_field_plans_nothing() {
    let plan = from_patch(&MetadataIntentPatch::default());
    assert!(plan.ops.is_empty());
    assert_eq!(plan.album_sort, None, "an absent album sort is kept");
    assert_eq!(plan.cover, None);
}

#[test]
fn album_sort_set_and_clear_are_explicit() {
    let set = from_patch(&MetadataIntentPatch {
        album_sort: Some(AlbumSortPatchOp::Set("Custom Sort".into())),
        ..Default::default()
    });
    assert_eq!(set.album_sort, Some(PatchOp::Set("Custom Sort".into())));

    let blank = from_patch(&MetadataIntentPatch {
        album_sort: Some(AlbumSortPatchOp::Set("  ".into())),
        ..Default::default()
    });
    assert_eq!(blank.album_sort, Some(PatchOp::Clear));
}

#[test]
fn the_plan_rejects_an_invalid_publication_date() {
    let error = MetadataWritePlan::from_patch(
        &MetadataIntentPatch {
            date: Some(PatchOp::Set("2024-13".into())),
            ..Default::default()
        },
        AudiobookMetadata::default(),
    )
    .expect_err("invalid year should be rejected");
    assert!(
        error.to_string().contains("YYYY"),
        "unexpected error: {error}"
    );
}

#[test]
fn the_plan_rejects_a_series_part_with_a_slash() {
    let error = MetadataWritePlan::from_patch(
        &MetadataIntentPatch {
            series_part: Some(PatchOp::Set("7/8".into())),
            ..Default::default()
        },
        AudiobookMetadata::default(),
    )
    .expect_err("series part with slash should be rejected");
    assert!(
        error
            .to_string()
            .contains("Series sequence (#) cannot include '/'"),
        "unexpected error: {error}"
    );
}

#[test]
fn artifact_fields_plan_only_what_the_edit_names() {
    let plan = from_patch(&MetadataIntentPatch {
        title: Some(PatchOp::Set("Read Compatible".into())),
        track: Some(PatchOp::Set((3, Some(12)))),
        disk: Some(PatchOp::Set((1, Some(2)))),
        comment: Some(PatchOp::Set("Reader note".into())),
        ..Default::default()
    });

    assert_eq!(
        plan.ops,
        [
            text(TextField::Title, "Read Compatible"),
            text(TextField::Comment, "Reader note"),
            position(PositionField::Track, 3, Some(12)),
            position(PositionField::Disk, 1, Some(2)),
        ]
    );
}

#[test]
fn artifact_clear_intents_plan_typed_clears() {
    let plan = from_patch(&MetadataIntentPatch {
        comment: Some(PatchOp::Clear),
        track: Some(PatchOp::Clear),
        disk: Some(PatchOp::Clear),
        ..Default::default()
    });

    assert_eq!(
        plan.ops,
        [
            cleared(TextField::Comment),
            FieldOp::Position {
                field: PositionField::Track,
                op: PatchOp::Clear
            },
            FieldOp::Position {
                field: PositionField::Disk,
                op: PatchOp::Clear
            },
        ]
    );
}

#[test]
fn a_blank_value_or_zero_position_in_an_edit_plans_a_clear() {
    let plan = from_patch(&MetadataIntentPatch {
        genre: Some(PatchOp::Set("   ".into())),
        track: Some(PatchOp::Set((0, Some(9)))),
        cover_art: Some(PatchOp::Set(Vec::new())),
        ..Default::default()
    });

    assert_eq!(
        plan.ops,
        [
            cleared(TextField::Genre),
            FieldOp::Position {
                field: PositionField::Track,
                op: PatchOp::Clear
            },
        ]
    );
    assert_eq!(plan.cover, Some(PatchOp::Clear));
}

#[test]
fn a_date_set_is_written_as_the_normalized_date() {
    let plan = from_patch(&MetadataIntentPatch {
        date: Some(PatchOp::Set(" 2024-09-01 ".into())),
        ..Default::default()
    });
    assert_eq!(plan.ops, [text(TextField::Date, "2024-09")]);
}

#[test]
fn a_subseries_name_edit_joins_the_source_series_and_leaves_the_part_alone() {
    let source = AudiobookMetadata {
        series: Some("Primary".into()),
        ..Default::default()
    };
    let plan = MetadataWritePlan::from_patch(
        &MetadataIntentPatch {
            subseries: Some(PatchOp::Set("Sub".into())),
            ..Default::default()
        },
        source,
    )
    .expect("primary series allows subseries name");

    assert_eq!(plan.ops, [text(TextField::Series, "Primary; Sub")]);
    assert!(plan.series_touched);
}

#[test]
fn a_part_edit_joins_the_source_subseries_part() {
    let source = AudiobookMetadata {
        series: Some("Primary".into()),
        series_part: Some("1".into()),
        subseries: Some("Sub".into()),
        subseries_part: Some("2".into()),
        ..Default::default()
    };
    let plan = MetadataWritePlan::from_patch(
        &MetadataIntentPatch {
            series_part: Some(PatchOp::Set("4".into())),
            ..Default::default()
        },
        source,
    )
    .expect("part edit");

    assert_eq!(plan.ops, [text(TextField::SeriesPart, "4; 2")]);
}

#[test]
fn clearing_the_series_plans_a_series_clear() {
    let source = AudiobookMetadata {
        series: Some("Primary".into()),
        ..Default::default()
    };
    let plan = MetadataWritePlan::from_patch(
        &MetadataIntentPatch {
            series: Some(PatchOp::Clear),
            ..Default::default()
        },
        source,
    )
    .expect("series clear");
    assert_eq!(plan.ops, [cleared(TextField::Series)]);
}

#[test]
fn unrelated_edits_do_not_reject_inherited_orphan_series_tags() {
    let source = AudiobookMetadata {
        series_part: Some("7".into()),
        ..Default::default()
    };
    let plan = MetadataWritePlan::from_patch(
        &MetadataIntentPatch {
            title: Some(PatchOp::Set("Retitled".into())),
            ..Default::default()
        },
        source,
    )
    .expect("non-series write intent should not validate inherited orphan");

    assert_eq!(plan.ops, [text(TextField::Title, "Retitled")]);
    assert!(!plan.series_touched);
}

#[test]
fn a_touched_orphan_series_shape_is_refused() {
    let error = MetadataWritePlan::from_patch(
        &MetadataIntentPatch {
            subseries: Some(PatchOp::Set("Sub".into())),
            ..Default::default()
        },
        AudiobookMetadata::default(),
    )
    .expect_err("orphan subseries should fail");
    assert!(error.to_string().contains("Sub-series requires a Series"));
}

#[test]
fn tags_read_from_a_file_plan_every_field_in_write_order() {
    let metadata = AudiobookMetadata {
        title: Some("Title".into()),
        artist: Some("Author".into()),
        album: Some("Album".into()),
        composer: Some("Narrator".into()),
        genre: Some("Genre".into()),
        date: Some(" 2020-05 ".into()),
        comment: Some("Note".into()),
        description: Some("About".into()),
        series: Some("Wheel of Time".into()),
        series_part: Some("3".into()),
        track: Some((7, Some(42))),
        disk: Some((2, Some(5))),
        album_sort: Some("Sort".into()),
        cover_art: Some(vec![1]),
        ..Default::default()
    };

    let plan = MetadataWritePlan::from_metadata(&metadata);

    assert_eq!(
        plan.ops,
        [
            text(TextField::Title, "Title"),
            text(TextField::Artist, "Author"),
            text(TextField::Album, "Album"),
            text(TextField::Composer, "Narrator"),
            text(TextField::Genre, "Genre"),
            text(TextField::Date, "2020-05"),
            text(TextField::Comment, "Note"),
            text(TextField::Description, "About"),
            text(TextField::Series, "Wheel of Time"),
            text(TextField::SeriesPart, "3"),
            position(PositionField::Track, 7, Some(42)),
            position(PositionField::Disk, 2, Some(5)),
        ]
    );
    assert_eq!(plan.album_sort, Some(PatchOp::Set("Sort".into())));
    assert_eq!(plan.cover, Some(PatchOp::Set(vec![1])));
    assert!(plan.series_touched);
}

#[test]
fn tags_read_from_a_file_clear_what_is_blank_zero_or_empty() {
    let plan = MetadataWritePlan::from_metadata(&AudiobookMetadata {
        comment: Some("   ".into()),
        genre: Some(String::new()),
        track: Some((0, None)),
        album_sort: Some(" ".into()),
        cover_art: Some(Vec::new()),
        ..Default::default()
    });

    assert_eq!(
        plan.ops,
        [
            cleared(TextField::Genre),
            cleared(TextField::Comment),
            FieldOp::Position {
                field: PositionField::Track,
                op: PatchOp::Clear
            },
        ]
    );
    assert_eq!(plan.album_sort, Some(PatchOp::Clear));
    assert_eq!(plan.cover, Some(PatchOp::Clear));
}

#[test]
fn tags_read_from_a_file_keep_text_untrimmed_except_the_date() {
    let plan = MetadataWritePlan::from_metadata(&AudiobookMetadata {
        title: Some(" Padded ".into()),
        date: Some(" 2020 ".into()),
        ..Default::default()
    });
    assert_eq!(
        plan.ops,
        [
            text(TextField::Title, " Padded "),
            text(TextField::Date, "2020")
        ]
    );
}

#[test]
fn tags_read_from_a_file_project_series_and_subseries_together() {
    let both = MetadataWritePlan::from_metadata(&AudiobookMetadata {
        series: Some("Primary".into()),
        series_part: Some("1".into()),
        subseries: Some("Sub".into()),
        subseries_part: Some("2".into()),
        ..Default::default()
    });
    assert_eq!(
        both.ops,
        [
            text(TextField::Series, "Primary; Sub"),
            text(TextField::SeriesPart, "1; 2")
        ]
    );

    let name_only = MetadataWritePlan::from_metadata(&AudiobookMetadata {
        series: Some("Primary".into()),
        subseries: Some("Sub".into()),
        ..Default::default()
    });
    assert_eq!(
        name_only.ops,
        [text(TextField::Series, "Primary; Sub")],
        "a representable subseries name must not clear series-part"
    );
}

#[test]
fn tags_read_from_a_file_plan_nothing_for_an_empty_read() {
    let plan = MetadataWritePlan::from_metadata(&AudiobookMetadata::default());
    assert!(plan.ops.is_empty());
    assert_eq!(
        (plan.album_sort, plan.cover, plan.series_touched),
        (None, None, false)
    );
}
