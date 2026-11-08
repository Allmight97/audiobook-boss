use super::*;
use crate::remote_source::tests::test_runtime;
use tempfile::TempDir;

fn refusal(result: Result<UiAction>) -> String {
    match result {
        Err(AppError::InvalidInput(message)) => message,
        _ => panic!("the intent should be refused as invalid input"),
    }
}

#[tokio::test]
async fn the_indexer_lane_is_busy_while_a_reset_holds_its_reservation() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    runtime
        .ui_begin(RemoteUiIntent::SelectLane {
            lane: ProviderId::Indexer,
        })
        .finish()
        .await
        .expect("lane");
    runtime.ui().snapshot.account_status = RemoteDraftStatus::Succeeded;
    assert!(!runtime.ui_snapshot().busy);

    let reservation = runtime.reset_lease().expect("an idle Indexer");
    assert!(runtime.ui_snapshot().busy);
    drop(reservation);

    assert!(!runtime.ui_snapshot().busy);
}

#[test]
fn a_sign_in_in_progress_is_refused_by_every_account_action_in_one_wording() {
    let wording = "Wait for the current account change to finish.";
    for auth in [RemoteAuthStatus::Starting, RemoteAuthStatus::Completing] {
        let mut ui = UiState::default();
        ui.audible.auth = auth;
        for intent in [
            RemoteUiIntent::SelectLane {
                lane: ProviderId::Audible,
            },
            RemoteUiIntent::Disconnect,
            RemoteUiIntent::AcquireSelected,
        ] {
            assert_eq!(refusal(ui.begin(intent)), wording);
        }
        assert_eq!(
            refusal(ui.begin(RemoteUiIntent::RefreshLibrary)),
            "Wait for the current Audible work before refreshing."
        );
    }
}

#[test]
fn the_indexer_refuses_work_in_one_wording_whatever_it_is_busy_with() {
    let wording = "Wait for the current Indexer search, grab, or save to finish.";
    let busy_states: [fn(&mut UiState); 4] = [
        |ui| ui.snapshot.indexer.searching = true,
        |ui| ui.indexer.grabbing = true,
        |ui| ui.indexer.saving = true,
        |ui| ui.indexer.resetting = true,
    ];
    for make_busy in busy_states {
        let mut ui = UiState::default();
        ui.snapshot.lane = ProviderId::Indexer;
        make_busy(&mut ui);
        let search = RemoteUiIntent::SearchReleases {
            author: "Author".into(),
            title: String::new(),
        };
        assert_eq!(refusal(ui.begin(search)), wording);
        assert_eq!(refusal(ui.begin(RemoteUiIntent::GrabSelected)), wording);
    }
}

#[test]
fn a_lane_change_supersedes_pending_reads_and_the_old_lanes_search() {
    let mut ui = UiState::default();
    let (account, library, search) = (
        ui.account_request,
        ui.audible.library_request,
        ui.indexer.search_request,
    );

    let request = ui.change_lane(ProviderId::Indexer);

    assert_eq!(request, account + 1);
    assert_eq!(ui.audible.library_request, library + 1);
    assert_eq!(ui.indexer.search_request, search + 1);
    assert_eq!(ui.snapshot.account_status, RemoteDraftStatus::Running);

    let again = ui.change_lane(ProviderId::Indexer);

    assert_eq!(again, request + 1);
    assert_eq!(
        ui.indexer.search_request,
        search + 1,
        "same lane keeps its search"
    );
}
