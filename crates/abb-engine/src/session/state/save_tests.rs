use super::*;

#[test]
fn a_failed_write_keeps_the_edit_pending_and_the_next_save_retries_it() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save(&["alpha"]);

    assert!(!desk.metadata().save_in_progress);
    assert_eq!(desk.pending("alpha"), Some(genre_patch()));
    assert!(desk.written.is_empty());

    desk.save(&[]);
    assert_eq!(desk.written, [(path("alpha"), genre_patch())]);
    assert!(desk.pending("alpha").is_none());
}

#[test]
fn an_invalid_edit_blocks_save_and_stages_nothing() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Date, "soon");

    assert!(desk.save(&[]).is_none());
    assert_eq!(desk.metadata().status, Some(MetadataStatus::SaveInvalid));
    assert!(desk.pending("alpha").is_none());
    assert!(desk.metadata().form.validation_message.is_some());
}

#[test]
fn save_with_no_export_running_writes_every_pending_edit_now() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.select(&[1]).expect("select beta");
    desk.type_into(MetadataField::Genre, "Mystery");

    let plan = desk.save(&[]).expect("save runs");

    assert_eq!(plan.immediate.len(), 2);
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Mystery"));
    assert_eq!(desk.disk[&path("beta")].genre.as_deref(), Some("Mystery"));
    assert!(!desk.metadata().has_pending_edits);
}

#[test]
fn save_on_a_local_source_in_flight_waits_for_every_export_reading_it() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    let reading = HashSet::from([path("alpha")]);

    let plan = desk.save_during(&reading, &[], &[]).expect("save accepted");

    assert_eq!((plan.immediate.len(), plan.waiting, plan.held), (0, 1, 0));
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Fantasy"));
    assert_eq!(desk.state.waiting_write_paths(), [path("alpha")]);

    desk.exports_now_reading(&reading);
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Fantasy"));

    desk.exports_now_reading(&HashSet::new());
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Mystery"));
    let metadata = desk.metadata();
    assert_eq!(
        metadata.status,
        Some(MetadataStatus::DeferredWritesFinished {
            written: 1,
            failed: 0
        })
    );
    assert!(!metadata.has_pending_edits);
    assert!(desk.state.waiting_write_paths().is_empty());
}

#[test]
fn save_on_a_temporary_source_in_flight_writes_no_file_and_keeps_the_edit() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    let reading = HashSet::from([path("alpha")]);

    let plan = desk
        .save_during(&reading, &[path("alpha")], &[])
        .expect("save accepted");

    assert_eq!((plan.immediate.len(), plan.waiting, plan.held), (0, 0, 1));
    desk.exports_now_reading(&HashSet::new());
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Fantasy"));
    assert!(desk.written.is_empty());
    assert_eq!(desk.pending("alpha"), Some(genre_patch()));
    assert!(desk.state.waiting_write_paths().is_empty());
}

#[test]
fn a_later_save_replaces_the_edit_a_waiting_write_will_apply() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    let reading = HashSet::from([path("alpha")]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save_during(&reading, &[], &[]);
    desk.type_into(MetadataField::Author, "Edited Author");
    desk.save_during(&reading, &[], &[]);

    assert_eq!(desk.state.waiting_write_paths(), [path("alpha")]);
    desk.exports_now_reading(&HashSet::new());

    let on_disk = &desk.disk[&path("alpha")];
    assert_eq!(on_disk.genre.as_deref(), Some("Mystery"));
    assert_eq!(on_disk.artist.as_deref(), Some("Edited Author"));
    assert!(!desk.metadata().has_pending_edits);
}

#[test]
fn a_later_save_keeps_the_waiting_edit_of_a_title_that_left_and_returned() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    let reading = HashSet::from([path("alpha")]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save_during(&reading, &[], &[]);
    desk.change(|set| {
        set.remove_file(0);
    })
    .expect("remove");
    desk.import(&[("alpha", None)]);
    desk.select(&[0]).expect("select returning title");
    desk.type_into(MetadataField::Author, "Edited Author");
    desk.save_during(&reading, &[], &[]);

    desk.exports_now_reading(&HashSet::new());

    let on_disk = &desk.disk[&path("alpha")];
    assert_eq!(on_disk.genre.as_deref(), Some("Mystery"));
    assert_eq!(on_disk.artist.as_deref(), Some("Edited Author"));
}

