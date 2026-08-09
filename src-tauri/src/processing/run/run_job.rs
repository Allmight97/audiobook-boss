use crate::audio;
use crate::audio::{AudioExecutionRequest, EncoderSettings, FileListInfo};
use crate::errors::{AppErrorEnvelope, Result};
use crate::metadata::CoverArtPassthroughPolicy;
use crate::output_artifact::{
    commit_supplemental_output_assets_for_output, OutputKind, ResolvedOutputPlan,
    SupplementalOutputAssetsCommitRequest,
};
use crate::processing::context::processing::ProgressEventListener;
use crate::processing::job_registry::{CancellationChecker, JobId};
use crate::processing::{
    OperationKind, OutputConfig, PreviewConfig, ProcessPayload, ProcessResultEntry,
    ProcessResultStatus, ProcessingContext, ProcessingSession, SupplementalProcessingAsset,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::OwnedSemaphorePermit;

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
    let operation_id = request.operation_id.clone();
    let RegisteredProcessingJob {
        job_id,
        permit: _permit,
        cancellation_checker,
        lifecycle_log,
    } = register_job_and_validate_output(
        &request.registry,
        &request.output_plan.resolved_path,
        request.operation_cancel.clone(),
        ProcessingJobLogContext {
            operation_id: operation_id.clone(),
            input_index: request.input_index,
            operation_kind: request.operation_kind,
        },
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
        operation_id,
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
        Ok(message) => match commit_supplemental_assets(
            request.output_plan.kind,
            &request.supplemental_assets,
            &request.output_plan.resolved_path,
        ) {
            Ok(()) => ProcessingJobTerminalOutcome::Success {
                message,
                preview_file_path: preview_path,
                preview_actual_seconds: preview_seconds_resolved,
            },
            Err(error) => classify_processing_error(error),
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
            lifecycle_log.log_terminal(ProcessingJobLogStatus::Success);
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
            lifecycle_log.log_terminal(ProcessingJobLogStatus::Cancelled);
            Err(error)
        }
        ProcessingJobTerminalOutcome::Failed(envelope) => {
            request
                .registry
                .fail_job(job_id, envelope.message.clone())
                .await;
            lifecycle_log.log_terminal(ProcessingJobLogStatus::Failed(&envelope));
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
    output_kind: OutputKind,
    assets: &[SupplementalProcessingAsset],
    output_path: &Path,
) -> Result<()> {
    let request = assets.iter().fold(
        SupplementalOutputAssetsCommitRequest::new(output_kind, output_path),
        |request, asset| request.with_asset(&asset.path, asset.size_bytes, &asset.sha256),
    );
    commit_supplemental_output_assets_for_output(request)
}

/// Job identity known before registration, used for lifecycle records.
pub(crate) struct ProcessingJobLogContext {
    pub(crate) operation_id: Option<String>,
    pub(crate) input_index: Option<usize>,
    pub(crate) operation_kind: OperationKind,
}

pub(crate) struct RegisteredProcessingJob {
    pub(crate) job_id: JobId,
    pub(crate) permit: OwnedSemaphorePermit,
    pub(crate) cancellation_checker: CancellationChecker,
    pub(crate) lifecycle_log: ProcessingJobLifecycleLog,
}

pub(crate) async fn register_job_and_validate_output(
    registry: &crate::ManagedJobRegistry,
    output_path: &Path,
    operation_cancel: Option<Arc<AtomicBool>>,
    log_context: ProcessingJobLogContext,
) -> Result<RegisteredProcessingJob> {
    let (job_id, permit) = registry
        .register_job_with_external_cancel(operation_cancel)
        .await?;
    let cancellation_checker = registry.cancellation_checker(job_id).await;
    let lifecycle_log = ProcessingJobLifecycleLog::start(ProcessingJobLogIdentity {
        operation_id: log_context.operation_id,
        job_id: job_id.to_string(),
        input_index: log_context.input_index,
        operation_kind: log_context.operation_kind,
    });

    if let Err(error) = crate::audio::validate_output_path(output_path) {
        let envelope = AppErrorEnvelope::from(&error);
        lifecycle_log.log_terminal(ProcessingJobLogStatus::Failed(&envelope));
        registry.fail_job(job_id, error.to_string()).await;
        return Err(error);
    }

    Ok(RegisteredProcessingJob {
        job_id,
        permit,
        cancellation_checker,
        lifecycle_log,
    })
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

fn build_processing_context(request: ProcessingContextRequest) -> (ProcessingContext, Option<f64>) {
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
    context.operation_id = request.operation_id;
    context.input_index = request.input_index;
    context.operation_kind = request.operation_kind;
    context.progress_listener = request.progress_listener;

    let preview_seconds_resolved = request.preview_seconds;
    if let Some(seconds) = preview_seconds_resolved {
        context.preview = Some(PreviewConfig::new(seconds));
        log::info!("Preview requested: total_seconds={:.3}", seconds);
    }

    (context, preview_seconds_resolved)
}

pub(crate) struct ProcessingJobLogIdentity {
    operation_id: Option<String>,
    job_id: String,
    input_index: Option<usize>,
    operation_kind: OperationKind,
}

pub(crate) struct ProcessingJobLifecycleLog {
    identity: ProcessingJobLogIdentity,
    started_at: Instant,
}

impl ProcessingJobLifecycleLog {
    fn start(identity: ProcessingJobLogIdentity) -> Self {
        log::info!(
            "{}",
            format_processing_job_record(
                &identity,
                ProcessingJobLogEvent::Started,
                ProcessingJobLogStatus::Running,
                None,
            )
        );
        Self {
            identity,
            started_at: Instant::now(),
        }
    }

    fn log_terminal(&self, status: ProcessingJobLogStatus<'_>) {
        let record = format_processing_job_record(
            &self.identity,
            ProcessingJobLogEvent::Terminal,
            status,
            Some(self.started_at.elapsed().as_millis()),
        );
        match status {
            ProcessingJobLogStatus::Running | ProcessingJobLogStatus::Success => {
                log::info!("{record}");
            }
            ProcessingJobLogStatus::Cancelled => log::warn!("{record}"),
            ProcessingJobLogStatus::Failed(_) => log::error!("{record}"),
        }
    }
}

#[derive(Clone, Copy)]
enum ProcessingJobLogEvent {
    Started,
    Terminal,
}

#[derive(Clone, Copy)]
enum ProcessingJobLogStatus<'a> {
    Running,
    Success,
    Cancelled,
    Failed(&'a AppErrorEnvelope),
}

fn format_processing_job_record(
    identity: &ProcessingJobLogIdentity,
    event: ProcessingJobLogEvent,
    status: ProcessingJobLogStatus<'_>,
    elapsed_ms: Option<u128>,
) -> String {
    let operation_id = identity.operation_id.as_deref().unwrap_or("foreground");
    let input_index = identity
        .input_index
        .map_or_else(|| "none".to_string(), |index| index.to_string());
    let mut record = format!(
        "processing_job event={} operation_id={} job_id={} input_index={} kind={} status={}",
        processing_job_event_label(event),
        operation_id,
        identity.job_id,
        input_index,
        crate::processing::operation_kind_log_label(identity.operation_kind),
        processing_job_status_label(status),
    );
    if let Some(elapsed_ms) = elapsed_ms {
        record.push_str(&format!(" elapsed_ms={elapsed_ms}"));
    }
    if let ProcessingJobLogStatus::Failed(failure) = status {
        record.push_str(&format!(
            " code={} category={}",
            failure.code.log_label(),
            failure.category.log_label(),
        ));
    }
    record
}

fn processing_job_event_label(event: ProcessingJobLogEvent) -> &'static str {
    match event {
        ProcessingJobLogEvent::Started => "started",
        ProcessingJobLogEvent::Terminal => "terminal",
    }
}

fn processing_job_status_label(status: ProcessingJobLogStatus<'_>) -> &'static str {
    match status {
        ProcessingJobLogStatus::Running => "running",
        ProcessingJobLogStatus::Success => "success",
        ProcessingJobLogStatus::Cancelled => "cancelled",
        ProcessingJobLogStatus::Failed(_) => "failed",
    }
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
    use super::*;
    use crate::errors::{AppErrorCategory, AppErrorCode};

    fn log_identity(
        operation_id: Option<&str>,
        job_id: &str,
        input_index: Option<usize>,
        operation_kind: OperationKind,
    ) -> ProcessingJobLogIdentity {
        ProcessingJobLogIdentity {
            operation_id: operation_id.map(str::to_string),
            job_id: job_id.to_string(),
            input_index,
            operation_kind,
        }
    }

    #[test]
    fn processing_job_record_format_pins_started_and_terminal_contracts() {
        assert_eq!(
            format_processing_job_record(
                &log_identity(None, "job-123", None, OperationKind::ProcessingMerge),
                ProcessingJobLogEvent::Started,
                ProcessingJobLogStatus::Running,
                None,
            ),
            "processing_job event=started operation_id=foreground job_id=job-123 input_index=none kind=processing_merge status=running"
        );

        assert_eq!(
            format_processing_job_record(
                &log_identity(
                    Some("operation-123"),
                    "job-456",
                    Some(4),
                    OperationKind::ProcessingBatch,
                ),
                ProcessingJobLogEvent::Terminal,
                ProcessingJobLogStatus::Success,
                Some(321),
            ),
            "processing_job event=terminal operation_id=operation-123 job_id=job-456 input_index=4 kind=processing_batch status=success elapsed_ms=321"
        );

        assert_eq!(
            format_processing_job_record(
                &log_identity(
                    Some("operation-123"),
                    "job-789",
                    Some(5),
                    OperationKind::ProcessingBatch,
                ),
                ProcessingJobLogEvent::Terminal,
                ProcessingJobLogStatus::Cancelled,
                Some(654),
            ),
            "processing_job event=terminal operation_id=operation-123 job_id=job-789 input_index=5 kind=processing_batch status=cancelled elapsed_ms=654"
        );
    }

    #[test]
    fn failed_processing_job_record_includes_stable_typed_error_fields() {
        let failure = AppErrorEnvelope::new(
            AppErrorCode::FfmpegError,
            AppErrorCategory::Toolchain,
            "path-free failure",
            None,
        );

        let record = format_processing_job_record(
            &log_identity(
                Some("operation-123"),
                "job-456",
                Some(2),
                OperationKind::ProcessingBatch,
            ),
            ProcessingJobLogEvent::Terminal,
            ProcessingJobLogStatus::Failed(&failure),
            Some(987),
        );

        assert_eq!(
            record,
            "processing_job event=terminal operation_id=operation-123 job_id=job-456 input_index=2 kind=processing_batch status=failed elapsed_ms=987 code=ffmpeg_error category=toolchain"
        );
        assert!(!record.contains(&failure.message));
    }
}
