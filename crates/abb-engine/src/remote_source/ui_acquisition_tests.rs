use super::*;
use crate::remote_source::providers::audible::tests::signed_in_vault;
use crate::remote_source::AcquisitionSelection;

#[derive(Default)]
struct Updates(std::sync::Mutex<Vec<crate::session::SessionUpdate>>);

impl crate::EventSink for Updates {
    fn emit(&self, event: crate::EngineEvent) {
        if let crate::EngineEvent::Session(update) = event {
            self.0.lock().expect("updates").push(update);
        }
    }
}

fn library_read_ended() -> AppError {
    AppError::ResourceCleanup("library read ended".into())
}

#[tokio::test]
async fn the_library_load_after_an_account_read_is_published_as_busy_while_it_runs() {
    let root = tempfile::TempDir::new().expect("temp dir");
    let updates = std::sync::Arc::new(Updates::default());
    let (library_read, entered, release) = vault_gate();
    let runtime = crate::remote_source::tests::test_runtime_with(
        &root,
        Some(Box::new(signed_in_vault().gating_reads(
            1,
            library_read,
            library_read_ended,
        ))),
        Some(updates.clone()),
    );

    let load = runtime.inner.tasks.spawn(
        runtime
            .ui_begin(RemoteUiIntent::SelectLane {
                lane: ProviderId::Audible,
            })
            .finish(),
    );
    entered.await.expect("library load reads the vault");

    let last = updates
        .0
        .lock()
        .expect("updates")
        .iter()
        .rev()
        .find_map(|update| update.remote.clone())
        .expect("a remote part");
    assert!(last.busy);
    assert_eq!(
        runtime.ui().audible.library_status,
        RemoteDraftStatus::Running
    );

    release.send(()).expect("release library read");
    assert!(load.await.expect("tracked load").is_err());
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
    let UiAction::Acquire(AcquisitionWork::Titles(plan)) = ui
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
    ui.set_acquisition(|acquisition| acquisition.acquiring = false);
    ui.begin(RemoteUiIntent::SelectLane {
        lane: ProviderId::Indexer,
    })
    .expect("change lane");
    assert!(ui.snapshot.selected_title_ids.is_empty());
    assert!(ui.begin(RemoteUiIntent::AcquireSelected).is_err());
}

#[test]
fn attaching_during_an_accepted_acquisition_retains_library_without_resending_it_on_progress() {
    let root = tempfile::TempDir::new().expect("temp dir");
    let updates = std::sync::Arc::new(Updates::default());
    let runtime =
        crate::remote_source::tests::test_runtime_with(&root, None, Some(updates.clone()));
    let request = runtime.ui().begin_library();
    assert!(runtime
        .ui()
        .library_result(request, &Ok(library(vec![title("retained", true, true)]))));
    runtime
        .ui()
        .begin(RemoteUiIntent::ToggleTitle {
            title_id: "retained".into(),
        })
        .expect("select title");
    runtime
        .ui()
        .begin(RemoteUiIntent::AcquireSelected)
        .expect("accept acquisition");
    let mut attached = crate::session::SessionUpdate::default();
    runtime.add_ui_parts(&mut attached, None);
    let (remote, library, acquisition) = (
        attached.remote,
        attached.remote_library,
        attached.remote_acquisition,
    );
    assert!(remote.is_some());
    assert!(acquisition.expect("attached acquisition").acquiring);
    assert_eq!(
        library.expect("attached library").titles[0].title_id,
        "retained"
    );
    runtime.publish_ui();
    assert_eq!(
        updates.0.lock().expect("events")[0]
            .remote_library
            .as_ref()
            .expect("changed library event")
            .titles[0]
            .title_id,
        "retained"
    );
    updates.0.lock().expect("events").clear();
    let since = runtime.ui_revisions();
    (runtime.inner.lifecycle.jobs.lock().expect("jobs")).insert(
        "progress-job".into(),
        crate::remote_source::job_state::tests::titles_job("progress-job"),
    );
    runtime.inner.lifecycle.apply_outcome(
        "progress-job",
        crate::remote_source::AcquisitionOutcome::failed(
            crate::remote_source::RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
            "fixture failure".into(),
        ),
    );
    let mut changed = crate::session::SessionUpdate::default();
    runtime.add_ui_parts(&mut changed, Some(since));
    let (remote, library, acquisition) = (
        changed.remote,
        changed.remote_library,
        changed.remote_acquisition,
    );
    assert!(remote.is_none());
    assert!(library.is_none());
    assert!(acquisition.expect("changed acquisition").job.is_some());
    let events = updates.0.lock().expect("progress events");
    assert_eq!(events.len(), 1);
    assert!(events[0].remote_acquisition.is_some());
    assert!(events[0].remote.is_none());
    assert!(events[0].remote_library.is_none());
}

fn acquisition_failed_with(
    kind: crate::remote_source::RemoteAcquisitionFailureKind,
) -> crate::remote_source::AcquisitionSnapshot {
    crate::remote_source::job_state::tests::failed(
        "job-1",
        vec![crate::remote_source::RemoteSourceDiagnostic {
            kind,
            title_id: Some("B000000001".into()),
            message: "Audible license request failed.".into(),
        }],
    )
    .snapshot()
}

#[test]
fn a_refused_license_during_acquisition_needs_auth() {
    let mut ui = UiState::default();
    reread_account(&mut ui);

    ui.acquisition_changed(acquisition_failed_with(
        crate::remote_source::RemoteAcquisitionFailureKind::AuthRequired,
    ));

    assert_eq!(
        ui.snapshot().account.expect("account").status,
        crate::remote_source::RemoteAccountStatus::NeedsAuth
    );
    assert_eq!(
        reread_account(&mut ui).status,
        crate::remote_source::RemoteAccountStatus::NeedsAuth
    );
}

