use crate::audio;
use crate::audio::file_list::FileListInfo;
use crate::audio::job_registry::{CancellationChecker, JobId};
use crate::audio::output_path::{
    build_output_path_preview, resolve_output_plan, CollisionPolicy, OutputKind,
    PlannedOutputAction, ResolvedOutputPlan,
};
use crate::audio::settings_encoder::{
    resolve_encoder_type, validate_encoder_settings, validate_requested_encoder_available,
    EncoderType,
};
use crate::audio::toolchain::{
    detect_encoder_availability, resolve_external_toolchain, validate_external_input_decoders,
    ExternalToolchainPreference,
};
use crate::audio::{QueueEvent, QueueItem};
use crate::commands::audio_types::{
    JobType, OutputNamingConfig, ProcessCommandResult, ProcessPayload, ProcessResultEntry,
    ProcessResultStatus, ProcessingPreflightPlan,
};
use crate::errors::sanitize_path_for_display;
use crate::errors::{AppError, AppErrorEnvelope, Result};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tauri::Emitter;
use tokio::sync::OwnedSemaphorePermit;

struct ProcessingInputs {
    output_naming: OutputNamingConfig,
    base_output_dir: PathBuf,
    preview_seconds: Option<f64>,
}

#[derive(Debug, Clone)]
struct PlannedProcessingJob {
    input_index: Option<usize>,
    input_path: Option<PathBuf>,
    output: ResolvedOutputPlan,
    metadata: Option<crate::metadata::AudiobookMetadata>,
}

#[derive(Debug, Clone)]
struct ResolvedProcessingPlan {
    job_type: JobType,
    preview_seconds: Option<f64>,
    collision_policy: CollisionPolicy,
    plan_signature: String,
    jobs: Vec<PlannedProcessingJob>,
}

