use audiobook_boss_lib::processing::job_registry::{
    AggregateJobStatus, JobId, JobRegistry, JobState,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;

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
async fn test_cancel_all_blocks_queued_registration_until_explicit_reset() {
    use tokio::sync::oneshot;
    use tokio::time::{sleep, Duration};

    let registry = Arc::new(JobRegistry::new(1));
    let (job_id1, permit1) = registry
        .register_job()
        .await
        .expect("Should register job 1");

    let queued_registry = Arc::clone(&registry);
    let (queued_tx, queued_rx) = oneshot::channel();
    tokio::spawn(async move {
        let result = queued_registry.register_job().await;
        let _ = queued_tx.send(result);
    });

    sleep(Duration::from_millis(20)).await;

    registry.cancel_all();
    assert!(registry.is_global_cancelled());

    drop(permit1);
    registry.complete_job(job_id1).await;

    let queued_result = queued_rx
        .await
        .expect("queued registration should complete");

    assert!(
        registry.is_global_cancelled(),
        "queued registration must not clear cancel-all"
    );
    assert!(
        queued_result.is_err(),
        "queued registration should fail once cancel-all is active"
    );

    let direct_retry = registry.register_job().await;
    assert!(
        direct_retry.is_err(),
        "new registration should stay blocked until a fresh command resets cancel-all"
    );

    registry.reset_global_cancel();
    assert!(
        !registry.is_global_cancelled(),
        "explicit reset should clear the global cancellation flag"
    );

    let (_job_id2, _permit2) = registry
        .register_job()
        .await
        .expect("fresh registration should succeed");
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

#[tokio::test]
async fn test_stress_concurrent_registration_respects_limit() {
    use std::sync::atomic::AtomicUsize;
    use tokio::time::Duration;

    let registry = Arc::new(JobRegistry::new(3));
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..50 {
        let reg = Arc::clone(&registry);
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        handles.push(tokio::spawn(async move {
            let (job_id, permit) = reg.register_job().await.expect("register");
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            let mut observed = peak.load(Ordering::SeqCst);
            while current > observed {
                match peak.compare_exchange(observed, current, Ordering::SeqCst, Ordering::SeqCst) {
                    Ok(_) => break,
                    Err(new_obs) => observed = new_obs,
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
            active.fetch_sub(1, Ordering::SeqCst);
            drop(permit);
            reg.complete_job(job_id).await;
        }));
    }

    for handle in handles {
        handle.await.expect("join");
    }

    assert!(
        peak.load(Ordering::SeqCst) <= 3,
        "max concurrent observed should respect limit"
    );
    let status = registry.get_aggregate_status().await;
    assert_eq!(status.active_jobs, 0);
    assert_eq!(status.total_jobs, 0);
}

#[tokio::test]
async fn test_scheduler_respects_max_in_flight() {
    use std::sync::atomic::AtomicUsize;
    use tokio::time::{sleep, Duration};

    let registry = JobRegistry::new(2);
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));

    let mut futures = Vec::new();
    for _ in 0..12 {
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        futures.push(async move {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            let mut observed = peak.load(Ordering::SeqCst);
            while current > observed {
                match peak.compare_exchange(observed, current, Ordering::SeqCst, Ordering::SeqCst) {
                    Ok(_) => break,
                    Err(new_obs) => observed = new_obs,
                }
            }

            sleep(Duration::from_millis(10)).await;
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(1usize)
        });
    }

    let results = registry.scheduler().run_batch(futures).await;

    assert_eq!(results.len(), 12);
    assert!(results.into_iter().all(|item| item.is_ok()));
    assert!(
        peak.load(Ordering::SeqCst) <= 2,
        "scheduler should not exceed max_concurrent"
    );
}

#[tokio::test]
async fn test_scheduler_preserves_input_order() {
    use tokio::time::{sleep, Duration};

    let registry = JobRegistry::new(2);
    let mut futures = Vec::new();

    for index in 0..5usize {
        futures.push(async move {
            sleep(Duration::from_millis((4 - index as u64) * 5)).await;
            Ok(index)
        });
    }

    let results = registry.scheduler().run_batch(futures).await;

    let ordered: Vec<usize> = results
        .into_iter()
        .map(|entry| entry.expect("expected success outcome"))
        .collect();

    assert_eq!(ordered, vec![0, 1, 2, 3, 4]);
}

#[tokio::test]
async fn test_scheduler_continues_scheduling_after_error_and_preserves_order() {
    use std::sync::atomic::AtomicUsize;
    use tokio::time::{sleep, Duration};

    let registry = JobRegistry::new(2);
    let started = Arc::new(AtomicUsize::new(0));
    let mut futures = Vec::new();

    for index in 0..6usize {
        let started = Arc::clone(&started);
        futures.push(async move {
            started.fetch_add(1, Ordering::SeqCst);

            if index == 1 {
                sleep(Duration::from_millis(5)).await;
                return JobId::parse("not-a-uuid").map(|_| index);
            }

            sleep(Duration::from_millis(30)).await;
            Ok(index)
        });
    }

    let outcomes = registry.scheduler().run_batch(futures).await;
    assert_eq!(outcomes.len(), 6);
    assert_eq!(
        started.load(Ordering::SeqCst),
        6,
        "scheduler should enqueue all work despite task failures"
    );

    assert!(outcomes[0].is_ok());
    assert!(outcomes[1].is_err());
    assert!(outcomes[2].is_ok());
    assert!(outcomes[3].is_ok());
    assert!(outcomes[4].is_ok());
    assert!(outcomes[5].is_ok());
}

#[tokio::test]
async fn test_scheduler_preserves_index_for_panicking_tasks() {
    use std::sync::atomic::AtomicUsize;
    use tokio::time::{sleep, Duration};

    let registry = JobRegistry::new(2);
    let started = Arc::new(AtomicUsize::new(0));
    let mut futures = Vec::new();

    for index in 0..4usize {
        let started = Arc::clone(&started);
        futures.push(async move {
            started.fetch_add(1, Ordering::SeqCst);

            if index == 0 {
                sleep(Duration::from_millis(30)).await;
                return Ok(index);
            }

            if index == 1 {
                panic!("scheduler panic regression");
            }

            sleep(Duration::from_millis(5)).await;
            Ok(index)
        });
    }

    let outcomes = registry.scheduler().run_batch(futures).await;

    assert_eq!(outcomes.len(), 4);
    assert_eq!(
        started.load(Ordering::SeqCst),
        4,
        "scheduler should continue scheduling after a task panic"
    );
    assert_eq!(outcomes[0].as_ref().expect("index 0 should succeed"), &0);
    assert!(
        outcomes[1].is_err(),
        "panic should map to the panicking task index"
    );
    assert_eq!(outcomes[2].as_ref().expect("index 2 should succeed"), &2);
    assert_eq!(outcomes[3].as_ref().expect("index 3 should succeed"), &3);
}

#[test]
fn aggregate_job_status_struct() {
    let status = AggregateJobStatus {
        active_jobs: 1,
        total_jobs: 2,
        max_concurrent: 3,
    };
    assert_eq!(status.active_jobs, 1);
    assert_eq!(status.total_jobs, 2);
    assert_eq!(status.max_concurrent, 3);
}

#[test]
fn job_state_variants_exist() {
    assert_eq!(JobState::Running, JobState::Running);
    assert_ne!(JobState::Running, JobState::Completed);
    assert_ne!(JobState::Completed, JobState::Cancelled);
}
