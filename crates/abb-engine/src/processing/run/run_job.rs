use crate::audio;
use crate::audio::{AudioExecutionRequest, FileListInfo};
use crate::errors::{AppError, AppErrorEnvelope, Result};
use crate::execution::{CancellationChecker, PublishGate};
use crate::host::Host;
use crate::metadata::CoverArtPassthroughPolicy;
use crate::output_artifact::{
    commit_supplemental_output_assets_for_output, OutputKind, ResolvedOutputPlan,
    SupplementalOutputAssetsCommitRequest,
};
use crate::processing::job_registry::{JobId, RegisteredJob};
use crate::processing::ProgressEventListener;
use crate::processing::{
    AudioHandling, JobIdentity, OperationKind, OutputConfig, ProcessPayload, ProcessResultEntry,
    ProcessResultStatus, ProcessingContext, ProcessingSession, SupplementalProcessingAsset,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::OwnedSemaphorePermit;
use tokio_util::sync::CancellationToken;

use crate::processing::terminal_outcomes::{
    classify_processing_error, terminal_failure_result, ProcessingJobTerminalOutcome,
};

pub(crate) struct ProcessingJobRequest {
    pub(crate) host: Host,
    pub(crate) registry: crate::ManagedJobRegistry,
    pub(crate) workspace_root: PathBuf,
    pub(crate) audio_handling: AudioHandling,
    pub(crate) audio_request: audio::TitleAudioRequest,
    pub(crate) audio_reason: Option<String>,
    pub(crate) metadata_intent: Option<crate::metadata::MetadataIntentPatch>,
    pub(crate) sample_rate: u32,
    pub(crate) aac_decoder: audio::AacDecoder,
    pub(crate) identity: JobIdentity,
    pub(crate) operation_kind: OperationKind,
    pub(crate) title_cancel: Option<CancellationToken>,
    pub(crate) output_plan: ResolvedOutputPlan,
    pub(crate) file_info: FileListInfo,
    pub(crate) metadata: Option<crate::metadata::AudiobookMetadata>,
    pub(crate) cover_art_passthrough: CoverArtPassthroughPolicy,
    pub(crate) supplemental_assets: Vec<SupplementalProcessingAsset>,
    pub(crate) progress_listener: Option<ProgressEventListener>,
    pub(crate) title_output: Option<Arc<crate::processing::TitleOutput>>,
}

pub(crate) async fn run_processing_job(
    request: ProcessingJobRequest,
) -> Result<ProcessResultEntry> {
    let RegisteredProcessingJob {
        job,
        identity,
        permit: _permit,
        cancellation_checker,
        lifecycle_log,
    } = register_job_and_validate_output(
        &request.registry,
        &request.output_plan.resolved_path,
        request.audio_request.format(),
        request.title_cancel.clone(),
        request.identity.clone(),
        request.operation_kind,
    )
    .await?;
    let job_id = job.id();
    log_audio_decision(&request, job_id);
    let _active_work = request.host.begin_active_work();

    let context = build_processing_context(ProcessingContextRequest {
        cancellation_checker,
        job_id,
        identity,
        sample_rate: request.sample_rate,
        aac_decoder: request.aac_decoder,
        operation_kind: request.operation_kind,
        progress_listener: request.progress_listener,
        title_output: request.title_output.clone(),
        output_plan: request.output_plan.clone(),
        workspace_root: request.workspace_root,
    });
    context
        .new_emitter()
        .emit_analyzing_start("Preparing audio job...");
    let output_path = Some(request.output_plan.resolved_path.display().to_string());
    let audio_result = audio::execute_audio_engine(
        AudioExecutionRequest::new(
            context,
            request.file_info,
            request.metadata,
            request.cover_art_passthrough,
            request.audio_handling,
        )
        .with_metadata_intent(request.metadata_intent),
    )
    .await;
    let result = title_outcome(audio_result, output_path, async move || {
        commit_supplemental_assets(
            request.output_plan.kind,
            request.supplemental_assets,
            request.output_plan.resolved_path,
        )
        .await
    })
    .await;

    settle_job(job, request.identity.input_index, &lifecycle_log, result).await
}

async fn settle_job(
    job: RegisteredJob,
    input_index: usize,
    lifecycle_log: &ProcessingJobLifecycleLog,
    result: ProcessingJobTerminalOutcome,
) -> Result<ProcessResultEntry> {
    let job_id = job.id();
    match result {
        ProcessingJobTerminalOutcome::Success {
            message,
            output_path,
            supplemental_warning,
            warnings,
        } => {
            job.complete();
            lifecycle_log.log_terminal(ProcessingJobLogStatus::Success);
            Ok(ProcessResultEntry {
                input_index,
                status: ProcessResultStatus::Success,
                message,
                error: None,
                output_path,
                job_id: Some(job_id.to_string()),
                supplemental_warning,
                warnings,
            })
        }
        ProcessingJobTerminalOutcome::Cancelled(error) => {
            job.complete();
            lifecycle_log.log_terminal(ProcessingJobLogStatus::Cancelled);
            Err(error)
        }
        ProcessingJobTerminalOutcome::Failed(envelope) => {
            job.fail(envelope.message.clone());
            lifecycle_log.log_terminal(ProcessingJobLogStatus::Failed(&envelope));
            Ok(terminal_failure_result(
                input_index,
                Some(job_id.to_string()),
                envelope,
            ))
        }
    }
}

fn log_audio_decision(request: &ProcessingJobRequest, job_id: JobId) {
    let reason = request
        .audio_reason
        .as_deref()
        .unwrap_or(match request.audio_handling {
            AudioHandling::Preserve => "Source audio can be copied into the selected output.",
            AudioHandling::Encode { .. } if request.output_plan.kind == OutputKind::Preview => {
                "Preview requires encoding."
            }
            AudioHandling::Encode { .. } => "Encoding settings were explicitly selected.",
        });
    log::info!(
        "audio_decision operation_id={} job_id={} input_index={:?} request={:?} action={:?} resolved_rate={:?} reason={:?} output_path={:?}",
        request.identity.operation_id.as_deref().unwrap_or("foreground"), job_id,
        request.identity.input_index, request.audio_request, request.audio_handling,
        request.sample_rate, reason, request.output_plan.resolved_path
    );
    log::info!(
        "metadata_intent job_id={} input_index={} intent={}",
        job_id,
        request.identity.input_index,
        describe_requested_fields(request.metadata_intent.as_ref())
    );
    for (index, source) in request.file_info.files.iter().enumerate() {
        log::info!(
            "audio_source job_id={} source_index={} source_path={:?}",
            job_id,
            index,
            source.path
        );
    }
}

fn describe_requested_fields(patch: Option<&crate::metadata::MetadataIntentPatch>) -> String {
    let described = patch
        .map(crate::metadata::MetadataIntentPatch::requested_ops)
        .unwrap_or_default()
        .iter()
        .map(|(field, op)| format!("{field}={op}"))
        .collect::<Vec<_>>();
    if described.is_empty() {
        "none".to_string()
    } else {
        described.join(",")
    }
}

pub(crate) fn supplemental_assets_for_input(
    payload: &ProcessPayload,
    index: usize,
) -> Vec<SupplementalProcessingAsset> {
    payload
        .sources_for(index)
        .into_iter()
        .filter_map(|source| source.input_id)
        .flat_map(|input_id| {
            payload
                .supplemental_assets_by_input_id
                .as_ref()
                .and_then(|assets| assets.get(&input_id))
                .cloned()
                .unwrap_or_default()
        })
        .collect()
}

#[expect(
    clippy::disallowed_methods,
    reason = "joined by commit_supplemental_assets"
)]
pub(crate) async fn commit_supplemental_assets(
    output_kind: OutputKind,
    assets: Vec<SupplementalProcessingAsset>,
    output_path: PathBuf,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let request = assets.iter().fold(
            SupplementalOutputAssetsCommitRequest::new(output_kind, &output_path),
            |request, asset| request.with_asset(&asset.path, asset.size_bytes, &asset.sha256),
        );
        commit_supplemental_output_assets_for_output(request)
    })
    .await
    .map_err(|error| AppError::General(format!("Companion publish stopped: {error}")))?
}

