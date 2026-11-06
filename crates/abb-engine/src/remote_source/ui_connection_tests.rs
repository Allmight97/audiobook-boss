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
