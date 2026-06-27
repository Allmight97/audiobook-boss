use crate::audio::validate_input_audio_path;
use crate::commands::CommandResult;
use crate::errors::{sanitize_path_str_for_display, AppError, AppErrorEnvelope, Result};
use crate::metadata::{plan_metadata_write, MetadataIntentPatch};
use crate::processing::{
    CancellationChecker, EventStage, OperationKind, OperationResultSummary, ProcessResultStatus,
    ProgressEvent,
};
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSaveResultEntry {
    pub input_index: usize,
    pub file_path: String,
    pub status: MetadataSaveResultStatus,
    pub message: String,
    pub error: Option<AppErrorEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSaveBatchResult {
    pub summary: MetadataSaveSummary,
    pub results: Vec<MetadataSaveResultEntry>,
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

#[tauri::command]
#[specta::specta]
pub async fn save_metadata_batch(
    window: tauri::Window,
    runtime: tauri::State<'_, crate::work_runtime::WorkRuntime>,
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    items: Vec<MetadataSaveRequest>,
) -> CommandResult<MetadataSaveBatchResult> {
    if items.is_empty() {
        return Err(AppError::InvalidInput("No metadata changes to save".to_string()).into());
    }

    // Metadata save is a WorkRuntime operation rendered in the Work Center.
    // The command still awaits and returns the per-file `MetadataSaveBatchResult`
    // (the frontend clears drafts only for files that succeeded), while progress
    // and terminal truth flow through the operation snapshot. Cancellation is
    // operation-scoped (`cancel_work_operation`); it does not honor the legacy
    // global-cancel flag.
    let file_paths: Vec<String> = items.iter().map(|item| item.file_path.clone()).collect();
    let (operation_id, cancel_flag) =
        runtime.begin_metadata_save_operation(&window, &file_paths)?;

    let (job_id, _permit) = match registry
        .register_job_with_external_cancel(Some(cancel_flag.clone()))
        .await
    {
        Ok(registration) => registration,
        Err(error) => {
            // The operation is already Running + cancellable when the permit wait
            // begins, so a cancel during that wait surfaces here as
            // `AppError::Cancellation`. Terminalize it as Cancelled (not Failed),
            // mirroring the processing path's cancellation handling. The command
            // still returns per-file results so the frontend can preserve pending
            // drafts without surfacing an intentional cancel as a save failure.
            terminalize_metadata_save_abort(&runtime, &window, &operation_id, &error)?;
            if matches!(error, AppError::Cancellation(_)) {
                return Ok(cancelled_metadata_save_batch(items));
            }
            return Err(error.into());
        }
    };
    let cancellation = registry
        .cancellation_checker(job_id)
        .await
        .with_operation_flag(Some(cancel_flag));

    let progress_runtime = (*runtime).clone();
    let progress_window = window.clone();
    let progress_operation_id = operation_id.clone();
    let result = save_metadata_batch_impl(items, cancellation, |progress| {
        progress_runtime.record_metadata_save_progress(
            &progress_window,
            &progress_operation_id,
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
            let child_terminals = metadata_save_child_terminals(&batch);
            runtime.finish_metadata_save_operation(
                &window,
                &operation_id,
                &batch.summary,
                &child_terminals,
            )?;
            Ok(batch)
        }
        Err(error) => {
            terminalize_metadata_save_abort(&runtime, &window, &operation_id, &error)?;
            Err(error.into())
        }
    }
}

/// Terminalizes a metadata-save operation that aborted before producing a result.
/// A cancellation (e.g. cancel during the permit wait) resolves to `Cancelled`;
/// any other abort resolves to `Failed`. Keeps cancel-vs-failure terminal truth
/// symmetric with the processing path.
fn terminalize_metadata_save_abort(
    runtime: &crate::work_runtime::WorkRuntime,
    window: &tauri::Window,
    operation_id: &crate::work_runtime::OperationId,
    error: &AppError,
) -> Result<()> {
    let message = error.to_string();
    if matches!(error, AppError::Cancellation(_)) {
        runtime.cancel_metadata_save_operation(window, operation_id, message)?;
    } else {
        runtime.fail_metadata_save_operation(window, operation_id, message)?;
    }
    Ok(())
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
                message: METADATA_SAVE_CANCELLED_MESSAGE.to_string(),
                error: None,
            })
            .collect(),
    )
}

