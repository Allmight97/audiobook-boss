use crate::errors::{AppError, Result};
use crate::processing::{classify_run_terminal, ProcessResultEntry, RunTerminalClass};

pub(in crate::processing) fn batch_results(
    results: Vec<ProcessResultEntry>,
) -> Result<Vec<ProcessResultEntry>> {
    let summary =
        abb_processing_core::summarize_result_statuses(results.iter().map(|entry| entry.status));
    if classify_run_terminal(&summary) == RunTerminalClass::Cancelled {
        return Err(AppError::cancelled());
    }
    Ok(results)
}
