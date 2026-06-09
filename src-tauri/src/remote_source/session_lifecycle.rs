use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use abb_remote_source_core::{
    acquisition_progress, AcquisitionProgress as CoreAcquisitionProgress, AcquisitionStage,
};
use tokio::task::AbortHandle;

use crate::errors::{AppError, Result};
use crate::remote_source::materializer::AaxcleanMaterializer;
use crate::remote_source::staging::RemoteSourceStaging;
use crate::remote_source::{types, AcquisitionJob as RemoteAcquisitionJob};

use super::{RemoteProviderId, RemoteSourceRuntime};

pub(super) struct RemoteAcquisitionLifecycle {
    pub(super) staging: RemoteSourceStaging,
    materializer: AaxcleanMaterializer,
    pub(super) jobs: Mutex<HashMap<String, RemoteAcquisitionJob>>,
    acquisition_tasks: Mutex<HashMap<String, AbortHandle>>,
}

impl RemoteAcquisitionLifecycle {
    pub(super) fn new(staging: RemoteSourceStaging, materializer: AaxcleanMaterializer) -> Self {
        Self {
            staging,
            materializer,
            jobs: Mutex::new(HashMap::new()),
            acquisition_tasks: Mutex::new(HashMap::new()),
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
        };
        self.jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .insert(job_id, job.clone());
        let spawned_job_id = job.job_id.clone();
        let abort_handle = tokio::spawn(async move {
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

    pub(super) fn update_job_progress(&self, job_id: &str, progress: CoreAcquisitionProgress) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                if job.status == types::RemoteAcquisitionStatus::Cancelled {
                    return;
                }
                job.progress = progress;
            }
        }
    }

    pub(super) fn replace_job_if_active(&self, job: RemoteAcquisitionJob) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if jobs.get(&job.job_id).is_some_and(|existing| {
                existing.status == types::RemoteAcquisitionStatus::Cancelled
            }) {
                return;
            }
            jobs.insert(job.job_id.clone(), job);
        }
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

    pub(super) fn mark_job_cancelled(&self, job_id: &str, provider_id: RemoteProviderId) {
        if let Ok(mut jobs) = self.jobs.lock() {
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

    fn store_acquisition_task(&self, job_id: &str, abort_handle: AbortHandle) {
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

    fn purge_session(&self, job_id: &str) -> Result<()> {
        self.abort_acquisition_task(job_id);
        self.staging.purge_session(job_id)?;
        self.jobs
            .lock()
            .map_err(|_| AppError::General("Remote acquisition job lock failed".to_string()))?
            .remove(job_id);
        Ok(())
    }
}

impl RemoteSourceRuntime {
    pub fn acquisition_status(&self, job_id: &str) -> Result<RemoteAcquisitionJob> {
        self.inner.lifecycle.acquisition_status(job_id)
    }

    pub fn cancel_acquisition(&self, job_id: &str) -> Result<RemoteAcquisitionJob> {
        self.inner.lifecycle.cancel_acquisition(job_id)
    }

    pub fn purge_session(&self, job_id: &str) -> Result<()> {
        self.inner.lifecycle.purge_session(job_id)
    }
}