async fn save_metadata_batch_impl<F>(
    items: Vec<MetadataSaveRequest>,
    cancellation: CancellationChecker,
    mut emit_progress: F,
) -> Result<MetadataSaveBatchResult>
where
    F: FnMut(MetadataSaveProgress),
{
    let total = items.len();
    let mut results = Vec::with_capacity(total);

    for (index, item) in items.into_iter().enumerate() {
        if cancellation.is_cancelled() {
            push_cancelled_entries(index, item, total, &mut results, &mut emit_progress);
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
                results.push(MetadataSaveResultEntry {
                    input_index: index,
                    file_path,
                    status: MetadataSaveResultStatus::Success,
                    message,
                    error: None,
                });
            }
            Err(error) => {
                let envelope = AppErrorEnvelope::from(error);
                let message = format!("Failed metadata save: {}", display_name);
                log::error!("{}: {}", message, envelope.message);
                emit_progress(MetadataSaveProgress {
                    input_index: index,
                    file_path: file_path.clone(),
                    stage: EventStage::Failed,
                    percentage: 100.0,
                    message: envelope.message.clone(),
                });
                results.push(MetadataSaveResultEntry {
                    input_index: index,
                    file_path,
                    status: MetadataSaveResultStatus::Failed,
                    message,
                    error: Some(envelope),
                });
            }
        }
    }

    Ok(MetadataSaveBatchResult::new(results))
}

fn push_cancelled_entries<F>(
    index: usize,
    item: MetadataSaveRequest,
    total: usize,
    results: &mut Vec<MetadataSaveResultEntry>,
    emit_progress: &mut F,
) where
    F: FnMut(MetadataSaveProgress),
{
    let message = METADATA_SAVE_CANCELLED_MESSAGE.to_string();
    emit_progress(MetadataSaveProgress {
        input_index: index,
        file_path: item.file_path.clone(),
        stage: EventStage::Cancelled,
        percentage: 0.0,
        message: format!("{} {}/{}", message, index + 1, total),
    });
    results.push(MetadataSaveResultEntry {
        input_index: index,
        file_path: item.file_path,
        status: MetadataSaveResultStatus::Cancelled,
        message,
        error: None,
    });
}

fn save_metadata_item(file_path: &str, metadata_patch: MetadataIntentPatch) -> Result<()> {
    let path = PathBuf::from(file_path);
    let validated_path = validate_input_audio_path(&path)?;
    log::info!("Saving metadata to: {}", validated_path.display());

    let write_plan = plan_metadata_write(&metadata_patch)?;
    crate::metadata::save_metadata_with_plan(&validated_path, &write_plan)?;

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

/// Maps the per-file batch result to the `(input_index, status, message)` child
/// terminals the WorkRuntime uses to authoritatively terminalize each child.
fn metadata_save_child_terminals(
    batch: &MetadataSaveBatchResult,
) -> Vec<(usize, ProcessResultStatus, String)> {
    batch
        .results
        .iter()
        .map(|entry| {
            let status = match entry.status {
                MetadataSaveResultStatus::Success => ProcessResultStatus::Success,
                MetadataSaveResultStatus::Cancelled => ProcessResultStatus::Cancelled,
                MetadataSaveResultStatus::Failed => ProcessResultStatus::Failed,
            };
            (entry.input_index, status, entry.message.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::PatchOp;
    use crate::processing::JobRegistry;

    fn title_patch(title: &str) -> MetadataIntentPatch {
        MetadataIntentPatch {
            title: PatchOp::Set(title.to_string()),
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
        let registry = JobRegistry::new(1);
        let (job_id, _permit) = registry.register_job().await.expect("register job");
        let cancellation = registry.cancellation_checker(job_id).await;

        let result = save_metadata_batch_impl(items, cancellation, |event| progress.push(event))
            .await
            .expect("batch should report per-file failures");

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
        let registry = JobRegistry::new(1);
        let (job_id, _permit) = registry.register_job().await.expect("register job");
        let cancellation = registry.cancellation_checker(job_id).await;
        registry.cancel_all();
        let mut progress = Vec::new();

        let result = save_metadata_batch_impl(items, cancellation, |event| progress.push(event))
            .await
            .expect("cancelled batch should return terminal item results");

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
        assert!(result.results.iter().all(|entry| {
            entry.status == MetadataSaveResultStatus::Cancelled
                && entry.message == METADATA_SAVE_CANCELLED_MESSAGE
                && entry.error.is_none()
        }));
    }
}
