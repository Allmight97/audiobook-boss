use super::*;

#[test]
fn keep_after_blank_revokes_the_bulk_clear() {
    for (field, shown) in [
        (MetadataField::Author, "Shared Author"),
        (MetadataField::Genre, ""),
        (MetadataField::Date, ""),
        (MetadataField::Title, ""),
    ] {
        let mut desk = two_books();
        desk.act(field, FieldAction::Blank);
        desk.act(field, FieldAction::Keep);
        let snapshot = desk.field(field);
        assert_eq!((snapshot.value.as_str(), snapshot.dirty), (shown, false));

        desk.type_into(MetadataField::Genre, "Mystery");
        desk.save(&[]);

        assert_eq!(
            desk.written_by_file(),
            HashMap::from([
                (path("alpha"), genre_patch()),
                (path("beta"), genre_patch())
            ])
        );
    }
}

#[test]
fn blank_clears_the_field_and_its_album_mirror_on_every_selected_title() {
    let mut desk = two_books();
    desk.act(MetadataField::Title, FieldAction::Blank);
    desk.save(&[]);

    let cleared = MetadataIntentPatch {
        title: Some(PatchOp::Clear),
        album: Some(PatchOp::Clear),
        ..Default::default()
    };
    assert_eq!(
        desk.written_by_file(),
        HashMap::from([(path("alpha"), cleared.clone()), (path("beta"), cleared)])
    );
}

#[test]
fn an_invalid_co_selected_input_receives_no_edit() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[]);
    desk.state
        .working_set
        .append_analyzed(vec![audio_file("broken", false)], &audio_request());
    desk.select(&[0, 1]).expect("select both");
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save(&[]);

    assert_eq!(
        desk.written_by_file(),
        HashMap::from([(path("alpha"), genre_patch())])
    );
    assert!(desk.pending("broken").is_none());
}

#[test]
fn inherited_values_are_neither_rewritten_nor_revalidated_by_an_unrelated_edit() {
    let odd = AudiobookMetadata {
        series_part: Some("7/8".to_string()),
        ..alpha_tags()
    };
    let mut desk = Desk::open(&[("alpha", Some(odd))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    let form = desk.metadata().form;
    assert!(matches!(
        form.series_part_warning,
        Some(SeriesPartWarning::Invalid { .. })
    ));
    assert!(form.validation_message.is_some());
    desk.save(&[]);

    assert_eq!(
        desk.written_by_file(),
        HashMap::from([(path("alpha"), genre_patch())])
    );
}

#[test]
fn a_text_edit_does_not_restage_the_unchanged_cover() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Title, "Renamed");
    desk.save(&[]);

    assert_eq!(
        desk.written_by_file(),
        HashMap::from([(
            path("alpha"),
            MetadataIntentPatch {
                title: set("Renamed"),
                album: set("Renamed"),
                ..Default::default()
            }
        )])
    );
}

#[test]
fn staging_reports_edits_with_no_valid_title_to_carry_them() {
    let mut desk = Desk::open(&[], &[]);
    desk.state
        .working_set
        .append_analyzed(vec![audio_file("broken", false)], &audio_request());
    desk.select(&[0]).expect("select");
    desk.type_into(MetadataField::Title, "Orphan");

    assert_eq!(desk.state.stage_bound_form(), StageOutcome::NoTarget);
}

#[test]
fn staging_carries_a_cover_clear_without_text_edits() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.clear_cover();

    assert_eq!(desk.state.stage_bound_form(), StageOutcome::Staged);
    assert_eq!(
        desk.state.pending_intents(&[path("alpha")]),
        [(
            path("alpha"),
            MetadataIntentPatch {
                cover_art: Some(PatchOp::Clear),
                ..Default::default()
            }
        )]
    );
}

#[test]
fn keep_after_staging_keeps_the_staged_value_and_its_intent() {
    let mut desk = two_books();
    desk.type_into(MetadataField::Genre, "Mystery");
    assert_eq!(desk.state.stage_bound_form(), StageOutcome::Staged);

    desk.act(MetadataField::Genre, FieldAction::Blank);
    desk.act(MetadataField::Genre, FieldAction::Keep);

    let field = desk.field(MetadataField::Genre);
    assert_eq!((field.value.as_str(), field.dirty), ("Mystery", false));
    assert_eq!(
        desk.state.pending_intents(&[path("alpha"), path("beta")]),
        [
            (path("alpha"), genre_patch()),
            (path("beta"), genre_patch())
        ]
    );
}

