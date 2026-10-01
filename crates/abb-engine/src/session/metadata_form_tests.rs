use super::*;

fn tagged() -> AudiobookMetadata {
    AudiobookMetadata {
        title: Some("Title".to_string()),
        artist: Some("Author".to_string()),
        date: Some("2024-07".to_string()),
        series: Some("Saga".to_string()),
        series_part: Some("7/8".to_string()),
        cover_art: Some(vec![1, 2, 3]),
        ..Default::default()
    }
}

fn set(value: &str) -> Option<PatchOp<String>> {
    Some(PatchOp::Set(value.to_string()))
}

#[test]
fn an_edited_title_is_trimmed_and_mirrors_the_album() {
    let mut form = MetadataForm::single(&tagged());
    form.set_value(MetadataField::Title, " New ".to_string());

    assert_eq!(
        form.compose_intent(),
        MetadataIntentPatch {
            title: set("New"),
            album: set("New"),
            ..Default::default()
        }
    );
}

#[test]
fn an_emptied_date_clears() {
    let mut form = MetadataForm::single(&tagged());
    form.set_value(MetadataField::Date, String::new());

    assert_eq!(
        form.compose_intent(),
        MetadataIntentPatch {
            date: Some(PatchOp::Clear),
            ..Default::default()
        }
    );
}

#[test]
fn an_emptied_mixed_field_is_a_bulk_blank() {
    let other = AudiobookMetadata {
        artist: Some("Other".to_string()),
        ..Default::default()
    };
    let mut form = MetadataForm::multi(&[tagged(), other], 2);
    form.set_value(MetadataField::Author, String::new());

    assert_eq!(
        form.compose_intent(),
        MetadataIntentPatch {
            artist: Some(PatchOp::Clear),
            ..Default::default()
        }
    );
    assert_eq!(
        form.snapshot().fields[MetadataField::Author.index()].action,
        FieldAction::Blank
    );
}

#[test]
fn untouched_fields_carry_no_intent_even_when_their_inherited_value_is_invalid() {
    let mut form = MetadataForm::single(&tagged());
    form.set_value(MetadataField::Author, "Someone Else".to_string());

    // "7/8" is an invalid book number the file already had; it is reported
    // on screen and left out of the edit.
    assert_eq!(
        form.compose_intent(),
        MetadataIntentPatch {
            artist: set("Someone Else"),
            ..Default::default()
        }
    );
    let snapshot = form.snapshot();
    assert!(matches!(
        snapshot.series_part_warning,
        Some(SeriesPartWarning::Invalid { .. })
    ));
    assert!(snapshot.validation_message.is_some());
}

#[test]
fn keep_after_blank_restores_what_the_field_showed() {
    let mut form = MetadataForm::single(&tagged());
    form.set_action(MetadataField::Author, FieldAction::Blank);
    assert_eq!(form.compose_intent().artist, Some(PatchOp::Clear));

    form.set_action(MetadataField::Author, FieldAction::Keep);

    assert!(!form.compose_intent().is_actionable());
    assert_eq!(form.trimmed(MetadataField::Author), "Author");
}

#[test]
fn staging_makes_the_current_values_the_baseline_keep_restores() {
    let mut form = MetadataForm::single(&tagged());
    form.set_value(MetadataField::Author, "Staged".to_string());
    form.reset_dirty();
    assert!(!form.has_dirty_fields());

    form.set_action(MetadataField::Author, FieldAction::Blank);
    form.set_action(MetadataField::Author, FieldAction::Keep);

    assert_eq!(form.trimmed(MetadataField::Author), "Staged");
}

#[test]
fn rehydration_keeps_fields_being_edited_and_takes_the_fresh_baseline() {
    let mut form = MetadataForm::single(&AudiobookMetadata::default());
    form.set_value(MetadataField::Author, "Typed while loading".to_string());

    form.rehydrate(MetadataForm::single(&tagged()));

    assert_eq!(form.trimmed(MetadataField::Author), "Typed while loading");
    assert_eq!(form.trimmed(MetadataField::Title), "Title");
    form.set_action(MetadataField::Author, FieldAction::Blank);
    form.set_action(MetadataField::Author, FieldAction::Keep);
    assert_eq!(form.trimmed(MetadataField::Author), "Author");
}

#[test]
fn several_titles_show_a_value_only_where_they_agree() {
    let other = AudiobookMetadata {
        artist: Some("Other".to_string()),
        series: Some(" Saga ".to_string()),
        ..Default::default()
    };
    let snapshot = MetadataForm::multi(&[tagged(), other], 2).snapshot();
    let field = |field: MetadataField| &snapshot.fields[field.index()];

    assert_eq!(snapshot.selection_count, 2);
    assert!(field(MetadataField::Author).mixed);
    assert_eq!(field(MetadataField::Author).value, "");
    assert!(!field(MetadataField::Series).mixed);
    assert_eq!(field(MetadataField::Series).value, "Saga");
}

#[test]
fn series_warnings_follow_the_values_on_screen() {
    let mut form = MetadataForm::single(&AudiobookMetadata::default());
    assert_eq!(form.snapshot().series_part_warning, None);

    form.set_value(MetadataField::Series, "Saga".to_string());
    assert_eq!(
        form.snapshot().series_part_warning,
        Some(SeriesPartWarning::MissingBookNumber)
    );

    form.set_value(MetadataField::SeriesPart, "2".to_string());
    form.set_value(MetadataField::Subseries, "Arc".to_string());
    assert_eq!(
        form.snapshot().subseries_part_warning,
        Some(SubseriesPartWarning::MissingNumber)
    );

    form.set_value(MetadataField::SubseriesPart, "2".to_string());
    let snapshot = form.snapshot();
    assert_eq!(
        snapshot.series_part_warning,
        Some(SeriesPartWarning::MatchesSubseriesPart)
    );
    assert_eq!(snapshot.subseries_part_warning, None);
    assert_eq!(snapshot.validation_message, None);
}

#[test]
fn lookup_values_arrive_as_explicit_edits() {
    let mut form = MetadataForm::single(&tagged());
    form.apply_lookup(&AudiobookMetadata {
        title: Some("Found".to_string()),
        date: Some(" 2020 ".to_string()),
        ..Default::default()
    });

    assert_eq!(
        form.compose_intent(),
        MetadataIntentPatch {
            title: set("Found"),
            album: set("Found"),
            date: set("2020"),
            ..Default::default()
        }
    );
}

#[test]
fn typed_text_replaces_an_earlier_blank() {
    let mut form = MetadataForm::single(&tagged());
    form.set_action(MetadataField::Author, FieldAction::Blank);
    form.set_value(MetadataField::Author, "Typed".to_string());

    assert_eq!(form.compose_intent().artist, set("Typed"));
}
