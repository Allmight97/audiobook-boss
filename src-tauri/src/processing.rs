pub mod context;
pub mod job_registry;
pub mod lifecycle;
pub(crate) mod plan;
pub mod preview_config;
pub mod progress;
pub(crate) mod run;
pub mod session;
mod terminal_outcomes;
pub mod types;

use serde::{Deserialize, Serialize};

/// Progress information for audio processing.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProcessingProgress {
    /// Current stage of processing.
    pub stage: ProcessingStage,
    /// Overall progress percentage (0-100).
    pub progress: f32,
    /// Current file being processed.
    pub current_file: Option<String>,
    /// Files completed.
    pub files_completed: usize,
    /// Total files to process.
    pub total_files: usize,
    /// Estimated time remaining in seconds.
    pub eta_seconds: Option<f64>,
}

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

pub use context::{OutputConfig, ProcessingContext, ProcessingContextBuilder};
pub use job_registry::{
    AggregateJobStatus, CancellationChecker, JobId, JobRegistry, JobState,
    MaxConcurrentJobsCapabilities,
};
pub use lifecycle::{OperationKind, OperationResultSummary};
pub use preview_config::PreviewConfig;
pub use progress::{
    calculate_stage_progress, converting_percentage_from_seconds, emit_progress_event,
    emit_queue_event, format_eta, EventStage, ProgressEmitter, ProgressEvent, ProgressReporter,
    QueueEvent, QueueItem,
};
pub use session::ProcessingSession;
pub use types::{
    CollisionPolicy, JobType, NamingPreset, OutputCollisionInfo, OutputCollisionKind, OutputKind,
    OutputNamingConfig, OutputReviewRequirement, PlannedOutput, PlannedOutputAction,
    ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessResultSummary, ProcessingPreflightPlan, SupplementalProcessingAsset,
};
