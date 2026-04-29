use super::plan::{
    build_processing_plan, enforce_plan_review, ensure_output_parent_dirs, log_output_plan,
    resolve_output_dir, ProcessingInputs, ResolvedProcessingPlan,
};
use super::terminal_outcomes::{
    build_all_skipped_batch_result, classify_processing_error, collect_batch_results,
    skipped_result, terminal_failure_result, ProcessingJobTerminalOutcome,
};
use crate::audio;
use crate::audio::file_list::FileListInfo;
use crate::audio::job_registry::{CancellationChecker, JobId};
use crate::audio::output_path::{OutputKind, PlannedOutputAction, ResolvedOutputPlan};
use crate::audio::settings_encoder::validate_encoder_settings;
use crate::audio::toolchain::ExternalToolchainPreference;
use crate::audio::{QueueEvent, QueueItem};
use crate::commands::audio_types::{
    JobType, ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessingPreflightPlan,
};
use crate::errors::{AppError, Result};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tauri::Emitter;
use tokio::sync::OwnedSemaphorePermit;

pub(crate) struct ProcessingRun;

pub async fn process_payload(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    payload: ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    ProcessingRun::execute(window, registry, payload, metadata, preview_seconds).await
}

pub fn preflight_payload(
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

        let inputs = ProcessingInputs {
            output_naming: payload.output_naming.clone().unwrap_or_default(),
            base_output_dir: resolve_output_dir(&payload.output_dir, true)?,
            preview_seconds: resolve_preview_seconds(preview_seconds),
        };
        let plan = build_processing_plan(&payload, metadata.as_ref(), &inputs)?;
        log_output_plan("process", &payload, &plan);
        enforce_plan_review(&payload, &plan)?;
        ensure_output_parent_dirs(&plan)?;

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

        let inputs = ProcessingInputs {
            output_naming: payload.output_naming.clone().unwrap_or_default(),
            base_output_dir: resolve_output_dir(&payload.output_dir, false)?,
            preview_seconds: resolve_preview_seconds(preview_seconds),
        };
        let plan = build_processing_plan(&payload, metadata.as_ref(), &inputs)?;
        log_output_plan("preflight", &payload, &plan);
        Ok(plan.to_public())
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
    audio::settings::validate_sample_rate_config(&sample_rate)?;
    Ok(sample_rate)
}

pub(crate) fn resolve_effective_processing_metadata(
    input_path: Option<&std::path::Path>,
    patch: Option<&crate::metadata::MetadataIntentPatch>,
) -> Result<Option<crate::metadata::AudiobookMetadata>> {
    match (input_path, patch) {
        (Some(path), Some(patch)) => {
            let source_metadata = crate::metadata::read_metadata(path)?;
            Ok(Some(patch.apply_to_metadata(source_metadata)?))
        }
        (Some(path), None) => Ok(Some(crate::metadata::read_metadata(path)?)),
        (None, Some(patch)) => Ok(Some(patch.to_processing_overlay()?)),
        (None, None) => Ok(None),
    }
}

pub(crate) fn resolve_naming_metadata(
    resolved_metadata: Option<&crate::metadata::AudiobookMetadata>,
    input_path: Option<&std::path::Path>,
    patch: Option<&crate::metadata::MetadataIntentPatch>,
) -> Option<crate::metadata::AudiobookMetadata> {
    let mut naming_metadata = resolved_metadata.cloned()?;

    if input_path.is_some() && patch.is_none() {
        scrub_legacy_source_series_parts_for_naming(&mut naming_metadata);
    }

    Some(naming_metadata)
}

fn scrub_legacy_source_series_parts_for_naming(metadata: &mut crate::metadata::AudiobookMetadata) {
    scrub_invalid_series_part_for_naming(&mut metadata.series_part);
    scrub_invalid_series_part_for_naming(&mut metadata.subseries_part);
}

fn scrub_invalid_series_part_for_naming(value: &mut Option<String>) {
    let should_clear = value
        .as_deref()
        .map(str::trim)
        .filter(|trimmed| !trimmed.is_empty())
        .is_some_and(|trimmed| crate::metadata::validate_series_part(trimmed).is_err());

    if should_clear {
        *value = None;
    }
}

