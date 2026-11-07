use crate::processing::ProgressEventListener;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
pub(crate) struct ProcessingRunOptions {
    pub(crate) operation_id: Option<String>,
    /// One cancel token per output title, indexed by input index. Cancelling
    /// the whole operation cancels every token.
    pub(crate) title_cancels: Vec<CancellationToken>,
    pub(crate) progress_listener: Option<ProgressEventListener>,
    /// One output record per title, indexed like `title_cancels`. Previews
    /// pass none.
    pub(crate) title_outputs: Vec<Arc<crate::processing::TitleOutput>>,
}
