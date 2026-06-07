use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Synchronous cancellation checker for use in tight processing loops
///
/// This avoids async overhead when checking cancellation frequently
pub struct CancellationChecker {
    pub(crate) job_flag: Arc<AtomicBool>,
    pub(crate) global_flag: Arc<AtomicBool>,
    pub(crate) operation_flag: Option<Arc<AtomicBool>>,
    pub(crate) honor_global: bool,
}

impl CancellationChecker {
    pub fn with_operation_flag(mut self, flag: Option<Arc<AtomicBool>>) -> Self {
        if flag.is_some() {
            self.honor_global = false;
        }
        self.operation_flag = flag;
        self
    }

    /// Checks if processing should be cancelled (job-specific OR global)
    pub fn is_cancelled(&self) -> bool {
        (self.honor_global && self.global_flag.load(Ordering::Acquire))
            || self.job_flag.load(Ordering::Acquire)
            || self
                .operation_flag
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Acquire))
    }
}
