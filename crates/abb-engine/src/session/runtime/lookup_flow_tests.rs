use super::*;
use crate::session::lookup::{LookupApplyMode, LookupStatus};

#[tokio::test]
async fn lookup_opened_with_no_valid_title_says_so_and_does_not_search() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[]).await;

    rig.send(SessionIntent::LookupOpen).await;

    let lookup = rig.lookup();
    assert!(lookup.open);
    assert_eq!(lookup.status, Some(LookupStatus::NoValidTitle));
    assert!(rig.searches().is_empty());
}

#[tokio::test]
async fn lookup_queues_the_selected_titles_and_searches_for_the_first() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    rig.session.transition(|state| {
        let saved = crate::metadata::MetadataIntentPatch {
            title: Some(crate::metadata::PatchOp::Set("Listed Name".to_string())),
            ..Default::default()
        };
        state.working_set.note_saved_names(&path("alpha"), &saved);
    });
    rig.answer_next_search(Ok(found(&["Found A", "Found B"])));

    rig.send(SessionIntent::LookupOpen).await;

    assert_eq!(
        rig.searches(),
        [(
            "ALPHA Author".to_string(),
            vec![MetadataSource::Audnexus, MetadataSource::Openlibrary]
        )]
    );
    let lookup = rig.lookup();
    assert!(lookup.is_queue_mode);
    assert_eq!(lookup.apply_mode, LookupApplyMode::Queue);
    assert_eq!(
        lookup
            .queue_position
            .map(|at| (at.index, at.total, at.path, at.label)),
        Some((0, 2, path("alpha"), "Listed Name".to_string()))
    );
    assert_eq!(
        lookup.status,
        Some(LookupStatus::Found {
            count: 2,
            partial: false,
            after: None
        })
    );
    assert!(lookup.has_searched);
}

#[tokio::test]
async fn the_open_lookup_is_published_before_its_search_answers() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    let answer = rig.hold_next_search();

    #[expect(
        clippy::disallowed_methods,
        reason = "joined by the_open_lookup_is_published_before_its_search_answers"
    )]
    let opening = tokio::spawn({
        let session = rig.session.clone();
        async move { session.dispatch(SessionIntent::LookupOpen).await }
    });
    while rig.searches().is_empty() {
        tokio::task::yield_now().await;
    }
    let published = rig.events.0.lock().expect("events").clone();
    let lookup = published
        .iter()
        .rev()
        .find_map(|update| update.lookup.clone())
        .expect("lookup was published");
    assert!(lookup.open);
    assert_eq!(lookup.status, Some(LookupStatus::Searching));

    answer
        .send(Ok(found(&["Found"])))
        .unwrap_or_else(|_| panic!("search is waiting"));
    opening.await.expect("open finishes");
}

#[tokio::test]
async fn a_search_sends_the_chosen_source_and_reports_partial_results() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetSource {
        source: LookupSource::Openlibrary,
    })
    .await;
    rig.answer_next_search(Ok(MetadataLookupResponse {
        diagnostics: vec![MetadataLookupDiagnostic {
            kind: MetadataLookupDiagnosticKind::SourceFailedPartialResults,
            source: Some(MetadataSource::Audnexus),
            message: "Audnexus was unavailable".to_string(),
        }],
        ..found(&["Found"])
    }));

    rig.send(SessionIntent::LookupSearch).await;

    assert_eq!(
        rig.searches().last().map(|(_, sources)| sources.clone()),
        Some(vec![MetadataSource::Openlibrary])
    );
    assert_eq!(
        rig.lookup().status,
        Some(LookupStatus::Found {
            count: 1,
            partial: true,
            after: None
        })
    );
}

#[tokio::test]
async fn applying_to_the_current_title_edits_its_form_and_stays_on_it() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found"])));
    rig.send(SessionIntent::LookupOpen).await;
    let searched = rig.searches().len();

    rig.send(rig.lookup_apply(0)).await;

    assert_eq!(rig.title_shown(), "Found");
    assert_eq!(
        rig.lookup().status,
        Some(LookupStatus::Applied {
            cover_failed: false
        })
    );
    assert_eq!(rig.searches().len(), searched, "no advance, no new search");
    assert_eq!(rig.selected(), [0]);
    assert!(
        rig.pending("alpha").is_none(),
        "applied values are form edits"
    );
}

