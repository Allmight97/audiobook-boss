use super::job_state::Registered;
use super::test_support::MemoryVault;
use super::*;
use abb_remote_source_core::{acquisition_progress, AcquisitionStage};
use tempfile::TempDir;

pub(crate) fn test_runtime(root: &TempDir) -> RemoteSourceRuntime {
    test_runtime_with(root, None, None)
}

pub(super) fn test_runtime_with(
    root: &TempDir,
    vault: Option<Box<dyn vault::SecretVault>>,
    events: Option<Arc<dyn crate::EventSink>>,
) -> RemoteSourceRuntime {
    let (settings, _, _) = crate::app_settings::SettingsRuntime::start(
        root.path().to_path_buf(),
        crate::power::PowerManager::default(),
    );
    build_runtime(root, settings, vault, events, None)
}

pub(crate) fn test_runtime_over(
    root: &TempDir,
    settings: crate::app_settings::SettingsRuntime,
) -> RemoteSourceRuntime {
    build_runtime(root, settings, None, None, None)
}

/// A runtime whose acquisitions come from `acquire` and whose vault holds
/// nothing, so no test reaches the provider or the keychain.
pub(crate) fn test_runtime_acquiring(
    root: &TempDir,
    settings: crate::app_settings::SettingsRuntime,
    acquire: AcquireTitles,
) -> RemoteSourceRuntime {
    build_runtime(root, settings, None, None, Some(acquire))
}

/// The acquisition jobs the runtime still keeps a record of.
pub(crate) fn recorded_job_ids(runtime: &RemoteSourceRuntime) -> Vec<String> {
    let jobs = runtime.inner.lifecycle.jobs.lock().expect("job records");
    jobs.keys().cloned().collect()
}

/// Offers one available Audible title in the library, as a loaded library does.
pub(crate) fn offer_library_title(runtime: &RemoteSourceRuntime, title_id: &str) {
    runtime.ui().library_loaded(vec![RemoteTitle {
        provider_id: RemoteProviderId::Audible,
        title_id: title_id.into(),
        title: title_id.into(),
        authors: Vec::new(),
        narrators: Vec::new(),
        cover: None,
        supplemental_pdf_available: false,
        availability: RemoteTitleAvailability {
            status: RemoteTitleAvailabilityStatus::Available,
            acquirable: true,
            label: "Available".into(),
            detail: None,
        },
    }]);
}

fn build_runtime(
    root: &TempDir,
    settings: crate::app_settings::SettingsRuntime,
    vault: Option<Box<dyn vault::SecretVault>>,
    events: Option<Arc<dyn crate::EventSink>>,
    acquire: Option<AcquireTitles>,
) -> RemoteSourceRuntime {
    let host = crate::host::Host::new(
        events.unwrap_or_else(|| Arc::new(crate::DiscardEvents)),
        crate::power::PowerManager::default(),
    );
    let ui = Arc::new(Mutex::new(ui::UiState::seeded(
        &settings.indexer_connection(),
    )));
    RemoteSourceRuntime {
        inner: Arc::new(RemoteSourceRuntimeInner {
            ui: ui.clone(),
            ui_host: host.clone(),
            power: crate::power::PowerManager::default(),
            settings,
            vault: vault.unwrap_or_else(|| Box::<MemoryVault>::default()),
            lifecycle: RemoteAcquisitionLifecycle::new(
                RemoteSourceStaging::new(root.path().to_path_buf()),
                host,
                ui,
                acquire.unwrap_or_else(session_lifecycle::audible_acquire),
            ),
            indexer_adapter: ReqwestProwlarrAdapter::new(),
            tasks: crate::engine_tasks::EngineTasks::default(),
            handoff: OnceLock::new(),
            indexer_turn: Arc::default(),
            sent_releases: Mutex::default(),
        }),
    }
}

fn register(runtime: &RemoteSourceRuntime, job_id: &str) {
    runtime
        .inner
        .lifecycle
        .jobs
        .lock()
        .expect("jobs lock")
        .insert(job_id.to_string(), Registered::new(job_id));
}

fn one_file_outcome(path: PathBuf) -> AcquisitionOutcome {
    AcquisitionOutcome {
        files: vec![types::MaterializedSourceFile {
            input_id: "input-1".to_string(),
            title_id: "B000000001".to_string(),
            path,
            size_bytes: 7,
            sha256: String::new(),
        }],
        ..AcquisitionOutcome::default()
    }
}

fn handoff_job(runtime: &RemoteSourceRuntime, job_id: &str) -> Option<types::AcquisitionJob> {
    runtime
        .inner
        .lifecycle
        .jobs
        .lock()
        .expect("jobs lock")
        .get(job_id)?
        .job_for_handoff()
}

