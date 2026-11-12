use crate::errors::{AppError, Result};
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

pub(super) struct TitleRun {
    pub(super) cancel: CancellationToken,
    pub(super) output: Option<Arc<TitleOutput>>,
}

impl RunMode {
    pub(super) fn title(&self, index: usize) -> Result<TitleRun> {
        let title = match self {
            Self::Export(titles) => titles.get(index).map(|title| TitleRun {
                cancel: title.cancel.clone(),
                output: Some(Arc::clone(&title.output)),
            }),
            Self::Preview(cancels) => cancels.get(index).map(|cancel| TitleRun {
                cancel: cancel.clone(),
                output: None,
            }),
        };
        title.ok_or_else(|| AppError::General(format!("No run entry for title {index}")))
    }
}
