use crate::processing::ProgressEventListener;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// One output title's cancel token and, for an export, its output record.
#[derive(Clone)]
pub(crate) struct TitleRun {
    pub(crate) index: usize,
    pub(crate) cancel: CancellationToken,
    pub(crate) output: Option<Arc<crate::processing::TitleOutput>>,
}

#[derive(Clone, Default)]
pub(crate) struct ProcessingRunOptions {
    pub(crate) operation_id: Option<String>,
    /// One entry per output title. Cancelling the whole operation cancels
    /// every title's token. A preview has no output records.
    pub(crate) titles: Vec<TitleRun>,
    pub(crate) progress_listener: Option<ProgressEventListener>,
}

impl ProcessingRunOptions {
    pub(crate) fn title(&self, index: usize) -> Option<&TitleRun> {
        self.titles.iter().find(|title| title.index == index)
    }
}
