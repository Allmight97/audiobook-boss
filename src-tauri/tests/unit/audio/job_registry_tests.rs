use audiobook_boss_lib::audio::job_registry::{AggregateJobStatus, JobId, JobRegistry, JobState};
use std::sync::Arc;
use std::sync::atomic::Ordering;

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
                match peak.compare_exchange(
                    observed,
                    current,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
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
