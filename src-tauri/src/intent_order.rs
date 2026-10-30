use std::collections::BTreeSet;
use std::time::Duration;

use tokio::sync::watch;

const MISSING_INTENT_WAIT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Position {
    client: u64,
    next: u64,
    running: bool,
    waiting: BTreeSet<u64>,
}

pub struct IntentOrder {
    position: watch::Sender<Position>,
}

impl Default for IntentOrder {
    fn default() -> Self {
        Self {
            position: watch::channel(Position::default()).0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Replaced,
    Late,
}

pub struct Turn<'a> {
    order: &'a IntentOrder,
    client: u64,
    sequence: u64,
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        self.order.position.send_if_modified(|position| {
            let current = position.client == self.client && position.next == self.sequence;
            if current {
                position.next += 1;
                position.running = false;
            }
            current
        });
    }
}

impl IntentOrder {
    pub fn attach(&self, client: u64) {
        self.position.send_replace(Position {
            client,
            ..Position::default()
        });
    }

    pub async fn turn(&self, client: u64, sequence: u64) -> Result<Turn<'_>, Refused> {
        self.position.send_if_modified(|position| {
            position.client == client && position.waiting.insert(sequence)
        });
        let mut outcome = self.wait_for_turn(client, sequence).await;
        // Checked and claimed in one step, so a reload between the wait and
        // the claim cannot hand an old frontend's intent a turn.
        self.position.send_if_modified(|position| {
            if position.client != client {
                outcome = Err(Refused::Replaced);
                return false;
            }
            position.waiting.remove(&sequence);
            if outcome.is_ok() {
                if position.next > sequence {
                    log::warn!("Intent {sequence} arrived after later intents ran; refused");
                    outcome = Err(Refused::Late);
                } else {
                    position.running = true;
                }
            }
            false
        });
        outcome.map(|()| Turn {
            order: self,
            client,
            sequence,
        })
    }

    async fn wait_for_turn(&self, client: u64, sequence: u64) -> Result<(), Refused> {
        let mut positions = self.position.subscribe();
        loop {
            let arrived = tokio::time::timeout(
                MISSING_INTENT_WAIT,
                positions.wait_for(|position| {
                    position.client != client || (position.next >= sequence && !position.running)
                }),
            )
            .await;
            match arrived {
                Ok(Ok(_)) => break,
                Ok(Err(_)) => return Err(Refused::Replaced),
                Err(_) => {
                    self.position.send_if_modified(|position| {
                        let earliest = position.waiting.first().copied();
                        let skip = position.client == client
                            && !position.running
                            && earliest.is_some_and(|earliest| earliest > position.next);
                        if let (true, Some(earliest)) = (skip, earliest) {
                            log::warn!(
                                "Intent {earliest} ran without an earlier intent that never arrived"
                            );
                            position.next = earliest;
                        }
                        skip
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
}
