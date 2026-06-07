use std::path::PathBuf;

use crate::errors::AppError;
use crate::remote_source::{types, AcquisitionJob as RemoteAcquisitionJob};
use abb_remote_source_core::{
    acquisition_progress, AcquisitionProgress as CoreAcquisitionProgress, AcquisitionStage,
};
use tokio::task::AbortHandle;

use super::{RemoteProviderId, RemoteSourceRuntime};

impl RemoteSourceRuntime {
    pub(super) fn cleanup_logout_sessions_without_handoff(&self) -> crate::errors::Result<()> {
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

    pub(super) async fn run_acquisition_job(
        &self,
        plan: super::AcquisitionPlan,
        job_id: String,
        job_dir: PathBuf,
    ) {
        let result = match plan.provider_id {
            super::RemoteProviderId::Audible => {
                super::providers::audible::AudibleProvider::acquire(
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

    pub(super) fn update_job_progress(&self, job_id: &str, progress: CoreAcquisitionProgress) {
        if let Ok(mut jobs) = self.inner.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                if job.status == types::RemoteAcquisitionStatus::Cancelled {
                    return;
                }
                job.progress = progress;
            }
        }
    }

    pub(super) fn replace_job_if_active(&self, job: RemoteAcquisitionJob) {
        if let Ok(mut jobs) = self.inner.jobs.lock() {
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

    pub(super) fn mark_job_cancelled(&self, job_id: &str, provider_id: RemoteProviderId) {
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

    pub(super) fn job_is_cancelled(&self, job_id: &str) -> bool {
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

    pub(super) fn job_is_active(&self, job_id: &str) -> bool {
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

    pub(super) fn store_acquisition_task(&self, job_id: &str, abort_handle: AbortHandle) {
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

    pub(super) fn remove_acquisition_task(&self, job_id: &str) {
        if let Ok(mut tasks) = self.inner.acquisition_tasks.lock() {
            tasks.remove(job_id);
        }
    }

    pub(super) fn abort_acquisition_task(&self, job_id: &str) {
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

    pub(super) fn abort_all_acquisition_tasks(&self) {
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

    pub(super) fn cleanup_cancelled_job_session(&self, job_id: &str) {
        if let Err(error) = self.inner.staging.purge_session(job_id) {
            log::warn!(
                "remote_source acquisition job_id={} cancelled_session_cleanup_failed={}",
                job_id,
                error
            );
        }
    }

    pub fn acquisition_status(&self, job_id: &str) -> crate::errors::Result<RemoteAcquisitionJob> {
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

    pub fn cancel_acquisition(&self, job_id: &str) -> crate::errors::Result<RemoteAcquisitionJob> {
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

    pub fn purge_session(&self, job_id: &str) -> crate::errors::Result<()> {
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
