use crate::processing::{ProgressEventListener, TitleOutput};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub(crate) struct ProcessingRun {
    pub(crate) operation_id: String,
    pub(crate) listener: ProgressEventListener,
    pub(crate) mode: RunMode,
}

pub(crate) enum RunMode {
    Export(Vec<ExportTitle>),
    Preview(Vec<CancellationToken>),
}

pub(crate) struct ExportTitle {
    pub(crate) cancel: CancellationToken,
    pub(crate) output: Arc<TitleOutput>,
}

impl RunMode {
    pub(super) fn cancel(&self, index: usize) -> CancellationToken {
        match self {
            Self::Export(titles) => titles[index].cancel.clone(),
            Self::Preview(cancels) => cancels[index].clone(),
        }
    }

    pub(super) fn output(&self, index: usize) -> Option<Arc<TitleOutput>> {
        match self {
            Self::Export(titles) => Some(Arc::clone(&titles[index].output)),
            Self::Preview(_) => None,
        }
    }
}
