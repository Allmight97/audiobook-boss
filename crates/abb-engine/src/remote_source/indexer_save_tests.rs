use super::*;
use crate::app_settings::SettingsIntent;
use crate::remote_source::tests::{test_runtime, test_runtime_with, MemoryVault};
use secrecy::ExposeSecret;
use tempfile::TempDir;

fn edit(url: &str, key: Option<&str>) -> RemoteUiIntent {
    RemoteUiIntent::EditConnection {
        base_url: Some(url.into()),
        category_ids: Some(vec![3040, 3030]),
        api_key: key.map(str::to_string),
    }
}

async fn run(runtime: &RemoteSourceRuntime, intent: RemoteUiIntent) -> Result<RemoteUiResult> {
    runtime.ui_begin(intent).finish().await
}

async fn save(
    runtime: &RemoteSourceRuntime,
    url: &str,
    key: Option<&str>,
) -> Result<RemoteUiResult> {
    run(runtime, edit(url, key)).await.expect("edit accepted");
    run(runtime, RemoteUiIntent::SaveConnection).await
}

fn key_for(runtime: &RemoteSourceRuntime, url: &str) -> Option<String> {
    runtime
        .inner
        .vault
        .get_secret(&format!("indexer.api_key:{url}"))
        .expect("read key")
        .map(|key| key.expose_secret().to_string())
}

fn seeded_with_a_saved_old_url(root: &TempDir, vault: MemoryVault) -> RemoteSourceRuntime {
    std::fs::write(
        root.path().join("settings.toml"),
        "[indexer]\nurl = \"http://old.test\"\ncategories = [3000, 3030]\n",
    )
    .expect("seed settings");
    vault.secrets.lock().expect("vault").insert(
        "indexer.api_key:http://old.test".into(),
        "old-key".to_string().into(),
    );
    test_runtime_with(root, Some(Box::new(vault)), None)
}

#[tokio::test]
async fn a_save_changes_the_settings_then_the_key_for_the_saved_url() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);

    let result = save(&runtime, "https://indexer.test/", Some("first-key")).await;

    assert!(matches!(result, Ok(RemoteUiResult::Saved)));
    let saved = runtime.inner.settings.indexer_connection();
    assert_eq!(saved.base_url.as_deref(), Some("https://indexer.test"));
    assert_eq!(saved.category_ids, [3030, 3040]);
    assert_eq!(
        key_for(&runtime, "https://indexer.test").as_deref(),
        Some("first-key")
    );
    let connection = runtime.ui_snapshot().connection;
    assert_eq!(connection.save, RemoteDraftStatus::Succeeded);
    assert!(connection.api_key_configured);
    assert!(!connection.api_key_entered);
    assert_eq!(connection.base_url, "https://indexer.test");
    let on_disk = std::fs::read_to_string(root.path().join("settings.toml")).expect("settings");
    assert!(
        on_disk.contains("url = \"https://indexer.test\""),
        "{on_disk}"
    );
    assert!(!on_disk.contains("first-key"));
}

#[tokio::test]
async fn search_and_grab_resolve_the_saved_url_categories_and_key() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    assert!(runtime.configured_indexer_connection().await.is_err());

    save(&runtime, "https://indexer.test", Some("first-key"))
        .await
        .expect("saved");
    run(
        &runtime,
        edit("https://typed-but-unsaved.test", Some("typed-key")),
    )
    .await
    .expect("edit");

    let connection = runtime
        .configured_indexer_connection()
        .await
        .expect("saved connection");
    assert_eq!(connection.base_url, "https://indexer.test");
    assert_eq!(connection.category_ids, [3030, 3040]);
    assert_eq!(connection.api_key.expose_secret(), "first-key");
}

#[tokio::test]
async fn a_vault_failure_after_an_applied_change_says_so_and_keeps_the_old_key() {
    let root = TempDir::new().expect("temp root");
    let vault = MemoryVault {
        fail_writes: true,
        ..Default::default()
    };
    let runtime = seeded_with_a_saved_old_url(&root, vault);

    let error = save(&runtime, "http://new.test", Some("new-key"))
        .await
        .err()
        .expect("the key write fails");

    assert!(error
        .to_string()
        .contains("The Indexer URL and categories changed, but the API key could not be updated"));
    let saved = runtime.inner.settings.indexer_connection();
    assert_eq!(saved.base_url.as_deref(), Some("http://new.test"));
    assert_eq!(
        key_for(&runtime, "http://old.test").as_deref(),
        Some("old-key")
    );
    assert!(matches!(
        runtime.ui_snapshot().connection.save,
        RemoteDraftStatus::Failed { .. }
    ));
}

