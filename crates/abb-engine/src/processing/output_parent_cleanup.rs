use crate::errors::Result;
use crate::output_artifact::OutputParentDirCleanup;
use crate::processing::{ProcessCommandResult, ProcessResultStatus};

pub(super) fn finalize_output_parent_cleanup(
    result: Result<ProcessCommandResult>,
    cleanup: &mut OutputParentDirCleanup,
) -> Result<ProcessCommandResult> {
    match result {
        Ok(result) if result_releases_output_parent_cleanup(&result) => {
            cleanup.release();
            Ok(result)
        }
        Ok(result) => {
            cleanup_output_parents_after_unsuccessful_result(cleanup);
            Ok(result)
        }
        Err(error) => {
            cleanup_output_parents_after_unsuccessful_result(cleanup);
            Err(error)
        }
    }
}

fn result_releases_output_parent_cleanup(result: &ProcessCommandResult) -> bool {
    result.results.iter().all(|entry| {
        matches!(
            entry.status,
            ProcessResultStatus::Success | ProcessResultStatus::Skipped
        )
    })
}

fn cleanup_output_parents_after_unsuccessful_result(cleanup: &mut OutputParentDirCleanup) {
    if let Err(error) = cleanup.cleanup_now() {
        log::warn!("output_parent_cleanup status=terminal_cleanup_err err={error}");
    }
}

#[cfg(test)]
#[path = "output_parent_cleanup_tests.rs"]
mod tests;
