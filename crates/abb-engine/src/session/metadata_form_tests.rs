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
fn keep_after_blank_restores_what_the_field_showed() {
    let mut form = MetadataForm::single(&tagged());
    form.set_action(MetadataField::Author, FieldAction::Blank);
    assert_eq!(form.compose_intent().artist, Some(PatchOp::Clear));

    form.set_action(MetadataField::Author, FieldAction::Keep);

    assert!(!form.compose_intent().is_actionable());
    assert_eq!(form.trimmed(MetadataField::Author), "Author");
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

type PatchSlot = fn(&MetadataIntentPatch) -> &Option<PatchOp<String>>;
type MetadataSlot = fn(&AudiobookMetadata) -> &Option<String>;

const PINNED: [(MetadataField, PatchSlot, MetadataSlot); 10] = [
    (MetadataField::Title, |p| &p.title, |m| &m.title),
    (MetadataField::Date, |p| &p.date, |m| &m.date),
    (MetadataField::Author, |p| &p.artist, |m| &m.artist),
    (MetadataField::Narrator, |p| &p.composer, |m| &m.composer),
    (MetadataField::Series, |p| &p.series, |m| &m.series),
    (
        MetadataField::SeriesPart,
        |p| &p.series_part,
        |m| &m.series_part,
    ),
    (MetadataField::Subseries, |p| &p.subseries, |m| &m.subseries),
    (
        MetadataField::SubseriesPart,
        |p| &p.subseries_part,
        |m| &m.subseries_part,
    ),
    (MetadataField::Genre, |p| &p.genre, |m| &m.genre),
    (
        MetadataField::Description,
        |p| &p.description,
        |m| &m.description,
    ),
];

fn every_field_filled() -> AudiobookMetadata {
    AudiobookMetadata {
        title: Some("Title".to_string()),
        date: Some("2024-07".to_string()),
        artist: Some("Author".to_string()),
        composer: Some("Narrator".to_string()),
        series: Some("Saga".to_string()),
        series_part: Some("7".to_string()),
        subseries: Some("Arc".to_string()),
        subseries_part: Some("2".to_string()),
        genre: Some("Fantasy".to_string()),
        description: Some("About".to_string()),
        ..Default::default()
    }
}

fn only_slot(field: MetadataField, op: PatchOp<String>) -> MetadataIntentPatch {
    let mut patch = MetadataIntentPatch::default();
    match field {
        MetadataField::Title => {
            patch.title = Some(op.clone());
            patch.album = Some(op);
        }
        MetadataField::Date => patch.date = Some(op),
        MetadataField::Author => patch.artist = Some(op),
        MetadataField::Narrator => patch.composer = Some(op),
        MetadataField::Series => patch.series = Some(op),
        MetadataField::SeriesPart => patch.series_part = Some(op),
        MetadataField::Subseries => patch.subseries = Some(op),
        MetadataField::SubseriesPart => patch.subseries_part = Some(op),
        MetadataField::Genre => patch.genre = Some(op),
        MetadataField::Description => patch.description = Some(op),
    }
    patch
}

#[test]
fn each_field_edits_only_its_own_patch_slot() {
    for (field, _, _) in PINNED {
        let mut typed = MetadataForm::single(&every_field_filled());
        typed.set_value(field, "  New value  ".to_string());
        assert_eq!(
            typed.compose_intent(),
            only_slot(field, PatchOp::Set("New value".to_string())),
            "{field:?} typed"
        );

        let mut emptied = MetadataForm::single(&every_field_filled());
        emptied.set_value(field, "   ".to_string());
        assert_eq!(
            emptied.compose_intent(),
            only_slot(field, PatchOp::Clear),
            "{field:?} emptied"
        );

        let mut blanked = MetadataForm::single(&every_field_filled());
        blanked.set_action(field, FieldAction::Blank);
        assert_eq!(
            blanked.compose_intent(),
            only_slot(field, PatchOp::Clear),
            "{field:?} blanked"
        );

        let untouched = MetadataForm::single(&every_field_filled());
        assert!(!untouched.compose_intent().is_actionable(), "{field:?}");
    }
}

#[test]
fn each_field_shows_its_own_stored_value() {
    let stored = every_field_filled();
    let snapshot = MetadataForm::single(&stored).snapshot();
    for (field, _, read) in PINNED {
        let expected = read(&stored).as_deref().unwrap_or_default();
        assert_eq!(snapshot.fields[field.index()].value, expected, "{field:?}");
    }
}

#[test]
fn a_whitespace_date_shows_empty_and_other_stored_text_keeps_its_spaces() {
    let padded = AudiobookMetadata {
        date: Some("  ".to_string()),
        title: Some(" Title ".to_string()),
        ..Default::default()
    };
    let single = MetadataForm::single(&padded).snapshot();
    assert_eq!(single.fields[MetadataField::Date.index()].value, "");
    assert_eq!(single.fields[MetadataField::Title.index()].value, " Title ");

    let dated = AudiobookMetadata {
        date: Some(" 2024 ".to_string()),
        title: Some(" Title ".to_string()),
        ..Default::default()
    };
    let multi = MetadataForm::multi(&[dated.clone(), dated], 2).snapshot();
    assert_eq!(multi.fields[MetadataField::Date.index()].value, " 2024 ");
    assert_eq!(multi.fields[MetadataField::Title.index()].value, "Title");
}

#[test]
fn a_lookup_fills_every_field_it_names_and_trims_only_the_date() {
    let mut form = MetadataForm::single(&AudiobookMetadata::default());
    form.apply_lookup(&AudiobookMetadata {
        date: Some(" 2020 ".to_string()),
        artist: Some(" Name ".to_string()),
        genre: Some("Mystery".to_string()),
        ..Default::default()
    });

    let snapshot = form.snapshot();
    let value = |field: MetadataField| snapshot.fields[field.index()].value.as_str();
    assert_eq!(value(MetadataField::Date), "2020");
    assert_eq!(value(MetadataField::Author), " Name ");
    assert_eq!(value(MetadataField::Genre), "Mystery");
    assert!(snapshot.fields[MetadataField::Genre.index()].dirty);
    assert!(!snapshot.fields[MetadataField::Title.index()].dirty);
}
#[test]
fn the_form_reads_back_as_metadata_with_empty_fields_absent() {
    let mut form = MetadataForm::single(&every_field_filled());
    form.set_value(MetadataField::Series, "  ".to_string());
    form.set_value(MetadataField::Genre, " Mystery ".to_string());

    let metadata = form.metadata();

    for (field, _, read) in PINNED {
        let expected = match field {
            MetadataField::Series => None,
            MetadataField::Genre => Some("Mystery".to_string()),
            _ => read(&every_field_filled()).clone(),
        };
        assert_eq!(read(&metadata), &expected, "{field:?}");
    }
    assert_eq!(metadata.album, None);
}
