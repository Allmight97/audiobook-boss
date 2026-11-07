mod runtime;
mod snapshot;
mod stage_table;
#[cfg(test)]
mod stage_table_tests;
mod state;
pub(crate) use snapshot::new_processing_snapshot;
pub(crate) use state::WorkRuntimeState;
pub(crate) use terminal::is_terminal;
#[cfg(test)]
mod state_tests;
mod terminal;
mod terminalize;
#[cfg(test)]
mod terminalize_tests;
mod types;

pub(crate) use runtime::{now_ms, InlineRunTerminal, WorkRuntime};
pub use types::{
    ChildJobSnapshot, ChildJobStatus, OperationId, OperationLogEntry, OperationSnapshot,
    OperationTerminalSummary, ProgressSnapshot, WorkOperationStatus, WorkOperationsSnapshot,
    WorkOperationsUpdate, WorkProgressStage,
};

pub(crate) use types::SubmitProcessingOperationRequest;
