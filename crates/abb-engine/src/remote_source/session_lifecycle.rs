use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use abb_remote_source_core::{
    acquisition_progress, AcquisitionProgress as CoreAcquisitionProgress, AcquisitionStage,
};
use tokio::task::AbortHandle;

use crate::errors::{AppError, Result};
use crate::host::{EngineEvent, Host};
use crate::remote_source::materializer::AaxcleanMaterializer;
use crate::remote_source::staging::RemoteSourceStaging;
use crate::remote_source::{types, AcquisitionJob as RemoteAcquisitionJob};

use super::{RemoteProviderId, RemoteSourceRuntime};

/// How often a download's progress reaches hosts; a stage change always does.
const PROGRESS_EVENT_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct RemoteAcquisitionLifecycle {
    pub(super) staging: RemoteSourceStaging,
    materializer: AaxcleanMaterializer,
    pub(super) jobs: Mutex<HashMap<String, RemoteAcquisitionJob>>,
    acquisition_tasks: Mutex<HashMap<String, AbortHandle>>,
    host: Host,
    /// When each job's progress last reached hosts.
    progress_sent: Mutex<HashMap<String, Instant>>,
}

impl RemoteAcquisitionLifecycle {
    pub(super) fn new(
        staging: RemoteSourceStaging,
        materializer: AaxcleanMaterializer,
        host: Host,
    ) -> Self {
        Self {
            staging,
            materializer,
            jobs: Mutex::new(HashMap::new()),
            acquisition_tasks: Mutex::new(HashMap::new()),
            host,
            progress_sent: Mutex::new(HashMap::new()),
        }
    }

    /// Tells hosts what `job` looks like now.
    fn publish(&self, job: &RemoteAcquisitionJob) {
        if let Ok(mut sent) = self.progress_sent.lock() {
            sent.insert(job.job_id.clone(), Instant::now());
        }
        self.host
            .emit(EngineEvent::Acquisition(Box::new(job.clone())));
    }

    fn publish_current(&self, job_id: &str) {
        let job = self
            .jobs
            .lock()
            .ok()
            .and_then(|jobs| jobs.get(job_id).cloned());
        if let Some(job) = job {
            self.publish(&job);
        }
    }

    pub(super) fn cleanup_abandoned_sessions(&self) -> Result<()> {
        self.staging.cleanup_abandoned_sessions()
    }

    pub(super) fn clear_jobs(&self) -> Result<()> {
        self.jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .clear();
        Ok(())
    }

    pub(super) fn cleanup_logout_sessions_without_handoff(&self) -> Result<()> {
        let mut job_ids = self
            .jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .values()
            .filter(|job| job.materialized_files.is_empty())
            .map(|job| job.job_id.clone())
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
    ) -> Result<RemoteAcquisitionJob> {
        if plan.selections.is_empty() {
            return Err(AppError::InvalidInput(
                "Select at least one remote title to acquire.".to_string(),
            ));
        }
        if runtime.inner.tasks.is_closed() {
            return Err(AppError::General("ABB is closing.".to_string()));
        }
        let job_id = uuid::Uuid::new_v4().to_string();
        let job_dir = self.staging.create_job_dir(&job_id)?;
        let job = RemoteAcquisitionJob {
            job_id: job_id.clone(),
            provider_id: plan.provider_id,
            status: types::RemoteAcquisitionStatus::Acquiring,
            progress: acquisition_progress(AcquisitionStage::License, Some(0.0), None, None),
            materialized_files: Vec::new(),
            supplemental_assets: Vec::new(),
            diagnostics: Vec::new(),
            handoff: None,
        };
        self.jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .insert(job_id, job.clone());
        self.publish(&job);
        let spawned_job_id = job.job_id.clone();
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
        self.store_acquisition_task(&job.job_id, abort_handle);
        Ok(job)
    }

