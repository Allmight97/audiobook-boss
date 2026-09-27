use super::run_job::{run_processing_job, supplemental_assets_for_input, ProcessingJobRequest};
use super::ProcessingRunOptions;
use crate::audio;
use crate::errors::{AppError, Result};
use crate::processing::context::processing::ProgressEventListener;
use crate::processing::plan::{ExecutionProcessingPlan, ResolvedProcessingPlan};
use crate::processing::progress::EmitContext;
use crate::processing::terminal_outcomes::{
    build_all_skipped_batch_result, collect_batch_results, emit_terminal_cancelled_event,
    emit_terminal_failed_event, emit_terminal_skipped_event, no_write_skipped_result,
};
use crate::processing::{
    emit_queue_event, OperationKind, ProcessCommandResult, ProcessPayload, ProcessResultEntry,
    QueueEvent, QueueItem,
};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::Ordering;

pub(crate) async fn dispatch_title_jobs(
    window: tauri::Window,
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
    let result = dispatch_batch_plan(
        window,
        registry,
        workspace_root,
        payload,
        plan,
        file_info,
        options,
    )
    .await;
    crate::processing::output_parent_cleanup::finalize_output_parent_cleanup(
        result,
        output_parent_cleanup,
    )
}

async fn dispatch_batch_plan(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    workspace_root: PathBuf,
    payload: &ProcessPayload,
    plan: ResolvedProcessingPlan,
    file_info: audio::FileListInfo,
    options: ProcessingRunOptions,
) -> Result<ProcessCommandResult> {
    if payload.input_files.is_empty() {
        return Err(AppError::InvalidInput(
            "No input files provided for processing".to_string(),
        ));
    }

    if let Some(result) = build_all_skipped_batch_result(&plan) {
        return Ok(result);
    }

    // Foreground (preview) runs drive the Status Panel from queue events; background
    // operations render from WorkRuntime snapshots, so they emit no queue event.
    if options.progress_listener.is_none() {
        emit_batch_queue_event(&window, &registry, &payload.input_files);
    }

    let mut scheduled_jobs: Vec<Pin<Box<dyn Future<Output = Result<ProcessResultEntry>> + Send>>> =
        Vec::new();
    let preview_seconds = plan.preview_seconds;
    for planned_job in plan.jobs {
        if let Some(skipped_entry) =
            no_write_skipped_result(planned_job.input_index, None, &planned_job.output)
        {
            emit_terminal_skipped_event(
                &window,
                options.progress_listener.as_ref(),
                EmitContext {
                    operation_kind: OperationKind::ProcessingBatch,
                    job_id: skipped_entry.job_id.clone(),
                    input_index: Some(skipped_entry.input_index),
                },
                &skipped_entry.message,
            );
            scheduled_jobs.push(Box::pin(async move { Ok(skipped_entry) }));
            continue;
        }

        let input_index = planned_job.input_index;
        let request = ProcessingJobRequest {
            window: window.clone(),
            registry: registry.clone(),
            workspace_root: workspace_root.clone(),
            encoder_settings: planned_job.audio_plan.settings.clone(),
            audio_handling: planned_job.audio_plan.handling,
            audio_request: payload.audio_requests[input_index].clone(),
            audio_reason: planned_job.audio_plan.reason.clone(),
            metadata_intent: planned_job.metadata_intent.clone(),
            sample_rate: audio::SampleRateConfig::Explicit(planned_job.audio_plan.sample_rate),
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
            preview_seconds,
            supplemental_assets: supplemental_assets_for_input(payload, input_index),
            progress_listener: options.progress_listener.clone(),
        };
        scheduled_jobs.push(Box::pin(run_title_job(request)));
    }

    let outcomes = registry.scheduler().run_batch(scheduled_jobs).await;
    let finalized_results = finalize_batch_results(
        &window,
        payload,
        outcomes,
        options.progress_listener.as_ref(),
    )?;

    Ok(ProcessCommandResult::new(finalized_results))
}

/// Runs one title unless its cancel flag is already set. A title cancelled
/// before its job started emits nothing else, so report it here instead of
/// leaving its row cancelling until the batch ends. Background operations
/// only; previews carry no flags.
async fn run_title_job(request: ProcessingJobRequest) -> Result<ProcessResultEntry> {
    let window = request.window.clone();
    let listener = request.progress_listener.clone();
    let input_index = request.input_index;
    let cancelled = request
        .title_cancel
        .as_ref()
        .is_some_and(|flag| flag.load(Ordering::Acquire));
    let outcome = if cancelled {
        Err(AppError::cancelled())
    } else {
        run_processing_job(request).await
    };
    if listener.is_some() && matches!(outcome, Err(AppError::Cancellation(_))) {
        emit_terminal_cancelled_event(
            &window,
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

fn emit_batch_queue_event(
    window: &tauri::Window,
    registry: &crate::ManagedJobRegistry,
    input_files: &[String],
) {
    let queue_items: Vec<QueueItem> = input_files
        .iter()
        .enumerate()
        .map(|(index, input)| QueueItem {
            input_index: index,
            file_path: input.clone(),
        })
        .collect();
    let queue_event = QueueEvent::new(
        OperationKind::ProcessingBatch,
        queue_items,
        registry.max_concurrent(),
    );
    emit_queue_event(window, &queue_event);
}

fn finalize_batch_results(
    window: &tauri::Window,
    payload: &ProcessPayload,
    outcomes: Vec<Result<ProcessResultEntry>>,
    progress_listener: Option<&ProgressEventListener>,
) -> Result<Vec<ProcessResultEntry>> {
    let finalized = collect_batch_results(payload.input_files.len(), outcomes)?;
    log::debug!(
        "batch terminal classification: {:?}",
        finalized.terminal_class
    );
    for event in finalized.failure_events {
        emit_terminal_failed_event(
            window,
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
