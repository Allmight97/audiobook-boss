use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Default)]
struct Queue {
    outstanding: Mutex<BTreeSet<u64>>,
    changed: tokio::sync::Notify,
}

impl Queue {
    fn outstanding(&self) -> std::sync::MutexGuard<'_, BTreeSet<u64>> {
        self.outstanding
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn release(&self, number: u64) {
        self.outstanding().remove(&number);
        self.changed.notify_waiters();
    }
}

#[derive(Default)]
pub(super) struct ImportOrder(Arc<Queue>);

pub(super) struct Turn {
    queue: Arc<Queue>,
    number: u64,
}

pub(super) struct Running {
    _turn: Turn,
}

impl ImportOrder {
    pub(super) fn take(&self) -> Turn {
        let mut outstanding = self.0.outstanding();
        let number = outstanding.last().map_or(0, |last| last + 1);
        outstanding.insert(number);
        Turn {
            queue: Arc::clone(&self.0),
            number,
        }
    }

    pub(super) async fn wait(&self, turn: Turn) -> Running {
        loop {
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.0.outstanding().first() == Some(&turn.number) {
                return Running { _turn: turn };
            }
            changed.await;
        }
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        self.queue.release(self.number);
    }
}

#[cfg(test)]
#[path = "import_order_tests.rs"]
mod tests;
