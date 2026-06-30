pub mod context;
pub mod job_registry;
pub mod lifecycle;
mod output_parent_cleanup;
pub(crate) mod plan;
pub mod preview_config;
pub mod progress;
pub(crate) mod run;
pub mod session;
mod terminal_outcomes;
pub mod types;

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
pub use context::{OutputConfig, ProcessingContext};
pub use job_registry::{
    AggregateJobStatus, CancellationChecker, JobId, JobRegistry, MaxConcurrentJobsCapabilities,
};
pub use lifecycle::{OperationKind, OperationResultSummary};
pub use preview_config::PreviewConfig;
pub use progress::{
    converting_percentage_from_seconds, emit_progress_event, emit_queue_event, EventStage,
    ProgressEmitter, ProgressEvent, QueueEvent, QueueItem,
};
pub use session::ProcessingSession;
pub use types::{
    JobType, ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessResultSummary, ProcessingPreflightPlan, SupplementalProcessingAsset,
};

#[cfg(test)]
mod contract_tests;
