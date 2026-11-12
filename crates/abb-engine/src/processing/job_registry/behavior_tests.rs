use super::{JobRegistry, MAX_CONCURRENT_JOBS, MIN_CONCURRENT_JOBS};
use crate::errors::AppError;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tokio_util::sync::CancellationToken;

fn unscoped<Fut>(futures: Vec<Fut>) -> Vec<(Fut, Option<CancellationToken>)> {
    futures.into_iter().map(|future| (future, None)).collect()
}

#[test]
fn test_max_concurrent_capabilities_match_registry_bounds() {
    let capabilities = JobRegistry::max_concurrent_jobs_capabilities();

    assert_eq!(capabilities.fixed_min, MIN_CONCURRENT_JOBS);
    assert_eq!(capabilities.fixed_max, MAX_CONCURRENT_JOBS);
    assert_eq!(JobRegistry::new(0).max_concurrent(), capabilities.fixed_min);
    assert_eq!(
        JobRegistry::new(usize::MAX).max_concurrent(),
        capabilities.fixed_max
    );
}

#[tokio::test]
async fn test_fail_job() {
    let registry = JobRegistry::new(2);
    let (job_id, _permit) = registry.register_job().await.expect("Should register job");

    registry.fail_job(job_id, "Test error".to_string());

    assert_eq!(
        registry
            .update_max_concurrent(1)
            .expect("failed job is untracked"),
        1
    );
}

#[tokio::test]
async fn test_update_max_rejected_when_active() {
    let registry = Arc::new(JobRegistry::new(2));
    let (_job_id, _permit) = registry.register_job().await.expect("Should register job");

    let result = registry.update_max_concurrent(1);
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
        #[expect(
            clippy::disallowed_methods,
            reason = "joined by test_stress_concurrent_registration_respects_limit"
        )]
        let handle = tokio::spawn(async move {
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
            reg.complete_job(job_id);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.expect("join");
    }

    assert!(
        peak.load(Ordering::SeqCst) <= 3,
        "max concurrent observed should respect limit"
    );
    assert_eq!(
        registry
            .update_max_concurrent(1)
            .expect("every job untracked after the batch"),
        1
    );
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

    let results = registry.run_batch(unscoped(futures)).await;

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

    let results = registry.run_batch(unscoped(futures)).await;

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
                return Err(AppError::General("task failed".to_string()));
            }

            sleep(Duration::from_millis(30)).await;
            Ok(index)
        });
    }

    let outcomes = registry.run_batch(unscoped(futures)).await;
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

    let outcomes = registry.run_batch(unscoped(futures)).await;

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

#[tokio::test]
async fn concurrency_cannot_change_while_a_run_is_held_even_between_its_jobs() {
    let registry = Arc::new(JobRegistry::new(2));
    let run = registry.hold_run();

    assert!(registry.update_max_concurrent(1).is_err());
    assert!(registry.reset_to_auto().is_err());

    drop(run);
    assert_eq!(registry.update_max_concurrent(1).expect("idle"), 1);
}

#[tokio::test(start_paused = true)]
async fn a_cancelled_queued_job_starts_without_waiting_for_the_job_ahead() {
    use std::future::Future;
    use std::pin::Pin;
    use tokio::sync::oneshot;
    use tokio::time::Instant;
    use tokio_util::sync::CancellationToken;

    type Job = Pin<Box<dyn Future<Output = crate::errors::Result<u8>> + Send>>;

    let registry = JobRegistry::new(1);
    let (release, blocked) = oneshot::channel::<()>();
    let (started, ran) = oneshot::channel::<()>();
    let ahead: Job = Box::pin(async move {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "the sender is dropped when the test ends the batch"
        )]
        let _ = blocked.await;
        Ok(0)
    });
    let queued: Job = Box::pin(async move {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "the receiver is gone if the test stops waiting"
        )]
        let _ = started.send(());
        Ok(1)
    });
    let token = CancellationToken::new();
    let batch = registry.run_batch(vec![(ahead, None), (queued, Some(token.clone()))]);
    let begun = Instant::now();
    let cancel_and_wait = async move {
        token.cancel();
        ran.await
            .expect("the cancelled job ran while the job ahead is blocked");
        let waited = begun.elapsed();
        #[expect(
            clippy::let_underscore_must_use,
            reason = "the receiver is gone if the batch ended first"
        )]
        let _ = release.send(());
        waited
    };

    let (results, waited) = tokio::join!(batch, cancel_and_wait);

    let results: Vec<u8> = results.into_iter().map(|r| r.expect("ok")).collect();
    assert_eq!(results, [0, 1]);
    assert_eq!(
        waited,
        std::time::Duration::ZERO,
        "the wake-up is the token, not a timer"
    );
}

#[tokio::test(start_paused = true)]
async fn a_queued_job_whose_token_is_cancelled_later_starts_out_of_turn_and_siblings_wait() {
    use std::future::Future;
    use std::pin::Pin;
    use tokio::sync::oneshot;
    use tokio_util::sync::CancellationToken;

    type Job = Pin<Box<dyn Future<Output = crate::errors::Result<u8>> + Send>>;

    let registry = JobRegistry::new(1);
    let (release, blocked) = oneshot::channel::<()>();
    let (started, ran) = oneshot::channel::<bool>();
    let sibling_started = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ahead: Job = Box::pin(async move {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "the sender is dropped when the test ends the batch"
        )]
        let _ = blocked.await;
        Ok(0)
    });
    let waiting: Job = Box::pin({
        let sibling_started = Arc::clone(&sibling_started);
        async move {
            sibling_started.store(true, Ordering::SeqCst);
            Ok(1)
        }
    });
    let cancelled: Job = Box::pin({
        let sibling_started = Arc::clone(&sibling_started);
        async move {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "the receiver is gone if the test stops waiting"
            )]
            let _ = started.send(sibling_started.load(Ordering::SeqCst));
            Ok(2)
        }
    });
    let token = CancellationToken::new();
    let batch = registry.run_batch(vec![
        (ahead, None),
        (waiting, Some(CancellationToken::new())),
        (cancelled, Some(token.clone())),
    ]);
    let cancel_and_wait = async move {
        tokio::task::yield_now().await;
        token.cancel();
        let sibling_had_started = ran.await.expect("the cancelled job ran out of turn");
        #[expect(
            clippy::let_underscore_must_use,
            reason = "the receiver is gone if the batch ended first"
        )]
        let _ = release.send(());
        sibling_had_started
    };

    let (results, sibling_had_started) = tokio::join!(batch, cancel_and_wait);

    assert!(
        !sibling_had_started,
        "the cancelled job jumped the queue; the waiting sibling did not start"
    );
    let results: Vec<u8> = results.into_iter().map(|r| r.expect("ok")).collect();
    assert_eq!(results, [0, 1, 2]);
}

#[tokio::test(start_paused = true)]
async fn external_cancel_interrupts_waiting_permit_acquire_without_waiting_for_a_timer() {
    let registry = JobRegistry::new(1);
    let (first_job, held_permit) = registry.register_job().await.expect("first job");
    let operation_cancel = CancellationToken::new();
    let registration = registry.register_job_with_external_cancel(&operation_cancel);
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
        .register_job_with_external_cancel(&crate::execution::cancelled_token())
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
    crate::test_cases::support::poll::until_true("the job is queued", || {
        registry.active_jobs() == 1
    })
    .await;

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
    crate::test_cases::support::poll::until_true("the job is queued", || {
        registry.active_jobs() == 1
    })
    .await;

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