#[test]
fn a_save_on_a_freed_file_joins_the_write_still_waiting_there() {
    for fails in [false, true] {
        let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
        desk.type_into(MetadataField::Genre, "Mystery");
        desk.save_during(&HashSet::from([path("alpha")]), &[], &[]);
        desk.change(|set| {
            set.remove_file(0);
        })
        .expect("remove");
        desk.import(&[("alpha", None)]);
        desk.select(&[0]).expect("select returning title");
        desk.type_into(MetadataField::Author, "Edited Author");

        let plan = desk.save(&[]).expect("save");
        assert_eq!((plan.immediate.len(), plan.waiting), (0, 1));
        let taken = desk.begin_deferred(&HashSet::new());
        let fail: &[&str] = if fails { &["alpha"] } else { &[] };
        desk.end_deferred(taken, fail);
        if fails {
            desk.save(&[]).expect("retry");
        }

        let on_disk = &desk.disk[&path("alpha")];
        assert_eq!(on_disk.genre.as_deref(), Some("Mystery"));
        assert_eq!(on_disk.artist.as_deref(), Some("Edited Author"));
    }
}

#[test]
fn a_save_while_a_waiting_write_runs_never_writes_the_file_alongside_it() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    let reading = HashSet::from([path("alpha")]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save_during(&reading, &[], &[]);
    let running = desk.begin_deferred(&HashSet::new());
    assert_eq!(running.len(), 1);

    for author in ["First", "Second"] {
        desk.type_into(MetadataField::Author, author);
        let plan = desk.save_during(&HashSet::new(), &[], &[]).expect("save");
        assert_eq!((plan.immediate.len(), plan.waiting), (0, 1));
        assert!(desk.begin_deferred(&HashSet::new()).is_empty());
    }

    desk.end_deferred(running, &[]);
    let next = desk.begin_deferred(&HashSet::new());
    desk.end_deferred(next, &[]);
    let on_disk = &desk.disk[&path("alpha")];
    assert_eq!(on_disk.genre.as_deref(), Some("Mystery"));
    assert_eq!(on_disk.artist.as_deref(), Some("Second"));
    assert!(desk.state.waiting_write_paths().is_empty());
}

#[test]
fn a_waiting_write_for_a_removed_title_reaches_it_when_it_returns() {
    for fails in [false, true] {
        let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
        let reading = HashSet::from([path("alpha")]);
        desk.type_into(MetadataField::Genre, "Mystery");
        desk.save_during(&reading, &[], &[]);
        desk.change(|set| {
            set.remove_file(0);
        })
        .expect("remove");
        assert_eq!(desk.state.waiting_write_paths(), [path("alpha")]);

        desk.import(&[("alpha", None)]);
        desk.select(&[0]).expect("select");
        assert_eq!(desk.field(MetadataField::Genre).value, "Fantasy");

        let fail: &[&str] = if fails { &["alpha"] } else { &[] };
        let taken = desk.begin_deferred(&HashSet::new());
        desk.end_deferred(taken, fail);

        assert_eq!(desk.field(MetadataField::Genre).value, "Mystery");
        assert_eq!(desk.pending("alpha").is_some(), fails);
        assert_eq!(
            desk.metadata().status,
            Some(MetadataStatus::DeferredWritesFinished {
                written: usize::from(!fails),
                failed: usize::from(fails),
            })
        );
    }
}

#[test]
fn a_failed_waiting_write_can_be_retried_after_its_title_returns_later() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save_during(&HashSet::from([path("alpha")]), &[], &[]);
    desk.change(|set| {
        set.remove_file(0);
    })
    .expect("remove");
    let taken = desk.begin_deferred(&HashSet::new());
    desk.end_deferred(taken, &["alpha"]);
    assert!(desk.state.waiting_write_paths().is_empty());
    desk.import(&[("alpha", None)]);
    desk.select(&[0]).expect("select returning title");
    assert_eq!(desk.field(MetadataField::Genre).value, "Mystery");
    desk.save_during(&HashSet::new(), &[], &[]);
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Mystery"));
}

#[test]
fn a_save_while_a_submission_is_prepared_waits_for_its_sources() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.output.set_directory("/library".to_string());
    let draft = desk
        .state
        .begin_submission(false)
        .expect("the submission starts");
    assert!(desk.state.working_set.order_locked());

    desk.type_into(MetadataField::Genre, "Mystery");
    let plan = desk.save_during(&HashSet::new(), &[], &[]).expect("save");
    assert_eq!((plan.immediate.len(), plan.waiting), (0, 1));

    desk.state
        .finish_submission(&draft, SubmissionStatus::Cancelled);
    assert!(!desk.state.working_set.order_locked());
    desk.exports_now_reading(&HashSet::new());
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Mystery"));
}

