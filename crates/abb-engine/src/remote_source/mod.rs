use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};

mod cancellation;
pub(crate) mod materializer;
mod providers;
mod scoped_output;
mod session_lifecycle;
mod staging;
mod types;
mod ui;

/// How long Amazon may take to register a completed sign-in.
const AUTH_REGISTRATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
#[cfg(test)]
pub(crate) use providers::audible::{run_protected_materialization, ProtectedMaterializationRun};
pub use ui::{
    IndexerDraftSnapshot, IndexerWorkSnapshot, ReleaseGrabSnapshot, ReleaseGrabStatus,
    RemoteAuthStatus, RemoteDraftStatus, RemoteLibrarySnapshot, RemoteUiIntent, RemoteUiSnapshot,
};
pub(crate) use ui::{RemoteUiResult, RemoteUiRun};
mod vault;

use materializer::AaxcleanMaterializer;
use providers::audible::{AudibleProvider, PendingAudibleAuth};
use providers::indexer::{IndexerProvider, ReqwestProwlarrAdapter};
use session_lifecycle::RemoteAcquisitionLifecycle;
use staging::RemoteSourceStaging;
use types::{
    AcquisitionJob as RemoteAcquisitionJob, AcquisitionPlan as RemoteAcquisitionPlan,
    ProviderId as RemoteProviderId, RemoteAuthCompletionRequest as RemoteAuthCompletion,
    RemoteAuthStartResponse as RemoteAuthStart, RemoteSourceAccountState as RemoteAccountState,
};
use vault::{KeyringSecretVault, SecretVault};

use crate::errors::{AppError, Result};

/// What the remote-source owner needs from the engine that constructs it.
pub(crate) struct RemoteSourceConfig {
    pub(crate) cache_dir: PathBuf,
    pub(crate) config_dir: PathBuf,
    /// Scopes stored credentials; a different identifier is a different vault.
    pub(crate) app_identifier: String,
    pub(crate) power: crate::power::PowerManager,
    /// Where acquisition progress is published.
    pub(crate) host: crate::host::Host,
    /// Host-supplied helper location; `None` resolves it beside the executable.
    pub(crate) aaxclean_helper: Option<PathBuf>,
    /// The engine's background tasks; acquisitions run here.
    pub(crate) tasks: crate::engine::EngineTasks,
}

/// Imports a finished acquisition's files into the session.
pub(crate) type Handoff = Arc<
    dyn Fn(RemoteAcquisitionJob) -> Pin<Box<dyn Future<Output = AcquisitionHandoff> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub(crate) struct RemoteSourceRuntime {
    inner: Arc<RemoteSourceRuntimeInner>,
}

struct RemoteSourceRuntimeInner {
    ui: Arc<Mutex<ui::UiState>>,
    ui_host: crate::host::Host,
    power: crate::power::PowerManager,
    config_dir: PathBuf,
    vault: Box<dyn SecretVault>,
    lifecycle: RemoteAcquisitionLifecycle,
    pending_audible_auth: Mutex<Option<PendingAudibleAuth>>,
    indexer_adapter: ReqwestProwlarrAdapter,
    tasks: crate::engine::EngineTasks,
    handoff: OnceLock<Handoff>,
    /// A connection save holds this exclusively; searches and grabs share it.
    indexer_turn: tokio::sync::RwLock<()>,
    /// Releases sent since the last search, by `(indexer_id, guid)`.
    sent_releases: Mutex<std::collections::HashSet<(i64, String)>>,
}

impl RemoteSourceRuntime {
    pub(crate) fn new(config: RemoteSourceConfig) -> Result<Self> {
        let ui = Arc::new(Mutex::default());
        Ok(Self {
            inner: Arc::new(RemoteSourceRuntimeInner {
                ui: ui.clone(),
                ui_host: config.host.clone(),
                power: config.power,
                config_dir: config.config_dir,
                vault: Box::new(KeyringSecretVault::for_app_identifier(
                    &config.app_identifier,
                )),
                lifecycle: RemoteAcquisitionLifecycle::new(
                    RemoteSourceStaging::new(config.cache_dir),
                    AaxcleanMaterializer::new_for_helper(config.aaxclean_helper),
                    config.host,
                    ui,
                ),
                pending_audible_auth: Mutex::new(None),
                indexer_adapter: ReqwestProwlarrAdapter::new(),
                tasks: config.tasks,
                handoff: OnceLock::new(),
                indexer_turn: tokio::sync::RwLock::new(()),
                sent_releases: Mutex::default(),
            }),
        })
    }