#[tokio::test]
async fn applying_in_a_queue_stages_the_edit_moves_on_and_searches_the_next_title() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    rig.answer_next_search(Ok(found(&["Found A"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.answer_next_search(Ok(found(&["Found B"])));

    rig.send(rig.lookup_apply(0)).await;

    assert_eq!(
        rig.pending("alpha"),
        Some(MetadataIntentPatch {
            title: Some(PatchOp::Set("Found A".to_string())),
            album: Some(PatchOp::Set("Found A".to_string())),
            ..Default::default()
        })
    );
    assert_eq!(rig.selected(), [1]);
    assert_eq!(
        rig.searches().last().map(|(query, _)| query.as_str()),
        Some("BETA Author")
    );
    let lookup = rig.lookup();
    assert_eq!(lookup.queue_position.map(|at| at.index), Some(1));
    assert_eq!(
        lookup.status,
        Some(LookupStatus::Found {
            count: 1,
            partial: false,
            after: Some(QueueStep::Applied)
        })
    );
    assert_eq!(lookup.results[0].title, "Found B");
}

#[tokio::test]
async fn replacing_the_cover_stages_the_result_image_with_the_text() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetReplaceCover { replace: true })
        .await;

    rig.send(rig.lookup_apply(0)).await;

    assert_eq!(
        rig.pending("alpha").and_then(|patch| patch.cover_art),
        Some(PatchOp::Set(written_cover()))
    );
    assert_eq!(rig.cover_shown().await, Some(written_cover()));
    assert_eq!(rig.title_shown(), "Found");
}

#[tokio::test]
async fn a_result_offers_a_cover_and_applying_it_shows_that_cover() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found"])));
    rig.send(SessionIntent::LookupOpen).await;
    let offered = rig.lookup().results[0].cover.clone().expect("a cover");
    let shown = rig
        .session
        .cover(&host_request(&offered))
        .await
        .expect("a result's cover is served")
        .expect("it has bytes");
    assert_eq!(shown.to_vec(), written_cover());

    rig.send(SessionIntent::LookupSetReplaceCover { replace: true })
        .await;
    rig.send(rig.lookup_apply(0)).await;

    assert_eq!(rig.cover_shown().await, Some(written_cover()));
}

#[tokio::test]
async fn a_cover_that_fails_to_load_does_not_stop_the_text_from_applying() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    rig.answer_next_search(Ok(found(&["Found A"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetReplaceCover { replace: true })
        .await;
    *rig.cover.lock().expect("cover") = Err(AppError::General("unreachable".to_string()));

    rig.send(rig.lookup_apply(0)).await;

    let alpha = rig.pending("alpha").expect("text applied");
    assert_eq!(alpha.title, Some(PatchOp::Set("Found A".to_string())));
    assert_eq!(alpha.cover_art, None);
    assert!(matches!(
        rig.lookup().status,
        Some(LookupStatus::Found {
            after: Some(QueueStep::AppliedWithoutCover),
            ..
        })
    ));
}

#[tokio::test]
async fn skipping_leaves_the_title_untouched_and_the_last_skip_completes_the_queue() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    rig.send(SessionIntent::LookupOpen).await;

    rig.send(SessionIntent::LookupSkip).await;
    assert!(rig.pending("alpha").is_none());
    assert_eq!(rig.lookup().queue_position.map(|at| at.index), Some(1));
    assert!(matches!(
        rig.lookup().status,
        Some(LookupStatus::Found {
            after: Some(QueueStep::Skipped),
            ..
        })
    ));

    rig.send(SessionIntent::LookupSkip).await;
    assert_eq!(
        rig.lookup().status,
        Some(LookupStatus::QueueComplete {
            cover_failed: false
        })
    );
}

#[tokio::test]
async fn a_result_the_gate_rejects_keeps_the_queue_on_its_title() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    let invalid = OnlineMetadataResult {
        published_date: Some("soon".to_string()),
        ..result("Found A")
    };
    rig.answer_next_search(Ok(MetadataLookupResponse {
        results: vec![invalid],
        diagnostics: Vec::new(),
    }));
    rig.send(SessionIntent::LookupOpen).await;
    let searched = rig.searches().len();

    rig.send(rig.lookup_apply(0)).await;

    let lookup = rig.lookup();
    assert_eq!(
        lookup.status,
        Some(LookupStatus::NextTitleRejected {
            cover_failed: false
        })
    );
    assert_eq!(lookup.queue_position.map(|at| at.index), Some(0));
    assert_eq!(lookup.results.len(), 1, "results stay for another try");
    assert_eq!(rig.searches().len(), searched);
    assert_eq!(rig.selected(), [0]);
    assert!(
        rig.pending("alpha").is_none(),
        "an invalid edit is never staged"
    );
}

