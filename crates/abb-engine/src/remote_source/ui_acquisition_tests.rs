//! How acquisition jobs reach the remote UI state and its published parts.
use super::*;

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
    #[derive(Default)]
    struct Updates(std::sync::Mutex<Vec<crate::session::SessionUpdate>>);
    impl crate::EventSink for Updates {
        fn emit(&self, event: crate::EngineEvent) {
            if let crate::EngineEvent::Session(update) = event {
                self.0.lock().expect("events").push(update);
            }
        }
    }
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
    let (remote, library, acquisition) = runtime.ui_parts(None);
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
    // The lifecycle's progress reducer advances only the acquisition part.
    runtime.inner.lifecycle.mark_job_failed(
        "progress-job",
        ProviderId::Audible,
        "fixture failure".into(),
    );
    let (remote, library, acquisition) = runtime.ui_parts(Some(since));
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
) -> crate::remote_source::AcquisitionJob {
    crate::remote_source::AcquisitionJob {
        job_id: "job-1".into(),
        provider_id: ProviderId::Audible,
        status: crate::remote_source::RemoteAcquisitionStatus::Failed,
        progress: abb_remote_source_core::acquisition_progress(
            abb_remote_source_core::AcquisitionStage::Failed,
            Some(1.0),
            None,
            None,
        ),
        materialized_files: Vec::new(),
        supplemental_assets: Vec::new(),
        diagnostics: vec![crate::remote_source::RemoteSourceDiagnostic {
            kind,
            title_id: Some("B000000001".into()),
            message: "Audible license request failed.".into(),
        }],
        handoff: None,
    }
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
    #[derive(Default)]
    struct Updates(std::sync::Mutex<Vec<crate::session::SessionUpdate>>);
    impl crate::EventSink for Updates {
        fn emit(&self, event: crate::EngineEvent) {
            if let crate::EngineEvent::Session(update) = event {
                self.0.lock().expect("updates").push(update);
            }
        }
    }
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
    let mut job = acquisition_failed_with(
        crate::remote_source::RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
    );
    job.status = crate::remote_source::RemoteAcquisitionStatus::Acquiring;
    job.diagnostics.clear();
    let job_id = job.job_id.clone();
    (runtime.inner.lifecycle.jobs.lock().expect("jobs")).insert(job_id.clone(), job);
    updates.0.lock().expect("updates").clear();
    // The first tick and each stage change publish at once, so no tick waits on the throttle.
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
    ui.disconnected(ProviderId::Audible);
    assert!(ui.acquisition.job.is_none());
    assert!(ui.acquisition.revision > before);
}