    /// Sets where finished acquisitions hand their files. Set once, by the
    /// engine, after the session exists.
    pub(crate) fn set_handoff(&self, handoff: Handoff) {
        let _ = self.inner.handoff.set(handoff);
    }

    /// Where staged downloads live. A source file under this root is
    /// temporary: it is removed once its title has been exported.
    pub(crate) fn staging_root(&self) -> PathBuf {
        self.inner.lifecycle.staging.session_root()
    }

    /// Stops every acquisition; their staged files are removed at the next
    /// start.
    pub(crate) fn abort_acquisitions(&self) {
        self.inner.lifecycle.abort_all_acquisition_tasks();
    }

    /// Downloads `abort_acquisitions` would stop.
    pub(crate) fn running_acquisitions(&self) -> Vec<String> {
        self.inner.lifecycle.running_acquisitions()
    }

    pub(crate) fn cleanup_abandoned_sessions(&self) -> Result<()> {
        self.inner.lifecycle.cleanup_abandoned_sessions()
    }

    pub(crate) fn account_state(
        &self,
        provider_id: RemoteProviderId,
    ) -> Result<RemoteAccountState> {
        match provider_id {
            RemoteProviderId::Audible => AudibleProvider::account_state(self.inner.vault.as_ref()),
            RemoteProviderId::Indexer => {
                IndexerProvider::account_state(&self.inner.config_dir, self.inner.vault.as_ref())
            }
        }
    }

    pub(crate) fn start_auth(&self, provider_id: RemoteProviderId) -> Result<RemoteAuthStart> {
        match provider_id {
            RemoteProviderId::Audible => {
                let (authorization_url, pending) = AudibleProvider::start_auth()?;
                *self.inner.pending_audible_auth.lock().map_err(|_| {
                    AppError::General("Remote auth state lock failed".to_string())
                })? = Some(pending);
                Ok(RemoteAuthStart {
                    provider_id,
                    authorization_url,
                    handoff_path_hint:
                        "Paste the final Amazon URL or use $TMPDIR/abb-audible-auth-response-url.txt / ABB_AUDIBLE_AUTH_RESPONSE_URL_PATH"
                            .to_string(),
                    message: "Open the authorization URL externally, sign in, then paste the final Amazon URL or save it to a local handoff file and complete auth from ABB.".to_string(),
                })
            }
            RemoteProviderId::Indexer => Err(AppError::InvalidInput(
                "Indexer uses Settings URL and API key configuration instead of browser auth."
                    .to_string(),
            )),
        }
    }

    pub(crate) async fn complete_auth(
        &self,
        request: RemoteAuthCompletion,
    ) -> Result<RemoteAccountState> {
        match request.provider_id {
            RemoteProviderId::Audible => {
                // A handoff path that can't be read keeps the sign-in, so the
                // user can correct the path without a new browser round trip.
                let response_url = read_handoff_url(request.response_url_handoff_path)?;
                let pending = self
                    .inner
                    .pending_audible_auth
                    .lock()
                    .map_err(|_| AppError::General("Remote auth state lock failed".to_string()))?
                    .take()
                    .ok_or_else(|| {
                        AppError::InvalidInput(
                            "Start Audible auth before completing the handoff.".to_string(),
                        )
                    })?;
                // Registration has no timeout of its own; a stalled Amazon
                // endpoint must not hold the remote surface until quit.
                let registration = async {
                    tokio::time::timeout(
                        AUTH_REGISTRATION_TIMEOUT,
                        AudibleProvider::register_auth(pending, &response_url),
                    )
                    .await
                    .map_err(|_| {
                        AppError::General(
                            "Amazon did not finish the sign-in in time. Connect again.".into(),
                        )
                    })?
                };
                let auth = self.inner.tasks.until_closing(registration).await?;
                // Registration is cancellable; the accepted durable write is
                // awaited by EngineTasks and must finish once started.
                let runtime = self.clone();
                let write = self.inner.tasks.admit(|| {
                    #[expect(clippy::disallowed_methods, reason = "joined by complete_auth")]
                    let write = tokio::task::spawn_blocking(move || {
                        AudibleProvider::persist_auth(runtime.inner.vault.as_ref(), &auth)
                    });
                    write
                })?;
                write.await.map_err(|_| {
                    AppError::General("Audible credential persistence failed.".into())
                })?
            }
            RemoteProviderId::Indexer => Err(AppError::InvalidInput(
                "Indexer uses Settings URL and API key configuration instead of browser auth."
                    .to_string(),
            )),
        }
    }