#[test]
fn earlier_text_and_cover_intent_survive_a_later_edit_and_second_handoff() {
    let other = AudiobookMetadata {
        artist: Some("Other Author".to_string()),
        ..beta_tags()
    };
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("other", Some(other))],
        &[0],
    );
    assert!(desk.state.apply_cover(vec![8, 9]));
    desk.select(&[1]).expect("select other");
    assert!(desk.state.apply_cover(vec![8, 9]));
    desk.select(&[0, 1]).expect("select both");
    desk.type_into(MetadataField::Author, "Edited Author");
    desk.state.stage_bound_form();

    let both = [path("alpha"), path("other")];
    let first = MetadataIntentPatch {
        artist: set("Edited Author"),
        cover_art: Some(PatchOp::Set(vec![8, 9])),
        ..Default::default()
    };
    assert_eq!(
        desk.state.pending_intents(&both),
        [
            (path("alpha"), first.clone()),
            (path("other"), first.clone())
        ]
    );

    desk.type_into(MetadataField::Title, "NMR 64k");
    desk.state.stage_bound_form();

    let second = MetadataIntentPatch {
        title: set("NMR 64k"),
        album: set("NMR 64k"),
        ..first
    };
    assert_eq!(
        desk.state.pending_intents(&both),
        [(path("alpha"), second.clone()), (path("other"), second)]
    );
}

#[test]
fn a_dirty_edit_is_staged_onto_the_previous_title_before_the_next_one_shows() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.type_into(MetadataField::Title, "Alpha draft");

    desk.select(&[1]).expect("gate accepts a valid edit");

    assert_eq!(desk.field(MetadataField::Title).value, "Beta");
    assert_eq!(
        desk.pending("alpha").and_then(|patch| patch.title),
        set("Alpha draft")
    );
    desk.select(&[0]).expect("select alpha again");
    assert_eq!(desk.field(MetadataField::Title).value, "Alpha draft");
}

#[test]
fn an_invalid_edit_keeps_the_selection_and_the_titles_where_they_are() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.type_into(MetadataField::Date, "soon");

    assert!(matches!(desk.select(&[1]), Err(GateBlock::Invalid(_))));
    assert!(matches!(
        desk.change(|set| {
            set.remove_file(0);
        }),
        Err(GateBlock::Invalid(_))
    ));

    assert_eq!(desk.state.working_set.files().len(), 2);
    assert_eq!(desk.state.working_set.selection(0).selected_indices, [0]);
    assert_eq!(desk.field(MetadataField::Date).value, "soon");
    assert!(matches!(
        desk.metadata().status,
        Some(MetadataStatus::DraftInvalid { .. })
    ));
}

#[test]
fn a_file_whose_tags_cannot_be_read_is_named_beside_its_blank_form() {
    let mut desk = Desk::open(&[("alpha", None), ("beta", None)], &[0]);
    assert_eq!(
        desk.metadata().status,
        Some(MetadataStatus::TagReadFailed {
            label: "alpha.m4b".to_string(),
            more: 0
        })
    );

    desk.select(&[0, 1]).expect("select both");
    assert_eq!(
        desk.metadata().status,
        Some(MetadataStatus::TagReadFailed {
            label: "alpha.m4b".to_string(),
            more: 1
        })
    );

    let mut readable = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    assert_eq!(readable.metadata().status, None);
}

#[test]
fn a_failed_second_read_of_a_title_whose_tags_arrived_reports_nothing() {
    let mut desk = Desk::open(&[], &[]);
    desk.disk.insert(path("alpha"), alpha_tags());
    desk.state
        .working_set
        .append_analyzed(vec![audio_file("alpha", true)], &audio_request());
    let mut reads = desk.state.rebind();
    reads.extend(desk.state.rebind());
    assert_eq!(
        reads.len(),
        2,
        "selecting the title again starts a second read"
    );
    let second = reads.pop().expect("second read");
    let first = reads.pop().expect("first read");

    let binding = desk.state.binding;
    desk.state.finish_reads(
        binding,
        vec![
            (first, Ok(alpha_tags())),
            (second, Err(AppError::General("file busy".to_string()))),
        ],
    );

    assert_eq!(desk.metadata().status, None);
    assert_eq!(desk.field(MetadataField::Title).value, "Alpha");
}

#[test]
fn an_unreadable_file_still_shows_its_staged_edits_when_selected_again() {
    let mut desk = Desk::open(&[("alpha", None), ("beta", None)], &[0]);
    desk.type_into(MetadataField::Author, "Edited Author");
    desk.select(&[1]).expect("select beta");
    desk.select(&[0]).expect("select alpha");

    assert_eq!(desk.field(MetadataField::Author).value, "Edited Author");
}

#[test]
fn an_edit_typed_while_tags_load_survives_their_arrival() {
    let mut desk = Desk::open(&[], &[]);
    desk.disk.insert(path("alpha"), alpha_tags());
    desk.state
        .working_set
        .append_analyzed(vec![audio_file("alpha", true)], &audio_request());
    desk.reads = desk.state.rebind();
    assert!(
        !desk.reads.is_empty(),
        "the selection asks for a source read"
    );

    desk.type_into(MetadataField::Author, "Typed while loading");
    desk.finish_reads();

    assert_eq!(
        desk.field(MetadataField::Author).value,
        "Typed while loading"
    );
    assert_eq!(desk.field(MetadataField::Title).value, "Alpha");
}

