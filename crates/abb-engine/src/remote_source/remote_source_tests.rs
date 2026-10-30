use super::*;
use abb_remote_source_core::{acquisition_progress, AcquisitionStage};
use secrecy::SecretString;
use std::collections::HashMap;
use tempfile::TempDir;

#[derive(Default)]
struct TestSecretVault(Mutex<HashMap<String, SecretString>>);

impl vault::SecretVault for TestSecretVault {
    fn get_secret(&self, key: &str) -> Result<Option<SecretString>> {
        Ok(self.0.lock().expect("test vault").get(key).cloned())
    }

    fn set_secret(&self, key: &str, value: SecretString) -> Result<()> {
        self.0.lock().expect("test vault").insert(key.into(), value);
        Ok(())
    }

    fn delete_secret(&self, key: &str) -> Result<()> {
        self.0.lock().expect("test vault").remove(key);
        Ok(())
    }
}

pub(crate) fn test_runtime(root: &TempDir) -> RemoteSourceRuntime {
    test_runtime_with(root, None, None)
}

pub(super) fn test_runtime_with(
    root: &TempDir,
    vault: Option<Box<dyn vault::SecretVault>>,
    events: Option<Arc<dyn crate::EventSink>>,
) -> RemoteSourceRuntime {
    let host = crate::host::Host::new(
        events.unwrap_or_else(|| Arc::new(crate::DiscardEvents)),
        crate::power::PowerManager::default(),
    );
    let ui = Arc::new(Mutex::default());
    RemoteSourceRuntime {
        inner: Arc::new(RemoteSourceRuntimeInner {
            ui: ui.clone(),
            ui_host: host.clone(),
            power: crate::power::PowerManager::default(),
            config_dir: root.path().to_path_buf(),
            vault: vault.unwrap_or_else(|| Box::<TestSecretVault>::default()),
            lifecycle: RemoteAcquisitionLifecycle::new(
                RemoteSourceStaging::new(root.path().to_path_buf()),
                AaxcleanMaterializer::for_tests(),
                host,
                ui,
            ),
            pending_audible_auth: Mutex::new(None),
            indexer_adapter: ReqwestProwlarrAdapter::new(),
            tasks: crate::engine::EngineTasks::default(),
            handoff: OnceLock::new(),
            indexer_turn: tokio::sync::RwLock::new(()),
            sent_releases: Mutex::default(),
        }),
    }
}

fn acquisition_job(job_id: &str, status: types::RemoteAcquisitionStatus) -> RemoteAcquisitionJob {
    RemoteAcquisitionJob {
        job_id: job_id.to_string(),
        provider_id: RemoteProviderId::Audible,
        status,
        progress: acquisition_progress(AcquisitionStage::License, Some(0.0), None, None),
        materialized_files: Vec::new(),
        supplemental_assets: Vec::new(),
        diagnostics: Vec::new(),
        handoff: None,
    }
}

#[tokio::test]
async fn indexer_grab_lane_cannot_create_acquisition_jobs() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let result = runtime
        .start_acquisition(RemoteAcquisitionPlan {
            provider_id: RemoteProviderId::Indexer,
            selections: Vec::new(),
        })
        .await;
    assert!(result
        .expect_err("Indexer acquisition must be rejected")
        .to_string()
        .contains("Indexer grabs do not create acquisition jobs"));
    assert!(runtime
        .inner
        .lifecycle
        .jobs
        .lock()
        .expect("jobs lock")
        .is_empty());
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
        .insert(
            job_id.to_string(),
            acquisition_job(job_id, types::RemoteAcquisitionStatus::Acquiring),
        );
    assert_eq!(runtime.running_acquisitions().len(), 1);

    runtime.inner.lifecycle.mark_job_failed(
        job_id,
        RemoteProviderId::Audible,
        "failed at once".to_string(),
    );
    assert!(runtime.running_acquisitions().is_empty());
}

#[test]
fn cancelled_job_keeps_terminal_state_when_background_result_arrives() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let job_id = "remote-job-1";
    runtime
        .inner
        .lifecycle
        .jobs
        .lock()
        .expect("jobs lock")
        .insert(
            job_id.to_string(),
            acquisition_job(job_id, types::RemoteAcquisitionStatus::Acquiring),
        );

    runtime
        .cancel_acquisition(job_id)
        .expect("cancel acquisition");
    runtime.inner.lifecycle.update_job_progress(
        job_id,
        acquisition_progress(AcquisitionStage::Download, Some(0.5), Some(5), Some(10)),
    );
    runtime
        .inner
        .lifecycle
        .replace_job_if_active(acquisition_job(
            job_id,
            types::RemoteAcquisitionStatus::Validated,
        ));
    runtime.inner.lifecycle.mark_job_failed(
        job_id,
        RemoteProviderId::Audible,
        "late provider failure".to_string(),
    );

    let job = runtime.acquisition_status(job_id).expect("job status");
    assert_eq!(job.status, types::RemoteAcquisitionStatus::Cancelled);
    assert_eq!(job.progress.stage, AcquisitionStage::Cancelled);
    assert!(job.materialized_files.is_empty());
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
    let mut job = acquisition_job(job_id, types::RemoteAcquisitionStatus::Validated);
    job.materialized_files.push(types::MaterializedSourceFile {
        input_id: "input-1".to_string(),
        title_id: "B000000001".to_string(),
        path: root.path().join("book.m4b"),
        size_bytes: 42,
        sha256: "abc123".to_string(),
    });
    job.supplemental_assets.push(types::SupplementalAsset {
        asset_id: "asset-1".to_string(),
        input_id: "input-1".to_string(),
        title_id: "B000000001".to_string(),
        path: root.path().join("book.pdf"),
        file_name: "Supplemental PDF.pdf".to_string(),
        size_bytes: 24,
        sha256: "def456".to_string(),
    });
    runtime
        .inner
        .lifecycle
        .jobs
        .lock()
        .expect("jobs lock")
        .insert(job_id.to_string(), job);

    let answered = runtime
        .cancel_acquisition(job_id)
        .expect("cancel acquisition");

    assert_eq!(answered.status, types::RemoteAcquisitionStatus::Validated);
    let stored = runtime.acquisition_status(job_id).expect("job status");
    assert_eq!(stored.materialized_files.len(), 1);
    assert_eq!(stored.supplemental_assets.len(), 1);
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
        let mut job = acquisition_job(job_id, types::RemoteAcquisitionStatus::Validated);
        job.materialized_files.push(types::MaterializedSourceFile {
            input_id: "input-1".to_string(),
            title_id: "B000000001".to_string(),
            path: audio.clone(),
            size_bytes: 7,
            sha256: String::new(),
        });
        let answered = answer.clone();
        runtime.set_handoff(Arc::new(move |_| {
            let answered = answered.clone();
            Box::pin(async move { answered })
        }));
        runtime.inner.lifecycle.replace_job_if_active(job.clone());

        runtime.inner.lifecycle.hand_off(&runtime, job).await;

        let stored = runtime.acquisition_status(job_id).expect("job status");
        assert_eq!(stored.handoff, Some(answer));
        assert!(audio.exists());
        assert_eq!(stored.materialized_files.is_empty(), !kept);
    }
}

