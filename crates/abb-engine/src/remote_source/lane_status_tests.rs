use super::super::{RemoteUiIntent, UiAction, UiState};
use super::*;
use crate::errors::{AppError, AppErrorEnvelope};
use crate::remote_source::providers::audible::AudibleRequestError;
use crate::remote_source::{
    ProviderId, RemoteAcquisitionFailureKind, RemoteAcquisitionStatus, RemoteLibraryResponse,
    RemoteSourceAccountState,
};
use abb_remote_source_core::{acquisition_progress, AcquisitionStage};

const WARNING: &str = "Sign-in won't be remembered after a restart.";

fn job(id: &str, status: RemoteAcquisitionStatus, stage: AcquisitionStage) -> AcquisitionJob {
    AcquisitionJob {
        job_id: id.into(),
        provider_id: ProviderId::Audible,
        status,
        progress: acquisition_progress(stage, Some(1.0), None, None),
        materialized_files: Vec::new(),
        supplemental_assets: Vec::new(),
        diagnostics: Vec::new(),
        handoff: None,
    }
}

fn imported(count: usize) -> AcquisitionJob {
    AcquisitionJob {
        handoff: Some(AcquisitionHandoff::Imported { count }),
        ..job(
            "imported",
            RemoteAcquisitionStatus::ImportedToFileList,
            AcquisitionStage::ImportHandoff,
        )
    }
}

fn removed(reason: HandoffRefusal) -> AcquisitionJob {
    AcquisitionJob {
        handoff: Some(AcquisitionHandoff::Removed { reason }),
        ..job(
            "removed",
            RemoteAcquisitionStatus::Validated,
            AcquisitionStage::ImportHandoff,
        )
    }
}

fn diagnostic(message: &str) -> RemoteSourceDiagnostic {
    RemoteSourceDiagnostic {
        kind: RemoteAcquisitionFailureKind::AuthRequired,
        title_id: None,
        message: message.into(),
    }
}

fn envelope(message: &str) -> AppErrorEnvelope {
    AppErrorEnvelope {
        message: message.into(),
        ..(&AppError::InvalidInput(String::new())).into()
    }
}

fn connected(provider_id: ProviderId, message: &str) -> Option<RemoteSourceAccountState> {
    Some(RemoteSourceAccountState {
        provider_id,
        status: RemoteAccountStatus::Connected,
        message: Some(message.into()),
    })
}

fn refresh(ui: &mut UiState) -> u64 {
    let Ok(UiAction::Library(request)) = ui.begin(RemoteUiIntent::RefreshLibrary) else {
        panic!("library refresh is accepted");
    };
    request
}

fn fail_library(ui: &mut UiState, request: u64) {
    let failure = Err(AudibleRequestError::Failed(AppError::InvalidInput(
        "Audible library is unavailable.".into(),
    )));
    assert!(ui.library_result(request, &failure));
}

fn settled_ui() -> UiState {
    let mut ui = UiState::default();
    ui.acquisition_changed(imported(2));
    ui
}

fn message(ui: &UiState) -> String {
    ui.snapshot().audible_message
}

#[test]
fn settling_a_job_publishes_its_line_on_the_remote_part() {
    let mut ui = UiState::default();
    let running = job(
        "imported",
        RemoteAcquisitionStatus::Acquiring,
        AcquisitionStage::Download,
    );
    ui.acquisition_changed(running.clone());
    assert!(ui.acquisition_changed(running).remote.is_none());
    let update = ui.acquisition_changed(imported(2));
    let remote = update.remote.expect("the settle carries the remote part");
    assert_eq!(remote.audible_message, "2 acquired titles imported.");
}

#[test]
fn a_library_failure_after_a_settled_job_shows_the_failure() {
    let mut ui = settled_ui();
    let request = refresh(&mut ui);
    fail_library(&mut ui, request);
    assert_eq!(
        message(&ui),
        "Invalid input: Audible library is unavailable."
    );
}

