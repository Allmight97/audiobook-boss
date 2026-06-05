use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use abb_remote_source_core::{
    acquisition_progress, AcquisitionProgress as CoreAcquisitionProgress, AcquisitionStage,
};
use tauri::Manager;
use tokio::task::AbortHandle;

mod materializer;
mod providers;
mod scoped_output;
mod staging;
mod types;
mod vault;

use materializer::AaxcleanMaterializer;
use providers::audible::{AudibleProvider, PendingAudibleAuth};
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

#[derive(Clone)]
pub struct RemoteSourceRuntime {
    inner: Arc<RemoteSourceRuntimeInner>,
}

struct RemoteSourceRuntimeInner {
    vault: Box<dyn SecretVault>,
    staging: RemoteSourceStaging,
    materializer: AaxcleanMaterializer,
    pending_audible_auth: Mutex<Option<PendingAudibleAuth>>,
    jobs: Mutex<HashMap<String, RemoteAcquisitionJob>>,
    acquisition_tasks: Mutex<HashMap<String, AbortHandle>>,
}

impl RemoteSourceRuntime {
    pub fn new(app: &tauri::AppHandle) -> Result<Self> {
        let cache_dir = app.path().app_cache_dir().map_err(|error| {
            AppError::General(format!("Failed to resolve app cache directory: {error}"))
        })?;
        Ok(Self {
            inner: Arc::new(RemoteSourceRuntimeInner {
                vault: Box::<KeyringSecretVault>::default(),
                staging: RemoteSourceStaging::new(cache_dir),
                materializer: AaxcleanMaterializer::from_app(app),
                pending_audible_auth: Mutex::new(None),
                jobs: Mutex::new(HashMap::new()),
                acquisition_tasks: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub fn cleanup_abandoned_sessions(&self) -> Result<()> {
        self.inner.staging.cleanup_abandoned_sessions()
    }

    pub fn list_providers(&self) -> Vec<RemoteProviderCapabilities> {
        vec![AudibleProvider::capabilities()]
    }

    pub fn account_state(&self, provider_id: RemoteProviderId) -> Result<RemoteAccountState> {
        match provider_id {
            RemoteProviderId::Audible => AudibleProvider::account_state(self.inner.vault.as_ref()),
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
        }
    }

    pub fn logout(&self, provider_id: RemoteProviderId) -> Result<RemoteAccountState> {
        self.abort_all_acquisition_tasks();
        match provider_id {
            RemoteProviderId::Audible => AudibleProvider::logout(self.inner.vault.as_ref())?,
        }
        self.cleanup_logout_sessions_without_handoff()?;
        self.inner
            .jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .clear();
        *self
            .inner
            .pending_audible_auth
            .lock()
            .map_err(|_| AppError::General("Remote auth state lock failed".to_string()))? = None;
        self.account_state(provider_id)
    }

    fn cleanup_logout_sessions_without_handoff(&self) -> Result<()> {
        let job_ids = self
            .inner
            .jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .values()
            .filter(|job| job.materialized_files.is_empty())
            .map(|job| job.job_id.clone())
            .collect::<Vec<_>>();

        for job_id in job_ids {
            self.inner.staging.purge_session(&job_id)?;
        }

        Ok(())
    }

    pub async fn load_library(&self, provider_id: RemoteProviderId) -> Result<RemoteLibrary> {
        match provider_id {
            RemoteProviderId::Audible => {
                AudibleProvider::load_library(self.inner.vault.as_ref()).await
            }
        }
    }

    pub async fn start_acquisition(
        &self,
        plan: RemoteAcquisitionPlan,
    ) -> Result<RemoteAcquisitionJob> {
        if plan.selections.is_empty() {
            return Err(AppError::InvalidInput(
                "Select at least one remote title to acquire.".to_string(),
            ));
        }
        let job_id = uuid::Uuid::new_v4().to_string();
        let job_dir = self.inner.staging.create_job_dir(&job_id)?;
        let job = RemoteAcquisitionJob {
            job_id: job_id.clone(),
            provider_id: plan.provider_id,
            status: types::RemoteAcquisitionStatus::Acquiring,
            progress: acquisition_progress(AcquisitionStage::License, Some(0.0), None, None),
            materialized_files: Vec::new(),
            supplemental_assets: Vec::new(),
            diagnostics: Vec::new(),
        };
        self.inner
            .jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .insert(job_id, job.clone());
        let runtime = self.clone();
        let spawned_job_id = job.job_id.clone();
        let abort_handle = tokio::spawn(async move {
            runtime
                .run_acquisition_job(plan, spawned_job_id, job_dir)
                .await;
        })
        .abort_handle();
        self.store_acquisition_task(&job.job_id, abort_handle);
        Ok(job)
    }

    async fn run_acquisition_job(
        &self,
        plan: RemoteAcquisitionPlan,
        job_id: String,
        job_dir: PathBuf,
    ) {
        let result = match plan.provider_id {
            RemoteProviderId::Audible => {
                AudibleProvider::acquire(
                    self.inner.vault.as_ref(),
                    &self.inner.materializer,
                    &plan,
                    &job_id,
                    &job_dir,
                    |progress| {
                        self.update_job_progress(&job_id, progress);
                    },
                    || self.job_is_cancelled(&job_id),
                )
                .await
            }
        };

        self.remove_acquisition_task(&job_id);

        match result {
            Ok(job) if self.job_is_cancelled(&job_id) => {
                self.cleanup_cancelled_job_session(&job_id);
                self.mark_job_cancelled(&job_id, plan.provider_id);
                log::info!(
                    "remote_source acquisition job_id={} status=cancelled_preserved",
                    job_id
                );
                let _ = job;
            }
            Ok(job) => self.replace_job_if_active(job),
            Err(AppError::Cancellation(_)) => {
                self.cleanup_cancelled_job_session(&job_id);
                self.mark_job_cancelled(&job_id, plan.provider_id);
                log::info!(
                    "remote_source acquisition job_id={} status=cancelled",
                    job_id
                );
            }
            Err(_error) if self.job_is_cancelled(&job_id) => {
                self.cleanup_cancelled_job_session(&job_id);
                self.mark_job_cancelled(&job_id, plan.provider_id);
                log::info!(
                    "remote_source acquisition job_id={} status=cancelled provider_error_suppressed=true",
                    job_id
                );
            }
            Err(error) => self.mark_job_failed(&job_id, plan.provider_id, error.to_string()),
        }
    }

    fn update_job_progress(&self, job_id: &str, progress: CoreAcquisitionProgress) {
        if let Ok(mut jobs) = self.inner.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                if job.status == types::RemoteAcquisitionStatus::Cancelled {
                    return;
                }
                job.progress = progress;
            }
        }
    }

    fn replace_job_if_active(&self, job: RemoteAcquisitionJob) {
        if let Ok(mut jobs) = self.inner.jobs.lock() {
            if jobs.get(&job.job_id).is_some_and(|existing| {
                existing.status == types::RemoteAcquisitionStatus::Cancelled
            }) {
                return;
            }
            jobs.insert(job.job_id.clone(), job);
        }
    }

    fn mark_job_failed(&self, job_id: &str, provider_id: RemoteProviderId, message: String) {
        if let Ok(mut jobs) = self.inner.jobs.lock() {
            let job = jobs
                .entry(job_id.to_string())
                .or_insert_with(|| RemoteAcquisitionJob {
                    job_id: job_id.to_string(),
                    provider_id,
                    status: types::RemoteAcquisitionStatus::Failed,
                    progress: acquisition_progress(AcquisitionStage::Failed, Some(1.0), None, None),
                    materialized_files: Vec::new(),
                    supplemental_assets: Vec::new(),
                    diagnostics: Vec::new(),
                });
            if job.status == types::RemoteAcquisitionStatus::Cancelled {
                return;
            }
            job.status = types::RemoteAcquisitionStatus::Failed;
            job.progress = acquisition_progress(AcquisitionStage::Failed, Some(1.0), None, None);
            job.diagnostics.push(types::RemoteSourceDiagnostic {
                kind: types::RemoteAcquisitionFailureKind::MaterializationFailed,
                title_id: None,
                message,
            });
        }
    }

    fn mark_job_cancelled(&self, job_id: &str, provider_id: RemoteProviderId) {
        if let Ok(mut jobs) = self.inner.jobs.lock() {
            let job = jobs
                .entry(job_id.to_string())
                .or_insert_with(|| RemoteAcquisitionJob {
                    job_id: job_id.to_string(),
                    provider_id,
                    status: types::RemoteAcquisitionStatus::Cancelled,
                    progress: acquisition_progress(
                        AcquisitionStage::Cancelled,
                        Some(1.0),
                        None,
                        None,
                    ),
                    materialized_files: Vec::new(),
                    supplemental_assets: Vec::new(),
                    diagnostics: Vec::new(),
                });
            job.status = types::RemoteAcquisitionStatus::Cancelled;
            job.progress = acquisition_progress(AcquisitionStage::Cancelled, Some(1.0), None, None);
            job.materialized_files.clear();
            job.supplemental_assets.clear();
            if !job
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == types::RemoteAcquisitionFailureKind::Cancelled)
            {
                job.diagnostics.push(types::RemoteSourceDiagnostic {
                    kind: types::RemoteAcquisitionFailureKind::Cancelled,
                    title_id: None,
                    message: "Remote source acquisition was cancelled.".to_string(),
                });
            }
        }
    }

    fn job_is_cancelled(&self, job_id: &str) -> bool {
        self.inner
            .jobs
            .lock()
            .ok()
            .and_then(|jobs| {
                jobs.get(job_id)
                    .map(|job| job.status == types::RemoteAcquisitionStatus::Cancelled)
            })
            .unwrap_or(false)
    }

    fn job_is_active(&self, job_id: &str) -> bool {
        self.inner
            .jobs
            .lock()
            .ok()
            .and_then(|jobs| {
                jobs.get(job_id).map(|job| {
                    matches!(
                        job.status,
                        types::RemoteAcquisitionStatus::Planned
                            | types::RemoteAcquisitionStatus::Acquiring
                            | types::RemoteAcquisitionStatus::Materialized
                    )
                })
            })
            .unwrap_or(false)
    }

    fn store_acquisition_task(&self, job_id: &str, abort_handle: AbortHandle) {
        let Ok(mut tasks) = self.inner.acquisition_tasks.lock() else {
            log::warn!(
                "remote_source acquisition job_id={} task_registry_store_failed=true",
                job_id
            );
            return;
        };
        tasks.insert(job_id.to_string(), abort_handle);
        drop(tasks);
        if !self.job_is_active(job_id) {
            self.remove_acquisition_task(job_id);
        }
    }

    fn remove_acquisition_task(&self, job_id: &str) {
        if let Ok(mut tasks) = self.inner.acquisition_tasks.lock() {
            tasks.remove(job_id);
        }
    }

    fn abort_acquisition_task(&self, job_id: &str) {
        self.inner.materializer.abort_job(job_id);
        if let Ok(mut tasks) = self.inner.acquisition_tasks.lock() {
            if let Some(handle) = tasks.remove(job_id) {
                handle.abort();
                log::info!(
                    "remote_source acquisition job_id={} task_aborted=true",
                    job_id
                );
            }
        }
    }

    fn abort_all_acquisition_tasks(&self) {
        self.inner.materializer.abort_all();
        if let Ok(mut tasks) = self.inner.acquisition_tasks.lock() {
            for (job_id, handle) in tasks.drain() {
                handle.abort();
                log::info!(
                    "remote_source acquisition job_id={} task_aborted=true",
                    job_id
                );
            }
        }
    }

    fn cleanup_cancelled_job_session(&self, job_id: &str) {
        if let Err(error) = self.inner.staging.purge_session(job_id) {
            log::warn!(
                "remote_source acquisition job_id={} cancelled_session_cleanup_failed={}",
                job_id,
                error
            );
        }
    }

    pub fn acquisition_status(&self, job_id: &str) -> Result<RemoteAcquisitionJob> {
        self.inner
            .jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .get(job_id)
            .cloned()
            .ok_or_else(|| {
                AppError::InvalidInput("Remote acquisition job was not found.".to_string())
            })
    }

    pub fn cancel_acquisition(&self, job_id: &str) -> Result<RemoteAcquisitionJob> {
        let mut jobs = self
            .inner
            .jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?;
        let job = jobs.get_mut(job_id).ok_or_else(|| {
            AppError::InvalidInput("Remote acquisition job was not found.".to_string())
        })?;
        job.status = types::RemoteAcquisitionStatus::Cancelled;
        job.progress = acquisition_progress(AcquisitionStage::Cancelled, Some(1.0), None, None);
        job.materialized_files.clear();
        job.supplemental_assets.clear();
        if !job
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == types::RemoteAcquisitionFailureKind::Cancelled)
        {
            job.diagnostics.push(types::RemoteSourceDiagnostic {
                kind: types::RemoteAcquisitionFailureKind::Cancelled,
                title_id: None,
                message: "Remote source acquisition was cancelled.".to_string(),
            });
        }
        let cancelled_job = job.clone();
        drop(jobs);
        self.abort_acquisition_task(job_id);
        self.cleanup_cancelled_job_session(job_id);
        Ok(cancelled_job)
    }

    pub fn purge_session(&self, job_id: &str) -> Result<()> {
        self.abort_acquisition_task(job_id);
        self.inner.staging.purge_session(job_id)?;
        self.inner
            .jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .remove(job_id);
        Ok(())
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
                vault: Box::<TestSecretVault>::default(),
                staging: RemoteSourceStaging::new(root.path().to_path_buf()),
                materializer: AaxcleanMaterializer::for_tests(),
                pending_audible_auth: Mutex::new(None),
                jobs: Mutex::new(HashMap::new()),
                acquisition_tasks: Mutex::new(HashMap::new()),
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
        }
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
        runtime.inner.jobs.lock().expect("jobs lock").insert(
            job_id.to_string(),
            acquisition_job(job_id, types::RemoteAcquisitionStatus::Acquiring),
        );

        runtime
            .cancel_acquisition(job_id)
            .expect("cancel acquisition");
        runtime.update_job_progress(
            job_id,
            acquisition_progress(AcquisitionStage::Download, Some(0.5), Some(5), Some(10)),
        );
        runtime.replace_job_if_active(acquisition_job(
            job_id,
            types::RemoteAcquisitionStatus::Validated,
        ));
        runtime.mark_job_failed(
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
    fn cancel_acquisition_clears_existing_import_handoff_references() {
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
            .jobs
            .lock()
            .expect("jobs lock")
            .insert(job_id.to_string(), job);

        let cancelled = runtime
            .cancel_acquisition(job_id)
            .expect("cancel acquisition");

        assert_eq!(cancelled.status, types::RemoteAcquisitionStatus::Cancelled);
        assert!(cancelled.materialized_files.is_empty());
        assert!(cancelled.supplemental_assets.is_empty());
        let stored = runtime.acquisition_status(job_id).expect("job status");
        assert!(stored.materialized_files.is_empty());
        assert!(stored.supplemental_assets.is_empty());
    }

    #[test]
    fn cancelled_job_cleanup_removes_session_files_without_removing_status() {
        let root = TempDir::new().expect("temp root");
        let runtime = test_runtime(&root);
        let job_id = "remote-job-2";
        let job_dir = runtime
            .inner
            .staging
            .create_job_dir(job_id)
            .expect("job dir");
        std::fs::write(job_dir.join("download.m4b"), b"payload").expect("write staged file");
        runtime.inner.jobs.lock().expect("jobs lock").insert(
            job_id.to_string(),
            acquisition_job(job_id, types::RemoteAcquisitionStatus::Cancelled),
        );

        runtime.cleanup_cancelled_job_session(job_id);

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
            .staging
            .create_job_dir(materialized_job_id)
            .expect("materialized job dir");
        let unmaterialized_job_dir = runtime
            .inner
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
        let mut jobs = runtime.inner.jobs.lock().expect("jobs lock");
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
        assert!(runtime.inner.jobs.lock().expect("jobs lock").is_empty());
    }
}
