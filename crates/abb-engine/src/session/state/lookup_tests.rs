use super::*;
use crate::errors::AppError;
use crate::metadata_lookup::{MetadataLookupResponse, MetadataSource, OnlineMetadataResult};
use crate::session::lookup::{
    LookupApplyMode, LookupSnapshot, LookupSource, LookupStatus, QueueStep,
};

fn result(title: &str) -> OnlineMetadataResult {
    OnlineMetadataResult {
        source: MetadataSource::Audnexus,
        source_id: title.to_string(),
        title: title.to_string(),
        authors: Vec::new(),
        narrators: Vec::new(),
        series: None,
        series_part: None,
        subseries: None,
        subseries_part: None,
        description: None,
        published_date: None,
        duration_seconds: None,
        cover_url: None,
        cover: None,
        audible_only: None,
    }
}

fn found(titles: &[&str]) -> Result<MetadataLookupResponse, AppError> {
    Ok(MetadataLookupResponse {
        results: titles.iter().map(|title| result(title)).collect(),
        diagnostics: Vec::new(),
    })
}

fn lookup(desk: &mut Desk) -> LookupSnapshot {
    desk.state.settle();
    desk.state.update_since(None).lookup.expect("lookup part")
}

fn searching(desk: &mut Desk) -> u64 {
    let (request, _) = desk.state.lookup_open();
    begin_search(desk, request);
    request
}

fn begin_search(desk: &mut Desk, request: u64) {
    let search = desk.state.lookup_search(request, None);
    assert!(
        matches!(search, Some(Some(_))),
        "a search with a query starts: {search:?}"
    );
    desk.state.settle();
}

fn answer(
    desk: &mut Desk,
    request: u64,
    response: Result<MetadataLookupResponse, AppError>,
) -> Option<()> {
    let outcome =
        desk.state
            .lookup_finish_search(request, None, response, std::time::Duration::ZERO);
    desk.state.settle();
    outcome
}

type Edit = fn(&mut LookupState);

fn one_book() -> Desk {
    Desk::open(&[("alpha", Some(alpha_tags()))], &[0])
}

#[test]
fn opening_with_several_titles_queues_them_and_opening_with_none_says_so() {
    let mut desk = two_books();
    desk.state.lookup_open();
    let opened = lookup(&mut desk);
    assert!(opened.open && opened.is_queue_mode);
    assert_eq!(opened.apply_mode, LookupApplyMode::Queue);
    assert_eq!(opened.title_query, "Alpha");
    assert_eq!(
        opened.queue_position.map(|position| position.total),
        Some(2)
    );

    desk.select(&[]).expect("an untouched form passes the gate");
    let (_, has_titles) = desk.state.lookup_open();
    let empty = lookup(&mut desk);
    assert!(!has_titles);
    assert_eq!(empty.status, Some(LookupStatus::NoValidTitle));
    assert_eq!(empty.apply_mode, LookupApplyMode::Current);
    assert!(empty.queue_position.is_none());
}

#[test]
fn an_empty_query_asks_for_one_and_starts_no_search() {
    let mut desk = one_book();
    desk.state.lookup_open();
    desk.state.lookup.edit_title_query("  ".to_string());
    desk.state.lookup.edit_author_query(String::new());
    let request = desk.state.lookup.begin_search();

    assert!(matches!(
        desk.state.lookup_search(request, None),
        Some(None)
    ));

    assert_eq!(
        lookup(&mut desk).status,
        Some(LookupStatus::QueryRequired { after: None })
    );
}

#[test]
fn a_failed_search_is_not_shown_as_no_matches() {
    let mut desk = one_book();
    let request = searching(&mut desk);
    answer(&mut desk, request, found(&["Earlier"])).expect("current");

    begin_search(&mut desk, request);
    answer(
        &mut desk,
        request,
        Err(AppError::General("network down".to_string())),
    )
    .expect("current");

    let failed = lookup(&mut desk);
    assert_eq!(
        failed.status,
        Some(LookupStatus::SearchFailed { after: None })
    );
    assert!(failed.results.is_empty());
    assert!(!failed.has_searched);
}

#[test]
fn editing_the_query_author_or_source_during_a_search_drops_its_answer() {
    let edits: [(&str, Edit); 3] = [
        ("query", |lookup| {
            lookup.edit_title_query("Changed".to_string())
        }),
        ("author", |lookup| {
            lookup.edit_author_query("Changed".to_string())
        }),
        ("source", |lookup| {
            lookup.edit_source(LookupSource::Openlibrary)
        }),
    ];
    for (name, edit) in edits {
        let mut desk = one_book();
        let request = searching(&mut desk);
        assert_eq!(lookup(&mut desk).status, Some(LookupStatus::Searching));

        edit(&mut desk.state.lookup);

        assert_eq!(
            answer(&mut desk, request, found(&["Late"])),
            None,
            "{name} edit"
        );
        let after = lookup(&mut desk);
        assert_eq!(
            after.status,
            Some(LookupStatus::SearchSuperseded),
            "{name} edit"
        );
        assert!(after.results.is_empty(), "{name} edit");
    }
}

#[test]
fn changing_how_results_are_applied_does_not_stop_a_search() {
    let mut desk = one_book();
    let request = searching(&mut desk);

    desk.state.lookup.set_apply_mode(LookupApplyMode::Queue);
    desk.state.lookup.set_replace_cover(true);

    assert_eq!(answer(&mut desk, request, found(&["Found"])), Some(()));
    let after = lookup(&mut desk);
    assert_eq!(
        after.status,
        Some(LookupStatus::Found {
            count: 1,
            partial: false,
            after: None
        })
    );
    assert_eq!(after.results[0].title, "Found");
}

