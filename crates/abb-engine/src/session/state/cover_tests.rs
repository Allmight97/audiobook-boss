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
fn saving_one_cover_changes_only_that_source_address() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    let (alpha, beta) = (desk.source_cover("alpha"), desk.source_cover("beta"));
    assert!(alpha.is_some() && beta.is_some());

    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save(&[]).expect("tag save");
    assert_eq!(desk.source_cover("alpha"), alpha);

    desk.state.apply_cover(vec![1]);
    desk.save(&[]).expect("cover save");
    assert_ne!(desk.source_cover("alpha"), alpha);
    assert_eq!(desk.source_cover("beta"), beta);
}

#[test]
fn a_later_cover_write_leaves_an_earlier_written_source_address_alone() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.state.apply_cover(vec![1]);
    desk.save(&[]).expect("alpha cover save");
    let alpha = desk.source_cover("alpha");
    let beta = desk.source_cover("beta");

    desk.select(&[1]).expect("select beta");
    desk.state.apply_cover(vec![2]);
    desk.save(&[]).expect("beta cover save");

    assert_eq!(desk.source_cover("alpha"), alpha);
    assert_ne!(desk.source_cover("beta"), beta);
}

#[test]
fn a_source_never_offers_an_address_it_offered_for_another_image() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    let mut offered = vec![desk.source_cover("alpha")];
    for byte in [1, 2] {
        desk.state.apply_cover(vec![byte]);
        desk.save(&[]).expect("cover save");
        offered.push(desk.source_cover("alpha"));
    }

    desk.change(|set| {
        set.remove_file(0);
    })
    .expect("remove");
    desk.import(&[("alpha", Some(alpha_tags()))]);
    assert_eq!(
        desk.source_cover("alpha"),
        offered.last().cloned().flatten()
    );

    let distinct: HashSet<_> = offered.iter().collect();
    assert_eq!(distinct.len(), offered.len());
}

#[test]
fn a_cover_written_after_its_title_was_grouped_leaves_the_anchor_address_alone() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.state.apply_cover(vec![1]);
    let reading = HashSet::from([path("alpha")]);
    desk.save_during(&reading, &[], &[]).expect("save waits");
    desk.select(&[0, 1]).expect("select both");
    desk.change(|set| {
        set.group_selected();
    })
    .expect("group");
    let anchor = desk.source_cover("alpha");

    desk.exports_now_reading(&HashSet::new());

    assert_eq!(desk.written.len(), 1, "the waiting write ran");
    assert_eq!(desk.source_cover("alpha"), anchor);
}

#[test]
fn the_offered_covers_parse_back_to_the_sources_and_the_cover_on_screen() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.state.apply_cover(vec![1]);
    desk.save(&[]).expect("cover save");

    for (source, cover) in desk.source_covers() {
        let request = CoverRequest::parse(&host_request(&cover.expect("a valid source")));
        assert_eq!(
            request.expect("parses"),
            CoverRequest::Audio { path: source }
        );
    }
    let image = desk.metadata().cover.image.expect("a cover shows");
    assert!(matches!(
        CoverRequest::parse(&host_request(&image)),
        Ok(CoverRequest::Session { .. })
    ));
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
