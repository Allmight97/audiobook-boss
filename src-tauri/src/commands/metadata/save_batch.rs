use crate::audio::path_validation::validate_input_audio_path;
use crate::commands::CommandResult;
use crate::errors::{sanitize_path_str_for_display, AppError, AppErrorEnvelope, Result};
use crate::metadata::{plan_metadata_write, MetadataIntentPatch};
use crate::processing::{
    CancellationChecker, EventStage, JobId, ProgressEvent, QueueEvent, QueueItem,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Emitter;

pub type MetadataSaveSummary = crate::processing::ProcessResultSummary;

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
    job_id: Option<String>,
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
            summary: MetadataSaveSummary {
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
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    items: Vec<MetadataSaveRequest>,
) -> CommandResult<MetadataSaveBatchResult> {
    if items.is_empty() {
        return Err(AppError::InvalidInput("No metadata changes to save".to_string()).into());
    }

    if registry.is_global_cancelled() && registry.get_aggregate_status().await.total_jobs == 0 {
        registry.reset_global_cancel();
    }

    let (job_id, _permit) = registry.register_job().await?;
    let cancellation = registry.cancellation_checker(job_id).await;
    emit_metadata_save_queue(&window, &items);
    let result = save_metadata_batch_impl(items, job_id, cancellation, |progress| {
        emit_metadata_save_progress(&window, progress);
    })
    .await;

    match &result {
        Ok(_) => registry.complete_job(job_id).await,
        Err(error) => registry.fail_job(job_id, error.to_string()).await,
    }

    Ok(result?)
}

async fn save_metadata_batch_impl<F>(
    items: Vec<MetadataSaveRequest>,
    job_id: JobId,
    cancellation: CancellationChecker,
    mut emit_progress: F,
) -> Result<MetadataSaveBatchResult>
where
    F: FnMut(MetadataSaveProgress),
{
    let total = items.len();
    let mut results = Vec::with_capacity(total);
    let job_id_string = job_id.to_string();

    for (index, item) in items.into_iter().enumerate() {
        if cancellation.is_cancelled() {
            push_cancelled_entries(
                index,
                item,
                total,
                &job_id_string,
                &mut results,
                &mut emit_progress,
            );
            continue;
        }

        let display_name = sanitize_path_str_for_display(&item.file_path);
        emit_progress(MetadataSaveProgress {
            input_index: index,
            file_path: item.file_path.clone(),
            stage: EventStage::Writing,
            percentage: 0.0,
            message: format!("Saving metadata {}/{}: {}", index + 1, total, display_name),
            job_id: Some(job_id_string.clone()),
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
                    job_id: Some(job_id_string.clone()),
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
                    job_id: Some(job_id_string.clone()),
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
    job_id: &str,
    results: &mut Vec<MetadataSaveResultEntry>,
    emit_progress: &mut F,
) where
    F: FnMut(MetadataSaveProgress),
{
    let message = "Metadata save cancelled.".to_string();
    emit_progress(MetadataSaveProgress {
        input_index: index,
        file_path: item.file_path.clone(),
        stage: EventStage::Cancelled,
        percentage: 0.0,
        message: format!("{} {}/{}", message, index + 1, total),
        job_id: Some(job_id.to_string()),
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

fn emit_metadata_save_queue(window: &tauri::Window, items: &[MetadataSaveRequest]) {
    let queue_event = QueueEvent {
        items: items
            .iter()
            .enumerate()
            .map(|(index, item)| QueueItem {
                input_index: index,
                file_path: item.file_path.clone(),
            })
            .collect(),
        max_concurrent: 1,
    };
    let _ = window.emit(crate::audio::constants::QUEUE_EVENT_NAME, &queue_event);
}

fn emit_metadata_save_progress(window: &tauri::Window, progress: MetadataSaveProgress) {
    let event = ProgressEvent {
        stage: progress.stage,
        percentage: progress.percentage,
        message: progress.message,
        current_file: Some(progress.file_path),
        eta_seconds: None,
        job_id: progress.job_id,
        input_index: Some(progress.input_index),
    };
    let _ = window.emit(crate::audio::constants::PROGRESS_EVENT_NAME, &event);
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

        let result =
            save_metadata_batch_impl(items, job_id, cancellation, |event| progress.push(event))
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

        let result =
            save_metadata_batch_impl(items, job_id, cancellation, |event| progress.push(event))
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
}
