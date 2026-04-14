use crate::audio;
use crate::audio::file_list::FileListInfo;
use crate::audio::output_path::{
    build_output_path, resolve_collision, resolve_collision_with_claimed,
};
use crate::audio::settings_encoder::{
    validate_encoder_settings, validate_requested_encoder_available,
};
use crate::audio::toolchain::{
    detect_encoder_availability, resolve_external_toolchain, ExternalToolchainPreference,
};
use crate::audio::{QueueEvent, QueueItem};
use crate::commands::audio_types::{
    JobType, OutputNamingConfig, ProcessCommandResult, ProcessPayload, ProcessResultEntry,
    ProcessResultStatus,
};
use crate::errors::sanitize_path_for_display;
use crate::errors::{AppError, AppErrorEnvelope, Result};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use tauri::Emitter;

struct ProcessingInputs {
    sample_rate: audio::SampleRateConfig,
    output_naming: OutputNamingConfig,
    base_output_dir: PathBuf,
    preview_seconds: Option<f64>,
}

pub async fn process_payload(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    payload: ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    validate_encoder_settings(&payload.settings)?;
    log_encoder_summary(&payload);

    let inputs = ProcessingInputs {
        sample_rate: resolve_sample_rate(&payload)?,
        output_naming: payload.output_naming.clone().unwrap_or_default(),
        base_output_dir: ensure_output_dir(&payload.output_dir)?,
        preview_seconds,
    };
    let job_type = payload.job_type.unwrap_or(JobType::Batch);

    match job_type {
        JobType::Merge => dispatch_merge_job(window, registry, &payload, metadata, &inputs).await,
        JobType::Batch => dispatch_batch_jobs(window, registry, &payload, metadata, &inputs).await,
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

fn ensure_output_dir(output_dir: &str) -> Result<PathBuf> {
    let base_output_dir = PathBuf::from(output_dir);
    if !base_output_dir.exists() {
        std::fs::create_dir_all(&base_output_dir).map_err(|e| {
            AppError::FileValidation(format!(
                "Failed to create output directory '{}': {}",
                sanitize_path_for_display(&base_output_dir),
                e
            ))
        })?;
    }
    Ok(base_output_dir)
}

fn resolve_effective_processing_metadata(
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

fn resolve_naming_metadata(
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

fn validate_batch_input_path(path: &std::path::Path) -> Result<PathBuf> {
    crate::audio::path_validation::validate_input_audio_path(path)
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
) -> Vec<ProcessResultEntry> {
    let mut ordered_results: Vec<Option<ProcessResultEntry>> =
        vec![None; payload.input_files.len()];

    for (index, outcome) in outcomes.into_iter().enumerate() {
        let entry = match outcome {
            Ok(mut success) => {
                success.input_index = Some(index);
                success
            }
            Err(error) => {
                let envelope: AppErrorEnvelope = error.into();
                let error_message = envelope.message.clone();
                emit_terminal_failed_event(window, index, &error_message);
                failure_result(Some(index), envelope)
            }
        };
        ordered_results[index] = Some(entry);
    }

    for (index, slot) in ordered_results.iter_mut().enumerate() {
        if slot.is_none() {
            let error_message = format!(
                "Missing terminal result for queued input index {index}; marking as failed"
            );
            emit_terminal_failed_event(window, index, &error_message);
            *slot = Some(failure_result(
                Some(index),
                AppErrorEnvelope::new(
                    crate::errors::AppErrorCode::InternalError,
                    crate::errors::AppErrorCategory::Internal,
                    error_message,
                    None,
                ),
            ));
        }
    }

    ordered_results.into_iter().flatten().collect()
}

async fn dispatch_merge_job(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    payload: &ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    inputs: &ProcessingInputs,
) -> Result<ProcessCommandResult> {
    let paths: Vec<PathBuf> = payload.input_files.iter().map(PathBuf::from).collect();
    let file_info = audio::get_file_list_info(&paths)?;
    let merge_source_path = file_info
        .files
        .first()
        .map(|file| file.path.clone())
        .ok_or_else(|| {
            AppError::InvalidInput("No input files provided for merge processing".to_string())
        })?;
    let merge_patch = payload
        .input_files
        .first()
        .and_then(|key| metadata.as_ref().and_then(|map| map.get(key)))
        .cloned();
    let merge_metadata =
        resolve_effective_processing_metadata(Some(&merge_source_path), merge_patch.as_ref())?;
    let merge_naming_metadata = resolve_naming_metadata(
        merge_metadata.as_ref(),
        Some(&merge_source_path),
        merge_patch.as_ref(),
    );
    let output_path = build_output_path(
        &inputs.base_output_dir,
        merge_naming_metadata.as_ref(),
        inputs.output_naming.clone(),
        Some(&merge_source_path),
    )?;
    let resolved_output = resolve_collision(&output_path)?;

    let result = run_processing_job(
        window,
        registry,
        payload.settings.clone(),
        payload.external_toolchain.clone(),
        inputs.sample_rate.clone(),
        None,
        resolved_output,
        file_info,
        merge_metadata,
        inputs.preview_seconds,
    )
    .await?;

    Ok(ProcessCommandResult::new(JobType::Merge, vec![result]))
}

async fn dispatch_batch_jobs(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    payload: &ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    inputs: &ProcessingInputs,
) -> Result<ProcessCommandResult> {
    if payload.input_files.is_empty() {
        return Err(AppError::InvalidInput(
            "No input files provided for batch processing".to_string(),
        ));
    }

    emit_batch_queue_event(&window, &registry, &payload.input_files);

    let mut scheduled_jobs: Vec<Pin<Box<dyn Future<Output = Result<ProcessResultEntry>> + Send>>> =
        Vec::new();
    let mut claimed_paths: HashSet<PathBuf> = HashSet::new();
    for (index, input) in payload.input_files.iter().enumerate() {
        let raw_path = PathBuf::from(input);
        let path = match validate_batch_input_path(&raw_path) {
            Ok(validated) => validated,
            Err(error) => {
                scheduled_jobs.push(Box::pin(async move { Err(error) }));
                continue;
            }
        };
        let file_patch = metadata.as_ref().and_then(|map| map.get(input)).cloned();
        let effective_metadata =
            resolve_effective_processing_metadata(Some(&path), file_patch.as_ref());
        let naming_metadata = effective_metadata.as_ref().ok().and_then(|value| {
            resolve_naming_metadata(value.as_ref(), Some(&path), file_patch.as_ref())
        });
        let output_path = build_output_path(
            &inputs.base_output_dir,
            naming_metadata.as_ref(),
            inputs.output_naming.clone(),
            Some(&path),
        );

        match effective_metadata.and_then(|resolved_metadata| {
            output_path.and_then(|candidate| {
                let resolved = resolve_collision_with_claimed(&candidate, &claimed_paths)?;
                claimed_paths.insert(resolved.clone());
                Ok((resolved, resolved_metadata))
            })
        }) {
            Ok((resolved_output, resolved_metadata)) => {
                let window_cloned = window.clone();
                let registry_cloned = registry.clone();
                let settings_cloned = payload.settings.clone();
                let external_toolchain_cloned = payload.external_toolchain.clone();
                let sr_cloned = inputs.sample_rate.clone();
                let md_cloned = resolved_metadata.clone();
                let preview_cloned = inputs.preview_seconds;

                scheduled_jobs.push(Box::pin(async move {
                    let file_info = audio::get_file_list_info(std::slice::from_ref(&path))?;
                    run_processing_job(
                        window_cloned,
                        registry_cloned,
                        settings_cloned,
                        external_toolchain_cloned,
                        sr_cloned,
                        Some(index),
                        resolved_output,
                        file_info,
                        md_cloned,
                        preview_cloned,
                    )
                    .await
                }));
            }
            Err(error) => {
                scheduled_jobs.push(Box::pin(async move { Err(error) }));
            }
        }
    }

    let outcomes = registry.scheduler().run_batch(scheduled_jobs).await;
    let finalized_results = finalize_batch_results(&window, payload, outcomes);

    Ok(ProcessCommandResult::new(JobType::Batch, finalized_results))
}

#[allow(clippy::too_many_arguments)]
async fn run_processing_job(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    encoder_settings: audio::settings_encoder::EncoderSettings,
    external_toolchain: Option<ExternalToolchainPreference>,
    sample_rate: audio::SampleRateConfig,
    input_index: Option<usize>,
    output_path: PathBuf,
    file_info: FileListInfo,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    preview_seconds: Option<f64>,
) -> Result<ProcessResultEntry> {
    let (job_id, _permit) = registry.register_job().await?;
    log::info!(
        "Job {} started for output: {}",
        job_id,
        output_path.display()
    );
    let cancellation_checker = registry.cancellation_checker(job_id).await;
    audio::settings::validate_output_path(&output_path)?;

    let (context, preview_seconds_resolved) = build_processing_context(
        window,
        cancellation_checker,
        job_id,
        encoder_settings.clone(),
        sample_rate,
        input_index,
        &output_path,
        preview_seconds,
    );
    let preview_path = build_preview_path(context.preview.as_ref(), &output_path);
    let result = execute_processing_job(
        context,
        file_info,
        metadata,
        encoder_settings,
        external_toolchain,
    )
    .await
    .map(|message| (message, preview_path, preview_seconds_resolved));

    match &result {
        Ok(_) => {
            registry.complete_job(job_id).await;
            log::info!("Job {} completed successfully", job_id);
        }
        Err(e) => {
            registry.fail_job(job_id, e.to_string()).await;
            log::error!("Job {} failed: {}", job_id, e);
        }
    }

    let (message, preview_path_opt, preview_seconds_used) = result?;

    Ok(ProcessResultEntry {
        input_index,
        status: ProcessResultStatus::Success,
        message,
        error: None,
        preview_file_path: preview_path_opt,
        preview_actual_seconds: preview_seconds_used,
        job_id: Some(job_id.to_string()),
    })
}

#[allow(clippy::too_many_arguments)]
fn build_processing_context(
    window: tauri::Window,
    cancellation_checker: crate::audio::job_registry::CancellationChecker,
    job_id: crate::audio::job_registry::JobId,
    encoder_settings: audio::settings_encoder::EncoderSettings,
    sample_rate: audio::SampleRateConfig,
    input_index: Option<usize>,
    output_path: &std::path::Path,
    preview_seconds: Option<f64>,
) -> (audio::ProcessingContext, Option<f64>) {
    let session =
        audio::session::ProcessingSession::from_job_registry(job_id.0, cancellation_checker);
    let mut context = audio::ProcessingContext::new(
        window,
        std::sync::Arc::new(session),
        encoder_settings,
        sample_rate,
        audio::OutputConfig::new(output_path.to_path_buf()),
    );
    context.job_id = Some(job_id.to_string());
    context.input_index = input_index;

    let preview_seconds_resolved = resolve_preview_seconds(preview_seconds);
    if let Some(seconds) = preview_seconds_resolved {
        context.preview = Some(crate::audio::context::PreviewConfig::new(seconds));
        log::info!("Preview requested: total_seconds={:.3}", seconds);
    }

    (context, preview_seconds_resolved)
}

fn resolve_preview_seconds(preview_seconds: Option<f64>) -> Option<f64> {
    let resolved = preview_seconds.or_else(|| {
        std::env::var("ABB_PREVIEW_SECONDS")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
    })?;

    (resolved.is_finite() && resolved > 0.0).then_some(resolved)
}

fn build_preview_path(
    preview: Option<&crate::audio::context::PreviewConfig>,
    output_path: &std::path::Path,
) -> Option<String> {
    preview.map(|_| {
        let parent = output_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let stem = output_path
            .file_stem()
            .map(|value| value.to_string_lossy())
            .unwrap_or_else(|| Cow::from("output"));
        parent
            .join(format!("{}.preview.m4b", stem))
            .display()
            .to_string()
    })
}

async fn execute_processing_job(
    context: audio::ProcessingContext,
    file_info: FileListInfo,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    encoder_settings: audio::settings_encoder::EncoderSettings,
    external_toolchain: Option<ExternalToolchainPreference>,
) -> Result<String> {
    let availability = detect_encoder_availability(external_toolchain.as_ref());
    validate_requested_encoder_available(encoder_settings.encoder_type, &availability)?;
    let resolved_encoder =
        audio::settings_encoder::resolve_encoder_type(&encoder_settings, &availability);

    if matches!(
        resolved_encoder,
        audio::settings_encoder::EncoderType::FdkHeAac
    ) {
        let resolution = resolve_external_toolchain(external_toolchain.as_ref());
        let toolchain = resolution.validated.ok_or_else(|| {
            AppError::toolchain_required("FDK AAC requires a validated external FFmpeg toolchain.")
        })?;
        return audio::external_fdk::process_audiobook_with_external_fdk(
            context,
            file_info.files,
            metadata,
            toolchain,
        )
        .await;
    }

    audio::processor::process_audiobook_with_context(context, file_info.files, metadata).await
}

fn emit_terminal_failed_event(window: &tauri::Window, input_index: usize, message: &str) {
    let event = audio::ProgressEvent {
        stage: "failed".to_string(),
        percentage: 0.0,
        message: message.to_string(),
        current_file: None,
        eta_seconds: None,
        job_id: None,
        input_index: Some(input_index),
    };
    let _ = window.emit(crate::audio::constants::PROGRESS_EVENT_NAME, &event);
}

fn failure_result(input_index: Option<usize>, error: AppErrorEnvelope) -> ProcessResultEntry {
    ProcessResultEntry {
        input_index,
        status: ProcessResultStatus::Failed,
        message: error.message.clone(),
        error: Some(error),
        preview_file_path: None,
        preview_actual_seconds: None,
        job_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_effective_processing_metadata, resolve_naming_metadata, validate_batch_input_path,
    };
    use crate::audio::output_path::build_output_path;
    use crate::commands::audio_types::OutputNamingConfig;
    use crate::metadata::{MetadataIntentPatch, PatchOp};
    use std::fs;
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
}
