mod job_registry;
mod output_parent_cleanup;
pub(crate) mod plan;
pub(crate) use crate::execution::preview_config;
pub(crate) mod run;
mod terminal_outcomes;
pub(crate) mod title_output;
mod types;

pub use abb_processing_core::{
    classify_run_terminal, OperationKind, OperationResultSummary, RunTerminalClass,
};
pub use job_registry::MaxConcurrentJobsCapabilities;
pub use title_output::{OutputUpdate, OutputUpdateStatus};
pub use types::{
    ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessingPreflightPlan, SupplementalProcessingAsset, TitleSource,
};

pub use crate::audio::AudioHandling;
#[cfg(test)]
pub(crate) use crate::execution::cancelled_token;
pub(crate) use crate::execution::{
    CancellationChecker, JobIdentity, OutputConfig, ProcessingContext, ProcessingSession,
    ProgressEmitter, ProgressEventListener, PREVIEW_SECONDS,
};
pub use crate::execution::{EventStage, ProgressEvent};
pub(crate) use job_registry::JobRegistry;
pub(crate) use title_output::TitleOutput;
