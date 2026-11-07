use super::*;

#[tokio::test]
async fn a_readable_but_wrong_handoff_url_keeps_the_pending_sign_in() {
    let root = tempfile::TempDir::new().expect("temp dir");
    let runtime = super::super::super::tests::test_runtime(&root);
    let start = runtime.ui_begin(RemoteUiIntent::StartAuth);
    start.finish().await.expect("auth started");

    let completion = runtime.ui_begin(RemoteUiIntent::CompleteAuth {
        response_url_handoff_path: Some("https://www.amazon.com/".into()),
    });
    assert!(completion.finish().await.is_err());

    let snapshot = runtime.ui_snapshot();
    assert_eq!(runtime.ui().audible.auth, RemoteAuthStatus::AwaitingHandoff);
    assert!(matches!(
        snapshot.account_status,
        RemoteDraftStatus::Failed { .. }
    ));
}

#[test]
fn a_failed_completion_that_kept_the_pending_sign_in_can_be_retried() {
    let mut ui = UiState::default();
    ui.audible.auth = RemoteAuthStatus::AwaitingHandoff;
    ui.begin(RemoteUiIntent::CompleteAuth {
        response_url_handoff_path: Some("/missing".into()),
    })
    .expect("handoff accepted");

    let refresh = ui.auth_completed(
        &Err(AppError::InvalidInput("Unreadable path.".into())),
        true,
    );

    assert_eq!(refresh, None);
    assert_eq!(ui.audible.auth, RemoteAuthStatus::AwaitingHandoff);
    assert!(matches!(
        ui.snapshot.account_status,
        RemoteDraftStatus::Failed { .. }
    ));
    assert!(ui.snapshot().audible_message.contains("Unreadable path."));
    ui.begin(RemoteUiIntent::CompleteAuth {
        response_url_handoff_path: Some("/good".into()),
    })
    .expect("retry admitted");
    assert_eq!(ui.audible.auth, RemoteAuthStatus::Completing);
    assert_eq!(ui.snapshot.account_status, RemoteDraftStatus::Idle);
}

#[test]
fn a_failed_registration_that_took_the_pending_sign_in_stays_failed() {
    let mut ui = UiState::default();
    ui.audible.auth = RemoteAuthStatus::AwaitingHandoff;
    ui.begin(RemoteUiIntent::CompleteAuth {
        response_url_handoff_path: None,
    })
    .expect("handoff accepted");

    ui.auth_completed(
        &Err(AppError::General("Registration refused.".into())),
        false,
    );

    assert!(matches!(ui.audible.auth, RemoteAuthStatus::Failed { .. }));
    assert!(ui
        .begin(RemoteUiIntent::CompleteAuth {
            response_url_handoff_path: None,
        })
        .is_err());
}