pub async fn process_payload(
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

pub fn preflight_payload(
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

fn resolve_output_dir(output_dir: &str, create_if_missing: bool) -> Result<PathBuf> {
    let base_output_dir = PathBuf::from(output_dir);
    if create_if_missing && !base_output_dir.exists() {
        std::fs::create_dir_all(&base_output_dir).map_err(|e| {
            AppError::FileValidation(format!(
                "Failed to create output directory '{}': {}",
                sanitize_path_for_display(&base_output_dir),
                e
            ))
        })?;
    }
    if !base_output_dir.exists() {
        return Err(AppError::FileValidation(format!(
            "Output directory does not exist: {}",
            sanitize_path_for_display(&base_output_dir)
        )));
    }
    if !base_output_dir.is_dir() {
        return Err(AppError::FileValidation(format!(
            "Output path is not a directory: {}",
            sanitize_path_for_display(&base_output_dir)
        )));
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

fn validate_external_processing_contract(payload: &ProcessPayload) -> Result<()> {
    let input_paths: Vec<PathBuf> = payload.input_files.iter().map(PathBuf::from).collect();
    let file_info = audio::get_file_list_info(&input_paths)?;
    validate_external_processing_contract_with_file_info(payload, &file_info)
}

fn validate_external_processing_contract_with_file_info(
    payload: &ProcessPayload,
    file_info: &FileListInfo,
) -> Result<()> {
    let availability = detect_encoder_availability(payload.external_toolchain.as_ref());
    validate_requested_encoder_available(payload.settings.encoder_type, &availability)?;
    let resolved_encoder = resolve_encoder_type(&payload.settings, &availability);
    if !matches!(resolved_encoder, EncoderType::FdkHeAac) {
        return Ok(());
    }

    let resolution = resolve_external_toolchain(payload.external_toolchain.as_ref());
    let toolchain = resolution.validated.ok_or_else(|| {
        AppError::toolchain_required("FDK AAC requires a validated external FFmpeg toolchain.")
    })?;
    validate_external_input_decoders(&file_info.files, &file_info.selected_decoders, &toolchain)?;
    Ok(())
}

fn resolve_collision_policy(payload: &ProcessPayload) -> CollisionPolicy {
    payload.collision_policy.unwrap_or(CollisionPolicy::Fail)
}

fn resolve_output_kind(preview_seconds: Option<f64>) -> OutputKind {
    if preview_seconds.is_some() {
        OutputKind::Preview
    } else {
        OutputKind::Final
    }
}

fn action_requires_output_write(action: PlannedOutputAction) -> bool {
    matches!(
        action,
        PlannedOutputAction::Write
            | PlannedOutputAction::ReplaceExisting
            | PlannedOutputAction::RenameNew
    )
}

fn action_is_hard_block(job: &PlannedProcessingJob) -> bool {
    matches!(job.output.action, PlannedOutputAction::ReviewRequired)
        && job.output.collision.as_ref().is_some_and(|collision| {
            matches!(
                collision.kind,
                crate::audio::output_path::OutputCollisionKind::SourceDestinationOverlap
                    | crate::audio::output_path::OutputCollisionKind::CanonicalPathOverlap
            )
        })
}

fn ensure_output_parent_dirs(plan: &ResolvedProcessingPlan) -> Result<()> {
    for job in &plan.jobs {
        if !action_requires_output_write(job.output.action) {
            continue;
        }
        if let Some(parent) = job.output.resolved_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                AppError::FileValidation(format!(
                    "Cannot create output directory '{}': {}",
                    sanitize_path_for_display(parent),
                    error
                ))
            })?;
        }
    }

    Ok(())
}

fn build_plan_signature(
    job_type: JobType,
    preview_seconds: Option<f64>,
    collision_policy: CollisionPolicy,
    jobs: &[PlannedProcessingJob],
) -> String {
    let mut lines = vec![
        format!("job_type={job_type:?}"),
        format!("preview_seconds={preview_seconds:?}"),
        format!("collision_policy={collision_policy:?}"),
    ];

    for job in jobs {
        let output = &job.output;
        let collision_kind = output
            .collision
            .as_ref()
            .map(|value| format!("{:?}", value.kind))
            .unwrap_or_else(|| "none".to_string());
        let collision_path = output
            .collision
            .as_ref()
            .and_then(|value| value.conflicting_path.as_ref())
            .map(|value| value.display().to_string())
            .unwrap_or_default();
        let collision_detail = output
            .collision
            .as_ref()
            .and_then(|value| value.detail.clone())
            .unwrap_or_default();
        let output_kind = format!("{:?}", output.kind);
        let output_action = format!("{:?}", output.action);
        let collision_summary = format!("{collision_path}::{collision_detail}");
        lines.push(format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            job.input_index
                .map(|value| value.to_string())
                .unwrap_or_else(|| "merge".to_string()),
            output.requested_path.display(),
            output.resolved_path.display(),
            output
                .rename_candidate
                .as_ref()
                .map(|value| value.display().to_string())
                .unwrap_or_default(),
            output_kind,
            output_action,
            collision_kind,
            collision_summary,
        ));
    }

    lines.join("\n")
}

fn output_plan_review_message(job: &PlannedProcessingJob) -> String {
    let destination = sanitize_path_for_display(&job.output.requested_path);
    match job.output.collision.as_ref().map(|value| value.kind) {
        Some(crate::audio::output_path::OutputCollisionKind::SourceDestinationOverlap)
        | Some(crate::audio::output_path::OutputCollisionKind::CanonicalPathOverlap) => format!(
            "Output path '{}' targets an input source file. Choose a different destination.",
            destination
        ),
        _ => format!(
            "Output collision review is required for '{}'. Re-run preflight and choose how to handle the collision.",
            destination
        ),
    }
}

fn enforce_plan_review(payload: &ProcessPayload, plan: &ResolvedProcessingPlan) -> Result<()> {
    let expected_signature = payload.preflight_signature.as_deref();
    let current_signature = plan.plan_signature.as_str();

    if let Some(signature) = expected_signature {
        if signature != current_signature {
            return Err(AppError::FileValidation(
                "Output collision state changed after review. Review the collision dialog and try again."
                    .to_string(),
            ));
        }
    }

    if resolve_collision_policy(payload) != CollisionPolicy::Fail && expected_signature.is_none() {
        return Err(AppError::InvalidInput(
            "Collision policy selections require a reviewed preflight plan.".to_string(),
        ));
    }

    if let Some(job) = plan.jobs.iter().find(|job| action_is_hard_block(job)) {
        return Err(AppError::FileValidation(output_plan_review_message(job)));
    }

    if let Some(job) = plan
        .jobs
        .iter()
        .find(|job| job.output.action == PlannedOutputAction::ReviewRequired)
    {
        return Err(AppError::FileValidation(output_plan_review_message(job)));
    }

    if expected_signature.is_none() && plan.jobs.iter().any(|job| job.output.collision.is_some()) {
        return Err(AppError::FileValidation(
            "Output collisions require review before processing. Open the collision dialog and choose how to continue."
                .to_string(),
        ));
    }

    Ok(())
}

