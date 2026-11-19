mod operations;
#[cfg(test)]
mod operations_tests;
mod runtime;
mod snapshot;
mod stage_table;
#[cfg(test)]
mod stage_table_tests;
mod state;
#[cfg(test)]
mod state_log_tail_tests;
pub(crate) use snapshot::new_processing_snapshot;
pub(crate) use state::WorkRuntimeState;
#[cfg(test)]
mod state_tests;
mod tally;
#[cfg(test)]
mod tally_tests;
mod terminal;
mod terminalize;
#[cfg(test)]
mod terminalize_tests;
mod types;

pub(crate) use runtime::{now_ms, InlineRunTerminal, WorkRuntime};
pub use types::{
    ChildCounts, ChildJobSnapshot, ChildJobStatus, OperationId, OperationLogEntry, OperationPhase,
    OperationSnapshot, ProgressSnapshot, WorkOperationStatus, WorkOperationsSnapshot,
    WorkOperationsUpdate, WorkProgressStage,
};

pub(crate) use types::SubmitProcessingOperationRequest;
