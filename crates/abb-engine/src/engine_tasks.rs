use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};

use crate::errors::{AppError, Result};
use tokio_util::task::TaskTracker;

/// Admission and shutdown share this lock: visible work is registered before
/// shutdown closes admission and enumerates what it must cancel. Internal
/// cleanup may still spawn while its already-tracked parent is settling.
#[derive(Clone, Default)]
pub(crate) struct EngineTasks {
    tracker: TaskTracker,
    admission: Arc<Mutex<()>>,
    /// Fires when shutdown begins, for work that has nothing to save and is
    /// dropped rather than waited for.
    closing: tokio_util::sync::CancellationToken,
}

impl EngineTasks {
    pub(crate) fn admit<T>(&self, register: impl FnOnce() -> T) -> Result<T> {
        let _turn = self
            .admission
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if self.is_closed() {
            return Err(AppError::Closing);
        }
        Ok(register())
    }

    pub(crate) fn close(&self) {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "a check that always passes cannot refuse the close"
        )]
        let _ = self.close_if(|| Ok::<(), ()>(()));
    }

    /// Closes admission only when `check` passes, in the same turn, so no
    /// work can be admitted between the check and the close.
    pub(crate) fn close_if<E>(
        &self,
        check: impl FnOnce() -> std::result::Result<(), E>,
    ) -> std::result::Result<(), E> {
        let _turn = self
            .admission
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        check()?;
        self.tracker.close();
        self.closing.cancel();
        Ok(())
    }

    pub(crate) async fn until_closing<T>(
        &self,
        work: impl Future<Output = Result<T>>,
    ) -> Result<T> {
        tokio::select! {
            result = work => result,
            () = self.closing.cancelled() => Err(AppError::Closing),
        }
    }

    fn is_closed(&self) -> bool {
        self.tracker.is_closed()
    }

    pub(crate) fn spawn<F>(&self, work: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.tracker.spawn(work)
    }

    pub(crate) async fn wait(&self) {
        self.tracker.wait().await;
    }
}