#[tokio::test]
async fn a_vault_failure_with_nothing_changed_reports_the_plain_error() {
    let root = TempDir::new().expect("temp root");
    let vault = MemoryVault {
        fail_writes: true,
        ..Default::default()
    };
    let runtime = seeded_with_a_saved_old_url(&root, vault);
    run(
        &runtime,
        RemoteUiIntent::EditConnection {
            base_url: Some("http://old.test".into()),
            category_ids: Some(vec![3000, 3030]),
            api_key: Some("replacement".into()),
        },
    )
    .await
    .expect("edit");

    let error = run(&runtime, RemoteUiIntent::SaveConnection)
        .await
        .err()
        .expect("the key write fails");

    assert!(error.to_string().contains("vault unavailable"));
    assert!(!error.to_string().contains("changed"));
}

#[tokio::test]
async fn a_rejected_settings_turn_writes_nothing_to_the_vault() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let refused = IndexerConnection {
        base_url: Some("https://user:secret@indexer.test".into()),
        category_ids: vec![3030],
    };
    let turn = SaveTurn {
        previous: runtime.inner.settings.indexer_connection(),
        settings: runtime
            .inner
            .settings
            .set_indexer_connection(refused.clone(), &runtime.inner.tasks),
        write: runtime
            .inner
            .indexer_turn
            .clone()
            .try_write_owned()
            .expect("no search or grab holds the guard"),
    };
    let save = IndexerSave {
        revision: 1,
        request: runtime.ui().save_request,
        connection: refused,
        api_key: Some("key".into()),
        turn,
    };

    let result = runtime.finish_indexer_save(save).await;

    assert!(result.is_err());
    assert_eq!(
        runtime.inner.settings.indexer_connection(),
        IndexerConnection::default()
    );
    assert_eq!(key_for(&runtime, "https://user:secret@indexer.test"), None);
}

#[tokio::test]
async fn a_key_without_a_url_is_refused_before_the_settings_turn_is_taken() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let revision = runtime.inner.settings.snapshot().await.revision;

    let error = save(&runtime, "", Some("key"))
        .await
        .err()
        .expect("refused");

    assert!(error
        .to_string()
        .contains("Configure Indexer URL before saving an API key."));
    assert_eq!(runtime.inner.settings.snapshot().await.revision, revision);
    assert_ne!(
        runtime.ui_snapshot().connection.save,
        RemoteDraftStatus::Running
    );
    assert!(run(&runtime, RemoteUiIntent::SaveConnection).await.is_err());
    run(&runtime, edit("", Some("")))
        .await
        .expect("the key is cleared");
    run(&runtime, RemoteUiIntent::SaveConnection)
        .await
        .expect("categories alone save without a vault");
}

#[tokio::test]
async fn loading_reads_only_whether_a_key_is_stored_for_the_saved_url() {
    let root = TempDir::new().expect("temp root");
    let runtime = seeded_with_a_saved_old_url(&root, MemoryVault::default());
    assert_eq!(runtime.ui_snapshot().connection.base_url, "http://old.test");
    run(&runtime, edit("http://typed.test", None))
        .await
        .expect("edit");

    run(&runtime, RemoteUiIntent::LoadConnection)
        .await
        .expect("load");

    let connection = runtime.ui_snapshot().connection;
    assert!(connection.api_key_configured);
    assert_eq!(connection.base_url, "http://typed.test");
    assert_eq!(connection.category_ids, [3040, 3030]);
}

#[tokio::test]
async fn an_applied_reset_replaces_the_draft_and_clears_the_key_fact() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    save(&runtime, "https://indexer.test", Some("first-key"))
        .await
        .expect("saved");
    run(&runtime, edit("https://indexer.test", Some("typed-key")))
        .await
        .expect("edit");
    let requests = {
        let state = runtime.ui();
        (state.save_request, state.test_request)
    };
    let reply = runtime
        .inner
        .settings
        .begin(SettingsIntent::Reset, &runtime.inner.tasks)
        .finish()
        .await;
    assert!(matches!(
        reply.outcome,
        crate::app_settings::SettingsOutcome::Applied
    ));

    runtime.saved_connection_replaced().await;

    let connection = runtime.ui_snapshot().connection;
    assert_eq!(connection.base_url, "");
    assert_eq!(connection.category_ids, [3000, 3030]);
    assert!(!connection.api_key_configured);
    assert!(!connection.api_key_entered);
    let state = runtime.ui();
    assert_eq!(
        (state.save_request, state.test_request),
        (requests.0 + 1, requests.1 + 1)
    );
    assert_eq!(state.api_key, None);
}

async fn reset_settings(runtime: &RemoteSourceRuntime) {
    let reply = runtime
        .inner
        .settings
        .begin(SettingsIntent::Reset, &runtime.inner.tasks)
        .finish()
        .await;
    assert!(matches!(
        reply.outcome,
        crate::app_settings::SettingsOutcome::Applied
    ));
}

