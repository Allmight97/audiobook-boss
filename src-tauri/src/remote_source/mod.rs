use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use abb_remote_source_core::{
    acquisition_progress, AcquisitionProgress as CoreAcquisitionProgress, AcquisitionStage,
};
use tauri::Manager;

mod providers;
mod staging;
mod types;
mod vault;

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
    pending_audible_auth: Mutex<Option<PendingAudibleAuth>>,
    jobs: Mutex<HashMap<String, RemoteAcquisitionJob>>,
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
                pending_audible_auth: Mutex::new(None),
                jobs: Mutex::new(HashMap::new()),
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
                        "$TMPDIR/abb-audible-auth-response-url.txt or ABB_AUDIBLE_AUTH_RESPONSE_URL_PATH"
                            .to_string(),
                    message: "Open the authorization URL externally, sign in, then save the final browser URL to a local handoff file and complete auth from ABB.".to_string(),
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
        match provider_id {
            RemoteProviderId::Audible => AudibleProvider::logout(self.inner.vault.as_ref())?,
        }
        self.cleanup_abandoned_sessions()?;
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
        tokio::spawn(async move {
            runtime
                .run_acquisition_job(plan, spawned_job_id, job_dir)
                .await;
        });
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
                    &plan,
                    &job_id,
                    &job_dir,
                    |progress| {
                        self.update_job_progress(&job_id, progress);
                    },
                )
                .await
            }
        };

        match result {
            Ok(job) => self.replace_job(job),
            Err(error) => self.mark_job_failed(&job_id, plan.provider_id, error.to_string()),
        }
    }

    fn update_job_progress(&self, job_id: &str, progress: CoreAcquisitionProgress) {
        if let Ok(mut jobs) = self.inner.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                job.progress = progress;
            }
        }
    }

    fn replace_job(&self, job: RemoteAcquisitionJob) {
        if let Ok(mut jobs) = self.inner.jobs.lock() {
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
            job.status = types::RemoteAcquisitionStatus::Failed;
            job.progress = acquisition_progress(AcquisitionStage::Failed, Some(1.0), None, None);
            job.diagnostics.push(types::RemoteSourceDiagnostic {
                kind: types::RemoteAcquisitionFailureKind::MaterializationFailed,
                title_id: None,
                message,
            });
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
        Ok(job.clone())
    }

    pub fn purge_session(&self, job_id: &str) -> Result<()> {
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

pub use types::*;

#[cfg(test)]
mod tests {
    use super::read_handoff_url;
    use tempfile::TempDir;

    #[test]
    fn read_handoff_url_reads_trimmed_non_symlink_file() {
        let root = TempDir::new().expect("temp root");
        let path = root.path().join("handoff.txt");
        std::fs::write(&path, " https://example.test/callback?code=abc \n").expect("write handoff");

        let url = read_handoff_url(Some(path)).expect("read handoff");

        assert_eq!(url, "https://example.test/callback?code=abc");
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
}
