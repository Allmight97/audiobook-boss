use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};

mod cancellation;
mod materializer;
mod providers;
mod scoped_output;
mod session_lifecycle;
mod staging;
mod types;
mod vault;

use materializer::AaxcleanMaterializer;
use providers::audible::{AudibleProvider, PendingAudibleAuth};
use providers::indexer::{IndexerProvider, ReqwestProwlarrAdapter};
use session_lifecycle::RemoteAcquisitionLifecycle;
use staging::RemoteSourceStaging;
use types::{
    AcquisitionJob as RemoteAcquisitionJob, AcquisitionPlan as RemoteAcquisitionPlan,
    ProviderId as RemoteProviderId, RemoteAuthCompletionRequest as RemoteAuthCompletion,
    RemoteAuthStartResponse as RemoteAuthStart, RemoteLibraryResponse as RemoteLibrary,
    RemoteSourceAccountState as RemoteAccountState,
    RemoteSourceProviderCapabilities as RemoteProviderCapabilities,
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
    pub(crate) tasks: tokio_util::task::TaskTracker,
}

/// Imports a finished acquisition's files into the session.
pub(crate) type Handoff = Arc<
    dyn Fn(RemoteAcquisitionJob) -> Pin<Box<dyn Future<Output = AcquisitionHandoff> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct RemoteSourceRuntime {
    inner: Arc<RemoteSourceRuntimeInner>,
}

struct RemoteSourceRuntimeInner {
    power: crate::power::PowerManager,
    config_dir: PathBuf,
    vault: Box<dyn SecretVault>,
    lifecycle: RemoteAcquisitionLifecycle,
    pending_audible_auth: Mutex<Option<PendingAudibleAuth>>,
    indexer_adapter: ReqwestProwlarrAdapter,
    tasks: tokio_util::task::TaskTracker,
    handoff: OnceLock<Handoff>,
    /// A connection save holds this exclusively; searches and grabs share it.
    indexer_turn: tokio::sync::RwLock<()>,
    /// Releases sent since the last search, by `(indexer_id, guid)`.
    sent_releases: Mutex<std::collections::HashSet<(i64, String)>>,
}

