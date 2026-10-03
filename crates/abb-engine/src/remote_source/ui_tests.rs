use super::*;
use crate::remote_source::types::{RemoteTitleAvailability, RemoteTitleAvailabilityStatus};

fn title(id: &str, acquirable: bool, pdf: bool) -> RemoteTitle {
    RemoteTitle {
        provider_id: ProviderId::Audible,
        title_id: id.into(),
        title: id.into(),
        authors: vec![],
        narrators: vec![],
        duration_seconds: None,
        cover_url: None,
        supplemental_pdf_available: pdf,
        acquired: false,
        availability: RemoteTitleAvailability {
            status: RemoteTitleAvailabilityStatus::Available,
            acquirable,
            label: "Available".into(),
            detail: None,
        },
        unsupported_reasons: vec![],
    }
}
fn edit(key: Option<&str>) -> RemoteUiIntent {
    RemoteUiIntent::EditConnection {
        base_url: Some("https://indexer.test".into()),
        category_ids: Some(vec![3030]),
        api_key: key.map(str::to_string),
    }
}
fn configured() -> RemoteIndexerConnection {
    RemoteIndexerConnection {
        base_url: Some("https://indexer.test".into()),
        category_ids: vec![3030],
        api_key_configured: true,
    }
}
fn release(indexer_id: i64) -> RemoteRelease {
    RemoteRelease {
        provider_id: ProviderId::Indexer,
        guid: "shared-guid".into(),
        indexer_id,
        title: format!("Book {indexer_id}"),
        indexer: "Indexer".into(),
        detail_url: None,
        size_bytes: 100,
        protocol: super::super::types::RemoteReleaseProtocol::Torrent,
        seeders: Some(10),
        categories: vec![],
    }
}
#[test]
fn acquisition_choices_enforce_availability_and_preserve_explicit_pdf_choice_on_refresh() {
    let mut ui = UiState::default();
    ui.library_loaded(vec![
        title("available", true, true),
        title("blocked", false, false),
    ]);
    assert!(ui
        .begin(RemoteUiIntent::ToggleTitle {
            title_id: "blocked".into()
        })
        .is_err());
    ui.begin(RemoteUiIntent::ToggleTitle {
        title_id: "available".into(),
    })
    .expect("select available");
    ui.begin(RemoteUiIntent::TogglePdf {
        title_id: "available".into(),
    })
    .expect("exclude PDF");
    ui.library_loaded(vec![title("available", true, true)]);
    let UiAction::Acquire(plan) = ui
        .begin(RemoteUiIntent::AcquireSelected)
        .expect("acquire selection")
    else {
        panic!("acquire action")
    };
    assert_eq!(
        plan.selections,
        vec![AcquisitionSelection {
            title_id: "available".into(),
            include_supplemental_pdf: false
        }]
    );
    assert!(ui.begin(RemoteUiIntent::AcquireSelected).is_err());
    ui.snapshot.acquiring = false;
    ui.begin(RemoteUiIntent::SelectLane {
        lane: ProviderId::Indexer,
    })
    .expect("change lane");
    assert!(ui.snapshot.selected_title_ids.is_empty());
    assert!(ui.begin(RemoteUiIntent::AcquireSelected).is_err());
}
#[test]
fn connection_readback_and_old_tests_cannot_erase_newer_drafts_or_expose_the_key() {
    let mut ui = UiState::default();
    let UiAction::Load { request, revision } =
        ui.begin(RemoteUiIntent::LoadConnection).expect("load")
    else {
        panic!("load action")
    };
    ui.begin(edit(Some("first-secret")))
        .expect("edit while loading");
    ui.loaded(request, revision, &Ok(configured()));
    assert!(ui.snapshot.connection.api_key_entered);
    let UiAction::Save { revision, update } =
        ui.begin(RemoteUiIntent::SaveConnection).expect("save")
    else {
        panic!("save action")
    };
    assert_eq!(update.api_key.as_deref(), Some("first-secret"));
    ui.begin(edit(Some("next-secret")))
        .expect("new edit while saving");
    ui.saved(revision, &Ok(configured()));
    assert_eq!(ui.api_key.as_deref(), Some("next-secret"));
    assert_eq!(ui.snapshot.connection.save, RemoteDraftStatus::Idle);
    let UiAction::Test {
        request, revision, ..
    } = ui.begin(RemoteUiIntent::TestConnection).expect("test")
    else {
        panic!("test action")
    };
    let _save = ui
        .begin(RemoteUiIntent::SaveConnection)
        .expect("save supersedes test");
    ui.tested(
        request,
        revision,
        &Ok(RemoteIndexerConnectionTestResult {
            ok: true,
            message: "Old test".into(),
        }),
    );
    assert_eq!(ui.snapshot.connection.test, RemoteDraftStatus::Idle);
    let json = serde_json::to_string(&ui.snapshot()).expect("snapshot JSON");
    assert!(!json.contains("secret"));
    assert!(!format!("{:?}", edit(Some("secret"))).contains("secret"));
}
#[test]
fn batch_captures_both_indexer_identities_refuses_save_and_retains_retryable_failures() {
    let mut ui = UiState::default();
    ui.begin(RemoteUiIntent::SelectLane {
        lane: ProviderId::Indexer,
    })
    .expect("indexer");
    ui.searched(
        ui.search_request,
        &Ok(RemoteReleaseSearchResponse {
            provider_id: ProviderId::Indexer,
            releases: vec![release(1), release(2)],
            diagnostics: vec![],
        }),
    );
    for indexer_id in [1, 2] {
        ui.begin(RemoteUiIntent::SelectRelease {
            indexer_id,
            guid: "shared-guid".into(),
            multi: true,
        })
        .expect("select");
    }
    let UiAction::Grab(releases) = ui.begin(RemoteUiIntent::GrabSelected).expect("batch") else {
        panic!("grab action")
    };
    assert_eq!(
        releases
            .iter()
            .map(|release| release.indexer_id)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert!(ui.begin(RemoteUiIntent::SaveConnection).is_err());
    assert!(ui
        .begin(RemoteUiIntent::SelectLane {
            lane: ProviderId::Audible
        })
        .is_err());
    ui.snapshot.indexer.grabbing = false;
    ui.snapshot.indexer.release_grabs.insert(
        release_key(1, "shared-guid"),
        ReleaseGrabSnapshot {
            status: ReleaseGrabStatus::Sent,
            message: "Sent".into(),
        },
    );
    ui.snapshot.indexer.release_grabs.insert(
        release_key(2, "shared-guid"),
        ReleaseGrabSnapshot {
            status: ReleaseGrabStatus::Error,
            message: "Failed".into(),
        },
    );
    let UiAction::Grab(retry) = ui
        .begin(RemoteUiIntent::GrabSelected)
        .expect("retry failed")
    else {
        panic!("grab action")
    };
    assert_eq!(retry, vec![release(2)]);
}
#[tokio::test]
async fn connection_save_persists_the_accepted_draft_and_invalidates_old_release_selection() {
    let root = tempfile::TempDir::new().expect("temporary config");
    let runtime = super::super::tests::test_runtime(&root);
    runtime.ui_begin(edit(None)).finish().await.expect("edit");
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
            .get_indexer_connection()
            .await
            .expect("persisted connection")
            .base_url
            .as_deref(),
        Some("https://indexer.test")
    );
    assert_eq!(
        runtime.ui_snapshot().connection.base_url,
        "https://next.test"
    );
    assert!(runtime
        .ui_snapshot()
        .indexer
        .selected_release_keys
        .is_empty());
}

