use crate::audio;
use crate::audio::{
    AudioExecutionRequest, EncoderSettings, FileListInfo,
};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::CoverArtPassthroughPolicy;
use crate::output_artifact::{
    commit_supplemental_output_asset, OutputKind, ResolvedOutputPlan,
    SupplementalOutputAssetCommitRequest,
};
use crate::processing::context::processing::ProgressEventListener;
use crate::processing::job_registry::{CancellationChecker, JobId};
use crate::processing::{
    OperationKind, OutputConfig, PreviewConfig, ProcessingContext, ProcessingSession,
    ProcessPayload, ProcessResultEntry, ProcessResultStatus, SupplementalProcessingAsset,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;
use std::sync::atomic::{AtomicBool};

use crate::processing::terminal_outcomes::{
    classify_processing_error, terminal_failure_result, ProcessingJobTerminalOutcome,
};

pub(crate) struct ProcessingJobRequest {
    pub(crate) window: tauri::Window,
    pub(crate) registry: crate::ManagedJobRegistry,
    pub(crate) workspace_root: PathBuf,
    pub(crate) encoder_settings: EncoderSettings,
    pub(crate) sample_rate: audio::SampleRateConfig,
    pub(crate) input_index: Option<usize>,
    pub(crate) operation_kind: OperationKind,
    pub(crate) operation_id: Option<String>,
    pub(crate) operation_cancel: Option<Arc<AtomicBool>>,
    pub(crate) output_plan: ResolvedOutputPlan,
    pub(crate) file_info: FileListInfo,
    pub(crate) metadata: Option<crate::metadata::AudiobookMetadata>,
    pub(crate) cover_art_passthrough: CoverArtPassthroughPolicy,
    pub(crate) preview_seconds: Option<f64>,
    pub(crate) supplemental_assets: Vec<SupplementalProcessingAsset>,
    pub(crate) progress_listener: Option<ProgressEventListener>,
}

pub(crate) async fn run_processing_job(
    request: ProcessingJobRequest,
) -> Result<ProcessResultEntry> {
    let (job_id, _permit, cancellation_checker) = register_job_and_validate_output(
        &request.registry,
        &request.output_plan.resolved_path,
        request.operation_cancel.clone(),
    )
    .await?;
    let cancellation_checker =
        cancellation_checker.with_operation_flag(request.operation_cancel.clone());

    let (context, preview_seconds_resolved) = build_processing_context(ProcessingContextRequest {
        window: request.window,
        cancellation_checker,
        job_id,
        encoder_settings: request.encoder_settings.clone(),
        sample_rate: request.sample_rate,
        input_index: request.input_index,
        operation_kind: request.operation_kind,
        operation_id: request.operation_id.clone(),
        progress_listener: request.progress_listener,
        output_plan: request.output_plan.clone(),
        workspace_root: request.workspace_root,
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
        Ok(message) => match commit_supplemental_assets_for_output(
            request.output_plan.kind,
            &request.supplemental_assets,
            &request.output_plan.resolved_path,
        ) {
            Ok(()) => ProcessingJobTerminalOutcome::Success {
                message,
                preview_file_path: preview_path,
                preview_actual_seconds: preview_seconds_resolved,
            },
            Err(error) => classify_processing_error(supplemental_commit_failure(
                &request.output_plan.resolved_path,
                error,
            )),
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

pub(crate) fn supplemental_assets_for_input(
    payload: &ProcessPayload,
    input_index: Option<usize>,
) -> Vec<SupplementalProcessingAsset> {
    let Some(index) = input_index else {
        return Vec::new();
    };
    let Some(input_id) = payload
        .input_ids
        .as_ref()
        .and_then(|ids| ids.get(index))
        .and_then(|value| value.as_ref())
    else {
        return Vec::new();
    };
    payload
        .supplemental_assets_by_input_id
        .as_ref()
        .and_then(|assets| assets.get(input_id))
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn commit_supplemental_assets(
    assets: &[SupplementalProcessingAsset],
    final_audio_path: &Path,
) -> Result<()> {
    for asset in assets {
        commit_supplemental_output_asset(
            SupplementalOutputAssetCommitRequest::new(&asset.path, final_audio_path)
                .with_expected_identity(asset.size_bytes, &asset.sha256),
        )?;
    }
    Ok(())
}

pub(crate) fn supplemental_commit_failure(
    final_audio_path: &Path,
    error: AppError,
) -> AppError {
    let detail = match error {
        AppError::FileValidation(message) => message,
        other => other.to_string(),
    };
    AppError::FileValidation(format!(
        "Audiobook output '{}' was created, but the requested Supplemental PDF could not be committed: {detail}",
        sanitize_path_for_display(final_audio_path)
    ))
}

pub(crate) fn commit_supplemental_assets_for_output(
    output_kind: OutputKind,
    assets: &[SupplementalProcessingAsset],
    output_path: &Path,
) -> Result<()> {
    if output_kind != OutputKind::Final {
        return Ok(());
    }
    commit_supplemental_assets(assets, output_path)
}

pub(crate) async fn register_job_and_validate_output(
    registry: &crate::ManagedJobRegistry,
    output_path: &Path,
    operation_cancel: Option<Arc<AtomicBool>>,
) -> Result<(JobId, OwnedSemaphorePermit, CancellationChecker)> {
    let (job_id, permit) = registry
        .register_job_with_external_cancel(operation_cancel)
        .await?;
    log::info!(
        "Job {} started for output: {}",
        job_id,
        output_path.display()
    );
    let cancellation_checker = registry.cancellation_checker(job_id).await;

    if let Err(error) = crate::audio::validate_output_path(output_path) {
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
    operation_id: Option<String>,
    progress_listener: Option<ProgressEventListener>,
    output_plan: ResolvedOutputPlan,
    workspace_root: PathBuf,
    preview_seconds: Option<f64>,
}

fn build_processing_context(
    request: ProcessingContextRequest,
) -> (ProcessingContext, Option<f64>) {
    let session =
        ProcessingSession::from_job_registry(request.job_id.0, request.cancellation_checker);
    let mut context = ProcessingContext::new_with_workspace_root(
        request.window,
        std::sync::Arc::new(session),
        request.encoder_settings,
        request.sample_rate,
        OutputConfig::from_plan(request.output_plan),
        request.workspace_root,
    );
    context.job_id = Some(request.job_id.to_string());
    context.input_index = request.input_index;
    context.operation_kind = request.operation_kind;
    context.operation_id = request.operation_id;
    context.progress_listener = request.progress_listener;

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