impl RemoteSourceRuntime {
    pub(crate) fn new(config: RemoteSourceConfig) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RemoteSourceRuntimeInner {
                power: config.power,
                config_dir: config.config_dir,
                vault: Box::new(KeyringSecretVault::for_app_identifier(
                    &config.app_identifier,
                )),
                lifecycle: RemoteAcquisitionLifecycle::new(
                    RemoteSourceStaging::new(config.cache_dir),
                    AaxcleanMaterializer::new_for_helper(config.aaxclean_helper),
                    config.host,
                ),
                pending_audible_auth: Mutex::new(None),
                indexer_adapter: ReqwestProwlarrAdapter::new()?,
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

    pub(crate) fn cleanup_abandoned_sessions(&self) -> Result<()> {
        self.inner.lifecycle.cleanup_abandoned_sessions()
    }

    pub fn list_providers(&self) -> Vec<RemoteProviderCapabilities> {
        vec![
            AudibleProvider::capabilities(),
            IndexerProvider::capabilities(),
        ]
    }

    pub fn account_state(&self, provider_id: RemoteProviderId) -> Result<RemoteAccountState> {
        match provider_id {
            RemoteProviderId::Audible => AudibleProvider::account_state(self.inner.vault.as_ref()),
            RemoteProviderId::Indexer => {
                IndexerProvider::account_state(&self.inner.config_dir, self.inner.vault.as_ref())
            }
        }
    }

    pub fn start_auth(&self, provider_id: RemoteProviderId) -> Result<RemoteAuthStart> {
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

    pub async fn complete_auth(&self, request: RemoteAuthCompletion) -> Result<RemoteAccountState> {
        match request.provider_id {
            RemoteProviderId::Audible => {
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
                let response_url = read_handoff_url(request.response_url_handoff_path)?;
                AudibleProvider::complete_auth(self.inner.vault.as_ref(), pending, &response_url)
                    .await
            }
            RemoteProviderId::Indexer => Err(AppError::InvalidInput(
                "Indexer uses Settings URL and API key configuration instead of browser auth."
                    .to_string(),
            )),
        }
    }

    pub fn logout(&self, provider_id: RemoteProviderId) -> Result<RemoteAccountState> {
        self.inner.lifecycle.abort_all_acquisition_tasks();
        match provider_id {
            RemoteProviderId::Audible => AudibleProvider::logout(self.inner.vault.as_ref())?,
            RemoteProviderId::Indexer => {
                IndexerProvider::logout(&self.inner.config_dir, self.inner.vault.as_ref())?
            }
        }
        self.inner
            .lifecycle
            .cleanup_logout_sessions_without_handoff()?;
        self.inner.lifecycle.clear_jobs()?;
        *self
            .inner
            .pending_audible_auth
            .lock()
            .map_err(|_| AppError::General("Remote auth state lock failed".to_string()))? = None;
        self.account_state(provider_id)
    }

    pub async fn load_library(&self, provider_id: RemoteProviderId) -> Result<RemoteLibrary> {
        match provider_id {
            RemoteProviderId::Audible => {
                AudibleProvider::load_library(self.inner.vault.as_ref()).await
            }
            RemoteProviderId::Indexer => Err(AppError::InvalidInput(
                "Indexer search uses release search instead of library scan.".to_string(),
            )),
        }
    }

    /// Searches the indexer. A new search forgets which releases were sent.
    pub async fn search_releases(
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
    pub async fn grab_release(
        &self,
        request: types::RemoteReleaseGrabRequest,
    ) -> Result<types::RemoteReleaseGrabResponse> {
        let _turn = self.indexer_turn()?;
        let key = (request.release.indexer_id, request.release.guid.clone());
        if self.sent_releases().contains(&key) {
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
        .await?;
        if response.accepted {
            self.sent_releases().insert(key);
        }
        Ok(response)
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
    pub async fn get_indexer_connection(&self) -> Result<types::RemoteIndexerConnection> {
        let runtime = self.clone();
        tokio::task::spawn_blocking(move || {
            IndexerProvider::get_connection(&runtime.inner.config_dir, runtime.inner.vault.as_ref())
        })
        .await
        .map_err(|error| AppError::General(error.to_string()))?
    }

    /// Saves the connection. Refused while a search or grab runs; releases
    /// found with the previous connection must be searched again.
    pub async fn update_indexer_connection(
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
        tokio::task::spawn_blocking(move || {
            IndexerProvider::update_connection(
                &runtime.inner.config_dir,
                runtime.inner.vault.as_ref(),
                update,
            )
        })
        .await
        .map_err(|error| AppError::General(error.to_string()))?
    }

    pub async fn test_indexer_connection(
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

    pub async fn start_acquisition(
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
mod tests {
    use super::*;
    use abb_remote_source_core::{acquisition_progress, AcquisitionStage};
    use secrecy::SecretString;
    use tempfile::TempDir;

    #[derive(Default)]
    struct TestSecretVault;

    impl vault::SecretVault for TestSecretVault {
        fn get_secret(&self, _key: &str) -> Result<Option<SecretString>> {
            Ok(None)
        }

        fn set_secret(&self, _key: &str, _value: SecretString) -> Result<()> {
            Ok(())
        }

        fn delete_secret(&self, _key: &str) -> Result<()> {
            Ok(())
        }
    }

    fn test_runtime(root: &TempDir) -> RemoteSourceRuntime {
        RemoteSourceRuntime {
            inner: Arc::new(RemoteSourceRuntimeInner {
                power: crate::power::PowerManager::default(),
                config_dir: root.path().to_path_buf(),
                vault: Box::<TestSecretVault>::default(),
                lifecycle: RemoteAcquisitionLifecycle::new(
                    RemoteSourceStaging::new(root.path().to_path_buf()),
                    AaxcleanMaterializer::for_tests(),
                    crate::host::Host::new(
                        std::sync::Arc::new(crate::DiscardEvents),
                        crate::power::PowerManager::default(),
                    ),
                ),
                pending_audible_auth: Mutex::new(None),
                indexer_adapter: ReqwestProwlarrAdapter::new().expect("indexer adapter"),
                tasks: tokio_util::task::TaskTracker::new(),
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
    async fn a_finished_job_records_its_handoff_and_drops_files_nothing_imported() {
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
            assert_eq!(audio.exists(), kept);
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
    fn logout_keeps_materialized_handoff_sessions_but_purges_unmaterialized_sessions() {
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
            types::RemoteAcquisitionStatus::Validated,
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
        let mut jobs = runtime.inner.lifecycle.jobs.lock().expect("jobs lock");
        jobs.insert(materialized_job_id.to_string(), materialized_job);
        jobs.insert(
            unmaterialized_job_id.to_string(),
            acquisition_job(
                unmaterialized_job_id,
                types::RemoteAcquisitionStatus::Acquiring,
            ),
        );
        drop(jobs);

        runtime
            .logout(RemoteProviderId::Audible)
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
            .cleanup_logout_sessions_without_handoff()
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