#[test]
fn disconnect_and_new_library_requests_expire_older_choices() {
    let mut ui = UiState::default();
    let old = ui.begin_library();
    let current = ui.begin_library();
    assert!(ui.library_reply(current, vec![title("new", true, true)]));
    assert!(!ui.library_reply(old, vec![title("old", true, true)]));
    ui.disconnected(ProviderId::Audible);
    assert!(!ui.library_reply(current, vec![title("new", true, true)]));
    assert!(ui
        .begin(RemoteUiIntent::ToggleTitle {
            title_id: "new".into()
        })
        .is_err());
    ui.library_loaded(vec![title("accepted", true, false)]);
    ui.begin(RemoteUiIntent::ToggleTitle {
        title_id: "accepted".into(),
    })
    .expect("fixture step succeeded");
    ui.begin(RemoteUiIntent::AcquireSelected)
        .expect("fixture step succeeded");
    assert!(ui.disconnect_allowed(ProviderId::Audible).is_err());
}

#[test]
fn a_save_waits_for_the_load_and_old_test_replies_cannot_replace_a_new_test() {
    let mut ui = UiState::default();
    let UiAction::Load { request, revision } = ui
        .begin(RemoteUiIntent::LoadConnection)
        .expect("fixture step succeeded")
    else {
        panic!("load")
    };
    ui.begin(edit(Some("key"))).expect("fixture step succeeded");
    // Nothing is saved while the stored connection is still loading.
    assert!(ui.begin(RemoteUiIntent::SaveConnection).is_err());
    ui.loaded(request, revision, &Ok(configured()));
    let UiAction::Save {
        revision: save_revision,
        ..
    } = ui
        .begin(RemoteUiIntent::SaveConnection)
        .expect("fixture step succeeded")
    else {
        panic!("save")
    };
    ui.saved(save_revision, &Ok(configured()));
    assert!(ui.snapshot.connection.api_key_configured);
    let UiAction::Test {
        request: first,
        revision: first_revision,
        ..
    } = ui
        .begin(RemoteUiIntent::TestConnection)
        .expect("fixture step succeeded")
    else {
        panic!("test")
    };
    let UiAction::Test {
        request: second,
        revision: second_revision,
        ..
    } = ui
        .begin(RemoteUiIntent::TestConnection)
        .expect("fixture step succeeded")
    else {
        panic!("test")
    };
    ui.tested(
        second,
        second_revision,
        &Ok(RemoteIndexerConnectionTestResult {
            ok: true,
            message: "New".into(),
        }),
    );
    ui.tested(
        first,
        first_revision,
        &Err(AppError::General("Old failure".into())),
    );
    assert_eq!(
        ui.snapshot
            .connection
            .test_result
            .as_ref()
            .expect("fixture step succeeded")
            .message,
        "New"
    );
    ui.begin(RemoteUiIntent::SelectLane {
        lane: ProviderId::Indexer,
    })
    .expect("fixture step succeeded");
    let UiAction::Search {
        request: search, ..
    } = ui
        .begin(RemoteUiIntent::SearchReleases {
            author: "Author".into(),
            title: "".into(),
        })
        .expect("fixture step succeeded")
    else {
        panic!("search")
    };
    ui.begin(RemoteUiIntent::SelectLane {
        lane: ProviderId::Audible,
    })
    .expect("fixture step succeeded");
    ui.searched(
        search,
        &Ok(RemoteReleaseSearchResponse {
            provider_id: ProviderId::Indexer,
            releases: vec![release(1)],
            diagnostics: vec![],
        }),
    );
    assert!(ui.snapshot.indexer.releases.is_empty());
    assert!(!ui.snapshot.indexer.searching);
}