#[test]
fn a_download_waits_for_the_export_and_the_submission_reading_it() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    let staged = &mut desk.state.staged;
    staged.register("job-1", "alpha", path("alpha"), Vec::new());
    staged.register("job-2", "beta", path("beta"), Vec::new());

    desk.state.working_set.remove_file(1);
    desk.state.settle();
    assert!(desk
        .state
        .begin_staged_removal(&HashSet::from([path("beta")]), Instant::now())
        .is_empty());
    assert_eq!(
        desk.state
            .begin_staged_removal(&HashSet::new(), Instant::now()),
        ["job-2"]
    );

    desk.state
        .staged
        .finish_export(&exported(&[("alpha", ChildJobStatus::Completed, false)]));
    desk.state.output.set_directory("/library".to_string());
    let draft = desk.state.begin_submission(false).expect("submission");
    assert!(desk
        .state
        .begin_staged_removal(&HashSet::new(), Instant::now())
        .is_empty());
    desk.state
        .finish_submission(&draft, SubmissionStatus::Cancelled);
    assert_eq!(
        desk.state
            .begin_staged_removal(&HashSet::new(), Instant::now()),
        ["job-1"]
    );
}

#[test]
fn a_download_being_removed_is_neither_written_nor_submitted() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state
        .staged
        .register("job-1", "alpha", path("alpha"), Vec::new());
    desk.state
        .staged
        .finish_export(&exported(&[("alpha", ChildJobStatus::Completed, false)]));
    assert_eq!(
        desk.state
            .begin_staged_removal(&HashSet::new(), Instant::now()),
        ["job-1"]
    );

    desk.type_into(MetadataField::Genre, "Mystery");
    let plan = desk
        .save_during(&HashSet::new(), &[path("alpha")], &[])
        .expect("save");
    assert_eq!(
        (plan.immediate.len(), plan.held),
        (0, 1),
        "the file is held"
    );
    desk.state.output.set_directory("/library".to_string());
    assert!(desk.state.begin_submission(false).is_none());
    assert_eq!(
        desk.state.output_snapshot(0).submission,
        Some(SubmissionStatus::Refused {
            reason: SubmitRefusal::SourceRemoved
        })
    );

    desk.state
        .finish_staged_removal("job-1", true, Instant::now());
    assert!(desk.state.staged.is_empty());
    assert!(
        !desk.state.working_set.files()[0].is_valid,
        "the removed download remains only as history"
    );
    let defaults = desk.state.audio.request();
    let mut next = crate::audio::AudioFile::new(path("beta"));
    next.is_valid = true;
    next.set_tag_title(Some("Beta".to_string()));
    desk.state
        .working_set
        .append_analyzed(vec![next], &defaults);
    desk.bind();
    let draft = desk
        .state
        .begin_submission(false)
        .expect("the next batch excludes removed files");
    assert_eq!(
        draft.payload.input_files,
        vec![path("beta").to_string_lossy().into_owned()]
    );
}

#[test]
fn no_download_is_removed_while_a_save_writes() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state
        .staged
        .register("job-1", "beta", path("beta"), Vec::new());
    desk.state.settle();
    desk.type_into(MetadataField::Genre, "Mystery");
    let plan = desk
        .state
        .begin_save(&HashSet::new(), |_| false)
        .expect("save");
    assert!(desk
        .state
        .begin_staged_removal(&HashSet::new(), Instant::now())
        .is_empty());

    let epoch = desk.state.epoch;
    desk.state.finish_save(
        epoch,
        &paths_of(&plan.immediate),
        &plan.immediate,
        MetadataStatus::SaveComplete {
            succeeded: 1,
            failed: 0,
            cancelled: 0,
            waiting: 0,
            held: 0,
            outputs: Default::default(),
        },
    );
    assert_eq!(
        desk.state
            .begin_staged_removal(&HashSet::new(), Instant::now()),
        ["job-1"]
    );
}

#[test]
fn a_failed_waiting_write_hands_its_edit_to_the_newer_write_waiting_on_the_same_file() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    let reading = HashSet::from([path("alpha")]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save_during(&reading, &[], &[]);
    let running = desk.begin_deferred(&HashSet::new());
    assert_eq!(running.len(), 1);
    desk.change(|set| {
        set.remove_file(0);
    })
    .expect("remove");
    desk.import(&[("alpha", None)]);
    desk.select(&[0]).expect("select returning title");
    desk.type_into(MetadataField::Author, "Second");
    let plan = desk.save_during(&HashSet::new(), &[], &[]).expect("save");
    assert_eq!((plan.immediate.len(), plan.waiting), (0, 1));

    desk.end_deferred(running, &["alpha"]);
    let next = desk.begin_deferred(&HashSet::new());
    assert_eq!(next.len(), 1);
    assert_eq!(
        next[0].patch.genre,
        Some(PatchOp::Set("Mystery".to_string()))
    );
    assert_eq!(
        next[0].patch.artist,
        Some(PatchOp::Set("Second".to_string()))
    );
    desk.end_deferred(next, &[]);
    let on_disk = &desk.disk[&path("alpha")];
    assert_eq!(on_disk.genre.as_deref(), Some("Mystery"));
    assert_eq!(on_disk.artist.as_deref(), Some("Second"));
}

