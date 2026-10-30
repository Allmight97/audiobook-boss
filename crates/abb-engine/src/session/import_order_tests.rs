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
