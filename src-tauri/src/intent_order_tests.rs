use std::sync::{Arc, Mutex};

use super::IntentOrder;

#[tokio::test]
async fn intents_that_arrive_out_of_order_run_in_the_order_sent() {
    let order = Arc::new(IntentOrder::default());
    order.attach(1);
    let ran = Arc::new(Mutex::new(Vec::new()));

    let arrivals = [2, 0, 1].map(|sequence| {
        let order = Arc::clone(&order);
        let ran = Arc::clone(&ran);
        #[expect(
            clippy::disallowed_methods,
            reason = "joined by intents_that_arrive_out_of_order_run_in_the_order_sent"
        )]
        let intent = tokio::spawn(async move {
            let turn = order.turn(1, sequence).await.expect("its turn");
            ran.lock().expect("ran").push(sequence);
            drop(turn);
        });
        intent
    });
    for arrival in arrivals {
        arrival.await.expect("intent ran");
    }

    assert_eq!(*ran.lock().expect("ran"), [0, 1, 2]);
}

#[tokio::test(start_paused = true)]
async fn an_intent_that_never_arrives_does_not_stall_the_ones_after_it() {
    let order = IntentOrder::default();
    order.attach(1);

    drop(order.turn(1, 1).await.expect("runs after the wait"));

    let started = tokio::time::Instant::now();
    drop(order.turn(1, 2).await.expect("next intent"));
    assert_eq!(started.elapsed(), std::time::Duration::ZERO);

    assert_eq!(order.turn(1, 0).await.err(), Some(super::Refused::Late));
}

#[tokio::test(start_paused = true)]
async fn a_waiting_or_running_intent_is_never_skipped() {
    let order = Arc::new(IntentOrder::default());
    order.attach(1);
    let first = order.turn(1, 0).await.expect("intent 0");
    let ran = Arc::new(Mutex::new(Vec::new()));
    let later = [2, 1].map(|sequence| {
        let order = Arc::clone(&order);
        let ran = Arc::clone(&ran);
        #[expect(
            clippy::disallowed_methods,
            reason = "joined by a_waiting_or_running_intent_is_never_skipped"
        )]
        let intent = tokio::spawn(async move {
            let turn = order.turn(1, sequence).await.expect("its turn");
            ran.lock().expect("ran").push(sequence);
            drop(turn);
        });
        intent
    });
    tokio::time::sleep(super::MISSING_INTENT_WAIT * 3).await;
    assert!(ran.lock().expect("ran").is_empty(), "intent 0 still runs");

    drop(first);
    for intent in later {
        intent.await.expect("intent ran");
    }
    assert_eq!(*ran.lock().expect("ran"), [1, 2]);
}

#[tokio::test]
async fn an_intent_from_a_replaced_frontend_is_refused() {
    let order = Arc::new(IntentOrder::default());
    order.attach(1);
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by an_intent_from_a_replaced_frontend_is_refused"
    )]
    let late = tokio::spawn({
        let order = Arc::clone(&order);
        async move { order.turn(1, 3).await.err() }
    });
    tokio::task::yield_now().await;

    order.attach(2);

    assert_eq!(
        late.await.expect("late intent settles"),
        Some(super::Refused::Replaced)
    );
    assert!(order.turn(2, 0).await.is_ok());
}