#[test]
fn a_newer_write_that_lands_ends_the_older_failed_write_of_the_same_field() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save_during(&HashSet::from([path("alpha")]), &[], &[]);
    let running = desk.begin_deferred(&HashSet::new());
    desk.type_into(MetadataField::Genre, "Horror");
    desk.save_during(&HashSet::new(), &[], &[]).expect("save");

    desk.end_deferred(running, &["alpha"]);
    let next = desk.begin_deferred(&HashSet::new());
    desk.end_deferred(next, &[]);

    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Horror"));
    assert!(desk.pending("alpha").is_none());
    assert_eq!(desk.field(MetadataField::Genre).value, "Horror");
}

#[test]
fn a_newer_write_that_also_fails_leaves_the_newest_value_pending_after_a_reimport() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save_during(&HashSet::from([path("alpha")]), &[], &[]);
    let running = desk.begin_deferred(&HashSet::new());
    desk.type_into(MetadataField::Genre, "Horror");
    desk.save_during(&HashSet::new(), &[], &[]).expect("save");

    desk.end_deferred(running, &["alpha"]);
    desk.change(|set| {
        set.remove_file(0);
    })
    .expect("remove");
    let next = desk.begin_deferred(&HashSet::new());
    desk.end_deferred(next, &["alpha"]);
    desk.import(&[("alpha", None)]);
    desk.select(&[0]).expect("select returning title");

    assert_eq!(desk.field(MetadataField::Genre).value, "Horror");
    assert_eq!(
        desk.pending("alpha").and_then(|patch| patch.genre),
        Some(PatchOp::Set("Horror".to_string()))
    );
}

#[test]
fn a_failed_write_keeps_the_waiting_retry_saved_with_no_edit_between() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save_during(&HashSet::from([path("alpha")]), &[], &[]);
    let running = desk.begin_deferred(&HashSet::new());
    desk.save_during(&HashSet::new(), &[], &[]).expect("save");
    desk.change(|set| {
        set.remove_file(0);
    })
    .expect("remove");

    desk.end_deferred(running, &["alpha"]);
    let retry = desk.begin_deferred(&HashSet::new());
    desk.end_deferred(retry, &["alpha"]);
    desk.import(&[("alpha", None)]);
    desk.select(&[0]).expect("select returning title");

    assert_eq!(desk.field(MetadataField::Genre).value, "Mystery");
}

#[test]
fn a_save_with_nothing_to_write_never_ends_the_save_running_after_it() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.type_into(MetadataField::Genre, "Mystery");
    let waiting = desk
        .state
        .begin_save(&HashSet::from([path("alpha")]), |_| false)
        .expect("save");
    assert!(waiting.immediate.is_empty());

    desk.select(&[1]).expect("select beta");
    desk.type_into(MetadataField::Genre, "Thriller");
    let reading = HashSet::from([path("alpha")]);
    let running = desk.state.begin_save(&reading, |_| false).expect("save");
    assert_eq!(running.immediate.len(), 1);

    let epoch = desk.state.epoch;
    desk.state
        .finish_save(epoch, &[], &[], MetadataStatus::NoPendingChanges);
    assert!(desk.metadata().save_in_progress);

    desk.state.finish_save(
        epoch,
        &paths_of(&running.immediate),
        &running.immediate,
        MetadataStatus::SaveCancelled,
    );
    assert!(!desk.metadata().save_in_progress);
}

#[test]
fn a_save_with_nothing_to_write_that_finishes_after_clear_all_leaves_the_empty_session_without_its_status(
) {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    let waiting = desk
        .state
        .begin_save(&HashSet::from([path("alpha")]), |_| false)
        .expect("save");
    assert!(waiting.immediate.is_empty());
    let epoch = desk.state.epoch;

    desk.change(|set| {
        set.clear_all();
    })
    .expect("clear all");
    desk.state
        .finish_save(epoch, &[], &[], MetadataStatus::NoPendingChanges);

    assert_eq!(desk.metadata().status, None);
}