#[test]
fn a_newer_search_keeps_its_results_when_an_earlier_one_answers_last() {
    let mut desk = one_book();
    let (earlier, _) = desk.state.lookup_open();
    begin_search(&mut desk, earlier);
    let newer = desk.state.lookup.begin_search();
    begin_search(&mut desk, newer);

    assert_eq!(answer(&mut desk, newer, found(&["Newer"])), Some(()));
    assert_eq!(answer(&mut desk, earlier, found(&["Earlier"])), None);

    assert_eq!(lookup(&mut desk).results[0].title, "Newer");
}

#[test]
fn closing_and_skipping_supersede_the_work_in_flight() {
    let mut desk = one_book();
    let request = searching(&mut desk);
    desk.state.lookup.close();
    assert_eq!(answer(&mut desk, request, found(&["Late"])), None);
    assert!(!lookup(&mut desk).open);

    let request = searching(&mut desk);
    desk.state.lookup.begin_skip();
    assert_eq!(answer(&mut desk, request, found(&["Late"])), None);
}

fn applying(desk: &mut Desk) -> (u64, ChosenResult) {
    let request = searching(desk);
    answer(desk, request, found(&["Found"])).expect("current");
    let revision = lookup(desk).revision;
    match desk.state.lookup_begin_apply(0, revision) {
        ApplyStart::Begun(request, chosen) => (request, *chosen),
        _ => panic!("the apply begins"),
    }
}

#[test]
fn applying_names_the_list_the_host_rendered_and_a_result_in_it() {
    let mut desk = one_book();
    let request = searching(&mut desk);
    answer(&mut desk, request, found(&["Old"])).expect("current");
    let rendered = lookup(&mut desk).revision;
    begin_search(&mut desk, request);
    answer(&mut desk, request, found(&["New"])).expect("current");
    let before = lookup(&mut desk);

    let stale = desk.state.lookup_begin_apply(0, rendered);
    let missing = desk.state.lookup_begin_apply(9, before.revision);

    assert!(matches!(stale, ApplyStart::Superseded));
    assert!(matches!(missing, ApplyStart::Superseded));
    let after = lookup(&mut desk);
    assert_eq!(after.status, before.status);
    assert_eq!(after.results, before.results);
}

#[test]
fn applying_supersedes_an_earlier_apply_and_editing_the_query_stops_it() {
    let mut desk = one_book();
    let (request, chosen) = applying(&mut desk);
    assert_eq!(lookup(&mut desk).status, Some(LookupStatus::Applying));

    desk.state.lookup.edit_title_query("Changed".to_string());

    assert_eq!(
        desk.state
            .lookup_finish_apply(request, &chosen, true, None, false),
        None
    );
    assert_eq!(
        lookup(&mut desk).status,
        Some(LookupStatus::ApplySuperseded)
    );
}

#[test]
fn a_finished_apply_reports_the_cover_and_the_form_edit() {
    let mut desk = one_book();
    let (request, chosen) = applying(&mut desk);

    let applied = desk
        .state
        .lookup_finish_apply(request, &chosen, true, None, true);

    assert_eq!(applied, Some(true));
    assert_eq!(
        lookup(&mut desk).status,
        Some(LookupStatus::Applied { cover_failed: true })
    );
    assert_eq!(desk.field(MetadataField::Title).value, "Found");
}

#[test]
fn an_apply_the_form_refuses_is_rejected_and_stopping_it_marks_it_superseded() {
    let mut desk = one_book();
    let (request, chosen) = applying(&mut desk);
    assert_eq!(
        desk.state
            .lookup_finish_apply(request, &chosen, false, None, false),
        Some(false)
    );
    assert_eq!(lookup(&mut desk).status, Some(LookupStatus::ApplyRejected));

    let (request, _) = applying(&mut desk);
    desk.state.lookup_stop_apply(request);
    assert_eq!(
        lookup(&mut desk).status,
        Some(LookupStatus::ApplySuperseded)
    );
}

#[test]
fn the_queue_advances_one_title_at_a_time_and_completes_after_the_last() {
    let mut desk = two_books();
    let (request, _) = desk.state.lookup_open();
    let step = QueueStep::Skipped;

    let Some(Some(NextTitle::Move { index, title, .. })) =
        desk.state.lookup_next_title(request, step)
    else {
        panic!("the queue has a next title");
    };
    assert!(desk.state.lookup_select_title(&title));
    desk.bind();
    assert_eq!(
        desk.state
            .lookup_move_to_next(request, step, index, &title, true),
        Some(true)
    );
    let moved = lookup(&mut desk);
    assert_eq!(moved.queue_position.map(|at| at.index), Some(1));
    assert_eq!(moved.title_query, "Beta");

    assert!(matches!(
        desk.state.lookup_next_title(request, step),
        Some(Some(NextTitle::Stay))
    ));
    assert_eq!(
        lookup(&mut desk).status,
        Some(LookupStatus::QueueComplete {
            cover_failed: false
        })
    );
}

#[test]
fn a_next_title_the_gate_refuses_keeps_the_queue_where_it_was() {
    let mut desk = two_books();
    let (request, _) = desk.state.lookup_open();
    let step = QueueStep::AppliedWithoutCover;
    let Some(Some(NextTitle::Move { index, title, .. })) =
        desk.state.lookup_next_title(request, step)
    else {
        panic!("the queue has a next title");
    };

    assert_eq!(
        desk.state
            .lookup_move_to_next(request, step, index, &title, false),
        Some(false)
    );

    let kept = lookup(&mut desk);
    assert_eq!(
        kept.status,
        Some(LookupStatus::NextTitleRejected { cover_failed: true })
    );
    assert_eq!(kept.queue_position.map(|at| at.index), Some(0));
}
