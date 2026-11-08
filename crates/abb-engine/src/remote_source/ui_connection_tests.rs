use super::*;

#[test]
fn save_and_test_refuse_a_refused_draft() {
    let mut ui = UiState::default();
    let refused = ui.begin(RemoteUiIntent::EditConnection {
        base_url: Some("not a url ::".into()),
        category_ids: None,
        api_key: None,
    });
    assert!(refused.is_err());
    assert!(ui.snapshot().connection.draft_error.is_some());
    assert!(ui.save_connection().is_err());
    assert!(ui.begin(RemoteUiIntent::TestConnection).is_err());

    ui.begin(RemoteUiIntent::EditConnection {
        base_url: Some("https://indexer.test".into()),
        category_ids: None,
        api_key: None,
    })
    .expect("accepted edit");
    assert!(ui.snapshot().connection.draft_error.is_none());
}

#[test]
fn an_api_key_a_header_cannot_carry_is_refused_with_a_reason_and_blocks_save_and_test() {
    let mut ui = UiState::default();
    let refused = ui.begin(RemoteUiIntent::EditConnection {
        base_url: Some("https://indexer.test".into()),
        category_ids: None,
        api_key: Some("key\nwith-a-newline".into()),
    });
    assert!(refused.is_err());
    let error = ui
        .snapshot()
        .connection
        .draft_error
        .expect("the refused key explains itself");
    assert!(error.message.contains("API key"), "{}", error.message);
    assert!(!ui.snapshot().connection.api_key_entered);
    assert!(ui.save_connection().is_err());
    assert!(ui.begin(RemoteUiIntent::TestConnection).is_err());

    ui.begin(RemoteUiIntent::EditConnection {
        base_url: None,
        category_ids: None,
        api_key: Some("a-plain-key".into()),
    })
    .expect("accepted key");
    assert!(ui.snapshot().connection.draft_error.is_none());
    assert!(ui.snapshot().connection.api_key_entered);
}

#[test]
fn a_category_edit_that_chooses_none_is_refused_and_the_draft_keeps_its_categories() {
    let mut ui = UiState::default();
    assert_eq!(ui.snapshot().connection.category_ids, [3000, 3030]);
    let edit = |ids: Vec<u32>| RemoteUiIntent::EditConnection {
        base_url: None,
        category_ids: Some(ids),
        api_key: None,
    };

    ui.begin(edit(vec![3030])).expect("one category");
    ui.begin(edit(Vec::new()))
        .expect("refusal is reported, not an error");
    let connection = ui.snapshot().connection;
    assert_eq!(connection.category_ids, [3030]);
    assert!(connection.empty_categories_refused);

    ui.begin(edit(vec![3000, 3030])).expect("two categories");
    let connection = ui.snapshot().connection;
    assert_eq!(connection.category_ids, [3000, 3030]);
    assert!(!connection.empty_categories_refused);
}

fn edit(
    base_url: Option<&str>,
    category_ids: Option<Vec<u32>>,
    api_key: Option<&str>,
) -> RemoteUiIntent {
    RemoteUiIntent::EditConnection {
        base_url: base_url.map(Into::into),
        category_ids,
        api_key: api_key.map(Into::into),
    }
}

#[test]
fn a_refused_api_key_stays_refused_through_a_url_edit_until_a_new_key_arrives() {
    let mut ui = UiState::default();
    ui.begin(edit(None, None, Some("old-key")))
        .expect("old key");
    assert!(ui.begin(edit(None, None, Some("key\nrefused"))).is_err());

    ui.begin(edit(Some("https://indexer.test"), None, None))
        .expect("the URL edit is accepted");

    let error = ui.snapshot().connection.draft_error.expect("still refused");
    assert!(error.message.contains("API key"), "{}", error.message);
    assert!(ui.save_connection().is_err());
    assert!(ui.begin(RemoteUiIntent::TestConnection).is_err());

    ui.begin(edit(None, None, Some("new-key")))
        .expect("new key");
    assert!(ui.snapshot().connection.draft_error.is_none());
    assert!(ui.save_connection().is_ok());
}

