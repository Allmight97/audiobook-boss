use super::run_job::{run_processing_job, supplemental_assets_for_input, ProcessingJobRequest};
use super::ProcessingRunOptions;
use crate::audio;
use crate::errors::{AppError, Result};
use crate::processing::context::processing::ProgressEventListener;
use crate::processing::plan::{ExecutionProcessingPlan, ResolvedProcessingPlan};
use crate::processing::progress::EmitContext;
use crate::processing::terminal_outcomes::{
    build_all_skipped_batch_result, collect_batch_results, emit_terminal_failed_event,
    emit_terminal_skipped_event, no_write_skipped_result,
};
use crate::processing::{
    emit_queue_event, JobType, OperationKind, ProcessCommandResult, ProcessPayload,
    ProcessResultEntry, QueueEvent, QueueItem,
};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::Ordering;

pub(crate) async fn dispatch_merge_job(
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
    let result = dispatch_merge_plan(
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

pub(crate) async fn dispatch_batch_jobs(
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

async fn dispatch_merge_plan(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    workspace_root: PathBuf,
    payload: &ProcessPayload,
    plan: ResolvedProcessingPlan,
    file_info: audio::FileListInfo,
    options: ProcessingRunOptions,
) -> Result<ProcessCommandResult> {
    if options.is_operation_cancelled() {
        return Err(AppError::cancelled());
    }

    let planned_job = plan.jobs.into_iter().next().ok_or_else(|| {
        AppError::InvalidInput("No output plan entries were built for merge processing".to_string())
    })?;

    if let Some(skipped) = no_write_skipped_result(None, None, &planned_job.output) {
        return Ok(ProcessCommandResult::new(JobType::Merge, vec![skipped]));
    }

    let result = run_processing_job(ProcessingJobRequest {
        window,
        registry,
        workspace_root,
        encoder_settings: planned_job.audio_plan.settings.clone(),
        audio_handling: planned_job.audio_plan.handling,
        audio_request: payload.audio_requests[0].clone(),
        audio_reason: planned_job.audio_plan.reason.clone(),
        metadata_intent: planned_job.metadata_intent,
        sample_rate: audio::SampleRateConfig::Explicit(planned_job.audio_plan.sample_rate),
        input_index: None,
        operation_kind: OperationKind::ProcessingMerge,
        operation_id: options.operation_id,
        operation_cancel: options.operation_cancel.clone(),
        output_plan: planned_job.output,
        file_info,
        metadata: planned_job.metadata,
        cover_art_passthrough: planned_job.cover_art_passthrough,
        preview_seconds: plan.preview_seconds,
        supplemental_assets: Vec::new(),
        progress_listener: options.progress_listener,
    })
    .await?;

    Ok(ProcessCommandResult::new(JobType::Merge, vec![result]))
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
            "No input files provided for batch processing".to_string(),
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
                    input_index: skipped_entry.input_index,
                },
                &skipped_entry.message,
            );
            scheduled_jobs.push(Box::pin(async move { Ok(skipped_entry) }));
            continue;
        }

        let window_cloned = window.clone();
        let registry_cloned = registry.clone();
        let settings_cloned = planned_job.audio_plan.settings.clone();
        let sr_cloned = audio::SampleRateConfig::Explicit(planned_job.audio_plan.sample_rate);
        let md_cloned = planned_job.metadata.clone();
        let cover_art_passthrough = planned_job.cover_art_passthrough;
        let preview_cloned = preview_seconds;
        let workspace_root_cloned = workspace_root.clone();
        let operation_cancel = options.operation_cancel.clone();
        let operation_id = options.operation_id.clone();
        let input_index = planned_job.input_index;
        let output = planned_job.output.clone();
        let source_paths = planned_job.source_paths.clone();
        let mut title_info = crate::processing::plan::title_file_info(&file_info, &source_paths)?;
        let supplemental_assets = supplemental_assets_for_input(payload, input_index);
        let progress_listener = options.progress_listener.clone();
        let chapter_plans = payload.chapter_plans.clone();
        let audio_handling = planned_job.audio_plan.handling;
        let audio_request = payload.audio_requests[input_index.expect("batch title index")].clone();
        let audio_reason = planned_job.audio_plan.reason.clone();
        let metadata_intent = planned_job.metadata_intent.clone();

        scheduled_jobs.push(Box::pin(async move {
            if operation_cancel
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Acquire))
            {
                return Err(AppError::cancelled());
            }
            audio::apply_chapter_plans(
                &mut title_info,
                chapter_plans.as_ref(),
                source_paths.len() > 1,
            )?;
            run_processing_job(ProcessingJobRequest {
                window: window_cloned,
                registry: registry_cloned,
                workspace_root: workspace_root_cloned,
                encoder_settings: settings_cloned,
                audio_handling,
                audio_request,
                audio_reason,
                metadata_intent,
                sample_rate: sr_cloned,
                input_index,
                operation_kind: OperationKind::ProcessingBatch,
                operation_id,
                operation_cancel,
                output_plan: output,
                file_info: title_info,
                metadata: md_cloned,
                cover_art_passthrough,
                preview_seconds: preview_cloned,
                supplemental_assets,
                progress_listener,
            })
            .await
        }));
    }

    let outcomes = registry.scheduler().run_batch(scheduled_jobs).await;
    let finalized_results = finalize_batch_results(
        &window,
        payload,
        outcomes,
        options.progress_listener.as_ref(),
    )?;

    Ok(ProcessCommandResult::new(JobType::Batch, finalized_results))
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
                input_index: event.input_index,
            },
            &event.message,
        );
    }

    Ok(finalized.results)
}