fn release(guid: &str) -> types::RemoteReleaseGrabRequest {
    types::RemoteReleaseGrabRequest {
        release: types::RemoteRelease {
            provider_id: RemoteProviderId::Indexer,
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
    runtime.sent_releases().insert((7, "release-1".to_string()));

    let again = runtime
        .grab_release(release("release-1"))
        .await
        .expect("answered without sending");
    assert!(again.accepted);
    assert!(runtime.grab_release(release("release-2")).await.is_err());
    assert!(runtime.grab_release(release("release-2")).await.is_err());

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
    let save = runtime
        .update_indexer_connection(
            serde_json::from_value(serde_json::json!({
                "baseUrl": "https://indexer.example",
            }))
            .expect("update"),
        )
        .await;
    assert!(
        matches!(save, Err(AppError::InvalidInput(message)) if message.contains("search or grab"))
    );
    drop(searching);
}

struct ReadCountingVault(Arc<std::sync::atomic::AtomicUsize>);

impl vault::SecretVault for ReadCountingVault {
    fn get_secret(&self, _key: &str) -> Result<Option<SecretString>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(None)
    }

    fn set_secret(&self, _key: &str, _value: SecretString) -> Result<()> {
        Ok(())
    }

    fn delete_secret(&self, _key: &str) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn no_credential_read_starts_once_the_engine_is_closing() {
    let root = TempDir::new().expect("temp root");
    std::fs::write(
        root.path().join("indexer.toml"),
        "url = \"http://indexer.test\"\ncategories = [3000]\n",
    )
    .expect("write connection");
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let runtime = test_runtime_with(
        &root,
        Some(Box::new(ReadCountingVault(reads.clone()))),
        None,
    );
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
fn cancelled_job_cleanup_removes_session_files_without_removing_status() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let job_id = "remote-job-2";
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
        .insert(
            job_id.to_string(),
            acquisition_job(job_id, types::RemoteAcquisitionStatus::Cancelled),
        );

    runtime
        .inner
        .lifecycle
        .cleanup_cancelled_job_session(job_id);

    assert!(!job_dir.exists());
    assert_eq!(
        runtime
            .acquisition_status(job_id)
            .expect("job status")
            .status,
        types::RemoteAcquisitionStatus::Cancelled
    );
}

#[test]
fn disconnecting_the_indexer_leaves_audible_downloads_and_records() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);
    let job_id = "audible-download";
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
            acquisition_job(job_id, types::RemoteAcquisitionStatus::Acquiring),
        );

    runtime
        .disconnect_credentials(RemoteProviderId::Indexer)
        .expect("indexer disconnect");

    assert!(job_dir.exists());
    assert!(runtime.acquisition_status(job_id).is_ok());
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
            acquisition_job(job_id, types::RemoteAcquisitionStatus::ImportedToFileList),
        );

    runtime.purge_session(job_id).expect("download removed");

    assert!(!job_dir.exists());
    assert!(runtime.acquisition_status(job_id).is_err());
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

    let mut materialized_job = acquisition_job(
        materialized_job_id,
        types::RemoteAcquisitionStatus::ImportedToFileList,
    );
    materialized_job
        .materialized_files
        .push(types::MaterializedSourceFile {
            input_id: "input-1".to_string(),
            title_id: "B000000001".to_string(),
            path: materialized_path.clone(),
            size_bytes: 5,
            sha256: "abc123".to_string(),
        });
    materialized_job.handoff = Some(AcquisitionHandoff::Imported { count: 1 });
    {
        let mut jobs = runtime.inner.lifecycle.jobs.lock().expect("jobs lock");
        jobs.insert(materialized_job_id.to_string(), materialized_job);
        jobs.insert(
            unmaterialized_job_id.to_string(),
            acquisition_job(
                unmaterialized_job_id,
                types::RemoteAcquisitionStatus::Failed,
            ),
        );
    }

    runtime
        .ui_begin(RemoteUiIntent::Disconnect {
            provider: ProviderId::Audible,
        })
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
    jobs.insert(
        bad_job_id.to_string(),
        acquisition_job(bad_job_id, types::RemoteAcquisitionStatus::Acquiring),
    );
    jobs.insert(
        good_job_id.to_string(),
        acquisition_job(good_job_id, types::RemoteAcquisitionStatus::Acquiring),
    );
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
