mod cancel;
mod permit;
#[cfg(test)]
mod tests;
mod types;

use crate::errors::{AppError, Result};
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::Semaphore;
use tokio::task::{Id, JoinSet};
use uuid::Uuid;

pub use cancel::CancellationChecker;
#[cfg(test)]
use types::AggregateJobStatus;
pub use types::{JobId, MaxConcurrentJobsCapabilities};

pub const MIN_CONCURRENT_JOBS: usize = 1;
pub const MAX_CONCURRENT_JOBS: usize = 8;

pub struct JobRegistry {
    admission: Mutex<Admission>,
    max_concurrent: AtomicUsize,
}

struct Admission {
    jobs: HashSet<Uuid>,
    semaphore: Arc<Semaphore>,
    runs: usize,
}

pub(crate) struct ActiveRun {
    registry: Arc<JobRegistry>,
}

impl Drop for ActiveRun {
    fn drop(&mut self) {
        let mut admission = self.registry.admission();
        admission.runs = admission.runs.saturating_sub(1);
    }
}

pub struct BatchScheduler<'a> {
    registry: &'a JobRegistry,
}

impl<'a> BatchScheduler<'a> {
    fn new(registry: &'a JobRegistry) -> Self {
        Self { registry }
    }

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
    pub fn new(max_concurrent: usize) -> Self {
        let effective_max = Self::normalize_max(max_concurrent);
        Self {
            admission: Mutex::new(Admission {
                jobs: HashSet::new(),
                semaphore: Arc::new(Semaphore::new(effective_max)),
                runs: 0,
            }),
            max_concurrent: AtomicUsize::new(effective_max),
        }
    }

    pub fn auto() -> Self {
        let cores = Self::detected_cores();
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

    /// Logical cores the process may use; respects cgroup/affinity limits and
    /// falls back to 1 on platforms that can't report it.
    fn detected_cores() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }

    pub fn default_max() -> usize {
        let cores = Self::detected_cores();
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

    pub fn scheduler(&self) -> BatchScheduler<'_> {
        BatchScheduler::new(self)
    }

    pub async fn complete_job(&self, job_id: JobId) {
        if self.remove_job(job_id) {
            log::info!("Job {} completed", job_id);
        }
    }

    pub async fn fail_job(&self, job_id: JobId, error: String) {
        if self.remove_job(job_id) {
            log::error!("Job {} failed: {}", job_id, error);
        }
    }

    pub(crate) fn hold_run(self: &Arc<Self>) -> ActiveRun {
        self.admission().runs += 1;
        ActiveRun {
            registry: Arc::clone(self),
        }
    }

    fn admission(&self) -> MutexGuard<'_, Admission> {
        // Admission holds no invariant a panicking holder could half-apply.
        self.admission
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn remove_job(&self, job_id: JobId) -> bool {
        self.admission().jobs.remove(&job_id.0)
    }

    #[cfg(test)]
    pub async fn get_aggregate_status(&self) -> AggregateJobStatus {
        let total = self.admission().jobs.len();

        AggregateJobStatus {
            active_jobs: total,
            total_jobs: total,
        }
    }

    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent.load(Ordering::SeqCst)
    }

    pub async fn update_max_concurrent(&self, max: usize) -> Result<usize> {
        let effective = Self::normalize_max(max);
        let mut admission = self.admission();
        if !admission.jobs.is_empty() || admission.runs > 0 {
            return Err(AppError::InvalidInput(
                "Cannot change max concurrency while jobs are active".to_string(),
            ));
        }
        admission.semaphore = Arc::new(Semaphore::new(effective));
        self.max_concurrent.store(effective, Ordering::SeqCst);
        Ok(effective)
    }

    pub async fn reset_to_auto(&self) -> Result<usize> {
        self.update_max_concurrent(Self::default_max()).await
    }
}

#[cfg(test)]
mod behavior_tests;