pub(crate) async fn title_outcome(
    audio: Result<crate::audio::AudioRunOutcome>,
    output_path: Option<String>,
    publish_companions: impl AsyncFnOnce() -> Result<()>,
) -> ProcessingJobTerminalOutcome {
    let audio = match audio {
        Ok(audio) => audio,
        Err(error) => return classify_processing_error(error),
    };
    let supplemental_warning = publish_companions().await.err().map(|error| {
        log::warn!("supplemental_pdf_commit status=failed title_status=success");
        AppErrorEnvelope::from(&error).message
    });
    ProcessingJobTerminalOutcome::Success {
        message: audio.message,
        output_path,
        supplemental_warning,
        warnings: audio.warnings,
    }
}

pub(crate) struct RegisteredProcessingJob {
    pub(crate) job: RegisteredJob,
    pub(crate) identity: JobIdentity,
    pub(crate) permit: OwnedSemaphorePermit,
    pub(crate) cancellation_checker: CancellationChecker,
    pub(crate) lifecycle_log: ProcessingJobLifecycleLog,
}

#[expect(
    clippy::disallowed_methods,
    reason = "joined by register_job_and_validate_output"
)]
pub(crate) async fn register_job_and_validate_output(
    registry: &crate::ManagedJobRegistry,
    output_path: &Path,
    format: audio::AudiobookFormat,
    title_cancel: Option<CancellationToken>,
    identity: JobIdentity,
    operation_kind: OperationKind,
) -> Result<RegisteredProcessingJob> {
    let (job_id, permit) = registry
        .register_job_with_external_cancel(title_cancel.clone())
        .await?;
    let job = RegisteredJob::adopt(registry, job_id);
    let cancellation_checker = CancellationChecker::new(title_cancel);
    let identity = identity.registered(job_id);
    let lifecycle_log = ProcessingJobLifecycleLog::start(identity.clone(), operation_kind);

    let checked_path = output_path.to_path_buf();
    let validation = tokio::task::spawn_blocking(move || {
        crate::audio::validate_output_target(&checked_path, format)
    })
    .await
    .map_err(|error| AppError::General(format!("Output check stopped: {error}")))
    .and_then(|validation| validation);
    if let Err(error) = validation {
        let envelope = AppErrorEnvelope::from(&error);
        lifecycle_log.log_terminal(ProcessingJobLogStatus::Failed(&envelope));
        job.fail(error.to_string());
        return Err(error);
    }

    Ok(RegisteredProcessingJob {
        job,
        identity,
        permit,
        cancellation_checker,
        lifecycle_log,
    })
}

