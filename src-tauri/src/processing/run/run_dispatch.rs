use super::run_job::{run_processing_job, supplemental_assets_for_input, ProcessingJobRequest};
use super::run_validation::resolve_sample_rate;
use super::ProcessingRunOptions;
use crate::audio;
use crate::errors::{AppError, Result};
use crate::processing::plan::{ExecutionProcessingPlan, ResolvedProcessingPlan};
use crate::processing::terminal_outcomes::{
    build_all_skipped_batch_result, collect_batch_results, emit_terminal_failed_event,
    emit_terminal_skipped_event, no_write_skipped_result,
};
use crate::processing::{
    emit_queue_event, JobType, ProcessCommandResult, ProcessPayload, ProcessResultEntry,
    QueueEvent, QueueItem,
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
        output_parent_cleanup,
    } = execution_plan;
    let result =
        dispatch_merge_plan(window, registry, workspace_root, payload, plan, options).await;
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
        output_parent_cleanup,
    } = execution_plan;
    let result =
        dispatch_batch_plan(window, registry, workspace_root, payload, plan, options).await;
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

    let paths: Vec<PathBuf> = payload.input_files.iter().map(PathBuf::from).collect();
    let file_info = audio::get_file_list_info(&paths)?;
    let result = run_processing_job(ProcessingJobRequest {
        window,
        registry,
        workspace_root,
        encoder_settings: payload.settings.clone(),
        sample_rate: resolve_sample_rate(payload)?,
        input_index: None,
        operation_kind: crate::processing::OperationKind::ProcessingMerge,
        operation_id: options.operation_id.clone(),
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

    emit_batch_queue_event(
        &window,
        &registry,
        &payload.input_files,
        options.operation_id.as_deref(),
    );

    let mut scheduled_jobs: Vec<Pin<Box<dyn Future<Output = Result<ProcessResultEntry>> + Send>>> =
        Vec::new();
    let preview_seconds = plan.preview_seconds;
    let sample_rate = resolve_sample_rate(payload)?;
    for planned_job in plan.jobs {
        if let Some(skipped_entry) =
            no_write_skipped_result(planned_job.input_index, None, &planned_job.output)
        {
            emit_terminal_skipped_event(
                &window,
                crate::processing::OperationKind::ProcessingBatch,
                options.operation_id.as_deref(),
                skipped_entry.input_index,
                skipped_entry.job_id.as_deref(),
                &skipped_entry.message,
            );
            scheduled_jobs.push(Box::pin(async move { Ok(skipped_entry) }));
            continue;
        }

        let window_cloned = window.clone();
        let registry_cloned = registry.clone();
        let settings_cloned = payload.settings.clone();
        let sr_cloned = sample_rate.clone();
        let md_cloned = planned_job.metadata.clone();
        let cover_art_passthrough = planned_job.cover_art_passthrough;
        let preview_cloned = preview_seconds;
        let workspace_root_cloned = workspace_root.clone();
        let operation_id = options.operation_id.clone();
        let operation_cancel = options.operation_cancel.clone();
        let input_index = planned_job.input_index;
        let output = planned_job.output.clone();
        let path = planned_job.input_path.clone().ok_or_else(|| {
            AppError::InvalidInput("Missing batch input path in output plan".to_string())
        })?;
        let supplemental_assets = supplemental_assets_for_input(payload, input_index);
        let progress_listener = options.progress_listener.clone();

        scheduled_jobs.push(Box::pin(async move {
            if operation_cancel
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Acquire))
            {
                return Err(AppError::cancelled());
            }
            let file_info = audio::get_file_list_info(std::slice::from_ref(&path))?;
            run_processing_job(ProcessingJobRequest {
                window: window_cloned,
                registry: registry_cloned,
                workspace_root: workspace_root_cloned,
                encoder_settings: settings_cloned,
                sample_rate: sr_cloned,
                input_index,
                operation_kind: crate::processing::OperationKind::ProcessingBatch,
                operation_id,
                operation_cancel,
                output_plan: output,
                file_info,
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
    let finalized_results =
        finalize_batch_results(&window, payload, outcomes, options.operation_id.as_deref())?;

    Ok(ProcessCommandResult::new(JobType::Batch, finalized_results))
}

fn emit_batch_queue_event(
    window: &tauri::Window,
    registry: &crate::ManagedJobRegistry,
    input_files: &[String],
    operation_id: Option<&str>,
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
        crate::processing::OperationKind::ProcessingBatch,
        queue_items,
        registry.max_concurrent(),
    )
    .with_operation_id(operation_id.map(|value| value.to_string()));
    emit_queue_event(window, &queue_event);
}

fn finalize_batch_results(
    window: &tauri::Window,
    payload: &ProcessPayload,
    outcomes: Vec<Result<ProcessResultEntry>>,
    operation_id: Option<&str>,
) -> Result<Vec<ProcessResultEntry>> {
    let finalized = collect_batch_results(payload.input_files.len(), outcomes)?;
    log::debug!(
        "batch terminal classification: {:?}",
        finalized.terminal_class
    );
    for event in finalized.failure_events {
        emit_terminal_failed_event(
            window,
            crate::processing::OperationKind::ProcessingBatch,
            operation_id,
            event.input_index,
            event.job_id.as_deref(),
            &event.message,
        );
    }

    Ok(finalized.results)
}
