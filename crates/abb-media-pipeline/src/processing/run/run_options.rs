use crate::processing::plan::InspectedProcessingPlan;
use crate::processing::{ProgressEventListener, TitleOutput};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub struct ProcessingRun {
    pub operation_id: String,
    pub listener: ProgressEventListener,
    pub mode: RunMode,
}

pub enum InspectedRun {
    Export(ExportPlan),
    Preview(PreviewPlan),
}

pub struct ExportPlan(InspectedProcessingPlan);

pub struct PreviewPlan(InspectedProcessingPlan);

pub enum RunMode {
    Export {
        plan: ExportPlan,
        titles: Vec<ExportTitle>,
    },
    Preview {
        plan: PreviewPlan,
        cancels: Vec<CancellationToken>,
    },
}

pub struct ExportTitle {
    pub cancel: CancellationToken,
    pub output: Arc<TitleOutput>,
}

pub(super) enum RunTitles {
    Export(Vec<ExportTitle>),
    Preview(Vec<CancellationToken>),
}

impl InspectedRun {
    pub(super) fn new(inspected: InspectedProcessingPlan, preview: bool) -> Self {
        if preview {
            Self::Preview(PreviewPlan(inspected))
        } else {
            Self::Export(ExportPlan(inspected))
        }
    }

    pub fn inspected(&self) -> &InspectedProcessingPlan {
        match self {
            Self::Export(ExportPlan(inspected)) | Self::Preview(PreviewPlan(inspected)) => {
                inspected
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn expect_export(self) -> ExportPlan {
        match self {
            Self::Export(plan) => plan,
            Self::Preview(_) => panic!("expected an export plan"),
        }
    }

    #[cfg(test)]
    pub(crate) fn into_inspected(self) -> InspectedProcessingPlan {
        match self {
            Self::Export(ExportPlan(inspected)) | Self::Preview(PreviewPlan(inspected)) => {
                inspected
            }
        }
    }
}

impl ExportPlan {
    pub(crate) fn inspected(&self) -> &InspectedProcessingPlan {
        &self.0
    }
}

impl PreviewPlan {
    pub fn inspected(&self) -> &InspectedProcessingPlan {
        &self.0
    }
}

impl RunMode {
    pub(super) fn into_parts(self) -> (InspectedProcessingPlan, RunTitles) {
        match self {
            Self::Export { plan, titles } => (plan.0, RunTitles::Export(titles)),
            Self::Preview { plan, cancels } => (plan.0, RunTitles::Preview(cancels)),
        }
    }
}

impl RunTitles {
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
