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

/// Processing stage enumeration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
pub enum ProcessingStage {
    /// Analyzing input files.
    Analyzing,
    /// Converting audio files.
    Converting,
    /// Writing metadata.
    WritingMetadata,
    /// Process completed.
    Completed,
    /// Process failed.
    Failed(String),
}

pub use abb_processing_core::{classify_run_terminal, RunTerminalClass};
pub(crate) use context::{OutputConfig, ProcessingContext};
pub use job_registry::MaxConcurrentJobsCapabilities;
pub(crate) use lifecycle::operation_kind_log_label;
pub use lifecycle::{OperationKind, OperationResultSummary};
pub(crate) use preview_config::{is_valid_preview_length, PreviewConfig};
pub use progress::{EventStage, ProgressEvent};
pub(crate) use session::ProcessingSession;
pub(crate) use title_output::TitleOutput;
pub use title_output::{OutputUpdate, OutputUpdateStatus};
pub use types::{
    AudioHandling, ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessResultSummary, ProcessingPreflightPlan, SupplementalProcessingAsset, TitleSource,
};

pub(crate) use job_registry::{CancellationChecker, JobRegistry};
pub(crate) use progress::{converting_percentage_from_seconds, ProgressEmitter};
#[cfg(test)]
pub(crate) fn preflight_payload(
    payload: ProcessPayload,
    metadata: Option<std::collections::HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> crate::Result<ProcessingPreflightPlan> {
    run::inspect_processing_plan(&payload, metadata.as_ref(), preview_seconds)
        .map(|inspected| inspected.plan.to_public())
}

pub(crate) use context::processing::ProgressEventListener;
pub(crate) use progress::{EtaEstimator, PROGRESS_CONVERTING_MAX, PROGRESS_CONVERTING_START};