#[tokio::test]
async fn a_failed_cover_is_still_reported_when_the_gate_stops_the_queue() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    let invalid = OnlineMetadataResult {
        published_date: Some("soon".to_string()),
        ..result("Found A")
    };
    rig.answer_next_search(Ok(MetadataLookupResponse {
        results: vec![invalid],
        diagnostics: Vec::new(),
    }));
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetReplaceCover { replace: true })
        .await;
    *rig.cover.lock().expect("cover") = Err(AppError::General("unreachable".to_string()));

    rig.send(rig.lookup_apply(0)).await;

    assert_eq!(
        rig.lookup().status,
        Some(LookupStatus::NextTitleRejected { cover_failed: true })
    );
}

#[tokio::test]
async fn a_failed_cover_is_still_reported_when_the_next_title_has_nothing_to_search() {
    let rig = rig();
    rig.load(&["alpha", "_"]);
    rig.select(&[0, 1]).await;
    rig.session.transition(|state| {
        state
            .tags
            .retain_paths(&std::collections::HashSet::from([path("alpha")]))
    });
    rig.answer_next_search(Ok(found(&["Found A"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetReplaceCover { replace: true })
        .await;
    *rig.cover.lock().expect("cover") = Err(AppError::General("unreachable".to_string()));

    rig.send(rig.lookup_apply(0)).await;

    assert_eq!(
        rig.lookup().status,
        Some(LookupStatus::QueryRequired {
            after: Some(QueueStep::AppliedWithoutCover)
        })
    );
}

#[tokio::test]
async fn a_result_is_not_applied_when_its_title_cannot_be_selected() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.select(&[1]).await;
    rig.type_into(MetadataField::Date, "soon").await;

    rig.send(rig.lookup_apply(0)).await;

    assert_eq!(rig.lookup().status, Some(LookupStatus::ApplyRejected));
    assert_eq!(rig.selected(), [1]);
    assert_eq!(rig.title_shown(), "BETA");
}

#[tokio::test]
async fn editing_the_query_during_an_apply_says_the_apply_stopped() {
    let (cover, pending) = tokio::sync::oneshot::channel();
    let rig = rig_with_cover(Some(pending));
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetReplaceCover { replace: true })
        .await;
    let applying = rig.session.begin(rig.lookup_apply(0));
    assert_eq!(rig.lookup().status, Some(LookupStatus::Applying));

    rig.send(SessionIntent::LookupSetTitleQuery {
        value: "Changed".to_string(),
    })
    .await;
    cover.send(Ok(vec![1, 2, 3])).expect("cover reply");

    assert_eq!(applying.finish().await.outcome, SessionOutcome::Superseded);
    assert_eq!(rig.lookup().status, Some(LookupStatus::ApplySuperseded));
    assert_eq!(rig.title_shown(), "ALPHA");
}

#[tokio::test]
async fn editing_the_query_while_the_queue_moves_to_the_next_title_says_the_apply_stopped() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    rig.answer_next_search(Ok(found(&["Found A"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.session.transition(|state| {
        state
            .tags
            .retain_paths(&std::collections::HashSet::from([path("alpha")]))
    });
    let next_search = rig.hold_next_search();
    let applying = rig.session.begin(rig.lookup_apply(0));
    while rig.selected() != [1] {
        tokio::task::yield_now().await;
    }

    rig.send(SessionIntent::LookupSetTitleQuery {
        value: "Changed".to_string(),
    })
    .await;
    next_search
        .send(Ok(found(&[])))
        .unwrap_or_else(|_| panic!("reply is waiting"));

    assert_eq!(applying.finish().await.outcome, SessionOutcome::Superseded);
    assert!(matches!(
        rig.lookup().status,
        Some(LookupStatus::ApplySuperseded | LookupStatus::SearchSuperseded)
    ));
    assert_eq!(rig.lookup().title_query, "Changed");
}

#[tokio::test]
async fn lookup_cover_reply_cannot_pull_the_selection_back_to_an_earlier_title() {
    let (cover, pending) = tokio::sync::oneshot::channel();
    let rig = rig_with_cover(Some(pending));
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found Alpha"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetReplaceCover { replace: true })
        .await;
    let applying = rig.session.begin(rig.lookup_apply(0));
    rig.select(&[1]).await;
    cover.send(Ok(vec![1, 2, 3])).expect("cover reply");
    assert_eq!(applying.finish().await.outcome, SessionOutcome::Superseded);
    assert_eq!(rig.lookup().status, Some(LookupStatus::ApplySuperseded));
    assert_eq!(rig.selected(), vec![1]);
    assert_eq!(rig.title_shown(), "BETA");
    assert!(rig.pending("alpha").is_none());
}
