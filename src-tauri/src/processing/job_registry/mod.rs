//! Job Registry for parallel batch processing
//!
//! Provides concurrent job management using a semaphore-based approach
//! to limit simultaneous processing operations.

mod cancel;
mod permit;
#[cfg(test)]
mod tests;
mod types;

use crate::errors::{AppError, Result};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::task::{Id, JoinSet};
use uuid::Uuid;

pub use cancel::CancellationChecker;
pub use types::{AggregateJobStatus, Job, JobId, JobState, MaxConcurrentJobsCapabilities};

pub const MIN_CONCURRENT_JOBS: usize = 1;
pub const MAX_CONCURRENT_JOBS: usize = 8;

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

/// Internal batch scheduler facade backed by JobRegistry concurrency settings.
pub struct BatchScheduler<'a> {
    registry: &'a JobRegistry,
}

impl<'a> BatchScheduler<'a> {
    fn new(registry: &'a JobRegistry) -> Self {
        Self { registry }
    }

    /// Runs a batch of futures with bounded in-flight concurrency.
    ///
    /// Tasks continue to be scheduled even when earlier tasks fail so callers
    /// can receive complete, deterministic per-index outcomes.
    pub async fn run_batch<R, Fut>(&self, futures: Vec<Fut>) -> Vec<Result<R>>
    where
        R: Send + 'static,
        Fut: Future<Output = Result<R>> + Send + 'static,
    {
        if futures.is_empty() {
            return Vec::new();
        }

        let max_in_flight = self.registry.max_concurrent().max(1);
        let total_tasks = futures.len();
        let mut pending: VecDeque<(usize, Fut)> = futures.into_iter().enumerate().collect();
        let mut join_set: JoinSet<Result<R>> = JoinSet::new();
        let mut task_indices: HashMap<Id, usize> = HashMap::with_capacity(total_tasks);
        let mut ordered_results: Vec<Option<Result<R>>> = Vec::with_capacity(total_tasks);
        ordered_results.resize_with(total_tasks, || None);

        for _ in 0..max_in_flight {
            if let Some((index, future)) = pending.pop_front() {
                spawn_indexed_task(&mut join_set, &mut task_indices, index, future);
            } else {
                break;
            }
        }

        while let Some(joined) = join_set.join_next_with_id().await {
            match joined {
                Ok((task_id, outcome)) => {
                    let Some(index) = task_indices.remove(&task_id) else {
                        log::error!(
                            "Batch scheduler completed task id {} without an index mapping",
                            task_id
                        );
                        continue;
                    };
                    ordered_results[index] = Some(outcome);
                }
                Err(join_error) => {
                    let error = AppError::General(format!("Batch task join error: {join_error}"));
                    let task_id = join_error.id();
                    if let Some(index) = task_indices.remove(&task_id) {
                        let slot = ordered_results
                            .get_mut(index)
                            .expect("task index should be within ordered result bounds");
                        *slot = Some(Err(error));
                        log::error!(
                            "Batch task join error preserved task index {} via task id {}",
                            index,
                            task_id
                        );
                    } else {
                        log::error!(
                            "Batch task join error for task id {} had no tracked input index",
                            task_id
                        );
                    }
                }
            }

            if let Some((next_index, next_future)) = pending.pop_front() {
                spawn_indexed_task(&mut join_set, &mut task_indices, next_index, next_future);
            }
        }

        ordered_results
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                value.unwrap_or_else(|| {
                    Err(AppError::General(format!(
                        "Batch scheduler missing result for task index {index}"
                    )))
                })
            })
            .collect()
    }
}

fn spawn_indexed_task<R, Fut>(
    join_set: &mut JoinSet<Result<R>>,
    task_indices: &mut HashMap<Id, usize>,
    index: usize,
    future: Fut,
) where
    R: Send + 'static,
    Fut: Future<Output = Result<R>> + Send + 'static,
{
    let abort_handle = join_set.spawn(future);
    task_indices.insert(abort_handle.id(), index);
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
        max.clamp(MIN_CONCURRENT_JOBS, MAX_CONCURRENT_JOBS)
    }

    pub fn default_max() -> usize {
        let cores = num_cpus::get();
        Self::normalize_max(cores / 2)
    }

    pub fn max_concurrent_jobs_capabilities() -> MaxConcurrentJobsCapabilities {
        MaxConcurrentJobsCapabilities {
            allow_auto: true,
            auto_effective: Self::default_max(),
            fixed_min: MIN_CONCURRENT_JOBS,
            fixed_max: MAX_CONCURRENT_JOBS,
            fixed_options: (MIN_CONCURRENT_JOBS..=MAX_CONCURRENT_JOBS).collect(),
        }
    }

    /// Returns a scheduler facade for bounded batch task orchestration.
    pub fn scheduler(&self) -> BatchScheduler<'_> {
        BatchScheduler::new(self)
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
            operation_flag: None,
            honor_global: true,
        }
    }

    /// Cancels all active jobs
    pub fn cancel_all(&self) {
        log::info!("Cancelling all jobs");
        self.global_cancel.store(true, Ordering::SeqCst);
    }

    /// Clears the global cancellation flag for a new top-level processing command.
    pub fn reset_global_cancel(&self) {
        self.global_cancel.store(false, Ordering::SeqCst);
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
