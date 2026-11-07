use super::*;

#[tokio::test]
async fn turns_run_in_the_order_taken_even_when_awaited_backwards() {
    let order = std::sync::Arc::new(ImportOrder::default());
    let first = order.take();
    let second = order.take();
    let ran = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by turns_run_in_the_order_taken_even_when_awaited_backwards"
    )]
    let later = tokio::spawn({
        let order = std::sync::Arc::clone(&order);
        let ran = std::sync::Arc::clone(&ran);
        async move {
            let _running = order.wait(second).await;
            ran.lock().expect("ran").push("second");
        }
    });
    tokio::task::yield_now().await;
    assert!(
        ran.lock().expect("ran").is_empty(),
        "second waits for first"
    );
    {
        let _running = order.wait(first).await;
        ran.lock().expect("ran").push("first");
    }
    later.await.expect("second runs");
    assert_eq!(*ran.lock().expect("ran"), ["first", "second"]);
}

const SERVED_WITHIN: std::time::Duration = std::time::Duration::from_secs(2);

#[tokio::test]
async fn a_turn_dropped_before_it_waits_does_not_block_later_turns() {
    let order = ImportOrder::default();
    let abandoned = order.take();
    let later = order.take();
    drop(abandoned);
    tokio::time::timeout(SERVED_WITHIN, order.wait(later))
        .await
        .expect("the later turn is served");
}

#[tokio::test]
async fn a_wait_cancelled_before_it_is_served_does_not_block_later_turns() {
    let order = ImportOrder::default();
    let first = order.take();
    let cancelled = order.take();
    let last = order.take();
    let running = order.wait(first).await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), order.wait(cancelled))
            .await
            .is_err(),
        "the second turn waits for the first"
    );
    drop(running);
    tokio::time::timeout(SERVED_WITHIN, order.wait(last))
        .await
        .expect("the last turn is served");
}
