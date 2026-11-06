use super::super::{release_key, ReleaseGrabSnapshot, ReleaseGrabStatus, RemoteUiIntent};
use super::*;
use crate::app_settings::SettingsIntent;
use crate::remote_source::tests::{test_runtime, test_runtime_with, MemoryVault};
use crate::remote_source::types::RemoteReleaseSearchResponse;
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
    let save = IndexerSave {
        accepted: AcceptedSave {
            revision: 1,
            connection: refused.clone(),
            api_key: Some("key".into()),
        },
        previous: runtime.inner.settings.indexer_connection(),
        settings: runtime
            .inner
            .settings
            .set_indexer_connection(refused, &runtime.inner.tasks),
        write: runtime
            .inner
            .indexer_turn
            .clone()
            .try_write_owned()
            .expect("no search or grab holds the guard"),
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
async fn a_test_that_finishes_after_a_reset_cannot_write_the_old_connection_back() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    run(&runtime, edit("http://127.0.0.1:1", Some("first-key")))
        .await
        .expect("edit");
    let testing = runtime.ui_begin(RemoteUiIntent::TestConnection);
    reset_settings(&runtime).await;
    runtime.saved_connection_replaced().await;

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

fn release(indexer_id: i64) -> crate::remote_source::types::RemoteRelease {
    crate::remote_source::types::RemoteRelease {
        provider_id: ProviderId::Indexer,
        guid: "shared-guid".into(),
        indexer_id,
        title: format!("Book {indexer_id}"),
        indexer: "Indexer".into(),
        detail_url: None,
        size_bytes: 100,
        protocol: crate::remote_source::types::RemoteReleaseProtocol::Torrent,
        seeders: Some(10),
        categories: vec![],
    }
}

/// Answers three grab requests in order. The second waits for `gate` after
/// reporting that it arrived, so a test can act while a batch is mid-flight.
#[expect(
    clippy::disallowed_methods,
    reason = "joined by a_reset_during_a_paused_grab_is_refused_and_every_outcome_stays_retryable"
)]
fn serve_grabs_pausing_on_the_second(
    listener: tokio::net::TcpListener,
    arrived: tokio::sync::oneshot::Sender<()>,
    gate: tokio::sync::oneshot::Receiver<()>,
) -> tokio::task::JoinHandle<Vec<i64>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    tokio::spawn(async move {
        let (mut arrived, mut gate) = (Some(arrived), Some(gate));
        let mut ids = Vec::new();
        for (index, status) in ["200 OK", "500 Internal Server Error", "200 OK"]
            .into_iter()
            .enumerate()
        {
            let (mut stream, _) = listener.accept().await.expect("accept a grab");
            let mut request = Vec::new();
            let body = loop {
                let mut bytes = [0; 1024];
                let count = stream.read(&mut bytes).await.expect("read a grab");
                assert_ne!(count, 0);
                request.extend_from_slice(&bytes[..count]);
                let text = String::from_utf8_lossy(&request).into_owned();
                if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.split_once(':')
                                .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                                .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if body.len() >= length {
                        break body.to_string();
                    }
                }
            };
            let json: serde_json::Value = serde_json::from_str(&body).expect("grab body");
            ids.push(json["indexerId"].as_i64().expect("indexer id"));
            if index == 1 {
                arrived.take().expect("one pause").send(()).ok();
                gate.take().expect("one pause").await.ok();
            }
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: 2\r\n\r\n{{}}"
                    )
                    .as_bytes(),
                )
                .await
                .expect("answer a grab");
        }
        ids
    })
}