#[tokio::test]
async fn grab_batch_sends_in_order_blocks_connection_changes_and_retries_only_failed_rows() {
    let root = tempfile::TempDir::new().expect("fixture step succeeded");
    let runtime = super::super::tests::test_runtime(&root);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fixture step succeeded");
    runtime
        .ui_begin(RemoteUiIntent::EditConnection {
            base_url: Some(format!(
                "http://{}",
                listener.local_addr().expect("fixture step succeeded")
            )),
            category_ids: None,
            api_key: Some("local-proof-key".into()),
        })
        .finish()
        .await
        .expect("fixture step succeeded");
    runtime
        .ui_begin(RemoteUiIntent::SaveConnection)
        .finish()
        .await
        .expect("fixture step succeeded");
    runtime
        .ui_begin(RemoteUiIntent::SelectLane {
            lane: ProviderId::Indexer,
        })
        .finish()
        .await
        .expect("fixture step succeeded");
    {
        let mut ui = runtime.ui();
        let request = ui.search_request;
        ui.searched(
            request,
            &Ok(RemoteReleaseSearchResponse {
                provider_id: ProviderId::Indexer,
                releases: vec![release(1), release(2)],
                diagnostics: vec![],
            }),
        );
    }
    for indexer_id in [1, 2] {
        runtime
            .ui_begin(RemoteUiIntent::SelectRelease {
                indexer_id,
                guid: "shared-guid".into(),
                multi: true,
            })
            .finish()
            .await
            .expect("fixture step succeeded");
    }
    let batch = runtime.ui_begin(RemoteUiIntent::GrabSelected);
    let serving = serve_grab_batch(listener);
    assert!(runtime
        .ui_begin(RemoteUiIntent::SaveConnection)
        .finish()
        .await
        .is_err());
    assert!(runtime.logout(ProviderId::Indexer).is_err());
    tokio::time::timeout(std::time::Duration::from_secs(10), batch.finish())
        .await
        .expect("fixture step succeeded")
        .expect("fixture step succeeded");
    let snapshot = runtime.ui_snapshot();
    assert_eq!(
        snapshot.indexer.release_grabs[&release_key(1, "shared-guid")].status,
        ReleaseGrabStatus::Sent
    );
    assert_eq!(
        snapshot.indexer.release_grabs[&release_key(2, "shared-guid")].status,
        ReleaseGrabStatus::Error
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        runtime.ui_begin(RemoteUiIntent::GrabSelected).finish(),
    )
    .await
    .expect("fixture step succeeded")
    .expect("fixture step succeeded");
    assert_eq!(serving.await.expect("fixture step succeeded"), [1, 2, 2]);
    assert_eq!(
        runtime.ui_snapshot().indexer.release_grabs[&release_key(2, "shared-guid")].status,
        ReleaseGrabStatus::Sent
    );
}

