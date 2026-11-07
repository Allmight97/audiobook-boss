use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use abb_remote_source_core::AcquisitionProgress as CoreAcquisitionProgress;
use tokio::task::AbortHandle;

use crate::errors::{AppError, Result};
use crate::host::{EngineEvent, Host};
use crate::remote_source::materializer::AaxcleanMaterializer;
use crate::remote_source::staging::RemoteSourceStaging;
use crate::remote_source::types;

use super::job_state::{AcquisitionOutcome, Registered};
use super::{RemoteProviderId, RemoteSourceRuntime};

const PROGRESS_EVENT_INTERVAL: Duration = Duration::from_millis(100);

/// Everything one acquisition needs. Reaching the provider and the stored
/// sign-in happens only behind [`AcquireTitles`], so a test that replaces it
/// never touches the network or the keychain.
pub(crate) struct AcquireRequest {
    pub(crate) runtime: RemoteSourceRuntime,
    pub(crate) plan: super::AcquisitionPlan,
    pub(crate) job_id: String,
    pub(crate) job_dir: PathBuf,
}

pub(crate) type AcquireTitles = Arc<
    dyn Fn(AcquireRequest) -> Pin<Box<dyn Future<Output = Result<AcquisitionOutcome>> + Send>>
        + Send
        + Sync,
>;

pub(super) fn audible_acquire() -> AcquireTitles {
    Arc::new(|request| {
        Box::pin(async move {
            let AcquireRequest {
                runtime,
                plan,
                job_id,
                job_dir,
            } = request;
            let lifecycle = &runtime.inner.lifecycle;
            let auth = runtime
                .vault_blocking(|runtime| {
                    super::providers::audible::AudibleProvider::stored_auth(
                        runtime.inner.vault.as_ref(),
                    )
                })
                .await?;
            super::providers::audible::AudibleProvider::acquire(
                auth,
                &lifecycle.materializer,
                &plan,
                &job_id,
                &job_dir,
                |progress| {
                    lifecycle.update_job_progress(&job_id, progress);
                },
                || lifecycle.job_is_cancelled(&job_id),
            )
            .await
        })
    })
}

pub(super) struct RemoteAcquisitionLifecycle {
    pub(super) staging: RemoteSourceStaging,
    materializer: AaxcleanMaterializer,
    pub(super) jobs: Mutex<HashMap<String, Registered>>,
    acquisition_tasks: Mutex<HashMap<String, AbortHandle>>,
    host: Host,
    ui: std::sync::Arc<Mutex<super::ui::UiState>>,
    progress_sent: Mutex<HashMap<String, Instant>>,
    acquire: AcquireTitles,
}

impl RemoteAcquisitionLifecycle {
    pub(super) fn new(
        staging: RemoteSourceStaging,
        materializer: AaxcleanMaterializer,
        host: Host,
        ui: std::sync::Arc<Mutex<super::ui::UiState>>,
        acquire: AcquireTitles,
    ) -> Self {
        Self {
            staging,
            materializer,
            jobs: Mutex::new(HashMap::new()),
            acquisition_tasks: Mutex::new(HashMap::new()),
            host,
            ui,
            progress_sent: Mutex::new(HashMap::new()),
            acquire,
        }
    }

    /// Tells hosts what the job looks like now. It reads the registered job
    /// under the UI guard, so a progress clone taken before a terminal change
    /// can never be published after it.
    fn publish(&self, job_id: &str) {
        let update = {
            let mut ui = self
                .ui
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(job) = self
                .jobs
                .lock()
                .ok()
                .and_then(|jobs| jobs.get(job_id).map(Registered::snapshot))
            else {
                return;
            };
            if let Ok(mut sent) = self.progress_sent.lock() {
                sent.insert(job_id.to_string(), Instant::now());
            }
            ui.acquisition_changed(job)
        };
        self.host.emit(EngineEvent::Session(update));
    }

    pub(super) fn cleanup_abandoned_sessions(&self) -> Result<()> {
        self.staging.cleanup_abandoned_sessions()
    }

    pub(super) fn clear_jobs(&self, provider_id: RemoteProviderId) -> Result<()> {
        self.jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .retain(|_, job| job.provider_id() != provider_id);
        Ok(())
    }

