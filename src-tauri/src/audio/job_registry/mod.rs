//! Job Registry for parallel batch processing
//!
//! Provides concurrent job management using a semaphore-based approach
//! to limit simultaneous processing operations.

mod cancel;
mod types;

use crate::errors::{AppError, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};
use uuid::Uuid;

pub use cancel::CancellationChecker;
pub use types::{AggregateJobStatus, Job, JobId, JobState};

/// Registry for managing concurrent processing jobs
///
/// Uses a semaphore to limit the number of concurrent jobs and provides
/// per-job cancellation support with a global cancel-all capability.
pub struct JobRegistry {
    /// Active jobs indexed by their ID
    jobs: RwLock<HashMap<Uuid, Job>>,
    /// Semaphore limiting concurrent jobs
    semaphore: RwLock<Arc<Semaphore>>,
    /// Maximum number of concurrent jobs
    max_concurrent: AtomicUsize,
    /// Global cancellation flag (cancels all jobs when set)
    global_cancel: Arc<AtomicBool>,
}

impl JobRegistry {
    /// Creates a new JobRegistry with the specified concurrency limit
    pub fn new(max_concurrent: usize) -> Self {
        let effective_max = Self::normalize_max(max_concurrent);
        Self {
            jobs: RwLock::new(HashMap::new()),
            semaphore: RwLock::new(Arc::new(Semaphore::new(effective_max))),
            max_concurrent: AtomicUsize::new(effective_max),
            global_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Creates a JobRegistry with auto-detected concurrency (num_cpus / 2)
    pub fn auto() -> Self {
        let cores = num_cpus::get();
        let max_concurrent = Self::normalize_max(cores / 2);
        log::info!(
            "JobRegistry auto-configured: {} cores detected, max_concurrent = {}",
            cores,
            max_concurrent
        );
        Self::new(max_concurrent)
    }

    fn normalize_max(max: usize) -> usize {
        max.clamp(1, 8)
    }

    pub fn default_max() -> usize {
        let cores = num_cpus::get();
        Self::normalize_max(cores / 2)
    }

    /// Registers a new job and acquires a semaphore permit
    ///
    /// This method will block if max_concurrent jobs are already running.
    /// Returns the JobId and an owned permit that must be held for the
    /// duration of processing.
    pub async fn register_job(&self) -> Result<(JobId, OwnedSemaphorePermit)> {
        // Clear global cancel flag when starting a new batch
        // (only if no jobs are currently running)
        {
            let jobs = self.jobs.read().await;
            let running_count = jobs
                .values()
                .filter(|j| j.state == JobState::Running)
                .count();
            if running_count == 0 {
                self.global_cancel.store(false, Ordering::SeqCst);
            }
        }

        // Acquire semaphore permit (blocks if at capacity)
        let semaphore = { self.semaphore.read().await.clone() };
        let permit = semaphore
            .acquire_owned()
            .await
            .map_err(|_| AppError::InvalidInput("Semaphore closed".to_string()))?;

        let job_id = JobId::new();
        let mut job = Job::new(job_id);
        job.state = JobState::Running;

        // Insert job into registry
        {
            let mut jobs = self.jobs.write().await;
            jobs.insert(job_id.0, job);
        }

        log::info!("Job {} registered and started", job_id);
        Ok((job_id, permit))
    }

    /// Gets the cancellation flag for a specific job
    pub async fn get_cancel_flag(&self, job_id: JobId) -> Arc<AtomicBool> {
        let jobs = self.jobs.read().await;
        jobs.get(&job_id.0)
            .map(|j| j.cancel_flag.clone())
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)))
    }

    /// Checks if a specific job is cancelled (per-job OR global)
    pub async fn is_cancelled(&self, job_id: JobId) -> bool {
        // Check global cancel first
        if self.global_cancel.load(Ordering::SeqCst) {
            return true;
        }

        // Then check per-job flag
        let jobs = self.jobs.read().await;
        jobs.get(&job_id.0)
            .map(|j| j.cancel_flag.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    /// Returns a synchronous cancellation checker for use in processing loops
    pub async fn cancellation_checker(&self, job_id: JobId) -> CancellationChecker {
        let job_flag = self.get_cancel_flag(job_id).await;
        CancellationChecker {
            job_flag,
            global_flag: self.global_cancel.clone(),
        }
    }

    /// Cancels all active jobs
    pub fn cancel_all(&self) {
        log::info!("Cancelling all jobs");
        self.global_cancel.store(true, Ordering::SeqCst);
    }

    /// Cancels a specific job
    pub async fn cancel_job(&self, job_id: JobId) -> Result<()> {
        let jobs = self.jobs.read().await;
        if let Some(job) = jobs.get(&job_id.0) {
            job.cancel_flag.store(true, Ordering::SeqCst);
            log::info!("Job {} cancellation requested", job_id);
            Ok(())
        } else {
            Err(AppError::InvalidInput(format!("Job {} not found", job_id)))
        }
    }

    /// Marks a job as completed and removes it from active tracking
    pub async fn complete_job(&self, job_id: JobId) {
        let mut jobs = self.jobs.write().await;
        if jobs.remove(&job_id.0).is_some() {
            log::info!("Job {} completed", job_id);
        }
    }

    /// Marks a job as failed
    pub async fn fail_job(&self, job_id: JobId, error: String) {
        let mut jobs = self.jobs.write().await;
        if jobs.remove(&job_id.0).is_some() {
            log::error!("Job {} failed: {}", job_id, error);
        }
    }

    /// Gets aggregate progress information
    pub async fn get_aggregate_status(&self) -> AggregateJobStatus {
        let jobs = self.jobs.read().await;
        let active = jobs
            .values()
            .filter(|j| j.state == JobState::Running)
            .count();
        let total = jobs.len();

        AggregateJobStatus {
            active_jobs: active,
            total_jobs: total,
            max_concurrent: self.max_concurrent(),
        }
    }

    /// Lists all active job IDs
    pub async fn list_active_jobs(&self) -> Vec<JobId> {
        let jobs = self.jobs.read().await;
        jobs.values()
            .filter(|j| j.state == JobState::Running)
            .map(|j| j.id)
            .collect()
    }

    /// Returns the maximum concurrency setting
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent.load(Ordering::SeqCst)
    }

    /// Checks if global cancellation is active
    pub fn is_global_cancelled(&self) -> bool {
        self.global_cancel.load(Ordering::SeqCst)
    }

    /// Updates the maximum concurrency. Requires no active or queued jobs.
    pub async fn update_max_concurrent(&self, max: usize) -> Result<usize> {
        let effective = Self::normalize_max(max);
        let status = self.get_aggregate_status().await;
        if status.active_jobs > 0 {
            return Err(AppError::InvalidInput(
                "Cannot change max concurrency while jobs are active".to_string(),
            ));
        }

        {
            let mut semaphore = self.semaphore.write().await;
            *semaphore = Arc::new(Semaphore::new(effective));
        }
        self.max_concurrent.store(effective, Ordering::SeqCst);
        Ok(effective)
    }

    /// Resets max concurrency to the auto-detected default when idle.
    pub async fn reset_to_auto(&self) -> Result<usize> {
        self.update_max_concurrent(Self::default_max()).await
    }
}
