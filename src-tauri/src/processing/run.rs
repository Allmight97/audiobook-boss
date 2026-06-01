use super::plan::{prepare_execution_plan, resolve_preflight_plan, ResolvedProcessingPlan};
use super::terminal_outcomes::{
    build_all_skipped_batch_result, classify_processing_error, collect_batch_results,
    emit_terminal_failed_event, emit_terminal_skipped_event, no_write_skipped_result,
    terminal_failure_result, ProcessingJobTerminalOutcome,
};
use crate::audio;
use crate::audio::{
    validate_encoder_settings, AudioExecutionRequest, EncoderSettings, FileListInfo,
};
use crate::errors::{AppError, Result};
use crate::metadata::CoverArtPassthroughPolicy;
use crate::output_artifact::{OutputKind, ResolvedOutputPlan};
use crate::processing::job_registry::{CancellationChecker, JobId};
use crate::processing::{
    emit_queue_event, OperationKind, OutputConfig, PreviewConfig, ProcessingContext,
    ProcessingSession, QueueEvent, QueueItem,
};
use crate::processing::{
    JobType, ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessingPreflightPlan,
};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::sync::OwnedSemaphorePermit;

pub(crate) struct ProcessingRun;

pub(crate) async fn process_payload(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    payload: ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    ProcessingRun::execute(window, registry, payload, metadata, preview_seconds).await
}

pub(crate) fn preflight_payload(
    payload: ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> Result<ProcessingPreflightPlan> {
    ProcessingRun::preflight(payload, metadata, preview_seconds)
}

impl ProcessingRun {
    pub(crate) async fn execute(
        window: tauri::Window,
        registry: crate::ManagedJobRegistry,
        payload: ProcessPayload,
        metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
        preview_seconds: Option<f64>,
    ) -> Result<ProcessCommandResult> {
        validate_encoder_settings(&payload.settings)?;
        validate_external_processing_contract(&payload)?;
        log_encoder_summary(&payload);

        let plan = prepare_execution_plan(&payload, metadata.as_ref(), preview_seconds)?;

        match plan.job_type {
            JobType::Merge => dispatch_merge_job(window, registry, &payload, plan).await,
            JobType::Batch => dispatch_batch_jobs(window, registry, &payload, plan).await,
        }
    }

    pub(crate) fn preflight(
        payload: ProcessPayload,
        metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
        preview_seconds: Option<f64>,
    ) -> Result<ProcessingPreflightPlan> {
        validate_encoder_settings(&payload.settings)?;
        validate_external_processing_contract(&payload)?;

        resolve_preflight_plan(&payload, metadata.as_ref(), preview_seconds)
    }
}

fn log_encoder_summary(payload: &ProcessPayload) {
    log::info!(
        "encoder summary: encoder={:?} bitrate={}k bitrate_mode={:?} channels={:?} sample_rate={:?} afterburner={} threads={:?}",
        payload.settings.encoder_type,
        payload.settings.bitrate_kbps,
        payload.settings.bitrate_mode,
        payload.settings.channels,
        payload.sample_rate,
        payload.settings.afterburner,
        payload.settings.threads
    );
}

fn resolve_sample_rate(payload: &ProcessPayload) -> Result<audio::SampleRateConfig> {
    let sample_rate = payload
        .sample_rate
        .clone()
        .unwrap_or(audio::SampleRateConfig::Auto);
    audio::validate_sample_rate_config(&sample_rate)?;
    Ok(sample_rate)
}

fn validate_external_processing_contract(payload: &ProcessPayload) -> Result<()> {
    let input_paths: Vec<PathBuf> = payload.input_files.iter().map(PathBuf::from).collect();
    let file_info = audio::get_file_list_info(&input_paths)?;
    validate_external_processing_contract_with_file_info(payload, &file_info)
}

fn validate_external_processing_contract_with_file_info(
    payload: &ProcessPayload,
    file_info: &FileListInfo,
) -> Result<()> {
    audio::validate_audio_engine_inputs(&payload.settings, file_info)?;
    Ok(())
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
) -> Result<Vec<ProcessResultEntry>> {
    let finalized = collect_batch_results(payload.input_files.len(), outcomes)?;
    log::debug!(
        "batch terminal classification: {:?}",
        finalized.terminal_class
    );
    for event in finalized.failure_events {
        emit_terminal_failed_event(
            window,
            OperationKind::ProcessingBatch,
            event.input_index,
            event.job_id.as_deref(),
            &event.message,
        );
    }

    Ok(finalized.results)
}

async fn dispatch_merge_job(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    payload: &ProcessPayload,
    plan: ResolvedProcessingPlan,
) -> Result<ProcessCommandResult> {
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
        encoder_settings: payload.settings.clone(),
        sample_rate: resolve_sample_rate(payload)?,
        input_index: None,
        operation_kind: OperationKind::ProcessingMerge,
        output_plan: planned_job.output,
        file_info,
        metadata: planned_job.metadata,
        cover_art_passthrough: planned_job.cover_art_passthrough,
        preview_seconds: plan.preview_seconds,
    })
    .await?;

    Ok(ProcessCommandResult::new(JobType::Merge, vec![result]))
}

