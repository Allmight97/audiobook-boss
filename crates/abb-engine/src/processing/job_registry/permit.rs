use super::{JobId, JobRegistry};
use crate::errors::{AppError, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const PERMIT_CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(50);

impl JobRegistry {
    /// Registers a new job and acquires a semaphore permit.
    ///
    /// This method will block if max_concurrent jobs are already running.
    /// Returns the JobId and an owned permit that must be held for the
    /// duration of processing.
    #[cfg(test)]
    pub async fn register_job(&self) -> Result<(JobId, OwnedSemaphorePermit)> {
        self.register_job_with_external_cancel(None).await
    }

    pub async fn register_job_with_external_cancel(
        &self,
        external_cancel: Option<Arc<AtomicBool>>,
    ) -> Result<(JobId, OwnedSemaphorePermit)> {
        if external_cancelled(&external_cancel) {
            return Err(AppError::cancelled());
        }

        let job_id = JobId::new();
        let semaphore = {
            let mut admission = self.admission();
            admission.jobs.insert(job_id.0);
            admission.semaphore.clone()
        };
        let pending = PendingAdmission {
            registry: self,
            job_id,
        };

        let cancelled = || external_cancelled(&external_cancel);
        let acquired = acquire_permit(semaphore, &cancelled)
            .await
            .and_then(|permit| {
                if cancelled() {
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

/// Removes a job whose admission failed, was cancelled, or was dropped while
/// waiting, so an abandoned admission never blocks reconfiguration.
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
    cancelled: &impl Fn() -> bool,
) -> Result<OwnedSemaphorePermit> {
    let acquire = semaphore.acquire_owned();
    tokio::pin!(acquire);

    loop {
        if cancelled() {
            return Err(AppError::cancelled());
        }

        tokio::select! {
            permit = &mut acquire => {
                return permit
                    .map_err(|_| AppError::InvalidInput("Semaphore closed".to_string()));
            }
            _ = tokio::time::sleep(PERMIT_CANCEL_POLL_INTERVAL) => {}
        }
    }
}

fn external_cancelled(external_cancel: &Option<Arc<AtomicBool>>) -> bool {
    external_cancel
        .as_ref()
        .is_some_and(|flag| flag.load(Ordering::SeqCst))
}