#[test]
fn a_library_load_after_a_settled_job_replaces_its_line() {
    let mut ui = settled_ui();
    let request = refresh(&mut ui);
    assert_eq!(message(&ui), "Loading Audible library.");
    let response = Ok(RemoteLibraryResponse {
        provider_id: ProviderId::Audible,
        titles: Vec::new(),
        diagnostics: Vec::new(),
    });
    assert!(ui.library_result(request, &response));
    assert_eq!(message(&ui), "0 Audible titles loaded.");
}

#[test]
fn a_job_settled_after_a_library_failure_outranks_it() {
    let mut ui = UiState::default();
    let request = refresh(&mut ui);
    fail_library(&mut ui, request);
    ui.acquisition_changed(imported(1));
    assert_eq!(message(&ui), "1 acquired title imported.");
}

#[test]
fn a_new_sign_in_replaces_a_settled_job_line() {
    let mut ui = settled_ui();
    ui.begin(RemoteUiIntent::StartAuth).expect("sign-in starts");
    assert_eq!(message(&ui), "Starting Audible authorization.");
    ui.snapshot.auth = RemoteAuthStatus::AwaitingHandoff;
    assert!(message(&ui).starts_with("Complete Audible authorization in your browser"));
}

#[test]
fn completing_a_sign_in_replaces_a_job_that_settled_while_it_waited() {
    let mut ui = UiState::default();
    ui.snapshot.auth = RemoteAuthStatus::AwaitingHandoff;
    ui.acquisition_changed(imported(1));
    ui.begin(RemoteUiIntent::CompleteAuth {
        response_url_handoff_path: None,
    })
    .expect("handoff accepted");
    assert_eq!(message(&ui), "Completing Audible connection.");
}

#[test]
fn republishing_a_settled_job_does_not_restore_its_replaced_line() {
    let mut ui = settled_ui();
    let request = refresh(&mut ui);
    fail_library(&mut ui, request);
    ui.acquisition_changed(imported(2));
    assert_eq!(
        message(&ui),
        "Invalid input: Audible library is unavailable."
    );
}

#[test]
fn disconnecting_clears_a_settled_job_line() {
    let mut ui = settled_ui();
    ui.disconnected();
    assert_eq!(message(&ui), "");
}

#[test]
fn a_sign_in_or_account_failure_outranks_a_settled_job() {
    let mut ui = settled_ui();
    ui.snapshot.account_status = RemoteDraftStatus::Failed {
        error: envelope("Account read failed."),
    };
    assert_eq!(message(&ui), "Account read failed.");
    ui.snapshot.auth = RemoteAuthStatus::Failed {
        error: envelope("Sign-in failed."),
    };
    assert_eq!(message(&ui), "Sign-in failed.");
}

#[test]
fn settled_jobs_keep_their_outcome_wording() {
    assert_eq!(settled_job_line(&imported(1)), "1 acquired title imported.");
    assert_eq!(
        settled_job_line(&imported(3)),
        "3 acquired titles imported."
    );
    assert_eq!(
        settled_job_line(&removed(HandoffRefusal::ImportFailed {
            error: envelope("The disk is full."),
        })),
        "The disk is full. Staged remote files were removed."
    );
    assert_eq!(
        settled_job_line(&removed(HandoffRefusal::NothingAdded)),
        "Acquired titles were not added to the input session. Staged remote files were removed."
    );
}

#[test]
fn a_failed_job_shows_its_reason_and_falls_back_to_the_stage_word() {
    let failed = job(
        "failed",
        RemoteAcquisitionStatus::Failed,
        AcquisitionStage::Failed,
    );
    assert_eq!(settled_job_line(&failed), "Acquisition failed.");
    let explained = AcquisitionJob {
        diagnostics: vec![
            diagnostic(" Token expired. "),
            diagnostic("Token expired."),
            diagnostic("  "),
            diagnostic("Retrying."),
        ],
        ..failed
    };
    assert_eq!(settled_job_line(&explained), "Token expired. Retrying.");
}