fn build_requested_output_path(
    base_output_dir: &Path,
    metadata: Option<&crate::metadata::AudiobookMetadata>,
    output_naming: OutputNamingConfig,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    build_output_path_preview(base_output_dir, metadata, output_naming, source_path)
}

fn build_processing_plan(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    inputs: &ProcessingInputs,
) -> Result<ResolvedProcessingPlan> {
    let job_type = payload.job_type.unwrap_or(JobType::Batch);
    let collision_policy = resolve_collision_policy(payload);
    let output_kind = resolve_output_kind(inputs.preview_seconds);
    let mut claimed_outputs: HashSet<PathBuf> = HashSet::new();
    let mut jobs = Vec::new();

    match job_type {
        JobType::Merge => jobs.push(build_merge_processing_job(
            payload,
            metadata,
            inputs,
            output_kind,
            collision_policy,
            &mut claimed_outputs,
        )?),
        JobType::Batch => jobs.extend(build_batch_processing_jobs(
            payload,
            metadata,
            inputs,
            output_kind,
            collision_policy,
            &mut claimed_outputs,
        )?),
    }

    let plan_signature =
        build_plan_signature(job_type, inputs.preview_seconds, collision_policy, &jobs);

    Ok(ResolvedProcessingPlan {
        job_type,
        preview_seconds: inputs.preview_seconds,
        collision_policy,
        plan_signature,
        jobs,
    })
}

fn log_output_plan(phase: &str, payload: &ProcessPayload, plan: &ResolvedProcessingPlan) {
    for job in &plan.jobs {
        if job.output.action == PlannedOutputAction::Write
            && job.output.collision.is_none()
            && job.output.kind == OutputKind::Final
        {
            continue;
        }

        log::info!(
            "output_plan phase={} reviewed={} policy={:?} input_index={:?} kind={:?} action={:?} requested={} resolved={} collision_kind={} collision_path={}",
            phase,
            payload.preflight_signature.is_some(),
            plan.collision_policy,
            job.input_index,
            job.output.kind,
            job.output.action,
            job.output.requested_path.display(),
            job.output.resolved_path.display(),
            job.output
                .collision
                .as_ref()
                .map(|value| format!("{:?}", value.kind))
                .unwrap_or_else(|| "none".to_string()),
            job.output
                .collision
                .as_ref()
                .and_then(|value| value.conflicting_path.as_ref())
                .map(|value| value.display().to_string())
                .unwrap_or_default(),
        );
    }
}

fn build_merge_processing_job(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    inputs: &ProcessingInputs,
    output_kind: OutputKind,
    collision_policy: CollisionPolicy,
    claimed_outputs: &mut HashSet<PathBuf>,
) -> Result<PlannedProcessingJob> {
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
        .and_then(|key| metadata.and_then(|map| map.get(key)))
        .cloned();
    let merge_metadata =
        resolve_effective_processing_metadata(Some(&merge_source_path), merge_patch.as_ref())?;
    let merge_naming_metadata = resolve_naming_metadata(
        merge_metadata.as_ref(),
        Some(&merge_source_path),
        merge_patch.as_ref(),
    );
    let requested_output = build_requested_output_path(
        &inputs.base_output_dir,
        merge_naming_metadata.as_ref(),
        inputs.output_naming.clone(),
        Some(&merge_source_path),
    )?;
    let source_paths = file_info
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let output = resolve_output_plan(
        &requested_output,
        output_kind,
        collision_policy,
        claimed_outputs,
        &source_paths,
    )?;
    if action_requires_output_write(output.action) {
        claimed_outputs.insert(output.resolved_path.clone());
    }

    Ok(PlannedProcessingJob {
        input_index: None,
        input_path: None,
        output,
        metadata: merge_metadata,
    })
}

