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