fn serve_grab_batch(listener: tokio::net::TcpListener) -> tokio::task::JoinHandle<Vec<i64>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    tokio::spawn(async move {
        let mut ids = Vec::new();
        for response_status in ["200 OK", "500 Internal Server Error", "200 OK"] {
            let (mut stream, _) = listener.accept().await.expect("fixture step succeeded");
            let mut request = Vec::new();
            loop {
                let mut bytes = [0; 1024];
                let count = stream
                    .read(&mut bytes)
                    .await
                    .expect("fixture step succeeded");
                assert_ne!(count, 0);
                request.extend_from_slice(&bytes[..count]);
                let text = String::from_utf8_lossy(&request);
                if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.split_once(':')
                                .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                                .map(|(_, value)| {
                                    value
                                        .trim()
                                        .parse::<usize>()
                                        .expect("fixture step succeeded")
                                })
                        })
                        .unwrap_or(0);
                    if body.len() >= length {
                        let json: serde_json::Value =
                            serde_json::from_str(body).expect("fixture step succeeded");
                        ids.push(json["indexerId"].as_i64().expect("fixture step succeeded"));
                        break;
                    }
                }
            }
            let body = "{}";
            stream.write_all(format!("HTTP/1.1 {response_status}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await.expect("fixture step succeeded");
        }
        ids
    })
}

#[tokio::test]
async fn shutdown_drops_an_indexer_search_waiting_on_a_silent_server() {
    let root = tempfile::TempDir::new().expect("temp dir");
    let runtime = super::super::tests::test_runtime(&root);
    // Accepts connections and never answers.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let _silent = tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            held.push(stream);
        }
    });
    for intent in [
        RemoteUiIntent::EditConnection {
            base_url: Some(format!("http://{address}")),
            category_ids: None,
            api_key: Some("local-proof-key".into()),
        },
        RemoteUiIntent::SaveConnection,
        RemoteUiIntent::SelectLane {
            lane: ProviderId::Indexer,
        },
    ] {
        runtime.ui_begin(intent).finish().await.expect("setup step");
    }
    let search = tokio::spawn(
        runtime
            .ui_begin(RemoteUiIntent::SearchReleases {
                author: "Author".into(),
                title: "Title".into(),
            })
            .finish(),
    );
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    runtime.inner.tasks.close();

    let ended = tokio::time::timeout(std::time::Duration::from_secs(2), search)
        .await
        .expect("the search ends at shutdown, not after the server's timeout")
        .expect("search task");
    assert!(ended.is_err());
}

#[test]
fn save_and_test_refuse_a_refused_draft_and_a_draft_still_loading() {
    let mut ui = UiState::default();
    let refused = ui.begin(RemoteUiIntent::EditConnection {
        base_url: Some("not a url ::".into()),
        category_ids: None,
        api_key: None,
    });
    assert!(refused.is_err());
    assert!(ui.snapshot().connection.draft_error.is_some());
    assert!(ui.begin(RemoteUiIntent::SaveConnection).is_err());
    assert!(ui.begin(RemoteUiIntent::TestConnection).is_err());

    // An accepted edit clears it.
    ui.begin(RemoteUiIntent::EditConnection {
        base_url: Some("https://indexer.test".into()),
        category_ids: None,
        api_key: None,
    })
    .expect("accepted edit");
    assert!(ui.snapshot().connection.draft_error.is_none());

    // While the saved connection loads, nothing is saved or tested.
    let mut loading = UiState::default();
    loading
        .begin(RemoteUiIntent::LoadConnection)
        .expect("load starts");
    assert!(loading.begin(RemoteUiIntent::SaveConnection).is_err());
}

#[test]
fn a_connection_that_loads_after_typing_keeps_the_typed_fields_and_fills_the_rest() {
    let mut ui = UiState::default();
    let Ok(UiAction::Load { request, revision }) = ui.begin(RemoteUiIntent::LoadConnection) else {
        panic!("load starts");
    };
    // Only categories were typed while the keychain read was pending.
    ui.begin(RemoteUiIntent::EditConnection {
        base_url: None,
        category_ids: Some(vec![3000]),
        api_key: None,
    })
    .expect("edit");

    ui.loaded(request, revision, &Ok(configured()));

    let connection = ui.snapshot().connection;
    assert_eq!(connection.category_ids, [3000]);
    assert_eq!(
        connection.base_url,
        configured().base_url.unwrap_or_default()
    );
}

#[test]
fn release_keys_keep_the_format_the_frontend_builds() {
    // `src/app/remoteSource/selection.ts` builds the same key with
    // `JSON.stringify([indexerId, guid])` to look rows up.
    assert_eq!(release_key(8, "same-guid"), r#"[8,"same-guid"]"#);
}
