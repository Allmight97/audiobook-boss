use crate::audio::validate_input_audio_path;
use crate::errors::{sanitize_path_str_for_display, AppError, AppErrorEnvelope, Result};
use crate::host::Host;
use crate::metadata::MetadataIntentPatch;
use crate::processing::{
    CancellationChecker, EventStage, OperationKind, OperationResultSummary, ProcessResultStatus,
    ProgressEvent,
};
use crate::work_runtime::{InlineRunTerminal, WorkRuntime};
use std::path::PathBuf;

const METADATA_SAVE_CANCELLED_MESSAGE: &str = "Metadata save cancelled.";

#[derive(Debug, Clone)]
pub(crate) struct MetadataSaveRequest {
    pub file_path: String,
    pub metadata_patch: MetadataIntentPatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataSaveResultStatus {
    Success,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetadataSaveResultEntry {
    pub input_index: usize,
    pub file_path: String,
    pub status: MetadataSaveResultStatus,
    /// The file's fingerprint just before and just after this write, when
    /// the write succeeded and both could be read.
    pub rewrite: Option<Rewrite>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rewrite {
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetadataSaveBatchResult {
    pub summary: OperationResultSummary,
    pub results: Vec<MetadataSaveResultEntry>,
}

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
                rewrite: None,
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
                    rewrite: None,
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
        #[expect(
            clippy::disallowed_methods,
            reason = "joined by save_metadata_batch_impl"
        )]
        let item_result = tokio::task::spawn_blocking({
            let file_path = file_path.clone();
            move || save_metadata_item(&file_path, metadata_patch)
        })
        .await
        .map_err(|error| AppError::General(format!("Metadata save task failed: {error}")))?;

        match item_result {
            Ok(rewrite) => {
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
                        rewrite,
                    },
                    message,
                ));
            }
            Err(error) => {
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
                        rewrite: None,
                    },
                    reason,
                ));
            }
        }
    }

    Ok(SavedMetadataBatch::new(outcomes))
}

fn save_metadata_item(
    file_path: &str,
    metadata_patch: MetadataIntentPatch,
) -> Result<Option<Rewrite>> {
    let path = PathBuf::from(file_path);
    let validated_path = validate_input_audio_path(&path)?;
    log::info!("Saving metadata to: {}", validated_path.display());

    let before = crate::metadata::source_fingerprint(&validated_path).ok();
    crate::metadata::save_metadata_intent(&validated_path, &metadata_patch)?;
    let after = crate::metadata::source_fingerprint(&validated_path).ok();

    log::info!("Metadata saved to: {}", validated_path.display());
    Ok(before
        .zip(after)
        .map(|(before, after)| Rewrite { before, after }))
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
#[path = "metadata_save_tests.rs"]
mod tests;