#[test]
fn a_refused_url_stays_refused_through_a_category_edit_until_a_new_url_arrives() {
    let mut ui = UiState::default();
    assert!(ui
        .begin(edit(Some("ftp://indexer.test"), None, None))
        .is_err());

    ui.begin(edit(None, Some(vec![3030]), None))
        .expect("the category edit is accepted");

    assert!(ui.snapshot().connection.draft_error.is_some());
    assert!(ui.save_connection().is_err());

    ui.begin(edit(Some("https://indexer.test"), None, None))
        .expect("new URL");
    assert!(ui.snapshot().connection.draft_error.is_none());
}

#[test]
fn each_field_keeps_its_own_refusal_and_the_key_is_shown_first() {
    let mut ui = UiState::default();
    assert!(ui
        .begin(edit(Some("ftp://indexer.test"), None, None))
        .is_err());
    assert!(ui.begin(edit(None, None, Some("key\nrefused"))).is_err());
    let shown = ui.snapshot().connection.draft_error.expect("refused");
    assert!(shown.message.contains("API key"), "{}", shown.message);

    ui.begin(edit(None, None, Some("a-plain-key")))
        .expect("key");
    let shown = ui
        .snapshot()
        .connection
        .draft_error
        .expect("URL still refused");
    assert!(shown.message.contains("Indexer URL"), "{}", shown.message);
    assert!(ui.save_connection().is_err());
}

#[test]
fn the_draft_holds_the_indexer_url_as_typed_and_save_shows_the_normalized_one() {
    let mut ui = UiState::default();
    ui.begin(edit(Some("HTTPS://Host:443/"), None, None))
        .expect("a valid URL is accepted as typed");
    assert_eq!(ui.snapshot().connection.base_url, "HTTPS://Host:443/");
    assert!(ui.snapshot().connection.draft_error.is_none());

    let accepted = ui.save_connection().expect("a valid draft saves");
    assert_eq!(
        accepted.connection.base_url.as_deref(),
        Some("https://host")
    );
    ui.saved(accepted.revision, &accepted.connection, &Ok(false));

    assert_eq!(ui.snapshot().connection.base_url, "https://host");
}

#[test]
fn a_refused_indexer_url_stays_in_the_draft_beside_its_reason() {
    let mut ui = UiState::default();

    assert!(ui
        .begin(edit(Some("ftp://indexer.test"), None, None))
        .is_err());

    let shown = ui.snapshot().connection;
    assert_eq!(shown.base_url, "ftp://indexer.test");
    assert!(shown.draft_error.is_some());
}

#[test]
fn a_test_sends_the_trimmed_typed_url_for_the_provider_to_normalize() {
    let mut ui = UiState::default();
    ui.begin(edit(Some("  HTTPS://Host:443/  "), None, Some("key")))
        .expect("edit");

    let Ok(UiAction::Test { update, .. }) = ui.begin(RemoteUiIntent::TestConnection) else {
        panic!("a valid draft tests");
    };

    assert_eq!(update.base_url.as_deref(), Some("HTTPS://Host:443/"));
}

fn option_ids(ui: &UiState) -> Vec<u32> {
    ui.snapshot()
        .connection
        .category_options
        .iter()
        .map(|option| option.id)
        .collect()
}

#[test]
fn the_snapshot_lists_the_category_options_with_labels_and_the_recommended_one() {
    let ui = UiState::default();

    let options = ui.snapshot().connection.category_options;

    let shown: Vec<_> = options
        .iter()
        .map(|option| (option.id, option.label.as_str(), option.recommended))
        .collect();
    assert_eq!(
        shown,
        vec![
            (3030, "Audiobooks (3030)", true),
            (3000, "Audio (3000)", false)
        ]
    );
}