    pub(super) fn cleanup_logout_sessions_without_handoff(
        &self,
        provider_id: RemoteProviderId,
    ) -> Result<()> {
        let mut job_ids = self
            .jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .iter()
            .filter(|(_, job)| job.provider_id() == provider_id && !job.holds_staged_files())
            .map(|(job_id, _)| job_id.clone())
            .collect::<Vec<_>>();
        job_ids.sort();

        let mut last_error = None;
        for job_id in job_ids {
            if let Err(error) = self.staging.purge_session(&job_id) {
                log::warn!(
                    "remote_source logout cleanup failed job_id={} error={}",
                    job_id,
                    error
                );
                last_error = Some(error);
            }
        }

        if let Some(error) = last_error {
            return Err(error);
        }

        Ok(())
    }

    pub(super) async fn start_acquisition(
        &self,
        runtime: RemoteSourceRuntime,
        plan: super::AcquisitionPlan,
    ) -> Result<()> {
        if plan.selections.is_empty() {
            return Err(AppError::InvalidInput(
                "Select at least one remote title to acquire.".to_string(),
            ));
        }
        let tasks = runtime.inner.tasks.clone();
        let job_id = tasks.admit(|| self.register_acquisition(runtime, plan))??;
        self.publish(&job_id);
        Ok(())
    }

    fn register_acquisition(
        &self,
        runtime: RemoteSourceRuntime,
        plan: super::AcquisitionPlan,
    ) -> Result<String> {
        let mut registered = self
            .acquisition_tasks
            .lock()
            .map_err(|_| AppError::General("Remote acquisition task lock failed".into()))?;
        let job_id = uuid::Uuid::new_v4().to_string();
        let job_dir = self.staging.create_job_dir(&job_id)?;
        {
            let mut jobs = self
                .jobs
                .lock()
                .map_err(|_| AppError::General("Remote acquisition job lock failed".into()))?;
            if jobs.values().any(|job| !job.settled()) {
                self.staging.purge_session(&job_id)?;
                return Err(AppError::InvalidInput(
                    "An Audible acquisition is still running or handing off its files.".into(),
                ));
            }
            jobs.insert(job_id.clone(), Registered::new(&job_id));
        }
        let spawned_job_id = job_id.clone();
        let tasks = runtime.inner.tasks.clone();
        let abort_handle = tasks
            .spawn(async move {
                runtime
                    .inner
                    .lifecycle
                    .run_acquisition_job(runtime.clone(), plan, spawned_job_id, job_dir)
                    .await;
            })
            .abort_handle();
        registered.insert(job_id.clone(), abort_handle);
        Ok(job_id)
    }

    async fn run_acquisition_job(
        &self,
        runtime: RemoteSourceRuntime,
        plan: super::AcquisitionPlan,
        job_id: String,
        job_dir: PathBuf,
    ) {
        let _active_work = runtime.inner.power.begin();
        let result = (self.acquire)(AcquireRequest {
            runtime: runtime.clone(),
            plan,
            job_id: job_id.clone(),
            job_dir,
        })
        .await;

        self.remove_acquisition_task(&job_id);

        match result {
            Ok(_) if self.job_is_cancelled(&job_id) => {
                self.finish_cancelled(&job_id);
                log::info!(
                    "remote_source acquisition job_id={} status=cancelled_preserved",
                    job_id
                );
            }
            Ok(outcome) => {
                self.apply_outcome(&job_id, outcome);
                self.hand_off(&runtime, &job_id).await;
            }
            Err(AppError::Cancellation(_)) => {
                self.finish_cancelled(&job_id);
                log::info!(
                    "remote_source acquisition job_id={} status=cancelled",
                    job_id
                );
            }
            Err(_error) if self.job_is_cancelled(&job_id) => {
                self.finish_cancelled(&job_id);
                log::info!(
                    "remote_source acquisition job_id={} status=cancelled provider_error_suppressed=true",
                    job_id
                );
            }
            Err(error) => {
                self.cleanup_cancelled_job_session(&job_id);
                self.apply_outcome(
                    &job_id,
                    AcquisitionOutcome::failed(
                        types::RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
                        error.to_string(),
                    ),
                );
            }
        }
    }

    pub(super) async fn hand_off(&self, runtime: &RemoteSourceRuntime, job_id: &str) {
        let Some(handoff) = runtime.inner.handoff.get() else {
            return;
        };
        let Some(job) = self
            .jobs
            .lock()
            .ok()
            .and_then(|jobs| jobs.get(job_id).and_then(Registered::job_for_handoff))
        else {
            return;
        };
        let result = handoff(job).await;
        let updated = self
            .jobs
            .lock()
            .ok()
            .and_then(|mut jobs| jobs.get_mut(job_id)?.handoff_done(result).then_some(()));
        if updated.is_some() {
            self.publish(job_id);
        }
    }