async fn dispatch_batch_jobs(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    payload: &ProcessPayload,
    plan: ResolvedProcessingPlan,
) -> Result<ProcessCommandResult> {
    if payload.input_files.is_empty() {
        return Err(AppError::InvalidInput(
            "No input files provided for batch processing".to_string(),
        ));
    }

    if let Some(result) = build_all_skipped_batch_result(&plan) {
        return Ok(result);
    }

    emit_batch_queue_event(&window, &registry, &payload.input_files);

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
                OperationKind::ProcessingBatch,
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
        let input_index = planned_job.input_index;
        let output = planned_job.output.clone();
        let path = planned_job.input_path.clone().ok_or_else(|| {
            AppError::InvalidInput("Missing batch input path in output plan".to_string())
        })?;

        scheduled_jobs.push(Box::pin(async move {
            let file_info = audio::get_file_list_info(std::slice::from_ref(&path))?;
            run_processing_job(ProcessingJobRequest {
                window: window_cloned,
                registry: registry_cloned,
                encoder_settings: settings_cloned,
                sample_rate: sr_cloned,
                input_index,
                operation_kind: OperationKind::ProcessingBatch,
                output_plan: output,
                file_info,
                metadata: md_cloned,
                cover_art_passthrough,
                preview_seconds: preview_cloned,
            })
            .await
        }));
    }

    let outcomes = registry.scheduler().run_batch(scheduled_jobs).await;
    let finalized_results = finalize_batch_results(&window, payload, outcomes)?;

    Ok(ProcessCommandResult::new(JobType::Batch, finalized_results))
}

struct ProcessingJobRequest {
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    encoder_settings: EncoderSettings,
    sample_rate: audio::SampleRateConfig,
    input_index: Option<usize>,
    operation_kind: OperationKind,
    output_plan: ResolvedOutputPlan,
    file_info: FileListInfo,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    cover_art_passthrough: CoverArtPassthroughPolicy,
    preview_seconds: Option<f64>,
}

async fn run_processing_job(request: ProcessingJobRequest) -> Result<ProcessResultEntry> {
    let (job_id, _permit, cancellation_checker) =
        register_job_and_validate_output(&request.registry, &request.output_plan.resolved_path)
            .await?;

    let (context, preview_seconds_resolved) = build_processing_context(ProcessingContextRequest {
        window: request.window,
        cancellation_checker,
        job_id,
        encoder_settings: request.encoder_settings.clone(),
        sample_rate: request.sample_rate,
        input_index: request.input_index,
        operation_kind: request.operation_kind,
        output_plan: request.output_plan.clone(),
        preview_seconds: request.preview_seconds,
    });
    let preview_path = (request.output_plan.kind == OutputKind::Preview)
        .then(|| request.output_plan.resolved_path.display().to_string());
    let result = match execute_processing_job(
        context,
        request.file_info,
        request.metadata,
        request.cover_art_passthrough,
        request.encoder_settings,
    )
    .await
    {
        Ok(message) => ProcessingJobTerminalOutcome::Success {
            message,
            preview_file_path: preview_path,
            preview_actual_seconds: preview_seconds_resolved,
        },
        Err(error) => classify_processing_error(error),
    };

    match result {
        ProcessingJobTerminalOutcome::Success {
            message,
            preview_file_path,
            preview_actual_seconds,
        } => {
            request.registry.complete_job(job_id).await;
            log::info!("Job {} completed successfully", job_id);
            Ok(ProcessResultEntry {
                input_index: request.input_index,
                status: ProcessResultStatus::Success,
                message,
                error: None,
                preview_file_path,
                preview_actual_seconds,
                job_id: Some(job_id.to_string()),
            })
        }
        ProcessingJobTerminalOutcome::Cancelled(error) => {
            request.registry.complete_job(job_id).await;
            log::warn!("Job {} cancelled: {}", job_id, error);
            Err(error)
        }
        ProcessingJobTerminalOutcome::Failed(envelope) => {
            request
                .registry
                .fail_job(job_id, envelope.message.clone())
                .await;
            log::error!("Job {} failed: {}", job_id, envelope.message);
            Ok(terminal_failure_result(
                request.input_index,
                Some(job_id.to_string()),
                envelope,
            ))
        }
    }
}

