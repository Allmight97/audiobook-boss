//! How acquisition jobs reach the remote UI state and its published parts.
use super::*;

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
    let (remote, library) = runtime.ui_parts(None);
    assert!(remote.expect("attached remote").acquiring);
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
    // The lifecycle's progress reducer advances only the remote part.
    runtime.inner.lifecycle.mark_job_failed(
        "progress-job",
        ProviderId::Audible,
        "fixture failure".into(),
    );
    let (remote, library) = runtime.ui_parts(Some(since));
    assert!(remote.is_some());
    assert!(library.is_none());
    let events = updates.0.lock().expect("progress events");
    assert_eq!(events.len(), 1);
    assert!(events[0]
        .remote
        .as_ref()
        .expect("progress remote")
        .acquisition
        .is_some());
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