fn build_batch_processing_jobs(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    inputs: &ProcessingInputs,
    output_kind: OutputKind,
    collision_policy: CollisionPolicy,
    claimed_outputs: &mut HashSet<PathBuf>,
) -> Result<Vec<PlannedProcessingJob>> {
    if payload.input_files.is_empty() {
        return Err(AppError::InvalidInput(
            "No input files provided for batch processing".to_string(),
        ));
    }

    let validated_input_paths: Vec<PathBuf> = payload
        .input_files
        .iter()
        .map(|input| validate_batch_input_path(&PathBuf::from(input)))
        .collect::<Result<_>>()?;

    let mut jobs = Vec::new();
    for (index, path) in validated_input_paths.iter().cloned().enumerate() {
        let input = &payload.input_files[index];
        let file_patch = metadata.and_then(|map| map.get(input)).cloned();
        let effective_metadata =
            resolve_effective_processing_metadata(Some(&path), file_patch.as_ref())?;
        let naming_metadata = resolve_naming_metadata(
            effective_metadata.as_ref(),
            Some(&path),
            file_patch.as_ref(),
        );
        let requested_output = build_requested_output_path(
            &inputs.base_output_dir,
            naming_metadata.as_ref(),
            inputs.output_naming.clone(),
            Some(&path),
        )?;
        let output = resolve_output_plan(
            &requested_output,
            output_kind,
            collision_policy,
            claimed_outputs,
            &validated_input_paths,
        )?;
        if action_requires_output_write(output.action) {
            claimed_outputs.insert(output.resolved_path.clone());
        }
        jobs.push(PlannedProcessingJob {
            input_index: Some(index),
            input_path: Some(path),
            output,
            metadata: effective_metadata,
        });
    }

    Ok(jobs)
}

impl ResolvedProcessingPlan {
    fn to_public(&self) -> ProcessingPreflightPlan {
        let outputs = self
            .jobs
            .iter()
            .map(|job| {
                job.output
                    .to_public(job.input_index, job.input_path.as_deref())
            })
            .collect();
        ProcessingPreflightPlan {
            job_type: self.job_type,
            preview_seconds: self.preview_seconds,
            collision_policy: self.collision_policy,
            plan_signature: self.plan_signature.clone(),
            outputs,
        }
    }
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
    let mut ordered_results: Vec<Option<ProcessResultEntry>> =
        vec![None; payload.input_files.len()];
    let mut cancellation_error: Option<AppError> = None;

    for (index, outcome) in outcomes.into_iter().enumerate() {
        let entry = match outcome {
            Ok(mut entry) => {
                if entry.input_index.is_none() {
                    entry.input_index = Some(index);
                }
                if let Some(error) = cancellation_error_for_failed_entry(&entry) {
                    cancellation_error.get_or_insert(error);
                    continue;
                }
                if entry.status == ProcessResultStatus::Failed {
                    emit_terminal_failed_event(
                        window,
                        entry.input_index,
                        entry.job_id.as_deref(),
                        &entry.message,
                    );
                }
                entry
            }
            Err(error) => {
                if is_cancellation_error(&error) {
                    cancellation_error.get_or_insert(error);
                    continue;
                }
                let envelope: AppErrorEnvelope = error.into();
                let error_message = envelope.message.clone();
                emit_terminal_failed_event(window, Some(index), None, &error_message);
                terminal_failure_result(Some(index), None, envelope)
            }
        };
        ordered_results[index] = Some(entry);
    }

    if let Some(error) = cancellation_error {
        return Err(error);
    }

    for (index, slot) in ordered_results.iter_mut().enumerate() {
        if slot.is_none() {
            let error_message = format!(
                "Missing terminal result for queued input index {index}; marking as failed"
            );
            emit_terminal_failed_event(window, Some(index), None, &error_message);
            *slot = Some(terminal_failure_result(
                Some(index),
                None,
                AppErrorEnvelope::new(
                    crate::errors::AppErrorCode::InternalError,
                    crate::errors::AppErrorCategory::Internal,
                    error_message,
                    None,
                ),
            ));
        }
    }

    Ok(ordered_results.into_iter().flatten().collect())
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

    let result = run_processing_job(
        window,
        registry,
        payload.settings.clone(),
        payload.external_toolchain.clone(),
        resolve_sample_rate(payload)?,
        None,
        planned_job.output,
        file_info,
        planned_job.metadata,
        plan.preview_seconds,
    )
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
        let preview_cloned = preview_seconds;
        let input_index = planned_job.input_index;
        let output = planned_job.output.clone();
        let path = planned_job.input_path.clone().ok_or_else(|| {
            AppError::InvalidInput("Missing batch input path in output plan".to_string())
        })?;