#[test]
fn one_title_draft_survives_grouping_source_reorder_save_and_separation() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.type_into(MetadataField::Title, "Alpha draft");
    desk.select(&[1]).expect("select beta");
    desk.type_into(MetadataField::Title, "Beta draft");
    desk.select(&[0, 1]).expect("select both");
    desk.change(|set| {
        set.group_selected();
    })
    .expect("group");
    assert_eq!(desk.field(MetadataField::Title).value, "Alpha draft");

    desk.state.working_set.reorder_sources("alpha", 0, 1);
    desk.bind();
    assert_eq!(desk.field(MetadataField::Title).value, "Alpha draft");

    desk.type_into(MetadataField::Title, "Grouped title");
    assert!(desk.save(&[]).is_none());
    assert!(desk.written.is_empty());
    assert_eq!(
        desk.metadata().status,
        Some(MetadataStatus::GroupedEditsKept)
    );
    assert_eq!(
        desk.pending("alpha").and_then(|patch| patch.title),
        set("Grouped title")
    );

    desk.change(|set| {
        set.ungroup("alpha");
    })
    .expect("ungroup");
    desk.select(&[0]).expect("first source");
    assert_eq!(desk.field(MetadataField::Title).value, "Beta draft");
    desk.select(&[1]).expect("second source");
    assert_eq!(desk.field(MetadataField::Title).value, "Grouped title");
}

#[test]
fn removing_a_title_forgets_its_tags_so_reimport_reads_them_again() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Author, "Edited");
    desk.change(|set| {
        set.remove_file(0);
    })
    .expect("remove");
    assert!(desk.state.known_tags(&path("alpha")).is_none());

    desk.disk.insert(
        path("alpha"),
        AudiobookMetadata {
            artist: Some("Changed On Disk".to_string()),
            ..alpha_tags()
        },
    );
    desk.import(&[("alpha", None)]);
    desk.select(&[0]).expect("select");

    assert_eq!(desk.field(MetadataField::Author).value, "Changed On Disk");
}

#[test]
fn selection_cannot_change_while_a_save_is_in_progress() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.type_into(MetadataField::Genre, "Mystery");
    let plan = desk
        .state
        .begin_save(&HashSet::new(), |_| false)
        .expect("save begins");

    assert_eq!(desk.select(&[1]), Err(GateBlock::SaveInProgress));

    let epoch = desk.state.epoch;
    desk.state.finish_save(
        epoch,
        &paths_of(&plan.immediate),
        &written_outcomes(&plan.immediate),
        MetadataStatus::SaveFailed {
            error: crate::errors::AppErrorEnvelope::from(&crate::errors::AppError::General(
                "write failed".into(),
            )),
        },
    );
    assert!(desk.select(&[1]).is_ok());
}

#[test]
fn a_lookup_result_applies_only_to_the_title_it_was_chosen_for() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    let alpha = QueuedTitle {
        title_id: "alpha".to_string(),
        path: path("alpha"),
        label: "alpha".to_string(),
    };
    let found = AudiobookMetadata {
        title: Some("Found".to_string()),
        ..Default::default()
    };

    desk.select(&[1]).expect("select beta");
    assert!(!desk.state.apply_lookup(&alpha, &found, None));
    assert_eq!(desk.field(MetadataField::Title).value, "Beta");

    desk.select(&[0]).expect("select alpha");
    assert!(desk.state.apply_lookup(&alpha, &found, None));
    desk.save(&[]);
    assert_eq!(
        desk.written_by_file()[&path("alpha")],
        MetadataIntentPatch {
            title: set("Found"),
            album: set("Found"),
            ..Default::default()
        }
    );
}

#[test]
fn reordering_the_selected_titles_keeps_the_form_bound_to_them() {
    let mut desk = two_books();
    desk.type_into(MetadataField::Genre, "Mystery");
    let binding = desk.state.binding;

    desk.state.working_set.reorder_files(0, 1);
    desk.import(&[("gamma", Some(beta_tags()))]);

    assert_eq!(desk.state.binding, binding);
    assert!(desk
        .state
        .set_field(binding, MetadataField::Date, "2003".to_string()));
    assert!(desk.field(MetadataField::Genre).dirty);
    desk.save(&[]);
    let on_disk = |desk: &Desk, name| desk.disk[&path(name)].genre.clone();
    assert_eq!(on_disk(&desk, "alpha").as_deref(), Some("Mystery"));
    assert_eq!(on_disk(&desk, "beta").as_deref(), Some("Mystery"));
}
