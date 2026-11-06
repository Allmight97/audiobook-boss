use super::*;

#[test]
fn a_cover_belongs_to_exactly_one_valid_selected_title() {
    let mut desk = two_books();
    assert!(!desk.state.apply_cover(vec![1]), "two titles are selected");

    desk.select(&[0]).expect("select alpha");
    assert!(desk.state.apply_cover(vec![1]));
    assert_eq!(desk.cover(), Some(vec![1]));
    assert!(desk.metadata().cover.custom);
}

#[test]
fn several_selected_titles_show_a_cover_only_when_theirs_match() {
    let mut desk = two_books();
    assert_eq!(desk.cover(), Some(vec![7, 7, 7]));

    desk.select(&[1]).expect("select beta");
    desk.state.apply_cover(vec![9]);
    desk.select(&[0, 1]).expect("select both");

    assert_eq!(desk.cover(), None);
}

#[test]
fn with_nothing_selected_the_first_valid_title_cover_shows() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[],
    );
    assert_eq!(desk.cover(), Some(vec![7, 7, 7]));
}

#[test]
fn a_saved_cover_gives_source_covers_a_new_address_and_a_tag_edit_does_not() {
    let covers_revision = |desk: &mut Desk| {
        desk.state.settle();
        desk.state
            .update_since(None)
            .titles
            .expect("titles")
            .covers_revision
    };
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    let before = covers_revision(&mut desk);

    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save(&[]).expect("tag save");
    assert_eq!(covers_revision(&mut desk), before);

    desk.state.apply_cover(vec![1]);
    desk.save(&[]).expect("cover save");
    assert!(covers_revision(&mut desk) > before);
}

#[test]
fn a_cover_changed_while_a_save_ran_is_still_unsaved_afterward() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.apply_cover(vec![1]);
    let plan = desk
        .state
        .begin_save(&HashSet::new(), |_| false)
        .expect("save begins");
    desk.state.apply_cover(vec![2]);

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
    desk.state.settle();

    assert!(desk.metadata().cover.custom);
    assert_eq!(
        desk.pending("alpha").and_then(|patch| patch.cover_art),
        Some(PatchOp::Set(vec![2]))
    );
    assert_eq!(desk.cover(), Some(vec![2]));
}

#[test]
fn a_cover_load_failure_is_reported_and_changes_nothing() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.begin_cover_request(true);
    let applied = desk.state.cover_load_finished(
        true,
        Err(AppError::InvalidInput(
            "Only HTTPS image URLs are allowed.".to_string(),
        )),
    );
    desk.state.settle();

    assert!(!applied);
    let cover = desk.metadata().cover;
    assert!(!cover.loading);
    assert!(matches!(cover.notice, Some(CoverNotice::LoadFailed { .. })));
    assert!(desk.pending("alpha").is_none());
}
