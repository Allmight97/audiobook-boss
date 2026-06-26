use crate::processing::context::processing::ProgressEventListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone, Default)]
pub(crate) struct ProcessingRunOptions {
    pub(crate) operation_cancel: Option<Arc<AtomicBool>>,
    pub(crate) progress_listener: Option<ProgressEventListener>,
}

impl ProcessingRunOptions {
    pub(crate) fn is_operation_cancelled(&self) -> bool {
        self.operation_cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire))
    }
}
