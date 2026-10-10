//! Imports take effect in the order they were accepted. Each takes a turn
//! when accepted and runs once every earlier turn has ended, whatever order
//! their tasks start in.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub(super) struct ImportOrder {
    next: AtomicU64,
    serving: AtomicU64,
    changed: tokio::sync::Notify,
}

/// A place in the import order, taken when the import is accepted.
pub(super) struct Turn(u64);

/// Held while an import runs; the next turn starts when it drops.
pub(super) struct Running<'a>(&'a ImportOrder);

impl ImportOrder {
    pub(super) fn take(&self) -> Turn {
        Turn(self.next.fetch_add(1, Ordering::SeqCst))
    }

    pub(super) async fn wait(&self, turn: Turn) -> Running<'_> {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.serving.load(Ordering::SeqCst) == turn.0 {
                return Running(self);
            }
            changed.await;
        }
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.serving.fetch_add(1, Ordering::SeqCst);
        self.0.changed.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
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
}