#[tokio::test]
async fn a_reset_during_a_paused_grab_is_refused_and_every_outcome_stays_retryable() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let url = format!("http://{}", listener.local_addr().expect("address"));
    save(&runtime, &url, Some("grab-key"))
        .await
        .expect("connection saved");
    run(
        &runtime,
        RemoteUiIntent::SelectLane {
            lane: ProviderId::Indexer,
        },
    )
    .await
    .expect("lane");
    {
        let mut state = runtime.ui();
        let request = state.search_request;
        state.searched(
            request,
            &Ok(RemoteReleaseSearchResponse {
                provider_id: ProviderId::Indexer,
                releases: vec![release(1), release(2)],
                diagnostics: vec![],
            }),
        );
    }
    for indexer_id in [1, 2] {
        run(
            &runtime,
            RemoteUiIntent::SelectRelease {
                indexer_id,
                guid: "shared-guid".into(),
                multi: true,
            },
        )
        .await
        .expect("select");
    }
    let (arrived, second_is_sending) = tokio::sync::oneshot::channel();
    let (release_second, gate) = tokio::sync::oneshot::channel();
    let serving = serve_grabs_pausing_on_the_second(listener, arrived, gate);
    let revision = runtime.inner.settings.snapshot().await.revision;

    {
        let _searching = runtime.inner.indexer_turn.read().await;
        assert!(
            runtime.reset_lease().is_err(),
            "a search or grab holding the lease refuses a Reset"
        );
    }
    let batch = runtime.ui_begin(RemoteUiIntent::GrabSelected);
    assert!(
        runtime.reset_lease().is_err(),
        "an accepted grab refuses a Reset before its lease is taken"
    );
    let (batch, ()) = tokio::join!(batch.finish(), async {
        second_is_sending.await.expect("the second release is sent");
        let Err(refused) = runtime.reset_lease() else {
            panic!("a Reset waits");
        };
        assert!(
            matches!(&refused, AppError::InvalidInput(message) if message.contains("before resetting settings")),
            "{refused}"
        );
        assert_eq!(
            runtime
                .inner
                .settings
                .indexer_connection()
                .base_url
                .as_deref(),
            Some(url.as_str())
        );
        assert_eq!(runtime.inner.settings.snapshot().await.revision, revision);
        release_second.send(()).expect("resume the batch");
    });
    batch.expect("the batch finishes");

    let status_of = |indexer_id| {
        runtime.ui_snapshot().indexer.release_grabs[&release_key(indexer_id, "shared-guid")]
            .status
            .clone()
    };
    assert_eq!(status_of(1), ReleaseGrabStatus::Sent);
    assert_eq!(status_of(2), ReleaseGrabStatus::Error);
    assert!(runtime
        .ui_snapshot()
        .indexer
        .message
        .contains("1 releases sent. 1 failed; retry their rows."));
    run(&runtime, RemoteUiIntent::GrabSelected)
        .await
        .expect("the failed row is retried");
    assert_eq!(status_of(2), ReleaseGrabStatus::Sent);
    assert_eq!(serving.await.expect("server"), [1, 2, 2]);
    assert!(
        runtime.reset_lease().is_ok(),
        "a Reset is accepted once the Indexer is idle"
    );
}

#[tokio::test]
async fn connection_save_persists_the_accepted_draft_and_invalidates_old_release_selection() {
    let root = TempDir::new().expect("temporary config");
    let runtime = test_runtime(&root);
    run(&runtime, edit("https://indexer.test", None))
        .await
        .expect("edit");
    runtime
        .ui_begin(RemoteUiIntent::SelectLane {
            lane: ProviderId::Indexer,
        })
        .finish()
        .await
        .expect("indexer lane");
    {
        let mut ui = runtime.ui();
        let request = ui.search_request;
        ui.searched(
            request,
            &Ok(RemoteReleaseSearchResponse {
                provider_id: ProviderId::Indexer,
                releases: vec![release(1)],
                diagnostics: vec![],
            }),
        );
    }
    runtime
        .ui_begin(RemoteUiIntent::SelectRelease {
            indexer_id: 1,
            guid: "shared-guid".into(),
            multi: false,
        })
        .finish()
        .await
        .expect("select release");
    runtime.ui().snapshot.indexer.release_grabs.insert(
        release_key(1, "shared-guid"),
        ReleaseGrabSnapshot {
            status: ReleaseGrabStatus::Error,
            message: "Failed".into(),
        },
    );
    let save = runtime.ui_begin(RemoteUiIntent::SaveConnection);
    runtime
        .ui_begin(RemoteUiIntent::EditConnection {
            base_url: Some("https://next.test".into()),
            api_key: None,
            category_ids: None,
        })
        .finish()
        .await
        .expect("new draft");
    save.finish().await.expect("finish accepted Save");
    assert_eq!(
        runtime
            .inner
            .settings
            .indexer_connection()
            .base_url
            .as_deref(),
        Some("https://indexer.test")
    );
    let snapshot = runtime.ui_snapshot();
    assert_eq!(snapshot.connection.base_url, "https://next.test");
    assert!(snapshot.indexer.releases.is_empty());
    assert!(snapshot.indexer.selected_release_keys.is_empty());
    assert!(snapshot.indexer.release_grabs.is_empty());
    assert!(runtime
        .ui_begin(RemoteUiIntent::GrabSelected)
        .finish()
        .await
        .is_err());
}

