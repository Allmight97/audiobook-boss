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
#[path = "intent_order_tests.rs"]
mod tests;
