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
}
