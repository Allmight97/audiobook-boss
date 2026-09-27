use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Synchronous cancellation checker for use in tight processing loops.
///
/// Cancellation is title-scoped: accepted background work carries its title's
/// flag, which title cancel or whole-operation cancel sets. Direct previews
/// carry none and cannot be cancelled by the backend.
#[derive(Debug)]
pub struct CancellationChecker {
    cancel_flag: Option<Arc<AtomicBool>>,
}

impl CancellationChecker {
    pub fn new(cancel_flag: Option<Arc<AtomicBool>>) -> Self {
        Self { cancel_flag }
    }

    /// Checks if this work was cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire))
    }
}
