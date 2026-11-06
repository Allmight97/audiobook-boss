mod context;
mod job_registry;
mod lifecycle;
mod output_parent_cleanup;
pub(crate) mod plan;
mod preview_config;
mod progress;
pub(crate) mod run;
mod session;
mod terminal_outcomes;
pub(crate) mod title_output;
mod types;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
pub enum ProcessingStage {
    Analyzing,
    Converting,
    WritingMetadata,
    Completed,
    Failed(String),
}

pub use abb_processing_core::{classify_run_terminal, RunTerminalClass};
pub(crate) use context::{OutputConfig, ProcessingContext};
pub use job_registry::MaxConcurrentJobsCapabilities;
pub(crate) use lifecycle::operation_kind_log_label;
pub use lifecycle::{OperationKind, OperationResultSummary};
pub(crate) use preview_config::PreviewConfig;
pub(crate) use preview_config::PREVIEW_SECONDS;
pub use progress::{EventStage, ProgressEvent};
pub(crate) use session::ProcessingSession;
pub(crate) use title_output::TitleOutput;
pub use title_output::{OutputUpdate, OutputUpdateStatus};
pub use types::{
    AudioHandling, ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessResultSummary, ProcessingPreflightPlan, SupplementalProcessingAsset, TitleSource,
};

pub(crate) use context::processing::ProgressEventListener;
pub(crate) use job_registry::{CancellationChecker, JobRegistry};
pub(crate) use progress::EtaEstimator;
pub(crate) use progress::{
    converting_percentage, converting_percentage_from_seconds, ProgressEmitter,
};
