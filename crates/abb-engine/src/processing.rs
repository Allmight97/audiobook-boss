mod context;
mod job_registry;
mod output_parent_cleanup;
pub(crate) mod plan;
pub(crate) mod preview_config;
mod progress;
pub(crate) mod run;
mod session;
mod terminal_outcomes;
pub(crate) mod title_output;
mod types;

pub use abb_processing_core::{
    classify_run_terminal, OperationKind, OperationResultSummary, RunTerminalClass,
};
pub(crate) use context::{OutputConfig, ProcessingContext};
pub use job_registry::MaxConcurrentJobsCapabilities;
pub(crate) use preview_config::PREVIEW_SECONDS;
pub use progress::{EventStage, ProgressEvent};
pub(crate) use session::ProcessingSession;
pub(crate) use title_output::TitleOutput;
pub use title_output::{OutputUpdate, OutputUpdateStatus};
pub use types::{
    AudioHandling, ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessingPreflightPlan, SupplementalProcessingAsset, TitleSource,
};

#[cfg(test)]
pub(crate) use job_registry::cancelled_token;
pub(crate) use job_registry::{CancellationChecker, JobRegistry};
pub(crate) use progress::{
    converting_percentage, converting_percentage_from_seconds, ProgressEmitter,
};
pub(crate) use progress::{EtaEstimator, ProgressEventListener};
