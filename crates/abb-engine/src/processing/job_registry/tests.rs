use super::JobRegistry;
use crate::errors::AppError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn external_cancel_interrupts_waiting_permit_acquire() {
    let registry = JobRegistry::new(1);
    let (_job_id, _held_permit) = registry.register_job().await.expect("first job");
    let operation_cancel = Arc::new(AtomicBool::new(false));
    let registration = registry.register_job_with_external_cancel(Some(operation_cancel.clone()));
    tokio::pin!(registration);

    tokio::select! {
        result = &mut registration => panic!("registration completed before cancel: {result:?}"),
        _ = sleep(Duration::from_millis(20)) => {}
    }

    operation_cancel.store(true, Ordering::SeqCst);
    let result = timeout(Duration::from_millis(250), &mut registration)
        .await
        .expect("registration should observe cancellation promptly");

    assert!(matches!(
        result.expect_err("registration should be cancelled"),
        AppError::Cancellation(_)
    ));
    assert_eq!(
        registry.get_aggregate_status().await.total_jobs,
        1,
        "the cancelled admission is no longer tracked"
    );
}

#[tokio::test]
async fn reconfiguration_rejects_a_job_waiting_for_admission() {
    let registry = Arc::new(JobRegistry::new(1));
    let (finished_job, finishing_permit) = registry.register_job().await.expect("first job");
    registry.complete_job(finished_job).await;
    let waiting_registry = Arc::clone(&registry);
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by reconfiguration_rejects_a_job_waiting_for_admission"
    )]
    let waiting = tokio::spawn(async move { waiting_registry.register_job().await });
    sleep(Duration::from_millis(20)).await;

    let result = registry.update_max_concurrent(4).await;

    assert!(
        result.is_err(),
        "a job queued for a permit must block reconfiguration"
    );
    assert_eq!(registry.max_concurrent(), 1);
    drop(finishing_permit);
    let (job_id, _permit) = waiting
        .await
        .expect("join waiting job")
        .expect("waiting job admitted");
    registry.complete_job(job_id).await;
    assert_eq!(
        registry
            .update_max_concurrent(4)
            .await
            .expect("idle update"),
        4
    );
}

#[tokio::test]
async fn dropped_admission_does_not_block_reconfiguration() {
    let registry = Arc::new(JobRegistry::new(1));
    let (finished_job, _finishing_permit) = registry.register_job().await.expect("first job");
    registry.complete_job(finished_job).await;
    let waiting_registry = Arc::clone(&registry);
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by dropped_admission_does_not_block_reconfiguration"
    )]
    let waiting = tokio::spawn(async move { waiting_registry.register_job().await });
    sleep(Duration::from_millis(20)).await;

    waiting.abort();
    let _ = waiting.await;

    assert_eq!(
        registry
            .update_max_concurrent(2)
            .await
            .expect("abandoned admission must not stay tracked"),
        2
    );
}