#[test]
fn a_saved_category_outside_the_known_list_stays_an_option_after_it_is_unchecked() {
    let mut ui = UiState::seeded(&IndexerConnection {
        base_url: None,
        category_ids: vec![3030, 3040],
    });
    assert_eq!(option_ids(&ui), vec![3030, 3000, 3040]);
    assert_eq!(
        ui.snapshot()
            .connection
            .category_options
            .last()
            .map(|option| option.label.as_str()),
        Some("Category 3040")
    );

    ui.begin(edit(None, Some(vec![3030]), None))
        .expect("unchecking 3040 is accepted");

    assert_eq!(ui.snapshot().connection.category_ids, vec![3030]);
    assert_eq!(option_ids(&ui), vec![3030, 3000, 3040]);
}

#[test]
fn the_snapshot_says_whether_the_draft_url_is_plain_http() {
    let mut ui = UiState::default();
    for (url, plain_http) in [
        ("http://indexer.test", true),
        ("  HTTP://indexer.test", true),
        ("https://indexer.test", false),
        ("indexer.test", false),
        ("", false),
    ] {
        let accepted = ui.begin(edit(Some(url), None, None)).is_ok();
        assert_eq!(accepted, url != "indexer.test", "{url:?}");
        assert_eq!(ui.snapshot().connection.uses_http, plain_http, "{url:?}");
    }
}

fn load_intent(ui: &mut UiState) -> UiAction {
    ui.begin(RemoteUiIntent::LoadConnection)
        .expect("load admitted")
}

#[test]
fn the_connection_load_moves_from_not_loaded_to_loading_to_loaded_and_retries_after_a_failure() {
    let mut ui = UiState::default();
    let UiAction::Load { request } = load_intent(&mut ui) else {
        panic!("the first load starts a read")
    };
    assert!(matches!(load_intent(&mut ui), UiAction::Done));

    ui.loaded(request, &Err(AppError::General("keychain locked".into())));
    assert!(matches!(
        ui.snapshot.connection.save,
        RemoteDraftStatus::Failed { .. }
    ));
    let UiAction::Load { request } = load_intent(&mut ui) else {
        panic!("a failed load can be retried")
    };

    ui.loaded(request, &Ok(true));
    assert!(ui.snapshot.connection.api_key_configured);
    assert!(matches!(load_intent(&mut ui), UiAction::Done));
}

#[test]
fn a_save_outdates_a_load_in_flight_and_refuses_indexer_work_until_it_ends() {
    let mut ui = UiState::default();
    ui.snapshot.lane = ProviderId::Indexer;
    let UiAction::Load { request } = load_intent(&mut ui) else {
        panic!("load starts")
    };
    ui.begin(RemoteUiIntent::EditConnection {
        base_url: Some("https://indexer.test".into()),
        category_ids: Some(vec![3030]),
        api_key: Some("key".into()),
    })
    .expect("edit");
    let save = ui.save_connection().expect("save accepted");
    assert_eq!(ui.snapshot.connection.save, RemoteDraftStatus::Running);
    let search = || RemoteUiIntent::SearchReleases {
        author: "Author".into(),
        title: String::new(),
    };
    assert!(ui.begin(search()).is_err());
    ui.begin(RemoteUiIntent::EditConnection {
        base_url: None,
        category_ids: Some(vec![3000]),
        api_key: None,
    })
    .expect("edit while saving");
    assert_eq!(ui.snapshot.connection.save, RemoteDraftStatus::Running);

    ui.loaded(request, &Ok(false));
    assert!(!ui.snapshot.connection.api_key_configured);
    assert!(matches!(load_intent(&mut ui), UiAction::Load { .. }));

    ui.saved(
        save.revision,
        &IndexerConnection {
            base_url: Some("https://indexer.test".into()),
            category_ids: vec![3030],
        },
        &Ok(true),
    );
    assert!(ui.begin(search()).is_ok());
    assert!(matches!(load_intent(&mut ui), UiAction::Done));
}