fn published_job(runtime: &RemoteSourceRuntime) -> AcquisitionSnapshot {
    let mut update = crate::session::SessionUpdate::default();
    runtime.add_ui_parts(&mut update, None);
    *update
        .remote_acquisition
        .expect("acquisition part")
        .job
        .expect("published job")
}

#[test]
fn read_handoff_url_reads_trimmed_non_symlink_file() {
    let root = TempDir::new().expect("temp root");
    let path = root.path().join("handoff.txt");
    std::fs::write(&path, " https://example.test/callback?code=abc \n").expect("write handoff");

    let url = read_handoff_url(Some(path)).expect("read handoff");

    assert_eq!(url, "https://example.test/callback?code=abc");
}

#[test]
fn read_handoff_url_accepts_direct_final_url_input() {
    let url = read_handoff_url(Some(PathBuf::from(
        " https://example.test/callback?code=abc&state=xyz ",
    )))
    .expect("read direct handoff URL");

    assert_eq!(url, "https://example.test/callback?code=abc&state=xyz");
}

#[test]
fn read_handoff_url_rejects_empty_file() {
    let root = TempDir::new().expect("temp root");
    let path = root.path().join("handoff.txt");
    std::fs::write(&path, " \n").expect("write handoff");

    let error = read_handoff_url(Some(path)).expect_err("empty handoff should fail");

    assert!(error.to_string().contains("handoff file was empty"));
}

#[cfg(unix)]
#[test]
fn read_handoff_url_rejects_symlink() {
    let root = TempDir::new().expect("temp root");
    let target = root.path().join("target.txt");
    let link = root.path().join("handoff.txt");
    std::fs::write(&target, "https://example.test/callback?code=abc").expect("write target");
    std::os::unix::fs::symlink(&target, &link).expect("create symlink");

    let error = read_handoff_url(Some(link)).expect_err("symlink should fail");

    assert!(error.to_string().contains("must not be a symlink"));
}

#[tokio::test]
async fn a_download_counts_as_running_until_its_job_ends() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let job_id = "remote-job-1";
    runtime
        .inner
        .lifecycle
        .jobs
        .lock()
        .expect("jobs lock")
        .insert(job_id.to_string(), Registered::new(job_id));
    assert_eq!(runtime.running_acquisitions().len(), 1);

    runtime.inner.lifecycle.apply_outcome(
        job_id,
        AcquisitionOutcome::failed(
            types::RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
            "failed at once".to_string(),
        ),
    );
    assert!(runtime.running_acquisitions().is_empty());
}

#[tokio::test]
async fn cancelled_job_keeps_terminal_state_when_background_result_arrives() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let job_id = "remote-job-1";
    let job_dir = runtime
        .inner
        .lifecycle
        .staging
        .create_job_dir(job_id)
        .expect("job dir");
    std::fs::write(job_dir.join("download.m4b"), b"payload").expect("write staged file");
    runtime
        .inner
        .lifecycle
        .jobs
        .lock()
        .expect("jobs lock")
        .insert(job_id.to_string(), Registered::new(job_id));

    runtime
        .cancel_acquisition(job_id)
        .expect("cancel acquisition");
    runtime.inner.tasks.close();
    runtime.inner.tasks.wait().await;
    runtime.inner.lifecycle.update_job_progress(
        job_id,
        acquisition_progress(AcquisitionStage::Download, Some(0.5), Some(5), Some(10)),
    );
    runtime
        .inner
        .lifecycle
        .apply_outcome(job_id, one_file_outcome(job_dir.join("download.m4b")));
    runtime.inner.lifecycle.apply_outcome(
        job_id,
        AcquisitionOutcome::failed(
            types::RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
            "late provider failure".to_string(),
        ),
    );

    let job = published_job(&runtime);
    assert!(!job_dir.exists());
    assert_eq!(job.status, types::RemoteAcquisitionStatus::Cancelled);
    assert_eq!(job.progress.stage, AcquisitionStage::Cancelled);
    assert!(job
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == types::RemoteAcquisitionFailureKind::Cancelled));
}

