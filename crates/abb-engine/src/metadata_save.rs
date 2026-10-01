//! Metadata batch save: writes each file's intent as one WorkRuntime operation.

use crate::audio::validate_input_audio_path;
use crate::errors::{sanitize_path_str_for_display, AppError, AppErrorEnvelope, Result};
use crate::host::Host;
use crate::metadata::MetadataIntentPatch;
use crate::processing::{
    CancellationChecker, EventStage, OperationKind, OperationResultSummary, ProcessResultStatus,
    ProgressEvent,
};
use crate::work_runtime::{InlineRunTerminal, WorkRuntime};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type MetadataSaveSummary = OperationResultSummary;
const METADATA_SAVE_CANCELLED_MESSAGE: &str = "Metadata save cancelled.";

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSaveRequest {
    pub file_path: String,
    pub metadata_patch: MetadataIntentPatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MetadataSaveResultStatus {
    Success,
    Cancelled,
    Failed,
}

/// Per-file outcome the frontend uses to clear or retain drafts. The reason
/// for each outcome is the operation child's terminal message in Work Center.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSaveResultEntry {
    pub input_index: usize,
    pub file_path: String,
    pub status: MetadataSaveResultStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSaveBatchResult {
    pub summary: MetadataSaveSummary,
    pub results: Vec<MetadataSaveResultEntry>,
}

/// A finished batch plus each file's `(input_index, status, reason)` terminal
/// for its WorkRuntime child.
struct SavedMetadataBatch {
    result: MetadataSaveBatchResult,
    children: Vec<(usize, ProcessResultStatus, String)>,
}

impl SavedMetadataBatch {
    fn new(outcomes: Vec<(MetadataSaveResultEntry, String)>) -> Self {
        let children = outcomes
            .iter()
            .map(|(entry, reason)| (entry.input_index, entry.status.into(), reason.clone()))
            .collect();
        let entries = outcomes.into_iter().map(|(entry, _)| entry).collect();
        Self {
            result: MetadataSaveBatchResult::new(entries),
            children,
        }
    }
}

impl From<MetadataSaveResultStatus> for ProcessResultStatus {
    fn from(status: MetadataSaveResultStatus) -> Self {
        match status {
            MetadataSaveResultStatus::Success => ProcessResultStatus::Success,
            MetadataSaveResultStatus::Cancelled => ProcessResultStatus::Cancelled,
            MetadataSaveResultStatus::Failed => ProcessResultStatus::Failed,
        }
    }
}

#[derive(Debug, Clone)]
struct MetadataSaveProgress {
    input_index: usize,
    file_path: String,
    stage: EventStage,
    percentage: f32,
    message: String,
}

impl MetadataSaveBatchResult {
    fn new(results: Vec<MetadataSaveResultEntry>) -> Self {
        let succeeded = results
            .iter()
            .filter(|result| result.status == MetadataSaveResultStatus::Success)
            .count();
        let cancelled = results
            .iter()
            .filter(|result| result.status == MetadataSaveResultStatus::Cancelled)
            .count();
        let failed = results.len().saturating_sub(succeeded + cancelled);
        Self {
            summary: OperationResultSummary {
                total: results.len(),
                succeeded,
                skipped: 0,
                cancelled,
                failed,
            },
            results,
        }
    }
}

pub(crate) async fn save_metadata_batch(
    host: &Host,
    runtime: &WorkRuntime,
    registry: &crate::ManagedJobRegistry,
    items: Vec<MetadataSaveRequest>,
) -> Result<MetadataSaveBatchResult> {
    if items.is_empty() {
        return Err(AppError::InvalidInput(
            "No metadata changes to save".to_string(),
        ));
    }

    // Metadata save is a WorkRuntime operation rendered in the Work Center.
    // The caller still awaits the per-file `MetadataSaveBatchResult` (drafts
    // clear only for files that succeeded), while progress and terminal truth
    // flow through the operation snapshot. Cancellation is operation-scoped
    // (`cancel_work_operation`).
    let file_paths: Vec<String> = items.iter().map(|item| item.file_path.clone()).collect();
    let (operation_id, cancel_flag) = runtime.begin_metadata_save_operation(host, &file_paths)?;

    let (job_id, _permit) = match registry
        .register_job_with_external_cancel(Some(cancel_flag.clone()))
        .await
    {
        Ok(registration) => registration,
        Err(error) => {
            // The operation is already Running + cancellable when the permit wait
            // begins, so a cancel during that wait surfaces here as
            // `AppError::Cancellation`, which WorkRuntime terminalizes as
            // Cancelled. The caller still gets per-file results so pending
            // drafts are preserved without reporting a failure.
            runtime.finish_metadata_save_operation(host, &operation_id, Err(&error))?;
            if matches!(error, AppError::Cancellation(_)) {
                return Ok(cancelled_metadata_save_batch(items));
            }
            return Err(error);
        }
    };
    let _active_work = host.begin_active_work();
    let cancellation = CancellationChecker::new(Some(cancel_flag));

    let result = save_metadata_batch_impl(items, cancellation, |progress| {
        runtime.record_metadata_save_progress(
            host,
            &operation_id,
            &metadata_save_progress_event(progress),
        );
    })
    .await;

    match &result {
        Ok(_) => registry.complete_job(job_id).await,
        Err(error) => registry.fail_job(job_id, error.to_string()).await,
    }

    match result {
        Ok(batch) => {
            let run = InlineRunTerminal {
                summary: &batch.result.summary,
                children: &batch.children,
            };
            runtime.finish_metadata_save_operation(host, &operation_id, Ok(run))?;
            Ok(batch.result)
        }
        Err(error) => {
            runtime.finish_metadata_save_operation(host, &operation_id, Err(&error))?;
            Err(error)
        }
    }
}

fn cancelled_metadata_save_batch(items: Vec<MetadataSaveRequest>) -> MetadataSaveBatchResult {
    MetadataSaveBatchResult::new(
        items
            .into_iter()
            .enumerate()
            .map(|(input_index, item)| MetadataSaveResultEntry {
                input_index,
                file_path: item.file_path,
                status: MetadataSaveResultStatus::Cancelled,
            })
            .collect(),
    )
}

async fn save_metadata_batch_impl<F>(
    items: Vec<MetadataSaveRequest>,
    cancellation: CancellationChecker,
    mut emit_progress: F,
) -> Result<SavedMetadataBatch>
where
    F: FnMut(MetadataSaveProgress),
{
    let total = items.len();
    let mut outcomes = Vec::with_capacity(total);

    for (index, item) in items.into_iter().enumerate() {
        if cancellation.is_cancelled() {
            let message = METADATA_SAVE_CANCELLED_MESSAGE.to_string();
            emit_progress(MetadataSaveProgress {
                input_index: index,
                file_path: item.file_path.clone(),
                stage: EventStage::Cancelled,
                percentage: 0.0,
                message: format!("{} {}/{}", message, index + 1, total),
            });
            outcomes.push((
                MetadataSaveResultEntry {
                    input_index: index,
                    file_path: item.file_path,
                    status: MetadataSaveResultStatus::Cancelled,
                },
                message,
            ));
            continue;
        }

        let display_name = sanitize_path_str_for_display(&item.file_path);
        emit_progress(MetadataSaveProgress {
            input_index: index,
            file_path: item.file_path.clone(),
            stage: EventStage::Writing,
            percentage: 0.0,
            message: format!("Saving metadata {}/{}: {}", index + 1, total, display_name),
        });

        let file_path = item.file_path;
        let metadata_patch = item.metadata_patch;
        let item_result = tokio::task::spawn_blocking({
            let file_path = file_path.clone();
            move || save_metadata_item(&file_path, metadata_patch)
        })
        .await
        .map_err(|error| AppError::General(format!("Metadata save task failed: {error}")))?;

        match item_result {
            Ok(()) => {
                let message = format!("Saved metadata: {}", display_name);
                emit_progress(MetadataSaveProgress {
                    input_index: index,
                    file_path: file_path.clone(),
                    stage: EventStage::Completed,
                    percentage: 100.0,
                    message: message.clone(),
                });
                outcomes.push((
                    MetadataSaveResultEntry {
                        input_index: index,
                        file_path,
                        status: MetadataSaveResultStatus::Success,
                    },
                    message,
                ));
            }
            Err(error) => {
                // The envelope message is the sanitized, user-facing reason.
                let reason = AppErrorEnvelope::from(error).message;
                log::error!("Failed metadata save: {display_name}: {reason}");
                emit_progress(MetadataSaveProgress {
                    input_index: index,
                    file_path: file_path.clone(),
                    stage: EventStage::Failed,
                    percentage: 100.0,
                    message: reason.clone(),
                });
                outcomes.push((
                    MetadataSaveResultEntry {
                        input_index: index,
                        file_path,
                        status: MetadataSaveResultStatus::Failed,
                    },
                    reason,
                ));
            }
        }
    }

    Ok(SavedMetadataBatch::new(outcomes))
}

fn save_metadata_item(file_path: &str, metadata_patch: MetadataIntentPatch) -> Result<()> {
    let path = PathBuf::from(file_path);
    let validated_path = validate_input_audio_path(&path)?;
    log::info!("Saving metadata to: {}", validated_path.display());

    crate::metadata::save_metadata_intent(&validated_path, &metadata_patch)?;

    log::info!("Metadata saved to: {}", validated_path.display());
    Ok(())
}

/// Builds the progress event fed to the metadata-save operation. `job_id` is
/// `None` on purpose: the operation matches progress to children by
/// `input_index`, and a shared `job_id` would cross-match in
/// `WorkRuntimeState::apply_progress_event`.
fn metadata_save_progress_event(progress: MetadataSaveProgress) -> ProgressEvent {
    ProgressEvent {
        operation_kind: OperationKind::MetadataSave,
        stage: progress.stage,
        percentage: progress.percentage,
        message: progress.message,
        current_file: Some(progress.file_path),
        eta_seconds: None,
        job_id: None,
        input_index: Some(progress.input_index),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::PatchOp;

    fn title_patch(title: &str) -> MetadataIntentPatch {
        MetadataIntentPatch {
            title: Some(PatchOp::Set(title.to_string())),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn metadata_batch_returns_ordered_per_file_failures_without_aborting() {
        let items = vec![
            MetadataSaveRequest {
                file_path: "/definitely/missing-a.m4b".to_string(),
                metadata_patch: title_patch("A"),
            },
            MetadataSaveRequest {
                file_path: "/definitely/missing-b.m4b".to_string(),
                metadata_patch: title_patch("B"),
            },
        ];
        let mut progress = Vec::new();
        let cancellation = CancellationChecker::new(None);

        let batch = save_metadata_batch_impl(items, cancellation, |event| progress.push(event))
            .await
            .expect("batch should report per-file failures");
        let result = &batch.result;

        assert_eq!(result.summary.total, 2);
        assert_eq!(result.summary.succeeded, 0);
        assert_eq!(result.summary.failed, 2);
        assert_eq!(result.results[0].input_index, 0);
        assert_eq!(result.results[1].input_index, 1);
        assert_eq!(result.results[0].status, MetadataSaveResultStatus::Failed);
        assert_eq!(result.results[1].status, MetadataSaveResultStatus::Failed);
        assert_eq!(progress.len(), 4);
        assert_eq!(progress[0].stage, EventStage::Writing);
        assert_eq!(progress[1].stage, EventStage::Failed);
        assert_eq!(progress[2].stage, EventStage::Writing);
        assert_eq!(progress[3].stage, EventStage::Failed);
        // Each failed child keeps its own actionable reason for Work Center.
        for (child, name) in batch
            .children
            .iter()
            .zip(["missing-a.m4b", "missing-b.m4b"])
        {
            assert_eq!(child.1, ProcessResultStatus::Failed);
            assert!(
                child.2.contains(name) && !child.2.starts_with("Failed metadata save"),
                "child reason should explain the failure: {}",
                child.2
            );
        }
    }

    #[tokio::test]
    async fn metadata_batch_cancels_remaining_items_between_writes() {
        let items = vec![
            MetadataSaveRequest {
                file_path: "/definitely/missing-a.m4b".to_string(),
                metadata_patch: title_patch("A"),
            },
            MetadataSaveRequest {
                file_path: "/definitely/missing-b.m4b".to_string(),
                metadata_patch: title_patch("B"),
            },
        ];
        let cancellation = CancellationChecker::new(Some(std::sync::Arc::new(
            std::sync::atomic::AtomicBool::new(true),
        )));
        let mut progress = Vec::new();

        let result = save_metadata_batch_impl(items, cancellation, |event| progress.push(event))
            .await
            .expect("cancelled batch should return terminal item results")
            .result;

        assert_eq!(result.summary.total, 2);
        assert_eq!(result.summary.succeeded, 0);
        assert_eq!(result.summary.failed, 0);
        assert_eq!(result.summary.cancelled, 2);
        assert!(result
            .results
            .iter()
            .all(|entry| entry.status == MetadataSaveResultStatus::Cancelled));
        assert!(progress
            .iter()
            .all(|event| event.stage == EventStage::Cancelled));
    }

    #[test]
    fn cancelled_metadata_save_batch_returns_per_file_cancelled_results() {
        let result = cancelled_metadata_save_batch(vec![
            MetadataSaveRequest {
                file_path: "/books/a.m4b".to_string(),
                metadata_patch: title_patch("A"),
            },
            MetadataSaveRequest {
                file_path: "/books/b.m4b".to_string(),
                metadata_patch: title_patch("B"),
            },
        ]);

        assert_eq!(result.summary.total, 2);
        assert_eq!(result.summary.succeeded, 0);
        assert_eq!(result.summary.failed, 0);
        assert_eq!(result.summary.cancelled, 2);
        assert_eq!(result.results[0].input_index, 0);
        assert_eq!(result.results[0].file_path, "/books/a.m4b");
        assert_eq!(result.results[1].input_index, 1);
        assert_eq!(result.results[1].file_path, "/books/b.m4b");
        assert!(result
            .results
            .iter()
            .all(|entry| entry.status == MetadataSaveResultStatus::Cancelled));
    }
}