    pub(super) fn update_job_progress(&self, job_id: &str, progress: CoreAcquisitionProgress) {
        let changed = {
            let Ok(mut jobs) = self.jobs.lock() else {
                return;
            };
            let Some(job) = jobs.get_mut(job_id) else {
                return;
            };
            job.show_progress(progress)
                .is_some_and(|new_stage| new_stage || self.progress_due(job_id))
        };
        if changed {
            self.publish(job_id);
        }
    }

    fn progress_due(&self, job_id: &str) -> bool {
        self.progress_sent.lock().map_or(true, |sent| {
            sent.get(job_id)
                .is_none_or(|at| at.elapsed() >= PROGRESS_EVENT_INTERVAL)
        })
    }

    pub(super) fn apply_outcome(&self, job_id: &str, outcome: AcquisitionOutcome) {
        self.change_job(job_id, |job| job.finish(outcome));
    }

    fn finish_cancelled(&self, job_id: &str) {
        self.cleanup_cancelled_job_session(job_id);
        self.change_job(job_id, Registered::cancel);
    }

    fn change_job(&self, job_id: &str, change: impl FnOnce(&mut Registered) -> bool) {
        let changed = self
            .jobs
            .lock()
            .ok()
            .and_then(|mut jobs| jobs.get_mut(job_id).map(change))
            .unwrap_or(false);
        if changed {
            self.publish(job_id);
        }
    }

    fn job_is_cancelled(&self, job_id: &str) -> bool {
        self.jobs
            .lock()
            .ok()
            .and_then(|jobs| jobs.get(job_id).map(Registered::is_cancelled))
            .unwrap_or(false)
    }

    fn remove_acquisition_task(&self, job_id: &str) {
        if let Ok(mut tasks) = self.acquisition_tasks.lock() {
            tasks.remove(job_id);
        }
    }

    fn abort_acquisition_task(&self, job_id: &str) {
        self.materializer.abort_job(job_id);
        if let Ok(mut tasks) = self.acquisition_tasks.lock() {
            if let Some(handle) = tasks.remove(job_id) {
                handle.abort();
                log::info!(
                    "remote_source acquisition job_id={} task_aborted=true",
                    job_id
                );
            }
        }
    }

    /// Downloads still in progress, by job state: a task handle can outlive
    /// a job that failed before its handle was stored.
    pub(super) fn running_acquisitions(&self) -> Vec<String> {
        self.jobs.lock().map_or_else(
            |_| Vec::new(),
            |jobs| {
                jobs.iter()
                    .filter(|(_, job)| job.is_running())
                    .map(|(job_id, _)| job_id.clone())
                    .collect()
            },
        )
    }

    pub(super) fn abort_all_acquisition_tasks(&self) {
        self.materializer.abort_all();
        if let Ok(mut tasks) = self.acquisition_tasks.lock() {
            for (job_id, handle) in tasks.drain() {
                handle.abort();
                log::info!(
                    "remote_source acquisition job_id={} task_aborted=true",
                    job_id
                );
            }
        }
    }

    pub(super) fn cleanup_cancelled_job_session(&self, job_id: &str) {
        if let Err(error) = self.staging.purge_session(job_id) {
            log::warn!(
                "remote_source acquisition job_id={} cancelled_session_cleanup_failed={}",
                job_id,
                error
            );
        }
    }

    fn cancel_acquisition(&self, job_id: &str) -> Result<types::AcquisitionSnapshot> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?;
        let job = jobs.get_mut(job_id).ok_or_else(|| {
            AppError::InvalidInput("Remote acquisition job was not found.".to_string())
        })?;
        if !job.cancel() {
            return Ok(job.snapshot());
        }
        let cancelled = job.snapshot();
        drop(jobs);
        self.abort_acquisition_task(job_id);
        self.cleanup_cancelled_job_session(job_id);
        self.publish(job_id);
        Ok(cancelled)
    }

    fn purge_session(&self, job_id: &str) -> Result<()> {
        self.abort_acquisition_task(job_id);
        self.staging.purge_session(job_id)?;
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.remove(job_id);
        }
        if let Ok(mut sent) = self.progress_sent.lock() {
            sent.remove(job_id);
        }
        Ok(())
    }
}

impl RemoteSourceRuntime {
    pub(crate) fn cancel_acquisition(&self, job_id: &str) -> Result<super::AcquisitionSnapshot> {
        self.inner.lifecycle.cancel_acquisition(job_id)
    }

    pub(crate) fn purge_session(&self, job_id: &str) -> Result<()> {
        self.inner.lifecycle.purge_session(job_id)
    }
}