#[test]
fn cancelling_a_finished_acquisition_keeps_its_files_for_the_session() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let job_id = "remote-job-handoff";
    let mut outcome = one_file_outcome(root.path().join("book.m4b"));
    outcome.assets.push(types::SupplementalAsset {
        asset_id: "asset-1".to_string(),
        input_id: "input-1".to_string(),
        title_id: "B000000001".to_string(),
        path: root.path().join("book.pdf"),
        file_name: "Supplemental PDF.pdf".to_string(),
        size_bytes: 24,
        sha256: "def456".to_string(),
    });
    register(&runtime, job_id);
    runtime.inner.lifecycle.apply_outcome(job_id, outcome);

    let answered = runtime
        .cancel_acquisition(job_id)
        .expect("cancel acquisition");

    assert_eq!(
        answered.job.status,
        types::RemoteAcquisitionStatus::Validated
    );
    let kept = handoff_job(&runtime, job_id).expect("the files still wait for the session");
    assert_eq!(kept.materialized_files.len(), 1);
    assert_eq!(kept.supplemental_assets.len(), 1);
}

#[tokio::test]
async fn a_finished_job_records_its_handoff() {
    for (answer, kept) in [
        (AcquisitionHandoff::Imported { count: 1 }, true),
        (
            AcquisitionHandoff::Removed {
                reason: HandoffRefusal::NothingAdded,
            },
            false,
        ),
    ] {
        let root = TempDir::new().expect("temp root");
        let runtime = test_runtime(&root);
        let job_id = "remote-job-handoff";
        let job_dir = runtime
            .inner
            .lifecycle
            .staging
            .create_job_dir(job_id)
            .expect("job dir");
        let audio = job_dir.join("book.m4b");
        std::fs::write(&audio, b"payload").expect("write staged file");
        register(&runtime, job_id);
        runtime
            .inner
            .lifecycle
            .apply_outcome(job_id, one_file_outcome(audio.clone()));
        let answered = answer.clone();
        runtime.set_handoff(Arc::new(move |_| {
            let answered = answered.clone();
            Box::pin(async move { answered })
        }));

        runtime.inner.lifecycle.hand_off(&runtime, job_id).await;

        let stored = published_job(&runtime);
        assert_eq!(stored.handoff, Some(answer));
        assert!(audio.exists());
        let held = runtime
            .inner
            .lifecycle
            .jobs
            .lock()
            .expect("jobs lock")
            .get(job_id)
            .is_some_and(Registered::holds_staged_files);
        assert_eq!(held, kept);
    }
}

fn release(guid: &str) -> types::RemoteReleaseGrabRequest {
    types::RemoteReleaseGrabRequest {
        release: types::RemoteRelease {
            provider_id: RemoteProviderId::Indexer,
            key: types::release_key(7, guid),
            guid: guid.to_string(),
            indexer_id: 7,
            title: "Example".to_string(),
            indexer: "Example Indexer".to_string(),
            detail_url: None,
            size_bytes: 1,
            protocol: types::RemoteReleaseProtocol::Torrent,
            seeders: None,
            categories: Vec::new(),
        },
    }
}

#[tokio::test]
async fn a_release_sent_in_this_search_is_not_sent_again() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    runtime
        .sent_releases()
        .insert(types::release_key(7, "release-1"));

    let again = runtime
        .grab_release(release("release-1"))
        .await
        .expect("answered without sending");
    assert!(again.accepted);
    assert!(runtime.grab_release(release("release-2")).await.is_err());
    assert!(runtime.grab_release(release("release-2")).await.is_err());

    #[expect(
        clippy::let_underscore_must_use,
        reason = "the search only refreshes the state this test inspects"
    )]
    let _ = runtime
        .search_releases(types::RemoteReleaseSearchRequest {
            author: None,
            title: Some("Example".to_string()),
            query: None,
        })
        .await;
    assert!(runtime.grab_release(release("release-1")).await.is_err());
}

#[tokio::test]
async fn a_connection_save_and_a_search_or_grab_refuse_each_other() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let saving = runtime.inner.indexer_turn.write().await;

    let refused = runtime.grab_release(release("release-1")).await;
    assert!(
        matches!(refused, Err(AppError::InvalidInput(message)) if message.contains("connection save"))
    );
    drop(saving);

    let searching = runtime.inner.indexer_turn.read().await;
    let revision = runtime.inner.settings.snapshot().await.revision;
    let save = runtime
        .ui_begin(RemoteUiIntent::SaveConnection)
        .finish()
        .await;
    assert!(
        matches!(save, Err(AppError::InvalidInput(message)) if message.contains("search, grab, or save"))
    );
    assert_eq!(runtime.inner.settings.snapshot().await.revision, revision);
    drop(searching);
}

