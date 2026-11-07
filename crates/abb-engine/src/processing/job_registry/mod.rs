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
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub use types::{JobId, MaxConcurrentJobsCapabilities};

pub const MIN_CONCURRENT_JOBS: usize = 1;
pub const MAX_CONCURRENT_JOBS: usize = 8;

pub struct JobRegistry {
    admission: Mutex<Admission>,
    max_concurrent: AtomicUsize,
    lock_observer: Mutex<Option<LockObserver>>,
}

pub(crate) type LockObserver = Arc<dyn Fn() + Send + Sync>;

struct Admission {
    jobs: HashSet<Uuid>,
    semaphore: Arc<Semaphore>,
    runs: usize,
    announced: bool,
}

impl Admission {
    fn locked(&self) -> bool {
        !self.jobs.is_empty() || self.runs > 0
    }
}

pub(crate) struct ActiveRun {
    registry: Arc<JobRegistry>,
}

impl Drop for ActiveRun {
    fn drop(&mut self) {
        let mut admission = self.registry.admission();
        admission.runs = admission.runs.saturating_sub(1);
        self.registry.announce_lock(admission);
    }
}

impl JobRegistry {
    /// Runs the futures `max_concurrent` at a time, in order. A queued future
    /// whose token is cancelled starts at once, so it fails fast instead of
    /// waiting behind the futures ahead of it.
    pub async fn run_batch<R, Fut>(
        &self,
        tasks: Vec<(Fut, Option<CancellationToken>)>,
    ) -> Vec<Result<R>>
    where
        R: Send + 'static,
        Fut: Future<Output = Result<R>> + Send + 'static,
    {
        if tasks.is_empty() {
            return Vec::new();
        }

        let max_in_flight = self.max_concurrent().max(1);
        let total_tasks = tasks.len();
        let mut cancels: Vec<Option<CancellationToken>> = Vec::with_capacity(total_tasks);
        let mut pending: VecDeque<(usize, Fut)> = VecDeque::with_capacity(total_tasks);
        for (index, (future, cancel)) in tasks.into_iter().enumerate() {
            cancels.push(cancel);
            pending.push_back((index, future));
        }
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

        let mut cancelled_queue: JoinSet<usize> = JoinSet::new();
        for &(index, _) in &pending {
            if let Some(token) = cancels[index].clone() {
                cancelled_queue.spawn(async move {
                    token.cancelled().await;
                    index
                });
            }
        }

        while !join_set.is_empty() {
            let joined = tokio::select! {
                joined = join_set.join_next_with_id() => joined.expect("join set is not empty"),
                Some(Ok(index)) = cancelled_queue.join_next() => {
                    if let Some(position) = pending.iter().position(|(queued, _)| *queued == index) {
                        if let Some((index, future)) = pending.remove(position) {
                            spawn_indexed_task(&mut join_set, &mut task_indices, index, future);
                        }
                    }
                    continue;
                }
            };
            match joined {
                Ok((task_id, outcome)) => {
                    let index = task_indices
                        .remove(&task_id)
                        .expect("every spawned task records its index");
                    ordered_results[index] = Some(outcome);
                }
                Err(join_error) => {
                    let error = AppError::General(format!("Batch task join error: {join_error}"));
                    let task_id = join_error.id();
                    let index = task_indices
                        .remove(&task_id)
                        .expect("every spawned task records its index");
                    ordered_results[index] = Some(Err(error));
                    log::error!(
                        "Batch task join error preserved task index {} via task id {}",
                        index,
                        task_id
                    );
                }
            }

            while join_set.len() < max_in_flight {
                let Some((next_index, next_future)) = pending.pop_front() else {
                    break;
                };
                spawn_indexed_task(&mut join_set, &mut task_indices, next_index, next_future);
            }
        }

        ordered_results
            .into_iter()
            .map(|value| value.expect("every task index gets a result"))
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
                announced: false,
            }),
            max_concurrent: AtomicUsize::new(effective_max),
            lock_observer: Mutex::new(None),
        }
    }

    pub fn auto() -> Self {
        let max_concurrent = Self::default_max();
        log::info!("JobRegistry auto-configured: max_concurrent = {max_concurrent}");
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
            auto_effective: Self::default_max(),
            fixed_min: MIN_CONCURRENT_JOBS,
            fixed_max: MAX_CONCURRENT_JOBS,
            fixed_options: (MIN_CONCURRENT_JOBS..=MAX_CONCURRENT_JOBS).collect(),
        }
    }

    pub fn complete_job(&self, job_id: JobId) {
        if self.remove_job(job_id) {
            log::info!("Job {} completed", job_id);
        }
    }

    pub fn fail_job(&self, job_id: JobId, error: String) {
        if self.remove_job(job_id) {
            log::info!("Job {} failed: {}", job_id, error);
        }
    }

    pub(crate) fn hold_run(self: &Arc<Self>) -> ActiveRun {
        let mut admission = self.admission();
        admission.runs += 1;
        self.announce_lock(admission);
        ActiveRun {
            registry: Arc::clone(self),
        }
    }

    fn admission(&self) -> MutexGuard<'_, Admission> {
        self.admission
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn remove_job(&self, job_id: JobId) -> bool {
        let mut admission = self.admission();
        let removed = admission.jobs.remove(&job_id.0);
        self.announce_lock(admission);
        removed
    }

    /// Whether the concurrency limit is refused right now: a job or a run is
    /// active.
    pub(crate) fn concurrency_locked(&self) -> bool {
        self.admission().locked()
    }

    /// Calls `observer` after every change of `concurrency_locked`, with no
    /// registry lock held.
    pub(crate) fn observe_concurrency_lock(&self, observer: LockObserver) {
        *self
            .lock_observer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(observer);
    }

    fn announce_lock(&self, mut admission: MutexGuard<'_, Admission>) {
        let locked = admission.locked();
        let changed = std::mem::replace(&mut admission.announced, locked) != locked;
        drop(admission);
        if !changed {
            return;
        }
        let observer = self
            .lock_observer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(observer) = observer {
            observer();
        }
    }

    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent.load(Ordering::SeqCst)
    }

    pub fn update_max_concurrent(&self, max: usize) -> Result<usize> {
        let effective = Self::normalize_max(max);
        let mut admission = self.admission();
        if admission.locked() {
            return Err(AppError::InvalidInput(
                "Cannot change max concurrency while jobs are active".to_string(),
            ));
        }
        admission.semaphore = Arc::new(Semaphore::new(effective));
        self.max_concurrent.store(effective, Ordering::SeqCst);
        Ok(effective)
    }

    pub fn reset_to_auto(&self) -> Result<usize> {
        self.update_max_concurrent(Self::default_max())
    }
}

#[cfg(test)]
mod behavior_tests;