async fn register_job_and_validate_output(
    registry: &crate::ManagedJobRegistry,
    output_path: &Path,
) -> Result<(JobId, OwnedSemaphorePermit, CancellationChecker)> {
    let (job_id, permit) = registry.register_job().await?;
    log::info!(
        "Job {} started for output: {}",
        job_id,
        output_path.display()
    );
    let cancellation_checker = registry.cancellation_checker(job_id).await;

    if let Err(error) = audio::validate_output_path(output_path) {
        registry.fail_job(job_id, error.to_string()).await;
        return Err(error);
    }

    Ok((job_id, permit, cancellation_checker))
}

struct ProcessingContextRequest {
    window: tauri::Window,
    cancellation_checker: crate::processing::job_registry::CancellationChecker,
    job_id: crate::processing::job_registry::JobId,
    encoder_settings: EncoderSettings,
    sample_rate: audio::SampleRateConfig,
    input_index: Option<usize>,
    operation_kind: OperationKind,
    output_plan: ResolvedOutputPlan,
    preview_seconds: Option<f64>,
}

fn build_processing_context(request: ProcessingContextRequest) -> (ProcessingContext, Option<f64>) {
    let session =
        ProcessingSession::from_job_registry(request.job_id.0, request.cancellation_checker);
    let mut context = ProcessingContext::new(
        request.window,
        std::sync::Arc::new(session),
        request.encoder_settings,
        request.sample_rate,
        OutputConfig::from_plan(request.output_plan),
    );
    context.job_id = Some(request.job_id.to_string());
    context.input_index = request.input_index;
    context.operation_kind = request.operation_kind;

    let preview_seconds_resolved = request.preview_seconds;
    if let Some(seconds) = preview_seconds_resolved {
        context.preview = Some(PreviewConfig::new(seconds));
        log::info!("Preview requested: total_seconds={:.3}", seconds);
    }

    (context, preview_seconds_resolved)
}

async fn execute_processing_job(
    context: ProcessingContext,
    file_info: FileListInfo,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    cover_art_passthrough: CoverArtPassthroughPolicy,
    encoder_settings: EncoderSettings,
) -> Result<String> {
    audio::execute_audio_engine(AudioExecutionRequest::new(
        context,
        file_info,
        metadata,
        cover_art_passthrough,
        encoder_settings,
    ))
    .await
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    #[tokio::test]
    async fn register_job_and_validate_output_cleans_up_failed_validation() {
        let registry = std::sync::Arc::new(crate::processing::JobRegistry::new(2));
        let temp_dir = TempDir::new().expect("create temp dir");
        let invalid_output = temp_dir.path().join("output.mp3");

        let error = match super::register_job_and_validate_output(&registry, &invalid_output).await
        {
            Ok(_) => panic!("invalid extension should fail validation"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("Output must be .m4b file, got: .mp3"),
            "unexpected error: {error}"
        );

        let status = registry.get_aggregate_status().await;
        assert_eq!(status.active_jobs, 0, "active jobs should be cleared");
        assert_eq!(status.total_jobs, 0, "tracked jobs should be cleared");
        assert!(
            registry.list_active_jobs().await.is_empty(),
            "active job list should be empty after failed validation"
        );
        assert_eq!(
            registry
                .update_max_concurrent(1)
                .await
                .expect("idle registry should allow concurrency updates"),
            1
        );
    }
}