    async fn run_acquisition_job(
        &self,
        runtime: RemoteSourceRuntime,
        plan: super::AcquisitionPlan,
        job_id: String,
        job_dir: PathBuf,
    ) {
        let _active_work = runtime.inner.power.begin();
        let result = match plan.provider_id {
            super::RemoteProviderId::Audible => {
                super::providers::audible::AudibleProvider::acquire(
                    runtime.inner.vault.as_ref(),
                    &self.materializer,
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
            super::RemoteProviderId::Indexer => Err(AppError::InvalidInput(
                "Indexer grabs do not create acquisition jobs.".to_string(),
            )),
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
            Ok(job) => {
                self.replace_job_if_active(job.clone());
                self.hand_off(&runtime, job).await;
            }
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

    /// Gives a finished job's files to the session and records how that went.
    /// The session decides when the staged files go, imported or not.
    pub(super) async fn hand_off(&self, runtime: &RemoteSourceRuntime, job: RemoteAcquisitionJob) {
        let ready = job.status == types::RemoteAcquisitionStatus::Validated
            && !job.materialized_files.is_empty()
            && !self.job_is_cancelled(&job.job_id);
        let Some(handoff) = runtime.inner.handoff.get().filter(|_| ready) else {
            return;
        };
        let job_id = job.job_id.clone();
        // The session removes an unimported download itself.
        let result = handoff(job).await;
        let updated = self.jobs.lock().ok().and_then(|mut jobs| {
            let job = jobs.get_mut(&job_id)?;
            match &result {
                types::AcquisitionHandoff::Imported { .. } => {
                    job.status = types::RemoteAcquisitionStatus::ImportedToFileList;
                }
                types::AcquisitionHandoff::Removed { .. } => {
                    job.materialized_files.clear();
                    job.supplemental_assets.clear();
                }
            }
            job.handoff = Some(result);
            Some(job.clone())
        });
        if let Some(job) = updated {
            self.publish(&job);
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
            if job.status == types::RemoteAcquisitionStatus::Cancelled {
                return;
            }
            let new_stage = job.progress.stage != progress.stage;
            job.progress = progress;
            (new_stage || self.progress_due(job_id)).then(|| job.clone())
        };
        if let Some(job) = changed {
            self.publish(&job);
        }
    }

    fn progress_due(&self, job_id: &str) -> bool {
        self.progress_sent.lock().map_or(true, |sent| {
            sent.get(job_id)
                .is_none_or(|at| at.elapsed() >= PROGRESS_EVENT_INTERVAL)
        })
    }

    pub(super) fn replace_job_if_active(&self, job: RemoteAcquisitionJob) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if jobs.get(&job.job_id).is_some_and(|existing| {
                existing.status == types::RemoteAcquisitionStatus::Cancelled
            }) {
                return;
            }
            jobs.insert(job.job_id.clone(), job.clone());
        }
        self.publish(&job);
    }

    pub(super) fn mark_job_failed(
        &self,
        job_id: &str,
        provider_id: RemoteProviderId,
        message: String,
    ) {
        if let Ok(mut jobs) = self.jobs.lock() {
            let job = jobs
                .entry(job_id.to_string())
                .or_insert_with(|| placeholder_job(job_id, provider_id));
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
        self.publish_current(job_id);
    }

    pub(super) fn mark_job_cancelled(&self, job_id: &str, provider_id: RemoteProviderId) {
        if let Ok(mut jobs) = self.jobs.lock() {
            let job = jobs
                .entry(job_id.to_string())
                .or_insert_with(|| placeholder_job(job_id, provider_id));
            mark_cancelled(job);
        }
        self.publish_current(job_id);
    }

    fn job_is_cancelled(&self, job_id: &str) -> bool {
        self.jobs
            .lock()
            .ok()
            .and_then(|jobs| {
                jobs.get(job_id)
                    .map(|job| job.status == types::RemoteAcquisitionStatus::Cancelled)
            })
            .unwrap_or(false)
    }

    fn job_is_active(&self, job_id: &str) -> bool {
        self.jobs
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

    pub(super) fn store_acquisition_task(&self, job_id: &str, abort_handle: AbortHandle) {
        let Ok(mut tasks) = self.acquisition_tasks.lock() else {
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

    pub(super) fn running_acquisitions(&self) -> usize {
        self.acquisition_tasks.lock().map_or(0, |tasks| tasks.len())
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

    fn acquisition_status(&self, job_id: &str) -> Result<RemoteAcquisitionJob> {
        self.jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .get(job_id)
            .cloned()
            .ok_or_else(|| {
                AppError::InvalidInput("Remote acquisition job was not found.".to_string())
            })
    }

    fn cancel_acquisition(&self, job_id: &str) -> Result<RemoteAcquisitionJob> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?;
        let job = jobs.get_mut(job_id).ok_or_else(|| {
            AppError::InvalidInput("Remote acquisition job was not found.".to_string())
        })?;
        // A finished job's files may already be in the session; there is
        // nothing left to cancel.
        if !matches!(
            job.status,
            types::RemoteAcquisitionStatus::Planned | types::RemoteAcquisitionStatus::Acquiring
        ) {
            return Ok(job.clone());
        }
        mark_cancelled(job);
        let cancelled_job = job.clone();
        drop(jobs);
        self.abort_acquisition_task(job_id);
        self.cleanup_cancelled_job_session(job_id);
        self.publish(&cancelled_job);
        Ok(cancelled_job)
    }

    fn purge_session(&self, job_id: &str) -> Result<()> {
        self.abort_acquisition_task(job_id);
        // The job's record stays: its handoff outcome may still be on its
        // way to hosts, and logout clears records.
        self.staging.purge_session(job_id)
    }
}

impl RemoteSourceRuntime {
    pub fn acquisition_status(&self, job_id: &str) -> Result<RemoteAcquisitionJob> {
        self.inner.lifecycle.acquisition_status(job_id)
    }

    pub fn cancel_acquisition(&self, job_id: &str) -> Result<RemoteAcquisitionJob> {
        self.inner.lifecycle.cancel_acquisition(job_id)
    }

    /// Removes a job's staged files. The session decides when.
    pub(crate) fn purge_session(&self, job_id: &str) -> Result<()> {
        self.inner.lifecycle.purge_session(job_id)
    }
}

/// A job record for an acquisition that terminalized before its record existed;
/// callers overwrite status and progress immediately.
fn placeholder_job(job_id: &str, provider_id: RemoteProviderId) -> RemoteAcquisitionJob {
    RemoteAcquisitionJob {
        job_id: job_id.to_string(),
        provider_id,
        status: types::RemoteAcquisitionStatus::Acquiring,
        progress: acquisition_progress(AcquisitionStage::License, Some(0.0), None, None),
        materialized_files: Vec::new(),
        supplemental_assets: Vec::new(),
        diagnostics: Vec::new(),
        handoff: None,
    }
}

fn mark_cancelled(job: &mut RemoteAcquisitionJob) {
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