        scheduled_jobs.push(Box::pin(async move {
            let file_info = audio::get_file_list_info(std::slice::from_ref(&path))?;
            run_processing_job(
                window_cloned,
                registry_cloned,
                settings_cloned,
                external_toolchain_cloned,
                sr_cloned,
                input_index,
                output,
                file_info,
                md_cloned,
                preview_cloned,
            )
            .await
        }));
    }

    let outcomes = registry.scheduler().run_batch(scheduled_jobs).await;
    let finalized_results = finalize_batch_results(&window, payload, outcomes)?;

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
    output_plan: ResolvedOutputPlan,
    file_info: FileListInfo,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    preview_seconds: Option<f64>,
) -> Result<ProcessResultEntry> {
    let (job_id, _permit, cancellation_checker) =
        register_job_and_validate_output(&registry, &output_plan.resolved_path).await?;

    let (context, preview_seconds_resolved) = build_processing_context(
        window,
        cancellation_checker,
        job_id,
        encoder_settings.clone(),
        sample_rate,
        input_index,
        output_plan.clone(),
        preview_seconds,
    );
    let preview_path = (output_plan.kind == OutputKind::Preview)
        .then(|| output_plan.resolved_path.display().to_string());
    let result = execute_processing_job(
        context,
        file_info,
        metadata,
        encoder_settings,
        external_toolchain,
    )
    .await
    .map(|message| (message, preview_path, preview_seconds_resolved));

    match result {
        Ok((message, preview_path_opt, preview_seconds_used)) => {
            registry.complete_job(job_id).await;
            log::info!("Job {} completed successfully", job_id);
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
        Err(error) => {
            if is_cancellation_error(&error) {
                registry.complete_job(job_id).await;
                log::warn!("Job {} cancelled: {}", job_id, error);
                return Err(error);
            }
            registry.fail_job(job_id, error.to_string()).await;
            log::error!("Job {} failed: {}", job_id, error);
            let envelope: AppErrorEnvelope = error.into();
            Ok(terminal_failure_result(
                input_index,
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

#[allow(clippy::too_many_arguments)]
fn build_processing_context(
    window: tauri::Window,
    cancellation_checker: crate::audio::job_registry::CancellationChecker,
    job_id: crate::audio::job_registry::JobId,
    encoder_settings: audio::settings_encoder::EncoderSettings,
    sample_rate: audio::SampleRateConfig,
    input_index: Option<usize>,
    output_plan: ResolvedOutputPlan,
    preview_seconds: Option<f64>,
) -> (audio::ProcessingContext, Option<f64>) {
    let session =
        audio::session::ProcessingSession::from_job_registry(job_id.0, cancellation_checker);
    let mut context = audio::ProcessingContext::new(
        window,
        std::sync::Arc::new(session),
        encoder_settings,
        sample_rate,
        audio::OutputConfig::from_plan(output_plan),
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
    let resolved = preview_seconds?;

    (resolved.is_finite() && resolved > 0.0).then_some(resolved)
}

async fn execute_processing_job(
    context: audio::ProcessingContext,
    file_info: FileListInfo,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    encoder_settings: audio::settings_encoder::EncoderSettings,
    external_toolchain: Option<ExternalToolchainPreference>,
) -> Result<String> {
    let FileListInfo {
        files,
        selected_decoders,
        ..
    } = file_info;
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
        validate_external_input_decoders(&files, &selected_decoders, &toolchain)?;
        return audio::external_fdk::process_audiobook_with_external_fdk(
            context,
            files,
            selected_decoders,
            metadata,
            toolchain,
        )
        .await;
    }

    audio::processor::process_audiobook_with_context(context, files, metadata).await
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

fn skipped_result(
    input_index: Option<usize>,
    job_id: Option<String>,
    output: &ResolvedOutputPlan,
) -> ProcessResultEntry {
    let message = format!(
        "Skipped existing output at '{}'",
        sanitize_path_for_display(&output.requested_path)
    );
    ProcessResultEntry {
        input_index,
        status: ProcessResultStatus::Skipped,
        message,
        error: None,
        preview_file_path: None,
        preview_actual_seconds: None,
        job_id,
    }
}

fn terminal_failure_result(
    input_index: Option<usize>,
    job_id: Option<String>,
    error: AppErrorEnvelope,
) -> ProcessResultEntry {
    ProcessResultEntry {
        input_index,
        status: ProcessResultStatus::Failed,
        message: error.message.clone(),
        error: Some(error),
        preview_file_path: None,
        preview_actual_seconds: None,
        job_id,
    }
}

fn is_cancellation_error(error: &AppError) -> bool {
    matches!(error, AppError::Cancellation(_))
}

fn cancellation_error_for_failed_entry(entry: &ProcessResultEntry) -> Option<AppError> {
    let envelope = entry.error.as_ref()?;
    if entry.status == ProcessResultStatus::Failed
        && envelope.category == crate::errors::AppErrorCategory::Cancellation
    {
        return Some(AppError::Cancellation(envelope.message.clone()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        cancellation_error_for_failed_entry, is_cancellation_error, preflight_payload,
        resolve_effective_processing_metadata, resolve_naming_metadata, terminal_failure_result,
        validate_batch_input_path, validate_external_processing_contract_with_file_info,
    };
    use crate::audio::file_list::FileListInfo;
    use crate::audio::output_path::{build_output_path, CollisionPolicy, OutputCollisionKind};
    use crate::audio::settings_encoder::{
        BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
    };
    use crate::audio::{AudioFile, DecoderSelection, ExternalToolchainPreference};
    use crate::commands::audio_types::{
        JobType, OutputNamingConfig, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    };
    use crate::errors::{AppError, AppErrorCategory, AppErrorCode, AppErrorEnvelope};
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
    fn terminal_failure_result_preserves_job_id_when_available() {
        let error = AppErrorEnvelope::new(
            AppErrorCode::InternalError,
            AppErrorCategory::Internal,
            "Processing failed".to_string(),
            None,
        );

        let entry = terminal_failure_result(Some(4), Some("job-123".to_string()), error);

        assert_eq!(entry.input_index, Some(4));
        assert_eq!(entry.job_id.as_deref(), Some("job-123"));
        assert_eq!(entry.status, ProcessResultStatus::Failed);
        assert!(entry.error.is_some());
    }

    #[test]
    fn cancellation_error_for_failed_entry_returns_cancelled_error() {
        let entry = ProcessResultEntry {
            input_index: Some(2),
            status: ProcessResultStatus::Failed,
            message: "Processing was cancelled".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::ProcessingCancelled,
                AppErrorCategory::Cancellation,
                "Processing was cancelled".to_string(),
                None,
            )),
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-123".to_string()),
        };

        let error = cancellation_error_for_failed_entry(&entry).expect("cancellation error");

        assert!(is_cancellation_error(&error));
        assert_eq!(error.to_string(), "Processing was cancelled");
    }

    #[test]
    fn non_cancellation_errors_stay_failed_results() {
        let entry = ProcessResultEntry {
            input_index: Some(2),
            status: ProcessResultStatus::Failed,
            message: "decoder unavailable".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::ToolchainRequired,
                AppErrorCategory::Toolchain,
                "decoder unavailable".to_string(),
                Some("ffmpeg missing".to_string()),
            )),
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-123".to_string()),
        };

        assert!(cancellation_error_for_failed_entry(&entry).is_none());
        assert!(!is_cancellation_error(&AppError::toolchain_required(
            "decoder unavailable"
        )));
    }

    #[test]
    fn mixed_cancel_and_fail_classification_keeps_failure_visible() {
        let cancelled = AppError::cancelled();
        let failed = ProcessResultEntry {
            input_index: Some(1),
            status: ProcessResultStatus::Failed,
            message: "decoder unavailable".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::ToolchainRequired,
                AppErrorCategory::Toolchain,
                "decoder unavailable".to_string(),
                Some("ffmpeg missing".to_string()),
            )),
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-2".to_string()),
        };

        assert!(is_cancellation_error(&cancelled));
        assert!(cancellation_error_for_failed_entry(&failed).is_none());
        assert_eq!(failed.status, ProcessResultStatus::Failed);
        assert_eq!(failed.job_id.as_deref(), Some("job-2"));
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