#[test]
fn a_title_level_license_failure_leaves_the_account_connected() {
    let mut ui = UiState::default();
    reread_account(&mut ui);

    ui.acquisition_changed(acquisition_failed_with(
        crate::remote_source::RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
    ));

    assert_eq!(
        ui.snapshot().account.expect("account").status,
        crate::remote_source::RemoteAccountStatus::Connected
    );
}

/// Bytes of each session update a download sends, for a library of `count` titles.
fn download_update_bytes(count: usize) -> Vec<(bool, bool, bool, usize)> {
    use abb_remote_source_core::{acquisition_progress, AcquisitionStage};
    let root = tempfile::TempDir::new().expect("temp dir");
    let updates = std::sync::Arc::new(Updates::default());
    let runtime =
        crate::remote_source::tests::test_runtime_with(&root, None, Some(updates.clone()));
    runtime.ui().library_loaded(
        (0..count)
            .map(|index| title(&format!("B{index:09}"), true, index % 5 == 0))
            .collect(),
    );
    for index in 0..50 {
        let _ = runtime.ui_begin(RemoteUiIntent::ToggleTitle {
            title_id: format!("B{index:09}"),
        });
    }
    let job_id = "job-1".to_string();
    (runtime.inner.lifecycle.jobs.lock().expect("jobs")).insert(
        job_id.clone(),
        crate::remote_source::job_state::tests::titles_job(&job_id),
    );
    runtime
        .ui()
        .set_acquisition(|acquisition| acquisition.acquiring = true);
    updates.0.lock().expect("updates").clear();
    for stage in [AcquisitionStage::License, AcquisitionStage::Download] {
        runtime.inner.lifecycle.update_job_progress(
            &job_id,
            acquisition_progress(stage, Some(0.5), Some(500), Some(1_000)),
        );
    }
    let sent = updates.0.lock().expect("updates");
    sent.iter()
        .map(|update| {
            (
                update.remote.is_some(),
                update.remote_library.is_some(),
                update.remote_acquisition.is_some(),
                serde_json::to_vec(update).expect("serialize").len(),
            )
        })
        .collect()
}

#[test]
fn a_download_tick_sends_only_the_acquisition_part_whatever_the_library_size() {
    let small = download_update_bytes(1_000);
    assert_eq!(small.len(), 2);
    assert!(small
        .iter()
        .all(|&(remote, library, acquisition, _)| !remote && !library && acquisition));
    assert_eq!(small, download_update_bytes(5_000));
}

#[test]
fn disconnecting_audible_clears_the_job_with_a_newer_acquisition_revision() {
    let mut ui = UiState::default();
    ui.acquisition_changed(acquisition_failed_with(
        crate::remote_source::RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
    ));
    let before = ui.acquisition.revision;
    ui.disconnected();
    assert!(ui.acquisition.job.is_none());
    assert!(ui.acquisition.revision > before);
}

#[test]
fn a_settling_job_frees_the_audible_lane_in_the_same_update() {
    let mut ui = UiState::default();
    ui.set_acquisition(|acquisition| acquisition.acquiring = true);
    assert!(ui.snapshot().busy);
    let running = crate::remote_source::job_state::tests::running(
        "job-1",
        abb_remote_source_core::AcquisitionStage::Download,
    )
    .snapshot();
    let tick = ui.acquisition_changed(running);
    ui.set_acquisition(|acquisition| acquisition.acquiring = false);
    assert!(tick.remote.is_none());
    assert!(ui.snapshot().busy);
    let settled = ui.acquisition_changed(acquisition_failed_with(
        crate::remote_source::RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
    ));
    assert!(!settled.remote.expect("busy flipped").busy);
}

#[test]
fn indexer_work_makes_only_the_indexer_lane_busy() {
    let mut ui = UiState::default();
    ui.snapshot.indexer.searching = true;
    assert!(!ui.snapshot().busy);
    ui.snapshot.lane = ProviderId::Indexer;
    assert!(ui.snapshot().busy);
}

#[tokio::test]
async fn disconnect_is_refused_while_an_acquisition_is_accepted_or_unsettled_with_one_message() {
    let root = tempfile::TempDir::new().expect("temp dir");
    let runtime = crate::remote_source::tests::test_runtime(&root);
    let refusal = "Wait for the Audible acquisition and handoff before disconnecting.";

    runtime
        .ui()
        .set_acquisition(|acquisition| acquisition.acquiring = true);
    let accepted = runtime.ui_begin(RemoteUiIntent::Disconnect).finish().await;
    assert_eq!(
        accepted.err().expect("accepted").to_string(),
        format!("Invalid input: {refusal}")
    );

    runtime
        .ui()
        .set_acquisition(|acquisition| acquisition.acquiring = false);
    let running = crate::remote_source::job_state::tests::running(
        "job-1",
        abb_remote_source_core::AcquisitionStage::Download,
    );
    runtime.ui().acquisition_changed(running.snapshot());
    let unsettled = runtime.ui_begin(RemoteUiIntent::Disconnect).finish().await;
    assert_eq!(
        unsettled.err().expect("unsettled").to_string(),
        format!("Invalid input: {refusal}")
    );

    runtime.ui().acquisition_changed(
        crate::remote_source::job_state::tests::imported("job-1", 1, Vec::new()).snapshot(),
    );
    runtime
        .ui_begin(RemoteUiIntent::Disconnect)
        .finish()
        .await
        .expect("a settled job lets Disconnect through");
}