#[test]
fn a_settled_import_or_removal_names_the_titles_that_failed() {
    let partial = AcquisitionJob {
        diagnostics: vec![
            diagnostic("One title failed."),
            diagnostic("One title failed."),
        ],
        ..imported(1)
    };
    assert_eq!(
        settled_job_line(&partial),
        "1 acquired title imported. One title failed."
    );
    let nothing_added = AcquisitionJob {
        diagnostics: vec![diagnostic("One title failed.")],
        ..removed(HandoffRefusal::NothingAdded)
    };
    assert_eq!(
        settled_job_line(&nothing_added),
        "Acquired titles were not added to the input session. Staged remote files were removed. One title failed."
    );
}

#[test]
fn audible_idle_rungs_keep_their_order_and_the_connected_note() {
    let mut ui = UiState::default();
    ui.snapshot.account = connected(ProviderId::Audible, WARNING);
    assert_eq!(message(&ui), WARNING);
    ui.snapshot.library_status = RemoteDraftStatus::Succeeded;
    assert_eq!(message(&ui), format!("0 Audible titles loaded. {WARNING}"));
    ui.library.diagnostics = vec![diagnostic("Some titles are hidden.")];
    assert_eq!(message(&ui), format!("Some titles are hidden. {WARNING}"));
    ui.snapshot.account_status = RemoteDraftStatus::Running;
    assert_eq!(message(&ui), "Updating connection.");
    ui.snapshot.auth = RemoteAuthStatus::Completing;
    assert_eq!(message(&ui), "Completing Audible connection.");
}

#[test]
fn a_running_connection_update_outranks_a_running_library_load_and_the_handoff_prompt() {
    let mut ui = UiState::default();
    ui.snapshot.auth = RemoteAuthStatus::AwaitingHandoff;
    ui.snapshot.library_status = RemoteDraftStatus::Running;
    assert_eq!(message(&ui), "Loading Audible library.");
    ui.snapshot.account_status = RemoteDraftStatus::Running;
    assert_eq!(message(&ui), "Updating connection.");
}

#[test]
fn a_settled_job_keeps_the_ephemeral_sign_in_note() {
    let mut ui = settled_ui();
    ui.snapshot.account = connected(ProviderId::Audible, WARNING);
    assert_eq!(
        message(&ui),
        format!("2 acquired titles imported. {WARNING}")
    );
}

#[test]
fn an_account_that_is_not_connected_still_shows_its_message() {
    let mut ui = UiState::default();
    ui.snapshot.account = Some(RemoteSourceAccountState {
        provider_id: ProviderId::Audible,
        status: RemoteAccountStatus::NeedsAuth,
        message: Some("Connect Audible.".into()),
    });
    assert_eq!(message(&ui), "Connect Audible.");
    ui.snapshot.library_status = RemoteDraftStatus::Succeeded;
    assert_eq!(message(&ui), "0 Audible titles loaded.");
}

#[test]
fn the_indexer_line_carries_its_account_failure_and_connected_note() {
    let mut ui = UiState::default();
    ui.snapshot.lane = ProviderId::Indexer;
    ui.snapshot.account = connected(ProviderId::Indexer, WARNING);
    ui.snapshot.indexer.message = "Indexer ready".into();
    assert_eq!(
        ui.snapshot().indexer.message,
        format!("Indexer ready {WARNING}")
    );
    ui.snapshot.account_status = RemoteDraftStatus::Failed {
        error: envelope("Account read failed."),
    };
    assert_eq!(
        ui.snapshot().indexer.message,
        format!("Account read failed. {WARNING}")
    );
    assert_eq!(ui.snapshot.indexer.message, "Indexer ready");
}

#[test]
fn audible_failures_stay_out_of_the_indexer_line() {
    let mut ui = UiState::default();
    ui.snapshot.lane = ProviderId::Indexer;
    ui.snapshot.auth = RemoteAuthStatus::Failed {
        error: envelope("Audible auth failed"),
    };
    ui.snapshot.library_status = RemoteDraftStatus::Failed {
        error: envelope("Audible library failed"),
    };
    ui.snapshot.indexer.message = "Indexer ready".into();
    assert_eq!(ui.snapshot().indexer.message, "Indexer ready");
}
