use super::{Job, JobId, JobRegistry};
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
    pub async fn register_job(&self) -> Result<(JobId, OwnedSemaphorePermit)> {
        self.register_job_with_external_cancel(None).await
    }

    pub async fn register_job_with_external_cancel(
        &self,
        external_cancel: Option<Arc<AtomicBool>>,
    ) -> Result<(JobId, OwnedSemaphorePermit)> {
        let honor_global = external_cancel.is_none();
        if (honor_global && self.global_cancel.load(Ordering::SeqCst))
            || external_cancelled(&external_cancel)
        {
            return Err(AppError::cancelled());
        }

        let semaphore = { self.semaphore.read().await.clone() };
        let permit = self
            .acquire_permit_with_external_cancel(semaphore, external_cancel.clone(), honor_global)
            .await?;

        if (honor_global && self.global_cancel.load(Ordering::SeqCst))
            || external_cancelled(&external_cancel)
        {
            drop(permit);
            return Err(AppError::cancelled());
        }

        let job_id = JobId::new();
        let job = Job::new(job_id);

        {
            let mut jobs = self.jobs.write().await;
            jobs.insert(job_id.0, job);
        }

        log::info!("Job {} registered and started", job_id);
        Ok((job_id, permit))
    }

    async fn acquire_permit_with_external_cancel(
        &self,
        semaphore: Arc<Semaphore>,
        external_cancel: Option<Arc<AtomicBool>>,
        honor_global: bool,
    ) -> Result<OwnedSemaphorePermit> {
        let acquire = semaphore.acquire_owned();
        tokio::pin!(acquire);

        loop {
            if (honor_global && self.global_cancel.load(Ordering::SeqCst))
                || external_cancelled(&external_cancel)
            {
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
}

fn external_cancelled(external_cancel: &Option<Arc<AtomicBool>>) -> bool {
    external_cancel
        .as_ref()
        .is_some_and(|flag| flag.load(Ordering::SeqCst))
}
