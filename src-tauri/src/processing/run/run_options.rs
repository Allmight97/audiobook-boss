use crate::processing::context::processing::ProgressEventListener;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Clone, Default)]
pub(crate) struct ProcessingRunOptions {
    pub(crate) operation_id: Option<String>,
    pub(crate) operation_cancel: Option<Arc<AtomicBool>>,
    pub(crate) progress_listener: Option<ProgressEventListener>,
}