    fn disconnect_credentials(&self, provider_id: RemoteProviderId) -> Result<RemoteAccountState> {
        match provider_id {
            RemoteProviderId::Audible => AudibleProvider::logout(self.inner.vault.as_ref())?,
            RemoteProviderId::Indexer => {
                IndexerProvider::logout(&self.inner.config_dir, self.inner.vault.as_ref())?
            }
        }
        // Once credentials are removed, reflect disconnection even if a
        // staging cleanup fails. Only this provider's jobs are cleaned up;
        // startup's abandoned-session sweep is the cleanup backstop.
        let account = self.account_state(provider_id);
        {
            let mut state = self.ui();
            state.disconnected(provider_id);
            state.account_disconnected(account.as_ref().ok().cloned());
        }
        if provider_id == RemoteProviderId::Audible {
            *self
                .inner
                .pending_audible_auth
                .lock()
                .map_err(|_| AppError::General("Remote auth state lock failed".into()))? = None;
        }
        self.inner
            .lifecycle
            .cleanup_logout_sessions_without_handoff(provider_id)?;
        self.inner.lifecycle.clear_jobs(provider_id)?;
        account
    }

    /// Searches the indexer. A new search forgets which releases were sent.
    pub(crate) async fn search_releases(
        &self,
        request: types::RemoteReleaseSearchRequest,
    ) -> Result<types::RemoteReleaseSearchResponse> {
        let _turn = self.indexer_turn()?;
        self.forget_sent_releases();
        IndexerProvider::search_releases(
            &self.inner.config_dir,
            self.inner.vault.as_ref(),
            &self.inner.indexer_adapter,
            request,
        )
        .await
    }

    /// Sends a release to the downloader, once per search: a release already
    /// sent since the last search is not sent again.
    pub(crate) async fn grab_release(
        &self,
        request: types::RemoteReleaseGrabRequest,
    ) -> Result<types::RemoteReleaseGrabResponse> {
        let _turn = self.indexer_turn()?;
        let key = (request.release.indexer_id, request.release.guid.clone());
        // Claimed before sending, so an overlapping grab of the same release
        // does not send it too; released again if the send is not accepted.
        if !self.sent_releases().insert(key.clone()) {
            return Ok(types::RemoteReleaseGrabResponse {
                provider_id: RemoteProviderId::Indexer,
                accepted: true,
                message: "Already sent to downloader.".to_string(),
                diagnostics: Vec::new(),
            });
        }
        let _active_work = self.inner.power.begin();
        let response = IndexerProvider::grab_release(
            &self.inner.config_dir,
            self.inner.vault.as_ref(),
            &self.inner.indexer_adapter,
            request,
        )
        .await;
        if !response.as_ref().is_ok_and(|response| response.accepted) {
            self.sent_releases().remove(&key);
        }
        response
    }

