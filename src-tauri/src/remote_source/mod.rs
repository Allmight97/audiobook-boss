use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tauri::Manager;

mod materializer;
mod providers;
mod scoped_output;
mod session_lifecycle;
mod staging;
mod types;
mod vault;

use materializer::AaxcleanMaterializer;
use providers::audible::{AudibleProvider, PendingAudibleAuth};
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

#[derive(Clone)]
pub struct RemoteSourceRuntime {
    inner: Arc<RemoteSourceRuntimeInner>,
}

struct RemoteSourceRuntimeInner {
    vault: Box<dyn SecretVault>,
    lifecycle: RemoteAcquisitionLifecycle,
    pending_audible_auth: Mutex<Option<PendingAudibleAuth>>,
}

impl RemoteSourceRuntime {
    pub fn new(app: &tauri::AppHandle) -> Result<Self> {
        let cache_dir = app.path().app_cache_dir().map_err(|error| {
            AppError::General(format!("Failed to resolve app cache directory: {error}"))
        })?;
        Ok(Self {
            inner: Arc::new(RemoteSourceRuntimeInner {
                vault: Box::<KeyringSecretVault>::default(),
                lifecycle: RemoteAcquisitionLifecycle::new(
                    RemoteSourceStaging::new(cache_dir),
                    AaxcleanMaterializer::from_app(app),
                ),
                pending_audible_auth: Mutex::new(None),
            }),
        })
    }

    pub fn cleanup_abandoned_sessions(&self) -> Result<()> {
        self.inner.lifecycle.cleanup_abandoned_sessions()
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
        self.inner.lifecycle.abort_all_acquisition_tasks();
        match provider_id {
            RemoteProviderId::Audible => AudibleProvider::logout(self.inner.vault.as_ref())?,
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
        }
    }

    pub async fn start_acquisition(
        &self,
        plan: RemoteAcquisitionPlan,
    ) -> Result<RemoteAcquisitionJob> {
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
                vault: Box::<TestSecretVault>::default(),
                lifecycle: RemoteAcquisitionLifecycle::new(
                    RemoteSourceStaging::new(root.path().to_path_buf()),
                    AaxcleanMaterializer::for_tests(),
                ),
                pending_audible_auth: Mutex::new(None),
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
            .lifecycle
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
