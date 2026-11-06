use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Synchronous cancellation checker for use in tight processing loops.
///
/// Cancellation is title-scoped: a title carries its flag, which title cancel,
/// preview cancel, or whole-operation cancel sets.
#[derive(Debug)]
pub struct CancellationChecker {
    cancel_flag: Option<Arc<AtomicBool>>,
}

impl CancellationChecker {
    pub fn new(cancel_flag: Option<Arc<AtomicBool>>) -> Self {
        Self { cancel_flag }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire))
    }
}
