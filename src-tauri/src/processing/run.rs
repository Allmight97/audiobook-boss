use super::plan::{prepare_execution_plan, resolve_preflight_plan, ResolvedProcessingPlan};
use super::terminal_outcomes::{
    build_all_skipped_batch_result, classify_processing_error, collect_batch_results,
    skipped_result, terminal_failure_result, ProcessingJobTerminalOutcome,
};
use crate::audio;
use crate::audio::file_list::FileListInfo;
use crate::audio::settings_encoder::validate_encoder_settings;
use crate::audio::toolchain::ExternalToolchainPreference;
use crate::errors::{AppError, Result};
use crate::output_artifact::{OutputKind, PlannedOutputAction, ResolvedOutputPlan};
use crate::processing::job_registry::{CancellationChecker, JobId};
use crate::processing::{
    JobType, ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessingPreflightPlan,
};
use crate::processing::{
    OutputConfig, PreviewConfig, ProcessingContext, ProcessingSession, ProgressEmitter, QueueEvent,
    QueueItem,
};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tauri::Emitter;
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
    audio::settings::validate_sample_rate_config(&sample_rate)?;
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
    cancellation_checker: crate::processing::job_registry::CancellationChecker,
    job_id: crate::processing::job_registry::JobId,
    encoder_settings: audio::settings_encoder::EncoderSettings,
    sample_rate: audio::SampleRateConfig,
    input_index: Option<usize>,
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
    let emitter = ProgressEmitter::with_context(
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
    let emitter = ProgressEmitter::with_context(
        window.clone(),
        job_id.map(|value| value.to_string()),
        input_index,
    );
    emitter.emit_terminal_skipped(message);
}

#[cfg(test)]
mod tests {
    use super::{preflight_payload, validate_external_processing_contract_with_file_info};
    use crate::audio::file_list::FileListInfo;
    use crate::audio::settings_encoder::{
        BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
    };
    use crate::audio::{AudioFile, DecoderSelection, ExternalToolchainPreference};
    use crate::metadata::{MetadataIntentPatch, PatchOp};
    use crate::output_artifact::{CollisionPolicy, OutputCollisionKind};
    use crate::processing::{JobType, OutputNamingConfig, ProcessPayload};
    use std::collections::HashMap;
    use std::fs::{self, set_permissions, write};
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
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
                    preset: crate::processing::NamingPreset::CustomTemplate,
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
            crate::output_artifact::PlannedOutputAction::ReviewRequired
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
            crate::output_artifact::PlannedOutputAction::ReviewRequired
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
