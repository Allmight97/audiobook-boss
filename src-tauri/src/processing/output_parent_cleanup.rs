use crate::errors::Result;
use crate::output_artifact::OutputParentDirCleanup;
use crate::processing::{ProcessCommandResult, ProcessResultStatus};

pub(super) fn finalize_output_parent_cleanup(
    result: Result<ProcessCommandResult>,
    cleanup: OutputParentDirCleanup,
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

fn cleanup_output_parents_after_unsuccessful_result(cleanup: OutputParentDirCleanup) {
    if let Err(error) = cleanup.cleanup_now() {
        log::warn!("output_parent_cleanup status=terminal_cleanup_err err={error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output_artifact::{
        ensure_output_parent_dirs, OutputKind, PlannedOutputAction, ResolvedOutputPlan,
    };
    use crate::processing::{JobType, ProcessResultEntry};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn output_plan(path: impl Into<PathBuf>) -> ResolvedOutputPlan {
        let path = path.into();
        ResolvedOutputPlan {
            kind: OutputKind::Final,
            requested_path: path.clone(),
            resolved_path: path,
            rename_candidate: None,
            collision: None,
            action: PlannedOutputAction::Write,
        }
    }

    fn result(status: ProcessResultStatus) -> ProcessCommandResult {
        ProcessCommandResult::new(
            JobType::Batch,
            vec![ProcessResultEntry {
                input_index: Some(0),
                status,
                message: "terminal".to_string(),
                error: None,
                preview_file_path: None,
                preview_actual_seconds: None,
                job_id: Some("job-1".to_string()),
            }],
        )
    }

    #[test]
    fn successful_result_releases_output_parent_cleanup() {
        let temp_dir = TempDir::new().expect("temp dir");
        let parent = temp_dir.path().join("author").join("title");
        let output = output_plan(parent.join("book.m4b"));
        let cleanup = ensure_output_parent_dirs(temp_dir.path(), [&output]).expect("parent dirs");

        finalize_output_parent_cleanup(Ok(result(ProcessResultStatus::Success)), cleanup)
            .expect("finalize cleanup");

        assert!(parent.exists(), "released cleanup should leave parent dirs");
    }

    #[test]
    fn cancelled_result_cleans_empty_output_parent() {
        let temp_dir = TempDir::new().expect("temp dir");
        let parent = temp_dir.path().join("author").join("title");
        let output = output_plan(parent.join("book.m4b"));
        let cleanup = ensure_output_parent_dirs(temp_dir.path(), [&output]).expect("parent dirs");

        finalize_output_parent_cleanup(Ok(result(ProcessResultStatus::Cancelled)), cleanup)
            .expect("finalize cleanup");

        assert!(
            !parent.exists(),
            "cancelled cleanup should prune empty parent dirs"
        );
    }
}
