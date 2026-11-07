use super::JobRegistry;
use crate::errors::AppError;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tokio_util::sync::CancellationToken;

#[tokio::test(start_paused = true)]
async fn external_cancel_interrupts_waiting_permit_acquire_without_waiting_for_a_timer() {
    let registry = JobRegistry::new(1);
    let (first_job, held_permit) = registry.register_job().await.expect("first job");
    let operation_cancel = CancellationToken::new();
    let registration = registry.register_job_with_external_cancel(Some(operation_cancel.clone()));
    tokio::pin!(registration);

    tokio::select! {
        result = &mut registration => panic!("registration completed before cancel: {result:?}"),
        _ = sleep(Duration::from_millis(20)) => {}
    }

    let cancelled_at = Instant::now();
    operation_cancel.cancel();
    let result = registration.await;

    assert_eq!(
        cancelled_at.elapsed(),
        Duration::ZERO,
        "the wake-up is the token, not a timer"
    );
    assert!(matches!(
        result.expect_err("registration should be cancelled"),
        AppError::Cancellation(_)
    ));
    drop(held_permit);
    registry.complete_job(first_job);
    assert_eq!(
        registry
            .update_max_concurrent(2)
            .expect("the cancelled admission is no longer tracked"),
        2
    );
}

#[tokio::test(start_paused = true)]
async fn a_cancelled_token_refuses_registration_before_it_is_tracked() {
    let registry = JobRegistry::new(1);

    let result = registry
        .register_job_with_external_cancel(Some(crate::execution::cancelled_token()))
        .await;

    assert!(matches!(
        result.expect_err("cancelled before registering"),
        AppError::Cancellation(_)
    ));
    assert_eq!(registry.active_jobs(), 0);
}

#[tokio::test]
async fn reconfiguration_rejects_a_job_waiting_for_admission() {
    let registry = Arc::new(JobRegistry::new(1));
    let (finished_job, finishing_permit) = registry.register_job().await.expect("first job");
    registry.complete_job(finished_job);
    let waiting_registry = Arc::clone(&registry);
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by reconfiguration_rejects_a_job_waiting_for_admission"
    )]
    let waiting = tokio::spawn(async move { waiting_registry.register_job().await });
    sleep(Duration::from_millis(20)).await;

    let result = registry.update_max_concurrent(4);

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
    registry.complete_job(job_id);
    assert_eq!(registry.update_max_concurrent(4).expect("idle update"), 4);
}

#[tokio::test]
async fn dropped_admission_does_not_block_reconfiguration() {
    let registry = Arc::new(JobRegistry::new(1));
    let (finished_job, _finishing_permit) = registry.register_job().await.expect("first job");
    registry.complete_job(finished_job);
    let waiting_registry = Arc::clone(&registry);
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by dropped_admission_does_not_block_reconfiguration"
    )]
    let waiting = tokio::spawn(async move { waiting_registry.register_job().await });
    sleep(Duration::from_millis(20)).await;

    waiting.abort();
    #[expect(
        clippy::let_underscore_must_use,
        reason = "the aborted task reports a cancellation, which is the point"
    )]
    let _ = waiting.await;

    assert_eq!(
        registry
            .update_max_concurrent(2)
            .expect("abandoned admission must not stay tracked"),
        2
    );
}

#[tokio::test]
async fn a_job_without_a_permit_is_tracked_but_never_waits_or_takes_a_slot() {
    let registry = JobRegistry::new(1);
    let (held_job, held_permit) = registry.register_job().await.expect("hold the only slot");

    let tagging = registry.register_job_without_permit();

    assert_eq!(registry.active_jobs(), 2);
    assert!(registry.update_max_concurrent(2).is_err());
    registry.complete_job(held_job);
    assert!(
        registry.update_max_concurrent(2).is_err(),
        "the permit-free job still counts as active"
    );
    registry.fail_job(tagging, "tag write failed".into());
    drop(held_permit);
    assert_eq!(registry.update_max_concurrent(2).expect("idle"), 2);
}