#[tokio::test]
async fn a_save_or_test_that_finishes_after_a_reset_cannot_write_the_old_connection_back() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    run(&runtime, edit("http://127.0.0.1:1", Some("first-key")))
        .await
        .expect("edit");
    let saving = runtime.ui_begin(RemoteUiIntent::SaveConnection);
    let testing = runtime.ui_begin(RemoteUiIntent::TestConnection);
    reset_settings(&runtime).await;
    runtime.saved_connection_replaced().await;

    saving.finish().await.expect("the Save still finishes");
    assert!(testing.finish().await.is_err(), "nothing listens there");

    let connection = runtime.ui_snapshot().connection;
    assert_eq!(connection.base_url, "");
    assert_eq!(connection.category_ids, [3000, 3030]);
    assert!(!connection.api_key_configured);
    assert_eq!(connection.save, RemoteDraftStatus::Idle);
    assert_eq!(connection.test, RemoteDraftStatus::Idle);
    assert_eq!(connection.test_result, None);
}

#[tokio::test]
async fn an_applied_reset_refreshes_the_indexer_account() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    run(
        &runtime,
        RemoteUiIntent::SelectLane {
            lane: ProviderId::Indexer,
        },
    )
    .await
    .expect("lane");
    save(&runtime, "https://indexer.test", Some("first-key"))
        .await
        .expect("saved");
    let connected = runtime.ui_snapshot().account.expect("account").status;
    assert_eq!(
        connected,
        crate::remote_source::RemoteAccountStatus::Connected
    );

    reset_settings(&runtime).await;
    runtime.saved_connection_replaced().await;

    let account = runtime.ui_snapshot().account.expect("account");
    assert_eq!(
        account.status,
        crate::remote_source::RemoteAccountStatus::NeedsAuth
    );
}

#[tokio::test]
async fn a_vault_failure_on_a_changed_url_shows_the_key_fact_of_the_new_url() {
    let root = TempDir::new().expect("temp root");
    let vault = MemoryVault {
        fail_writes: true,
        ..Default::default()
    };
    let runtime = seeded_with_a_saved_old_url(&root, vault);
    run(&runtime, RemoteUiIntent::LoadConnection)
        .await
        .expect("load");
    assert!(runtime.ui_snapshot().connection.api_key_configured);

    save(&runtime, "http://new.test", Some("new-key"))
        .await
        .err()
        .expect("the key write fails");

    assert!(!runtime.ui_snapshot().connection.api_key_configured);
}

#[tokio::test]
async fn a_save_whose_settings_turn_runs_after_a_reset_shows_the_connection_in_effect() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    run(&runtime, edit("https://indexer.test", Some("first-key")))
        .await
        .expect("edit");
    let saving = runtime.ui_begin(RemoteUiIntent::SaveConnection);
    runtime.saved_connection_replaced().await;
    assert_eq!(runtime.ui_snapshot().connection.base_url, "");

    saving.finish().await.expect("the Save still finishes");

    let saved = runtime.inner.settings.indexer_connection();
    assert_eq!(saved.base_url.as_deref(), Some("https://indexer.test"));
    let connection = runtime.ui_snapshot().connection;
    assert_eq!(connection.base_url, "https://indexer.test");
    assert_eq!(connection.category_ids, [3030, 3040]);
    assert!(connection.api_key_configured);
    assert_eq!(connection.save, RemoteDraftStatus::Idle);
}

#[tokio::test]
async fn a_reset_clears_a_refused_draft_old_failures_and_search_results() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    run(
        &runtime,
        RemoteUiIntent::EditConnection {
            base_url: Some("not a url ::".into()),
            category_ids: Some(vec![0]),
            api_key: None,
        },
    )
    .await
    .err()
    .expect("refused edit");
    assert!(runtime.ui_snapshot().connection.draft_error.is_some());
    runtime.ui().snapshot.indexer.message = "3 releases found.".into();
    reset_settings(&runtime).await;

    runtime.saved_connection_replaced().await;

    let snapshot = runtime.ui_snapshot();
    assert!(snapshot.connection.draft_error.is_none());
    assert!(!snapshot.connection.empty_categories_refused);
    assert_eq!(snapshot.indexer.message, "");
    let error = run(&runtime, RemoteUiIntent::TestConnection)
        .await
        .err()
        .expect("there is no URL to test yet");
    assert!(!error.to_string().contains("Correct the Indexer connection"));
    assert!(run(&runtime, RemoteUiIntent::SaveConnection).await.is_ok());
}

#[tokio::test]
async fn a_reset_clears_an_old_save_failure() {
    let root = TempDir::new().expect("temp root");
    let vault = MemoryVault {
        fail_writes: true,
        ..Default::default()
    };
    let runtime = seeded_with_a_saved_old_url(&root, vault);
    save(&runtime, "http://new.test", Some("new-key"))
        .await
        .err()
        .expect("the key write fails");
    assert!(matches!(
        runtime.ui_snapshot().connection.save,
        RemoteDraftStatus::Failed { .. }
    ));
    reset_settings(&runtime).await;

    runtime.saved_connection_replaced().await;

    assert_eq!(
        runtime.ui_snapshot().connection.save,
        RemoteDraftStatus::Idle
    );
}