pub(crate) fn validate_batch_input_path(path: &std::path::Path) -> Result<PathBuf> {
    crate::audio::path_validation::validate_input_audio_path(path)
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
    let adapter = audio::processor::resolve_processor_adapter(
        &payload.settings,
        payload.external_toolchain.as_ref(),
    )?;
    adapter.validate_inputs(file_info)?;
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
    let queue_event = QueueEvent {
        items: queue_items,
        max_concurrent: registry.max_concurrent(),
    };
    let _ = window.emit(crate::audio::constants::QUEUE_EVENT_NAME, &queue_event);
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
    let paths: Vec<PathBuf> = payload.input_files.iter().map(PathBuf::from).collect();
    let file_info = audio::get_file_list_info(&paths)?;
    let planned_job = plan.jobs.into_iter().next().ok_or_else(|| {
        AppError::InvalidInput("No output plan entries were built for merge processing".to_string())
    })?;

    if planned_job.output.action == PlannedOutputAction::SkipExisting {
        return Ok(ProcessCommandResult::new(
            JobType::Merge,
            vec![skipped_result(None, None, &planned_job.output)],
        ));
    }

    let result = run_processing_job(ProcessingJobRequest {
        window,
        registry,
        encoder_settings: payload.settings.clone(),
        external_toolchain: payload.external_toolchain.clone(),
        sample_rate: resolve_sample_rate(payload)?,
        input_index: None,
        output_plan: planned_job.output,
        file_info,
        metadata: planned_job.metadata,
        allow_passthrough_cover_art: planned_job.allow_passthrough_cover_art,
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
        if planned_job.output.action == PlannedOutputAction::SkipExisting {
            let input_index = planned_job.input_index;
            let output = planned_job.output.clone();
            let skipped_entry = skipped_result(input_index, None, &output);
            emit_terminal_skipped_event(
                &window,
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
        let external_toolchain_cloned = payload.external_toolchain.clone();
        let sr_cloned = sample_rate.clone();
        let md_cloned = planned_job.metadata.clone();
        let allow_passthrough_cover_art = planned_job.allow_passthrough_cover_art;
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
                external_toolchain: external_toolchain_cloned,
                sample_rate: sr_cloned,
                input_index,
                output_plan: output,
                file_info,
                metadata: md_cloned,
                allow_passthrough_cover_art,
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
    encoder_settings: audio::settings_encoder::EncoderSettings,
    external_toolchain: Option<ExternalToolchainPreference>,
    sample_rate: audio::SampleRateConfig,
    input_index: Option<usize>,
    output_plan: ResolvedOutputPlan,
    file_info: FileListInfo,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    allow_passthrough_cover_art: bool,
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
        output_plan: request.output_plan.clone(),
        preview_seconds: request.preview_seconds,
    });
    let preview_path = (request.output_plan.kind == OutputKind::Preview)
        .then(|| request.output_plan.resolved_path.display().to_string());
    let result = match execute_processing_job(
        context,
        request.file_info,
        request.metadata,
        request.allow_passthrough_cover_art,
        request.encoder_settings,
        request.external_toolchain,
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

    if let Err(error) = audio::settings::validate_output_path(output_path) {
        registry.fail_job(job_id, error.to_string()).await;
        return Err(error);
    }

    Ok((job_id, permit, cancellation_checker))
}

struct ProcessingContextRequest {
    window: tauri::Window,
    cancellation_checker: crate::audio::job_registry::CancellationChecker,
    job_id: crate::audio::job_registry::JobId,
    encoder_settings: audio::settings_encoder::EncoderSettings,
    sample_rate: audio::SampleRateConfig,
    input_index: Option<usize>,
    output_plan: ResolvedOutputPlan,
    preview_seconds: Option<f64>,
}

fn build_processing_context(
    request: ProcessingContextRequest,
) -> (audio::ProcessingContext, Option<f64>) {
    let session = audio::session::ProcessingSession::from_job_registry(
        request.job_id.0,
        request.cancellation_checker,
    );
    let mut context = audio::ProcessingContext::new(
        request.window,
        std::sync::Arc::new(session),
        request.encoder_settings,
        request.sample_rate,
        audio::OutputConfig::from_plan(request.output_plan),
    );
    context.job_id = Some(request.job_id.to_string());
    context.input_index = request.input_index;

    let preview_seconds_resolved = resolve_preview_seconds(request.preview_seconds);
    if let Some(seconds) = preview_seconds_resolved {
        context.preview = Some(crate::audio::context::PreviewConfig::new(seconds));
        log::info!("Preview requested: total_seconds={:.3}", seconds);
    }

    (context, preview_seconds_resolved)
}

fn resolve_preview_seconds(preview_seconds: Option<f64>) -> Option<f64> {
    let resolved = preview_seconds?;

    (resolved.is_finite() && resolved > 0.0).then_some(resolved)
}

async fn execute_processing_job(
    context: audio::ProcessingContext,
    file_info: FileListInfo,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    allow_passthrough_cover_art: bool,
    encoder_settings: audio::settings_encoder::EncoderSettings,
    external_toolchain: Option<ExternalToolchainPreference>,
) -> Result<String> {
    let FileListInfo {
        files,
        selected_decoders,
        ..
    } = file_info;
    let adapter = audio::processor::resolve_processor_adapter(
        &encoder_settings,
        external_toolchain.as_ref(),
    )?;
    adapter
        .execute(
            context,
            files,
            selected_decoders,
            metadata,
            allow_passthrough_cover_art,
        )
        .await
}

fn emit_terminal_failed_event(
    window: &tauri::Window,
    input_index: Option<usize>,
    job_id: Option<&str>,
    message: &str,
) {
    let emitter = audio::ProgressEmitter::with_context(
        window.clone(),
        job_id.map(|value| value.to_string()),
        input_index,
    );
    emitter.emit_terminal_failed(message);
}

fn emit_terminal_skipped_event(
    window: &tauri::Window,
    input_index: Option<usize>,
    job_id: Option<&str>,
    message: &str,
) {
    let emitter = audio::ProgressEmitter::with_context(
        window.clone(),
        job_id.map(|value| value.to_string()),
        input_index,
    );
    emitter.emit_terminal_skipped(message);
}

#[cfg(test)]
mod tests {
    use super::{
        preflight_payload, resolve_effective_processing_metadata, resolve_naming_metadata,
        validate_batch_input_path, validate_external_processing_contract_with_file_info,
    };
    use crate::audio::file_list::FileListInfo;
    use crate::audio::output_path::{build_output_path, CollisionPolicy, OutputCollisionKind};
    use crate::audio::settings_encoder::{
        BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
    };
    use crate::audio::{AudioFile, DecoderSelection, ExternalToolchainPreference};
    use crate::commands::audio_types::{JobType, OutputNamingConfig, ProcessPayload};
    use crate::metadata::{MetadataIntentPatch, PatchOp};
    use std::collections::HashMap;
    use std::fs::{self, set_permissions, write};
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::TempDir;

    fn sample_source_metadata() -> crate::metadata::AudiobookMetadata {
        crate::metadata::AudiobookMetadata {
            title: Some("A Change of Plans".to_string()),
            artist: Some("Dennis E. Taylor".to_string()),
            album: Some("A Change of Plans".to_string()),
            series: Some("Checking".to_string()),
            ..crate::metadata::AudiobookMetadata::new()
        }
    }

    #[test]
    fn resolve_effective_processing_metadata_no_patch_reads_source_or_errors() {
        let missing_source = Path::new("/path/that/does/not/exist/input.m4b");
        let outcome = resolve_effective_processing_metadata(Some(missing_source), None);
        assert!(
            outcome.is_err(),
            "missing input should fail read, not return empty metadata"
        );
    }

    #[test]
    fn resolve_effective_processing_metadata_partial_set_and_clear_patch() {
        let patch = MetadataIntentPatch {
            series: PatchOp::Set("once again".to_string()),
            artist: PatchOp::Clear,
            ..Default::default()
        };

        let merged = patch
            .apply_to_metadata(sample_source_metadata())
            .expect("patch applies");

        assert_eq!(merged.artist, None);
        assert_eq!(merged.title.as_deref(), Some("A Change of Plans"));
        assert_eq!(merged.series.as_deref(), Some("once again"));
    }

    #[test]
    fn resolve_effective_processing_metadata_uses_overlay_without_source_file() {
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Overlay Only".to_string()),
            ..Default::default()
        };

        let resolved =
            resolve_effective_processing_metadata(None, Some(&patch)).expect("metadata resolves");

        assert_eq!(
            resolved.and_then(|value| value.title),
            Some("Overlay Only".to_string())
        );
    }

    #[test]
    fn resolve_effective_processing_metadata_rejects_invalid_patch_values() {
        let patch = MetadataIntentPatch {
            date: PatchOp::Set("2024-99".to_string()),
            ..Default::default()
        };

        let err = resolve_effective_processing_metadata(None, Some(&patch))
            .expect_err("invalid patch should fail");
        assert!(
            err.to_string().contains("Publication date"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolved_metadata_drives_naming_and_encoding_metadata_coherently() {
        let base = sample_source_metadata();
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Renamed Title".to_string()),
            ..Default::default()
        };
        let effective = patch
            .apply_to_metadata(base)
            .expect("metadata should resolve");
        let output_path = build_output_path(
            Path::new("/tmp"),
            Some(&effective),
            OutputNamingConfig::default(),
            None,
        )
        .expect("output path should build");

        assert!(
            output_path.to_string_lossy().contains("Renamed Title"),
            "output naming should use resolved metadata"
        );
        assert_eq!(effective.title.as_deref(), Some("Renamed Title"));
    }

    #[test]
    fn resolve_preview_seconds_only_uses_explicit_request() {
        assert_eq!(super::resolve_preview_seconds(None), None);
        assert_eq!(super::resolve_preview_seconds(Some(30.0)), Some(30.0));
        assert_eq!(super::resolve_preview_seconds(Some(-1.0)), None);
    }

    #[tokio::test]
    async fn register_job_and_validate_output_cleans_up_failed_validation() {
        let registry = std::sync::Arc::new(crate::audio::JobRegistry::new(2));
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
    fn resolve_naming_metadata_scrubs_legacy_series_parts_for_untouched_source() {
        let metadata = crate::metadata::AudiobookMetadata {
            title: Some("Legacy Source".to_string()),
            series: Some("Series".to_string()),
            series_part: Some("7/8".to_string()),
            subseries: Some("Subseries".to_string()),
            subseries_part: Some("2/3".to_string()),
            ..Default::default()
        };

        let naming =
            resolve_naming_metadata(Some(&metadata), Some(Path::new("/tmp/source.m4b")), None)
                .expect("naming metadata should exist");

        assert_eq!(naming.title.as_deref(), Some("Legacy Source"));
        assert_eq!(naming.series.as_deref(), Some("Series"));
        assert_eq!(naming.series_part, None);
        assert_eq!(naming.subseries.as_deref(), Some("Subseries"));
        assert_eq!(naming.subseries_part, None);

        let output_path = build_output_path(
            Path::new("/tmp"),
            Some(&naming),
            OutputNamingConfig::default(),
            Some(Path::new("/tmp/source.m4b")),
        )
        .expect("legacy source naming should no longer fail");

        assert!(
            output_path.to_string_lossy().contains("Legacy Source"),
            "output naming should still use source metadata title"
        );
    }

    #[test]
    fn resolve_naming_metadata_keeps_patch_validation_strict() {
        let metadata = crate::metadata::AudiobookMetadata {
            title: Some("Patched Source".to_string()),
            series: Some("Series".to_string()),
            series_part: Some("7/8".to_string()),
            ..Default::default()
        };
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Renamed".to_string()),
            ..Default::default()
        };

        let naming = resolve_naming_metadata(
            Some(&metadata),
            Some(Path::new("/tmp/source.m4b")),
            Some(&patch),
        )
        .expect("naming metadata should exist");

        let err = build_output_path(
            Path::new("/tmp"),
            Some(&naming),
            OutputNamingConfig::default(),
            Some(Path::new("/tmp/source.m4b")),
        )
        .expect_err("patched legacy series part should remain a hard failure");

        assert!(
            err.to_string().contains("Series sequence"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_batch_input_path_rejects_symlink_before_metadata_read() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let source = temp_dir.path().join("source.m4b");
        fs::write(&source, b"not audio, but enough for path validation").expect("write source");
        let symlink = temp_dir.path().join("source-link.m4b");

        #[cfg(unix)]
        std::os::unix::fs::symlink(&source, &symlink).expect("create symlink");
        #[cfg(not(unix))]
        std::os::windows::fs::symlink_file(&source, &symlink).expect("create symlink");

        let err = validate_batch_input_path(&symlink).expect_err("symlink should be rejected");
        assert!(
            err.to_string().contains("Symlinks are not supported"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn external_fdk_preflight_rejects_unsupported_named_decoder() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let ffmpeg_path = write_fake_external_ffmpeg(temp_dir.path(), true, false);
        let payload = ProcessPayload {
            input_files: vec!["/books/input.m4b".to_string()],
            output_dir: "/tmp".to_string(),
            settings: EncoderSettings {
                encoder_type: EncoderType::FdkHeAac,
                bitrate_kbps: 64,
                bitrate_mode: BitrateMode::Vbr(3),
                channels: ChannelConfig::Auto,
                afterburner: true,
                threads: ThreadSetting::Auto,
                twoloop: true,
            },
            external_toolchain: Some(ExternalToolchainPreference {
                override_path: Some(ffmpeg_path.to_string_lossy().to_string()),
            }),
            sample_rate: None,
            job_type: Some(JobType::Batch),
            output_naming: None,
            collision_policy: None,
            preflight_signature: None,
        };
        let file_info = FileListInfo {
            files: vec![AudioFile {
                path: Path::new("/books/input.m4b").to_path_buf(),
                size: Some(1.0),
                duration: Some(5.0),
                format: Some("M4B".to_string()),
                bitrate: None,
                sample_rate: None,
                channels: None,
                codec_label: Some("AAC".to_string()),
                selected_decoder: Some("Apple AAC".to_string()),
                is_valid: true,
                error: None,
            }],
            selected_decoders: vec![Some(DecoderSelection {
                decoder_id: "aac_at".to_string(),
                decoder_label: "Apple AAC".to_string(),
            })],
            total_duration: 5.0,
            total_size: 1.0,
            valid_count: 1,
            invalid_count: 0,
        };

        let err = validate_external_processing_contract_with_file_info(&payload, &file_info)
            .expect_err("unsupported named decoder should fail preflight");

        assert!(err.to_string().contains("does not expose decoder 'aac_at'"));
    }

    #[test]
    fn batch_preflight_blocks_outputs_that_target_other_selected_sources() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let source_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("media")
            .join("media_20sec.mp3");
        let alpha_path = temp_dir.path().join("alpha.m4b");
        let beta_path = temp_dir.path().join("beta.m4b");
        fs::copy(&source_fixture, &alpha_path).expect("copy alpha fixture");
        fs::copy(&source_fixture, &beta_path).expect("copy beta fixture");

        let mut metadata = HashMap::new();
        metadata.insert(
            alpha_path.to_string_lossy().to_string(),
            MetadataIntentPatch {
                title: PatchOp::Set("beta".to_string()),
                ..Default::default()
            },
        );
        metadata.insert(
            beta_path.to_string_lossy().to_string(),
            MetadataIntentPatch {
                title: PatchOp::Set("gamma".to_string()),
                ..Default::default()
            },
        );

        let plan = preflight_payload(
            ProcessPayload {
                input_files: vec![
                    alpha_path.to_string_lossy().to_string(),
                    beta_path.to_string_lossy().to_string(),
                ],
                output_dir: temp_dir.path().to_string_lossy().to_string(),
                settings: EncoderSettings {
                    encoder_type: EncoderType::Auto,
                    bitrate_kbps: 64,
                    bitrate_mode: BitrateMode::Vbr(3),
                    channels: ChannelConfig::Auto,
                    afterburner: true,
                    threads: ThreadSetting::Auto,
                    twoloop: true,
                },
                external_toolchain: None,
                sample_rate: None,
                job_type: Some(JobType::Batch),
                output_naming: Some(OutputNamingConfig {
                    preset: crate::commands::audio_types::NamingPreset::CustomTemplate,
                    include_year: false,
                    custom_template: Some("{title}".to_string()),
                }),
                collision_policy: Some(CollisionPolicy::ReplaceExisting),
                preflight_signature: None,
            },
            Some(metadata),
            None,
        )
        .expect("preflight plan");

        let first_output = &plan.outputs[0];
        let collision_kind = first_output.collision.as_ref().map(|value| value.kind);
        assert!(
            matches!(
                collision_kind,
                Some(OutputCollisionKind::SourceDestinationOverlap)
                    | Some(OutputCollisionKind::CanonicalPathOverlap)
            ),
            "expected source overlap hard block, received {collision_kind:?}"
        );
        assert_eq!(
            first_output.action,
            crate::audio::output_path::PlannedOutputAction::ReviewRequired
        );
        assert_eq!(first_output.resolved_path, beta_path.to_string_lossy());
    }

    #[test]
    fn batch_preflight_marks_existing_output_file_for_review() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let source_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("media")
            .join("media_20sec.mp3");
        let input_path = temp_dir.path().join("input.m4b");
        fs::copy(&source_fixture, &input_path).expect("copy fixture");

        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Collision Title".to_string()),
            artist: PatchOp::Set("Collision Author".to_string()),
            ..Default::default()
        };
        let expected_output = temp_dir
            .path()
            .join("Collision Author")
            .join("Collision Title")
            .join("Collision Title.m4b");
        fs::create_dir_all(
            expected_output
                .parent()
                .expect("expected output should have parent"),
        )
        .expect("create output directory");
        fs::write(&expected_output, b"existing output").expect("write existing output");

        let mut metadata = HashMap::new();
        metadata.insert(input_path.to_string_lossy().to_string(), patch);

        let plan = preflight_payload(
            ProcessPayload {
                input_files: vec![input_path.to_string_lossy().to_string()],
                output_dir: temp_dir.path().to_string_lossy().to_string(),
                settings: EncoderSettings {
                    encoder_type: EncoderType::Auto,
                    bitrate_kbps: 64,
                    bitrate_mode: BitrateMode::Vbr(3),
                    channels: ChannelConfig::Auto,
                    afterburner: true,
                    threads: ThreadSetting::Auto,
                    twoloop: true,
                },
                external_toolchain: None,
                sample_rate: None,
                job_type: Some(JobType::Batch),
                output_naming: None,
                collision_policy: None,
                preflight_signature: None,
            },
            Some(metadata),
            None,
        )
        .expect("preflight plan");

        let first_output = &plan.outputs[0];
        assert_eq!(
            first_output.requested_path,
            expected_output.to_string_lossy()
        );
        assert_eq!(
            first_output.resolved_path,
            expected_output.to_string_lossy()
        );
        assert_eq!(
            first_output.collision.as_ref().map(|value| value.kind),
            Some(OutputCollisionKind::ExistingFile)
        );
        assert_eq!(
            first_output.action,
            crate::audio::output_path::PlannedOutputAction::ReviewRequired
        );
    }

    fn write_fake_external_ffmpeg(
        root: &Path,
        include_fdk_encoder: bool,
        include_aac_at_decoder: bool,
    ) -> std::path::PathBuf {
        std::fs::create_dir_all(root).expect("create fake ffmpeg root");
        let script_path = root.join("fake-ffmpeg");
        let encoder_line = if include_fdk_encoder {
            "echo ' V..... libfdk_aac'"
        } else {
            "echo ' V..... aac'"
        };
        let decoder_line = if include_aac_at_decoder {
            "echo ' V..... aac_at'"
        } else {
            "echo ' V..... libfdk_aac'"
        };
        let script = format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = \"-version\" ]; then\n    echo 'ffmpeg version fake'\n    exit 0\n  fi\n  if [ \"$arg\" = \"-encoders\" ]; then\n    {encoder_line}\n    exit 0\n  fi\n  if [ \"$arg\" = \"-decoders\" ]; then\n    {decoder_line}\n    exit 0\n  fi\ndone\nlast=\"\"\nfor arg in \"$@\"; do\n  last=\"$arg\"\ndone\n: > \"$last\"\necho 'out_time_ms=5000'\nexit 0\n"
        );
        write(&script_path, script).expect("write fake ffmpeg");
        let mut permissions = std::fs::metadata(&script_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        set_permissions(&script_path, permissions).expect("chmod fake ffmpeg");
        script_path
    }
}
