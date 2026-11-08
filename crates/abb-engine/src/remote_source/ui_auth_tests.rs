use super::super::lane_tests::awaiting_handoff;
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
    assert!(matches!(
        runtime.ui().audible.sign_in,
        AudibleSignIn::AwaitingHandoff(_)
    ));
    assert!(matches!(
        snapshot.account_status,
        RemoteDraftStatus::Failed { .. }
    ));
}

fn begin_completion(ui: &mut UiState) -> PendingAudibleAuth {
    let Ok(UiAction::CompleteAuth { pending, .. }) = ui.begin(RemoteUiIntent::CompleteAuth {
        response_url_handoff_path: Some("/path".into()),
    }) else {
        panic!("the handoff is accepted")
    };
    pending
}

#[test]
fn a_failed_completion_that_kept_the_pending_sign_in_can_be_retried() {
    let mut ui = UiState::default();
    ui.audible.sign_in = awaiting_handoff();
    let pending = begin_completion(&mut ui);

    let refresh = ui.auth_completed(
        &Err(AppError::InvalidInput("Unreadable path.".into())),
        Some(pending),
    );

    assert_eq!(refresh, None);
    assert!(matches!(
        ui.audible.sign_in,
        AudibleSignIn::AwaitingHandoff(_)
    ));
    assert!(matches!(
        ui.snapshot.account_status,
        RemoteDraftStatus::Failed { .. }
    ));
    assert!(ui.snapshot().audible_message.contains("Unreadable path."));
    begin_completion(&mut ui);
    assert!(matches!(ui.audible.sign_in, AudibleSignIn::Completing));
    assert_eq!(ui.snapshot.account_status, RemoteDraftStatus::Idle);
}

#[test]
fn a_failed_registration_that_took_the_pending_sign_in_stays_failed() {
    let mut ui = UiState::default();
    ui.audible.sign_in = awaiting_handoff();
    begin_completion(&mut ui);

    ui.auth_completed(
        &Err(AppError::General("Registration refused.".into())),
        None,
    );

    assert!(matches!(ui.audible.sign_in, AudibleSignIn::Failed(_)));
    assert!(ui
        .begin(RemoteUiIntent::CompleteAuth {
            response_url_handoff_path: None,
        })
        .is_err());
}

#[test]
fn a_start_that_fails_after_an_earlier_start_leaves_no_pending_sign_in() {
    let mut ui = UiState::default();
    ui.audible.sign_in = awaiting_handoff();
    ui.begin(RemoteUiIntent::StartAuth).expect("a new start");
    assert!(matches!(ui.audible.sign_in, AudibleSignIn::Starting));

    let failed = ui.auth_started(Err(AppError::General("No authorization URL.".into())));

    assert!(failed.is_err());
    assert!(matches!(ui.audible.sign_in, AudibleSignIn::Failed(_)));
    assert!(ui
        .begin(RemoteUiIntent::CompleteAuth {
            response_url_handoff_path: None,
        })
        .is_err());
}

#[tokio::test]
async fn a_wrong_handoff_url_leaves_the_sign_in_completable_again() {
    let root = tempfile::TempDir::new().expect("temp dir");
    let runtime = super::super::super::tests::test_runtime(&root);
    runtime
        .ui_begin(RemoteUiIntent::StartAuth)
        .finish()
        .await
        .expect("auth started");
    let wrong = || RemoteUiIntent::CompleteAuth {
        response_url_handoff_path: Some("https://www.amazon.com/".into()),
    };

    assert!(runtime.ui_begin(wrong()).finish().await.is_err());
    let Err(second) = runtime.ui_begin(wrong()).finish().await else {
        panic!("a wrong URL cannot complete a sign-in");
    };

    assert!(
        !second.to_string().contains("Start Audible auth"),
        "the pending sign-in survived the first wrong URL: {second}"
    );
}

#[tokio::test]
async fn disconnecting_while_awaiting_the_handoff_discards_the_sign_in() {
    let root = tempfile::TempDir::new().expect("temp dir");
    let runtime = super::super::super::tests::test_runtime(&root);
    runtime
        .ui_begin(RemoteUiIntent::StartAuth)
        .finish()
        .await
        .expect("auth started");

    assert!(runtime
        .ui_begin(RemoteUiIntent::Disconnect)
        .finish()
        .await
        .is_ok());

    let Err(refused) = runtime
        .ui_begin(RemoteUiIntent::CompleteAuth {
            response_url_handoff_path: Some("https://www.amazon.com/".into()),
        })
        .finish()
        .await
    else {
        panic!("no sign-in is waiting");
    };
    assert!(refused.to_string().contains("Start Audible auth"));
    assert!(!runtime.ui().audible.disconnecting());
}