struct ProcessingContextRequest {
    cancellation_checker: CancellationChecker,
    job_id: JobId,
    identity: JobIdentity,
    sample_rate: u32,
    aac_decoder: audio::AacDecoder,
    operation_kind: OperationKind,
    progress_listener: Option<ProgressEventListener>,
    title_output: Option<Arc<crate::processing::TitleOutput>>,
    output_plan: ResolvedOutputPlan,
    workspace_root: PathBuf,
}

fn build_processing_context(request: ProcessingContextRequest) -> ProcessingContext {
    let session =
        ProcessingSession::with_cancellation(request.job_id.0, request.cancellation_checker);
    let context = ProcessingContext {
        session: Arc::new(session),
        sample_rate: request.sample_rate,
        aac_decoder: request.aac_decoder,
        output: OutputConfig::from_plan(request.output_plan),
        workspace_root: request.workspace_root,
        identity: request.identity,
        operation_kind: request.operation_kind,
        progress_listener: request.progress_listener,
        publish_gate: request
            .title_output
            .map(|output| output as Arc<dyn PublishGate>),
    };

    if context.is_preview() {
        log::info!(
            "Preview requested: total_seconds={:.3}",
            crate::processing::PREVIEW_SECONDS
        );
    }

    context
}

pub(crate) struct ProcessingJobLifecycleLog {
    identity: JobIdentity,
    operation_kind: OperationKind,
    started_at: Instant,
}

#[cfg(test)]
#[path = "run_job_registration_tests.rs"]
mod registration_tests;

impl ProcessingJobLifecycleLog {
    fn start(identity: JobIdentity, operation_kind: OperationKind) -> Self {
        log::info!(
            "{}",
            format_processing_job_record(
                &identity,
                operation_kind,
                ProcessingJobLogEvent::Started,
                ProcessingJobLogStatus::Running,
                None,
            )
        );
        Self {
            identity,
            operation_kind,
            started_at: Instant::now(),
        }
    }

    fn log_terminal(&self, status: ProcessingJobLogStatus<'_>) {
        let record = format_processing_job_record(
            &self.identity,
            self.operation_kind,
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
    identity: &JobIdentity,
    operation_kind: OperationKind,
    event: ProcessingJobLogEvent,
    status: ProcessingJobLogStatus<'_>,
    elapsed_ms: Option<u128>,
) -> String {
    let operation_id = identity.operation_id.as_deref().unwrap_or("foreground");
    let job_id = identity.job_id.as_deref().unwrap_or("none");
    let mut record = format!(
        "processing_job event={} operation_id={} job_id={} input_index={} kind={} status={}",
        processing_job_event_label(event),
        operation_id,
        job_id,
        identity.input_index,
        operation_kind.log_label(),
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

#[cfg(test)]
#[path = "run_job_tests.rs"]
mod tests;
