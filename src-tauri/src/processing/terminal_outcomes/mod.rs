mod batch;
mod classification;
mod entries;
mod events;
#[cfg(test)]
mod tests;

pub(super) use batch::collect_batch_results;
pub(super) use classification::{classify_processing_error, ProcessingJobTerminalOutcome};
pub(super) use entries::{
    build_all_skipped_batch_result, no_write_skipped_result, terminal_failure_result,
};
pub(super) use events::{emit_terminal_failed_event, emit_terminal_skipped_event};
