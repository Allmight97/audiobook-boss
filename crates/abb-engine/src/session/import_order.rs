use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub(super) struct ImportOrder {
    next: AtomicU64,
    serving: AtomicU64,
    changed: tokio::sync::Notify,
}

pub(super) struct Turn(u64);

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
#[path = "import_order_tests.rs"]
mod tests;
