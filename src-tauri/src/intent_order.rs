//! Runs a frontend's intents in the order it sent them.
//!
//! IPC may deliver two requests sent back to back in either order, and each
//! request runs as its own task. The frontend numbers its intents; an intent
//! waits here until every earlier one has taken its turn.

use std::time::Duration;

use tokio::sync::watch;

/// How long an intent waits for an earlier one that has not arrived. An
/// intent the frontend numbered but never delivered must not stall every
/// intent after it.
const MISSING_INTENT_WAIT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Position {
    /// Identifies the attached frontend; a reload attaches a new one.
    client: u64,
    /// The sequence number whose turn it is.
    next: u64,
}

pub struct IntentOrder {
    position: watch::Sender<Position>,
}

impl Default for IntentOrder {
    fn default() -> Self {
        Self {
            position: watch::channel(Position { client: 0, next: 0 }).0,
        }
    }
}

/// One intent's turn. The next intent may go once this is dropped.
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
            }
            current
        });
    }
}

impl IntentOrder {
    /// A frontend started: its intents count from zero, and intents still
    /// arriving from an earlier frontend are refused.
    pub fn attach(&self, client: u64) {
        self.position.send_replace(Position { client, next: 0 });
    }

    /// Waits until it is `sequence`'s turn. Returns `None` for an intent from
    /// a frontend that is no longer attached.
    pub async fn turn(&self, client: u64, sequence: u64) -> Option<Turn<'_>> {
        let mut positions = self.position.subscribe();
        let arrived = tokio::time::timeout(
            MISSING_INTENT_WAIT,
            positions.wait_for(|position| position.client != client || position.next >= sequence),
        )
        .await;
        let position = match arrived {
            Ok(position) => *position.ok()?,
            Err(_) => {
                log::warn!("Intent {sequence} ran without an earlier intent that never arrived");
                self.position.send_if_modified(|position| {
                    let behind = position.client == client && position.next < sequence;
                    if behind {
                        position.next = sequence;
                    }
                    behind
                });
                *self.position.borrow()
            }
        };
        (position.client == client).then_some(Turn {
            order: self,
            client,
            sequence,
        })
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
            tokio::spawn(async move {
                let turn = order.turn(1, sequence).await.expect("attached frontend");
                ran.lock().expect("ran").push(sequence);
                drop(turn);
            })
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

        // Intent 0 is never delivered.
        drop(order.turn(1, 1).await.expect("runs after the wait"));

        // Later intents are not delayed again.
        let started = tokio::time::Instant::now();
        drop(order.turn(1, 2).await.expect("next intent"));
        assert_eq!(started.elapsed(), std::time::Duration::ZERO);
    }

    #[tokio::test]
    async fn an_intent_from_a_replaced_frontend_is_refused() {
        let order = Arc::new(IntentOrder::default());
        order.attach(1);
        let late = tokio::spawn({
            let order = Arc::clone(&order);
            async move { order.turn(1, 3).await.is_some() }
        });
        tokio::task::yield_now().await;

        order.attach(2);

        assert!(!late.await.expect("late intent settles"));
        assert!(order.turn(2, 0).await.is_some());
    }
}