#[tokio::test]
async fn no_credential_read_starts_once_the_engine_is_closing() {
    let root = TempDir::new().expect("temp root");
    std::fs::write(
        root.path().join("settings.toml"),
        "[indexer]\nurl = \"http://indexer.test\"\ncategories = [3000]\n",
    )
    .expect("write connection");
    let vault = MemoryVault::default();
    let reads = vault.read_counter();
    let runtime = test_runtime_with(&root, Some(Box::new(vault)), None);
    runtime.inner.tasks.close();

    let result = runtime
        .search_releases(types::RemoteReleaseSearchRequest {
            author: None,
            title: Some("Example".to_string()),
            query: None,
        })
        .await;

    assert!(matches!(result, Err(AppError::Closing)));
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn releasing_a_download_forgets_its_job() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let job_id = "released-download";
    let job_dir = runtime
        .inner
        .lifecycle
        .staging
        .create_job_dir(job_id)
        .expect("job dir");
    runtime
        .inner
        .lifecycle
        .jobs
        .lock()
        .expect("jobs lock")
        .insert(
            job_id.to_string(),
            job_state::tests::imported(job_id, 1, Vec::new()),
        );

    runtime.purge_session(job_id).expect("download removed");

    assert!(!job_dir.exists());
    assert!(!runtime
        .inner
        .lifecycle
        .jobs
        .lock()
        .expect("jobs lock")
        .contains_key(job_id));
}

#[tokio::test]
async fn logout_keeps_materialized_handoff_sessions_but_purges_unmaterialized_sessions() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let materialized_job_id = "remote-job-materialized";
    let unmaterialized_job_id = "remote-job-unmaterialized";
    let materialized_job_dir = runtime
        .inner
        .lifecycle
        .staging
        .create_job_dir(materialized_job_id)
        .expect("materialized job dir");
    let unmaterialized_job_dir = runtime
        .inner
        .lifecycle
        .staging
        .create_job_dir(unmaterialized_job_id)
        .expect("unmaterialized job dir");
    let materialized_path = materialized_job_dir.join("book.m4b");
    std::fs::write(&materialized_path, b"audio").expect("write materialized file");
    std::fs::write(unmaterialized_job_dir.join("source.aax"), b"protected")
        .expect("write protected source");

    let mut materialized_job = Registered::new(materialized_job_id);
    materialized_job.finish(one_file_outcome(materialized_path.clone()));
    materialized_job.handoff_done(AcquisitionHandoff::Imported { count: 1 });
    {
        let mut jobs = runtime.inner.lifecycle.jobs.lock().expect("jobs lock");
        jobs.insert(materialized_job_id.to_string(), materialized_job);
        jobs.insert(
            unmaterialized_job_id.to_string(),
            job_state::tests::failed(unmaterialized_job_id, Vec::new()),
        );
    }

    runtime
        .ui_begin(RemoteUiIntent::Disconnect)
        .finish()
        .await
        .expect("logout should preserve handoff session");

    assert!(materialized_path.exists());
    assert!(materialized_job_dir.exists());
    assert!(!unmaterialized_job_dir.exists());
    assert!(runtime
        .inner
        .lifecycle
        .jobs
        .lock()
        .expect("jobs lock")
        .is_empty());
}

#[cfg(unix)]
#[test]
fn logout_cleanup_attempts_remaining_sessions_after_purge_failure() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let bad_job_id = "a-broken-session";
    let good_job_id = "z-good-session";
    let good_job_dir = runtime
        .inner
        .lifecycle
        .staging
        .create_job_dir(good_job_id)
        .expect("good job dir");
    std::fs::write(good_job_dir.join("source.aax"), b"protected").expect("write protected source");

    let outside_target = root.path().join("outside-target");
    std::fs::create_dir_all(&outside_target).expect("outside target");
    std::fs::create_dir_all(runtime.inner.lifecycle.staging.session_root()).expect("session root");
    std::os::unix::fs::symlink(
        &outside_target,
        runtime
            .inner
            .lifecycle
            .staging
            .session_root()
            .join(bad_job_id),
    )
    .expect("create bad session symlink");

    let mut jobs = runtime.inner.lifecycle.jobs.lock().expect("jobs lock");
    jobs.insert(bad_job_id.to_string(), Registered::new(bad_job_id));
    jobs.insert(good_job_id.to_string(), Registered::new(good_job_id));
    drop(jobs);

    let error = runtime
        .inner
        .lifecycle
        .cleanup_logout_sessions_without_handoff(RemoteProviderId::Audible)
        .expect_err("bad session should report cleanup error");

    assert!(error
        .to_string()
        .contains("Refusing to cleanup path outside"));
    assert!(
        !good_job_dir.exists(),
        "cleanup should continue after the bad session and remove later stale sessions"
    );
}
