use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Synchronous cancellation checker for use in tight processing loops
///
/// This avoids async overhead when checking cancellation frequently
pub struct CancellationChecker {
    pub(crate) job_flag: Arc<AtomicBool>,
    pub(crate) global_flag: Arc<AtomicBool>,
}

impl CancellationChecker {
    /// Checks if processing should be cancelled (job-specific OR global)
    pub fn is_cancelled(&self) -> bool {
        self.global_flag.load(Ordering::Acquire) || self.job_flag.load(Ordering::Acquire)
    }
}