#[tokio::test]
async fn a_key_read_that_finishes_after_a_save_cannot_overwrite_the_saved_key_fact() {
    for late in [Ok(false), Err(AppError::General("keychain locked".into()))] {
        let root = TempDir::new().expect("temp root");
        let runtime = test_runtime(&root);
        let loading = runtime.ui_begin(RemoteUiIntent::LoadConnection);
        let load_request = runtime.ui().save_request;
        save(&runtime, "https://indexer.test", Some("typed-key"))
            .await
            .expect("saved while the key read waits");
        let after_save = runtime.ui_snapshot().connection;
        assert!(after_save.api_key_configured);
        assert_eq!(after_save.save, RemoteDraftStatus::Succeeded);

        runtime.ui().loaded(load_request, &late);

        let connection = runtime.ui_snapshot().connection;
        assert!(connection.api_key_configured, "{late:?}");
        assert_eq!(connection.save, RemoteDraftStatus::Succeeded, "{late:?}");
        drop(loading);
    }
}

#[tokio::test]
async fn a_save_with_a_key_stores_it_when_the_config_folder_cannot_be_written() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    std::fs::remove_dir_all(root.path()).expect("remove the config folder");
    std::fs::write(root.path(), b"").expect("a regular file where the config folder was");

    let result = save(&runtime, "https://indexer.test", Some("typed-key")).await;

    assert!(matches!(result, Ok(RemoteUiResult::Saved)));
    assert!(runtime.inner.settings.snapshot().await.save_error.is_some());
    assert_eq!(
        runtime
            .inner
            .settings
            .indexer_connection()
            .base_url
            .as_deref(),
        Some("https://indexer.test")
    );
    assert_eq!(
        key_for(&runtime, "https://indexer.test").as_deref(),
        Some("typed-key")
    );
    assert!(runtime.ui_snapshot().connection.api_key_configured);
    std::fs::remove_file(root.path()).expect("remove the blocking file");
    std::fs::create_dir_all(root.path()).expect("restore the config folder");
}

fn search() -> RemoteUiIntent {
    RemoteUiIntent::SearchReleases {
        author: "Author".into(),
        title: String::new(),
    }
}

#[tokio::test]
async fn an_admitted_reset_refuses_a_new_search_or_grab_until_it_ends() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    save(&runtime, "https://indexer.test", Some("key"))
        .await
        .expect("saved");
    run(
        &runtime,
        RemoteUiIntent::SelectLane {
            lane: ProviderId::Indexer,
        },
    )
    .await
    .expect("lane");
    {
        let mut state = runtime.ui();
        let request = state.search_request;
        state.searched(
            request,
            &Ok(RemoteReleaseSearchResponse {
                provider_id: ProviderId::Indexer,
                releases: vec![release(1)],
                diagnostics: vec![],
            }),
        );
    }
    run(
        &runtime,
        RemoteUiIntent::SelectRelease {
            indexer_id: 1,
            guid: "shared-guid".into(),
            multi: false,
        },
    )
    .await
    .expect("select");

    let reservation = runtime.reset_lease().expect("an idle Indexer");
    assert!(runtime.ui().begin(search()).is_err());
    assert!(runtime.ui().begin(RemoteUiIntent::GrabSelected).is_err());
    drop(reservation);

    assert!(
        runtime.ui().begin(RemoteUiIntent::GrabSelected).is_ok(),
        "dropping the reservation admits a Grab"
    );
    runtime.ui().snapshot.indexer.grabbing = false;
    assert!(runtime.ui().begin(search()).is_ok());
}

#[tokio::test]
async fn a_refused_reset_leaves_search_and_grab_admitted() {
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
    assert!(runtime.ui().begin(search()).is_ok(), "a Search is accepted");
    assert!(
        runtime.reset_lease().is_err(),
        "a Reset waits for the Search"
    );
    runtime.ui().snapshot.indexer.searching = false;
    assert!(
        runtime.ui().begin(search()).is_ok(),
        "the refused Reset reserved nothing"
    );
    runtime.ui().snapshot.indexer.searching = false;

    let reservation = runtime
        .reset_lease()
        .expect("the refused Reset kept neither the lease nor the reservation");
    drop(reservation);

    assert!(runtime.ui().begin(search()).is_ok());
}