    /// A share of the indexer turn, refused while a connection save runs.
    fn indexer_turn(&self) -> Result<tokio::sync::RwLockReadGuard<'_, ()>> {
        self.inner.indexer_turn.try_read().map_err(|_| {
            AppError::InvalidInput(
                "Wait for the Indexer connection save to finish before searching or grabbing."
                    .to_string(),
            )
        })
    }

    fn sent_releases(&self) -> std::sync::MutexGuard<'_, std::collections::HashSet<(i64, String)>> {
        self.inner
            .sent_releases
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn forget_sent_releases(&self) {
        self.sent_releases().clear();
    }

    // Reading or writing the connection can wait on an OS credential prompt,
    // so both run on a blocking thread and never stall the caller's executor.
    pub(crate) async fn get_indexer_connection(&self) -> Result<types::RemoteIndexerConnection> {
        let runtime = self.clone();
        #[expect(
            clippy::disallowed_methods,
            reason = "joined by get_indexer_connection"
        )]
        let read = tokio::task::spawn_blocking(move || {
            IndexerProvider::get_connection(&runtime.inner.config_dir, runtime.inner.vault.as_ref())
        });
        read.await
            .map_err(|error| AppError::General(error.to_string()))?
    }

    /// Saves the connection. Refused while a search or grab runs; releases
    /// found with the previous connection must be searched again.
    pub(crate) async fn update_indexer_connection(
        &self,
        update: types::RemoteIndexerConnectionUpdate,
    ) -> Result<types::RemoteIndexerConnection> {
        let _turn = self.inner.indexer_turn.try_write().map_err(|_| {
            AppError::InvalidInput(
                "Wait for the current search or grab to finish before saving the Indexer connection."
                    .to_string(),
            )
        })?;
        self.forget_sent_releases();
        let runtime = self.clone();
        #[expect(
            clippy::disallowed_methods,
            reason = "joined by update_indexer_connection"
        )]
        let write = tokio::task::spawn_blocking(move || {
            IndexerProvider::update_connection(
                &runtime.inner.config_dir,
                runtime.inner.vault.as_ref(),
                update,
            )
        });
        write
            .await
            .map_err(|error| AppError::General(error.to_string()))?
    }

    pub(crate) async fn test_indexer_connection(
        &self,
        update: types::RemoteIndexerConnectionUpdate,
    ) -> Result<types::RemoteIndexerConnectionTestResult> {
        IndexerProvider::test_connection(
            &self.inner.config_dir,
            self.inner.vault.as_ref(),
            &self.inner.indexer_adapter,
            update,
        )
        .await
    }

    pub(crate) async fn start_acquisition(
        &self,
        plan: RemoteAcquisitionPlan,
    ) -> Result<RemoteAcquisitionJob> {
        if plan.provider_id == RemoteProviderId::Indexer {
            return Err(AppError::InvalidInput(
                "Indexer grabs do not create acquisition jobs. Use grab release instead."
                    .to_string(),
            ));
        }
        self.inner
            .lifecycle
            .start_acquisition(self.clone(), plan)
            .await
    }
}

fn read_handoff_url(path: Option<PathBuf>) -> Result<String> {
    let path = match path {
        Some(path) => path,
        None => std::env::var("ABB_AUDIBLE_AUTH_RESPONSE_URL_PATH")
            .map(PathBuf::from)
            .map_err(|_| {
                AppError::InvalidInput(
                    "Provide a handoff path or set ABB_AUDIBLE_AUTH_RESPONSE_URL_PATH.".to_string(),
                )
            })?,
    };
    if let Some(response_url) = direct_response_url_from_input(&path) {
        return Ok(response_url);
    }
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() {
        return Err(AppError::InvalidInput(
            "Audible auth handoff path must not be a symlink.".to_string(),
        ));
    }
    let content = fs::read_to_string(path)?;
    let response_url = content.trim().to_string();
    if response_url.is_empty() {
        return Err(AppError::InvalidInput(
            "Audible auth handoff file was empty.".to_string(),
        ));
    }
    Ok(response_url)
}

fn direct_response_url_from_input(path: &Path) -> Option<String> {
    let input = path.as_os_str().to_str()?.trim();
    if input.starts_with("https://") || input.starts_with("http://") {
        return Some(input.to_string());
    }
    None
}

pub use types::*;

#[cfg(test)]
pub(crate) mod tests {
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

    fn acquisition_job(
        job_id: &str,
        status: types::RemoteAcquisitionStatus,
    ) -> RemoteAcquisitionJob {
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

        // A job that ended counts no longer, whatever task handle remains.
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

        // The engine may already be importing these files.
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
            // Removing an unimported download is the session's (`staged.rs`).
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

        // No connection is saved, so a real send would fail.
        let again = runtime
            .grab_release(release("release-1"))
            .await
            .expect("answered without sending");
        assert!(again.accepted);
        assert!(runtime.grab_release(release("release-2")).await.is_err());
        // A send that failed releases its claim, so it can be retried.
        assert!(runtime.grab_release(release("release-2")).await.is_err());

        // A new search, or a saved connection, forgets what was sent.
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
        std::fs::write(good_job_dir.join("source.aax"), b"protected")
            .expect("write protected source");

        let outside_target = root.path().join("outside-target");
        std::fs::create_dir_all(&outside_target).expect("outside target");
        std::fs::create_dir_all(runtime.inner.lifecycle.staging.session_root())
            .expect("session root");
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
}
