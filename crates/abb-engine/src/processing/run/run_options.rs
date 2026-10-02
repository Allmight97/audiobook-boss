use crate::processing::context::processing::ProgressEventListener;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Clone, Default)]
pub(crate) struct ProcessingRunOptions {
    pub(crate) operation_id: Option<String>,
    /// One cancel flag per output title, indexed by input index. Cancelling
    /// the whole operation sets every flag. Direct previews pass none.
    pub(crate) title_cancels: Vec<Arc<AtomicBool>>,
    pub(crate) progress_listener: Option<ProgressEventListener>,
    /// One output record per title, indexed like `title_cancels`. Previews
    /// pass none.
    pub(crate) title_outputs: Vec<Arc<crate::processing::TitleOutput>>,
}
