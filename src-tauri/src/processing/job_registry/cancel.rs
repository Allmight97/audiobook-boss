use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Synchronous cancellation checker for use in tight processing loops.
///
/// Cancellation is operation-scoped: accepted background work carries its
/// operation's flag. Direct previews carry none and cannot be cancelled by
/// the backend.
pub struct CancellationChecker {
    operation_flag: Option<Arc<AtomicBool>>,
}

impl CancellationChecker {
    pub fn new(operation_flag: Option<Arc<AtomicBool>>) -> Self {
        Self { operation_flag }
    }

    /// Checks if the owning operation was cancelled
    pub fn is_cancelled(&self) -> bool {
        self.operation_flag
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire))
    }
}
