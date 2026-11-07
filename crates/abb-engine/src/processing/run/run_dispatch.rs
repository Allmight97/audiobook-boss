use super::run_job::{run_processing_job, supplemental_assets_for_input, ProcessingJobRequest};
use super::ProcessingRunOptions;
use crate::audio;
use crate::errors::{AppError, Result};
use crate::host::Host;
use crate::output_artifact::OutputParentDirCleanup;
use crate::processing::plan::{ExecutionProcessingPlan, ResolvedProcessingPlan};
use crate::processing::progress::EmitContext;
use crate::processing::terminal_outcomes::{
    build_all_skipped_batch_result, collect_batch_results, emit_terminal_cancelled_event,
    emit_terminal_failed_event, emit_terminal_skipped_event, no_write_skipped_result,
};
use crate::processing::ProcessResultStatus;
use crate::processing::ProgressEventListener;
use crate::processing::{OperationKind, ProcessCommandResult, ProcessPayload, ProcessResultEntry};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use tokio_util::sync::CancellationToken;

pub(crate) async fn dispatch_title_jobs(
    host: Host,
    registry: crate::ManagedJobRegistry,
    workspace_root: PathBuf,
    payload: &ProcessPayload,
    execution_plan: ExecutionProcessingPlan,
    options: ProcessingRunOptions,
) -> Result<ProcessCommandResult> {
    let ExecutionProcessingPlan {
        plan,
        file_info,
        output_parent_cleanup,
    } = execution_plan;
    let folders = Arc::new(Mutex::new(output_parent_cleanup));
    let batch = Batch {
        plan,
        file_info,
        folders: Arc::clone(&folders),
    };
    let result = dispatch_batch_plan(host, registry, workspace_root, payload, batch, options).await;
    let mut folders = folders.lock().unwrap_or_else(PoisonError::into_inner);
    crate::processing::output_parent_cleanup::finalize_output_parent_cleanup(result, &mut folders)
}

struct Batch {
    plan: ResolvedProcessingPlan,
    file_info: audio::FileListInfo,
    folders: Arc<Mutex<OutputParentDirCleanup>>,
}

async fn dispatch_batch_plan(
    host: Host,
    registry: crate::ManagedJobRegistry,
    workspace_root: PathBuf,
    payload: &ProcessPayload,
    batch: Batch,
    options: ProcessingRunOptions,
) -> Result<ProcessCommandResult> {
    let Batch {
        plan,
        file_info,
        folders,
    } = batch;
    if let Some(result) = build_all_skipped_batch_result(&plan) {
        return Ok(result);
    }

    let mut scheduled_jobs: Vec<Pin<Box<dyn Future<Output = Result<ProcessResultEntry>> + Send>>> =
        Vec::new();
    let preview = plan.preview;
    let mut queue_cancels = Vec::new();
    for planned_job in plan.jobs {
        if let Some(skipped_entry) =
            no_write_skipped_result(planned_job.input_index, None, &planned_job.output)
        {
            emit_terminal_skipped_event(
                options.progress_listener.as_ref(),
                EmitContext {
                    operation_kind: OperationKind::ProcessingBatch,
                    job_id: skipped_entry.job_id.clone(),
                    input_index: Some(skipped_entry.input_index),
                },
                &skipped_entry.message,
            );
            if let Some(title) = options.title_outputs.get(planned_job.input_index) {
                title.end();
            }
            scheduled_jobs.push(Box::pin(async move { Ok(skipped_entry) }));
            queue_cancels.push(None);
            continue;
        }

        let input_index = planned_job.input_index;
        let request = ProcessingJobRequest {
            host: host.clone(),
            registry: registry.clone(),
            workspace_root: workspace_root.clone(),
            audio_handling: planned_job.audio_plan.handling.clone(),
            audio_request: payload.audio_requests[input_index].clone(),
            audio_reason: planned_job.audio_plan.reason.clone(),
            metadata_intent: planned_job.metadata_intent.clone(),
            sample_rate: audio::SampleRateConfig::Explicit(planned_job.audio_plan.sample_rate),
            aac_decoder: payload.accepted_aac_decoder()?,
            input_index,
            operation_kind: OperationKind::ProcessingBatch,
            operation_id: options.operation_id.clone(),
            title_cancel: options.title_cancels.get(input_index).cloned(),
            output_plan: planned_job.output.clone(),
            file_info: crate::processing::plan::title_file_info(
                &file_info,
                &planned_job.source_paths,
            )?,
            metadata: planned_job.metadata.clone(),
            cover_art_passthrough: planned_job.cover_art_passthrough,
            preview,
            supplemental_assets: supplemental_assets_for_input(payload, input_index),
            progress_listener: options.progress_listener.clone(),
            title_output: options.title_outputs.get(input_index).cloned(),
        };
        queue_cancels.push(options.title_cancels.get(input_index).cloned());
        scheduled_jobs.push(Box::pin(run_title_job(request, Arc::clone(&folders))));
    }

    let outcomes = registry.run_batch(scheduled_jobs, queue_cancels).await;
    let finalized_results = finalize_batch_results(outcomes, options.progress_listener.as_ref())?;

    Ok(ProcessCommandResult::new(finalized_results))
}

/// Runs one title unless its cancel token is already cancelled. A title cancelled
/// before its job started emits nothing else, so report it here instead of
/// leaving its row cancelling until the batch ends. A title that ends without publishing
/// removes the empty folders made only for it before it reports.
async fn run_title_job(
    request: ProcessingJobRequest,
    folders: Arc<Mutex<OutputParentDirCleanup>>,
) -> Result<ProcessResultEntry> {
    let listener = request.progress_listener.clone();
    let input_index = request.input_index;
    let title_output = request.title_output.clone();
    let cancelled = request
        .title_cancel
        .as_ref()
        .is_some_and(CancellationToken::is_cancelled);
    let outcome = if cancelled {
        Err(AppError::cancelled())
    } else {
        run_processing_job(request).await
    };
    let published = outcome.as_ref().is_ok_and(|entry| {
        matches!(
            entry.status,
            ProcessResultStatus::Success | ProcessResultStatus::Skipped
        )
    });
    let mut folders = folders.lock().unwrap_or_else(PoisonError::into_inner);
    if let Err(error) = folders.end_title(input_index, published) {
        log::warn!(
            "output_parent_cleanup status=title_cleanup_err input_index={input_index} err={error}"
        );
    }
    drop(folders);
    if let Some(title) = &title_output {
        title.end();
    }
    if listener.is_some() && matches!(outcome, Err(AppError::Cancellation(_))) {
        emit_terminal_cancelled_event(
            listener.as_ref(),
            EmitContext {
                operation_kind: OperationKind::ProcessingBatch,
                job_id: None,
                input_index: Some(input_index),
            },
            "Processing was cancelled",
        );
    }
    outcome
}

fn finalize_batch_results(
    outcomes: Vec<Result<ProcessResultEntry>>,
    progress_listener: Option<&ProgressEventListener>,
) -> Result<Vec<ProcessResultEntry>> {
    let finalized = collect_batch_results(outcomes)?;
    for event in finalized.failure_events {
        emit_terminal_failed_event(
            progress_listener,
            EmitContext {
                operation_kind: OperationKind::ProcessingBatch,
                job_id: event.job_id.clone(),
                input_index: Some(event.input_index),
            },
            &event.message,
        );
    }

    Ok(finalized.results)
}
