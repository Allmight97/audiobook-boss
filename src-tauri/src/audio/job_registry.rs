//! Job Registry for parallel batch processing
//!
//! Provides concurrent job management using a semaphore-based approach
//! to limit simultaneous processing operations.

use crate::errors::{AppError, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};
use uuid::Uuid;

/// Unique identifier for a processing job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub Uuid);

impl JobId {
    /// Creates a new unique JobId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parses a JobId from a string
    pub fn parse(s: &str) -> Result<Self> {
        let uuid = Uuid::parse_str(s)
            .map_err(|e| AppError::InvalidInput(format!("Invalid job ID: {}", e)))?;
        Ok(Self(uuid))
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// State of a processing job
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobState {
    /// Job is waiting for a semaphore permit
    Queued,
    /// Job is actively processing
    Running,
    /// Job completed successfully
    Completed,
    /// Job was cancelled by user
    Cancelled,
    /// Job failed with an error
    Failed(String),
}

/// Represents a single processing job
#[derive(Debug)]
pub struct Job {
    /// Unique job identifier
    pub id: JobId,
    /// Current job state
    pub state: JobState,
    /// Per-job cancellation flag
    pub cancel_flag: Arc<AtomicBool>,
}

impl Job {
    /// Creates a new job with the given ID
    pub fn new(id: JobId) -> Self {
        Self {
            id,
            state: JobState::Queued,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

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
        if let Some(job) = jobs.get_mut(&job_id.0) {
            job.state = JobState::Completed;
            log::info!("Job {} completed", job_id);
        }
        // Remove from registry to prevent memory leaks
        jobs.remove(&job_id.0);
    }

    /// Marks a job as failed
    pub async fn fail_job(&self, job_id: JobId, error: String) {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(&job_id.0) {
            job.state = JobState::Failed(error.clone());
            log::error!("Job {} failed: {}", job_id, error);
        }
        jobs.remove(&job_id.0);
    }

    /// Gets aggregate progress information
    pub async fn get_aggregate_status(&self) -> AggregateJobStatus {
        let jobs = self.jobs.read().await;
        let active = jobs
            .values()
            .filter(|j| j.state == JobState::Running)
            .count();
        let queued = jobs
            .values()
            .filter(|j| j.state == JobState::Queued)
            .count();
        let total = jobs.len();

        AggregateJobStatus {
            active_jobs: active,
            queued_jobs: queued,
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
        if status.active_jobs > 0 || status.queued_jobs > 0 {
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

/// Synchronous cancellation checker for use in tight processing loops
///
/// This avoids async overhead when checking cancellation frequently
pub struct CancellationChecker {
    job_flag: Arc<AtomicBool>,
    global_flag: Arc<AtomicBool>,
}

impl CancellationChecker {
    /// Checks if processing should be cancelled (job-specific OR global)
    pub fn is_cancelled(&self) -> bool {
        self.global_flag.load(Ordering::Relaxed) || self.job_flag.load(Ordering::Relaxed)
    }
}

/// Aggregate status of all jobs in the registry
#[derive(Debug, Clone)]
pub struct AggregateJobStatus {
    /// Number of actively processing jobs
    pub active_jobs: usize,
    /// Number of queued jobs waiting for permits
    pub queued_jobs: usize,
    /// Total jobs in registry
    pub total_jobs: usize,
    /// Maximum concurrent jobs allowed
    pub max_concurrent: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_id_uniqueness() {
        let id1 = JobId::new();
        let id2 = JobId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_job_id_parse() {
        let id = JobId::new();
        let id_str = id.to_string();
        let parsed = JobId::parse(&id_str).expect("Should parse valid UUID");
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_job_id_parse_invalid() {
        let result = JobId::parse("not-a-uuid");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_registry_new() {
        let registry = JobRegistry::new(2);
        assert_eq!(registry.max_concurrent(), 2);
    }

    #[tokio::test]
    async fn test_registry_auto() {
        let registry = JobRegistry::auto();
        // Should be between 1 and 8
        assert!(registry.max_concurrent() >= 1);
        assert!(registry.max_concurrent() <= 8);
    }

    #[tokio::test]
    async fn test_register_job() {
        let registry = JobRegistry::new(2);
        let (job_id, _permit) = registry.register_job().await.expect("Should register job");

        let status = registry.get_aggregate_status().await;
        assert_eq!(status.active_jobs, 1);
        assert_eq!(status.total_jobs, 1);

        // Verify job ID is valid
        assert!(!job_id.to_string().is_empty());
    }

    #[tokio::test]
    async fn test_complete_job_removes_from_registry() {
        let registry = JobRegistry::new(2);
        let (job_id, _permit) = registry.register_job().await.expect("Should register job");

        registry.complete_job(job_id).await;

        let status = registry.get_aggregate_status().await;
        assert_eq!(status.active_jobs, 0);
        assert_eq!(status.total_jobs, 0);
    }

    #[tokio::test]
    async fn test_cancel_all() {
        let registry = JobRegistry::new(2);
        let (job_id, _permit) = registry.register_job().await.expect("Should register job");

        assert!(!registry.is_cancelled(job_id).await);

        registry.cancel_all();

        assert!(registry.is_cancelled(job_id).await);
        assert!(registry.is_global_cancelled());
    }

    #[tokio::test]
    async fn test_cancel_specific_job() {
        let registry = JobRegistry::new(2);
        let (job_id1, _permit1) = registry.register_job().await.expect("Should register job");
        let (job_id2, _permit2) = registry.register_job().await.expect("Should register job");

        // Cancel only job1
        registry
            .cancel_job(job_id1)
            .await
            .expect("Should cancel job");

        // job1 should be cancelled, job2 should not
        assert!(registry.is_cancelled(job_id1).await);
        assert!(!registry.is_cancelled(job_id2).await);
    }

    #[tokio::test]
    async fn test_cancellation_checker() {
        let registry = JobRegistry::new(2);
        let (job_id, _permit) = registry.register_job().await.expect("Should register job");

        let checker = registry.cancellation_checker(job_id).await;
        assert!(!checker.is_cancelled());

        registry.cancel_all();
        assert!(checker.is_cancelled());
    }

    #[tokio::test]
    async fn test_semaphore_limits_concurrent_jobs() {
        let registry = Arc::new(JobRegistry::new(2));

        // Register 2 jobs - should succeed immediately
        let (job_id1, permit1) = registry.register_job().await.expect("Job 1");
        let (job_id2, permit2) = registry.register_job().await.expect("Job 2");

        let status = registry.get_aggregate_status().await;
        assert_eq!(status.active_jobs, 2);

        // Complete job1 to release permit
        drop(permit1);
        registry.complete_job(job_id1).await;

        // Now we can register another job
        let (_job_id3, _permit3) = registry.register_job().await.expect("Job 3");

        // Cleanup
        drop(permit2);
        registry.complete_job(job_id2).await;
    }

    #[tokio::test]
    async fn test_fail_job() {
        let registry = JobRegistry::new(2);
        let (job_id, _permit) = registry.register_job().await.expect("Should register job");

        registry.fail_job(job_id, "Test error".to_string()).await;

        let status = registry.get_aggregate_status().await;
        assert_eq!(status.total_jobs, 0); // Job removed after failure
    }

    #[tokio::test]
    async fn test_update_max_when_idle() {
        let registry = JobRegistry::new(2);
        let updated = registry
            .update_max_concurrent(4)
            .await
            .expect("Should update when idle");
        assert_eq!(updated, 4);
        assert_eq!(registry.max_concurrent(), 4);
    }

    #[tokio::test]
    async fn test_update_max_rejected_when_active() {
        let registry = Arc::new(JobRegistry::new(2));
        let (_job_id, _permit) = registry.register_job().await.expect("Should register job");

        let result = registry.update_max_concurrent(1).await;
        assert!(result.is_err(), "Should not allow update while active");
        assert_eq!(registry.max_concurrent(), 2);
    }
}
