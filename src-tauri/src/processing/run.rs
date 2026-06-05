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
use crate::output_artifact::{
    commit_supplemental_output_asset, OutputKind, ResolvedOutputPlan,
    SupplementalOutputAssetCommitRequest,
};
use crate::processing::job_registry::{CancellationChecker, JobId};
use crate::processing::{
    emit_queue_event, OperationKind, OutputConfig, PreviewConfig, ProcessingContext,
    ProcessingSession, QueueEvent, QueueItem,
};
use crate::processing::{
    JobType, ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessingPreflightPlan, SupplementalProcessingAsset,
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
        supplemental_assets: Vec::new(),
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
        let supplemental_assets = supplemental_assets_for_input(payload, input_index);

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
                supplemental_assets,
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
    supplemental_assets: Vec<SupplementalProcessingAsset>,
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

fn supplemental_assets_for_input(
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

/// TOCTOU guard for a staged Supplemental PDF: confirms the on-disk file still
/// matches the size and content hash recorded at acquisition time. Structural
/// PDF policy (regular-file, size limit, magic bytes) is the commit boundary's
/// job in [`commit_supplemental_output_asset`]; this check owns only identity
/// drift between staging and commit.
fn validate_supplemental_asset(asset: &SupplementalProcessingAsset) -> Result<()> {
    let metadata = std::fs::symlink_metadata(&asset.path).map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot inspect Supplemental PDF source '{}': {}",
            crate::errors::sanitize_path_for_display(&asset.path),
            error
        ))
    })?;
    if metadata.len() != asset.size_bytes {
        return Err(AppError::FileValidation(
            "Supplemental PDF source size changed before output commit.".to_string(),
        ));
    }

    let bytes = std::fs::read(&asset.path).map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot read Supplemental PDF source '{}': {}",
            crate::errors::sanitize_path_for_display(&asset.path),
            error
        ))
    })?;
    if abb_media_core::sha256_hex(&bytes) != asset.sha256 {
        return Err(AppError::FileValidation(
            "Supplemental PDF source hash changed before output commit.".to_string(),
        ));
    }

    Ok(())
}

fn commit_supplemental_assets(
    assets: &[SupplementalProcessingAsset],
    final_audio_path: &Path,
) -> Result<()> {
    for asset in assets {
        validate_supplemental_asset(asset)?;
        commit_supplemental_output_asset(SupplementalOutputAssetCommitRequest::new(
            &asset.path,
            final_audio_path,
        ))?;
    }
    Ok(())
}

fn commit_supplemental_assets_for_output(
    output_kind: OutputKind,
    assets: &[SupplementalProcessingAsset],
    output_path: &Path,
) -> Result<()> {
    if output_kind != OutputKind::Final {
        return Ok(());
    }
    commit_supplemental_assets(assets, output_path)
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
    use super::*;
    use crate::audio::{BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting};
    use crate::processing::{JobType, ProcessPayload};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn encoder_settings() -> EncoderSettings {
        EncoderSettings {
            encoder_type: EncoderType::Auto,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: ChannelConfig::Auto,
            afterburner: true,
            threads: ThreadSetting::Auto,
            twoloop: true,
        }
    }

    fn process_payload(overrides: impl FnOnce(&mut ProcessPayload)) -> ProcessPayload {
        let mut payload = ProcessPayload {
            input_files: vec!["/books/input.m4b".to_string()],
            input_ids: None,
            output_dir: "/tmp/out".to_string(),
            settings: encoder_settings(),
            sample_rate: None,
            job_type: Some(JobType::Batch),
            output_naming: None,
            collision_policy: None,
            preflight_signature: None,
            supplemental_assets_by_input_id: None,
        };
        overrides(&mut payload);
        payload
    }

    fn supplemental_asset(
        path: std::path::PathBuf,
        input_id: &str,
        bytes: &[u8],
    ) -> SupplementalProcessingAsset {
        SupplementalProcessingAsset {
            asset_id: "asset-1".to_string(),
            input_id: input_id.to_string(),
            title_id: "B000000001".to_string(),
            path,
            file_name: "Supplemental PDF.pdf".to_string(),
            size_bytes: bytes.len() as u64,
            sha256: abb_media_core::sha256_hex(bytes),
        }
    }

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

    #[test]
    fn supplemental_assets_for_input_selects_by_file_list_input_id() {
        let asset = SupplementalProcessingAsset {
            asset_id: "asset-1".to_string(),
            input_id: "current-input-2".to_string(),
            title_id: "B000000001".to_string(),
            path: "/staged/book.pdf".into(),
            file_name: "Supplemental PDF.pdf".to_string(),
            size_bytes: 128,
            sha256: "hash".to_string(),
        };
        let mut assets = HashMap::new();
        assets.insert("current-input-2".to_string(), vec![asset.clone()]);
        let payload = process_payload(|payload| {
            payload.input_files = vec![
                "/books/first.m4b".to_string(),
                "/books/second.m4b".to_string(),
            ];
            payload.input_ids = Some(vec![
                Some("current-input-1".to_string()),
                Some("current-input-2".to_string()),
            ]);
            payload.supplemental_assets_by_input_id = Some(assets);
        });

        assert!(supplemental_assets_for_input(&payload, None).is_empty());
        assert!(supplemental_assets_for_input(&payload, Some(0)).is_empty());
        assert_eq!(
            supplemental_assets_for_input(&payload, Some(1)),
            vec![asset]
        );
    }

    #[test]
    fn supplemental_asset_commit_runs_only_for_final_outputs() {
        let root = TempDir::new().expect("temp root");
        let pdf_bytes = b"%PDF-1.7\nbody";
        let source = root.path().join("source.pdf");
        std::fs::write(&source, pdf_bytes).expect("write source pdf");
        let asset = supplemental_asset(source, "current-input-1", pdf_bytes);
        let preview_audio = root.path().join("Preview.m4b");
        let final_audio = root.path().join("Book.m4b");

        commit_supplemental_assets_for_output(
            OutputKind::Preview,
            std::slice::from_ref(&asset),
            &preview_audio,
        )
        .expect("preview should not commit");
        assert!(
            !root.path().join("Preview - Supplemental PDF.pdf").exists(),
            "preview output must not commit acquired PDFs"
        );

        commit_supplemental_assets_for_output(OutputKind::Final, &[asset], &final_audio)
            .expect("final should commit");
        assert_eq!(
            std::fs::read(root.path().join("Book - Supplemental PDF.pdf")).expect("read commit"),
            pdf_bytes
        );
    }

    #[test]
    fn stale_supplemental_assets_fail_instead_of_silently_dropping_pdf() {
        let root = TempDir::new().expect("temp root");
        let pdf_bytes = b"%PDF-1.7\nbody";
        let source = root.path().join("source.pdf");
        std::fs::write(&source, pdf_bytes).expect("write source pdf");
        let mut asset = supplemental_asset(source, "current-input-1", pdf_bytes);
        asset.sha256 = "stale-hash".to_string();
        let final_audio = root.path().join("Book.m4b");

        let error =
            commit_supplemental_assets_for_output(OutputKind::Final, &[asset], &final_audio)
                .expect_err("stale supplemental asset should fail");

        assert!(
            error.to_string().contains("hash changed"),
            "unexpected error: {error}"
        );
        assert!(
            !root.path().join("Book - Supplemental PDF.pdf").exists(),
            "bad supplemental asset must not be silently dropped or committed"
        );
    }
}
