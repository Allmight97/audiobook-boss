use super::{JobId, JobRegistry};
use crate::errors::{AppError, Result};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

impl JobRegistry {
    #[cfg(test)]
    pub(crate) fn active_jobs(&self) -> usize {
        self.admission().jobs.len()
    }

    #[cfg(test)]
    pub async fn register_job(&self) -> Result<(JobId, OwnedSemaphorePermit)> {
        self.register_job_with_external_cancel(&CancellationToken::new())
            .await
    }

    /// Tracks a job that is not an encode: it counts as active work but takes
    /// no concurrency permit, so it never waits behind one.
    pub fn register_job_without_permit(&self) -> JobId {
        let job_id = JobId::new();
        let mut admission = self.admission();
        admission.jobs.insert(job_id.0);
        self.announce_lock(admission);
        log::info!("Job {} registered without a permit", job_id);
        job_id
    }

    pub async fn register_job_with_external_cancel(
        &self,
        external_cancel: &CancellationToken,
    ) -> Result<(JobId, OwnedSemaphorePermit)> {
        if external_cancel.is_cancelled() {
            return Err(AppError::cancelled());
        }

        let job_id = JobId::new();
        let semaphore = {
            let mut admission = self.admission();
            admission.jobs.insert(job_id.0);
            let semaphore = admission.semaphore.clone();
            self.announce_lock(admission);
            semaphore
        };
        let pending = PendingAdmission {
            registry: self,
            job_id,
        };

        let acquired = acquire_permit(semaphore, external_cancel)
            .await
            .and_then(|permit| {
                if external_cancel.is_cancelled() {
                    Err(AppError::cancelled())
                } else {
                    Ok(permit)
                }
            });
        let permit = acquired?;
        std::mem::forget(pending);
        log::info!("Job {} registered", job_id);
        Ok((job_id, permit))
    }
}

struct PendingAdmission<'a> {
    registry: &'a JobRegistry,
    job_id: JobId,
}

impl Drop for PendingAdmission<'_> {
    fn drop(&mut self) {
        self.registry.remove_job(self.job_id);
    }
}

async fn acquire_permit(
    semaphore: Arc<Semaphore>,
    cancel: &CancellationToken,
) -> Result<OwnedSemaphorePermit> {
    let acquire = semaphore.acquire_owned();
    tokio::pin!(acquire);
    let cancelled = cancel.cancelled();
    tokio::pin!(cancelled);

    tokio::select! {
        biased;
        () = &mut cancelled => Err(AppError::cancelled()),
        permit = &mut acquire => permit
            .map_err(|_| AppError::InvalidInput("Semaphore closed".to_string())),
    }
}
