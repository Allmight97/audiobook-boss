use crate::audio::FileListInfo;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::{
    plan_metadata_outcome, CoverArtPassthroughPolicy, MetadataOutcomePlan, MetadataOutcomeRequest,
    NamingMetadata,
};
use crate::output_artifact::OutputNamingConfig;
use crate::output_artifact::{
    build_output_path_preview, enforce_output_plan_review, ensure_output_parent_dirs,
    CollisionPolicy, OutputKind, OutputParentDirCleanup, OutputPlanLedger, OutputPlanReview,
    PlannedOutputAction, ResolvedOutputPlan,
};
use crate::processing::{ProcessPayload, ProcessingPreflightPlan};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct ProcessingInputs {
    output_naming: OutputNamingConfig,
    base_output_dir: PathBuf,
    preview_seconds: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedProcessingJob {
    pub(crate) input_index: usize,
    pub(crate) input_path: PathBuf,
    pub(crate) source_paths: Vec<PathBuf>,
    pub(crate) output: ResolvedOutputPlan,
    pub(crate) metadata: Option<crate::metadata::AudiobookMetadata>,
    /// The anchor's own tags as read for this plan.
    pub(crate) source_metadata: Option<crate::metadata::AudiobookMetadata>,
    pub(crate) cover_art_passthrough: CoverArtPassthroughPolicy,
    pub(crate) audio_plan: crate::audio::TitleAudioPlan,
    pub(crate) metadata_intent: Option<crate::metadata::MetadataIntentPatch>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedProcessingPlan {
    pub(crate) preview_seconds: Option<f64>,
    pub(crate) collision_policy: CollisionPolicy,
    pub(crate) plan_signature: String,
    pub(crate) jobs: Vec<PlannedProcessingJob>,
}

pub(crate) struct ExecutionProcessingPlan {
    pub(crate) plan: ResolvedProcessingPlan,
    pub(crate) file_info: FileListInfo,
    pub(crate) output_parent_cleanup: OutputParentDirCleanup,
}

/// The source inspection and resolved plan the user reviewed. Execution keeps
/// these exact facts; Audio checks their fingerprints after scheduler waits.
pub(crate) struct InspectedProcessingPlan {
    pub(crate) plan: ResolvedProcessingPlan,
    pub(crate) file_info: FileListInfo,
}

/// Preflight requires an existing output folder. Execution may find it missing
/// (removed after review); the output owner recreates it only after the
/// reviewed plan is enforced.
fn resolve_output_dir(output_dir: &str, allow_missing: bool) -> Result<PathBuf> {
    let base_output_dir = PathBuf::from(output_dir);
    if !base_output_dir.exists() {
        if allow_missing {
            return Ok(base_output_dir);
        }
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

fn build_plan_signature(
    preview_seconds: Option<f64>,
    collision_policy: CollisionPolicy,
    jobs: &[PlannedProcessingJob],
) -> String {
    let mut lines = vec![
        format!("preview_seconds={preview_seconds:?}"),
        format!("collision_policy={collision_policy:?}"),
    ];

    for job in jobs {
        lines.push(format!(
            "sources={:?};audio={:?}",
            job.source_paths, job.audio_plan
        ));
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
            "{}|{}|{}|{}|{}|{}|{}|{}|{:?}",
            job.input_index,
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
            job.audio_plan.handling,
        ));
    }

    lines.join("\n")
}

fn build_requested_output_path(
    base_output_dir: &Path,
    metadata: Option<&NamingMetadata>,
    output_naming: OutputNamingConfig,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    build_output_path_preview(base_output_dir, metadata, output_naming, source_path)
}

fn build_processing_inputs(
    payload: &ProcessPayload,
    allow_missing_output_dir: bool,
    preview_seconds: Option<f64>,
) -> Result<ProcessingInputs> {
    Ok(ProcessingInputs {
        output_naming: payload.output_naming.clone().unwrap_or_default(),
        base_output_dir: resolve_output_dir(&payload.output_dir, allow_missing_output_dir)?,
        preview_seconds: resolve_preview_seconds(preview_seconds),
    })
}

fn resolve_preview_seconds(preview_seconds: Option<f64>) -> Option<f64> {
    let resolved = preview_seconds?;

    (resolved.is_finite() && resolved > 0.0).then_some(resolved)
}

fn build_processing_plan(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    inputs: &ProcessingInputs,
    file_info: &FileListInfo,
) -> Result<ResolvedProcessingPlan> {
    let collision_policy = resolve_collision_policy(payload);
    let mut output_ledger = OutputPlanLedger::new();

    payload.validate_audio_requests()?;

    let jobs = build_title_processing_jobs(
        payload,
        metadata,
        inputs,
        collision_policy,
        &mut output_ledger,
        file_info,
    )?;

    let plan_signature = build_plan_signature(inputs.preview_seconds, collision_policy, &jobs);

    Ok(ResolvedProcessingPlan {
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

/// The preflight plan with each title's planning detail.
pub(crate) fn resolve_processing_plan(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
    file_info: &FileListInfo,
) -> Result<ResolvedProcessingPlan> {
    let inputs = build_processing_inputs(payload, false, preview_seconds)?;
    let plan = build_processing_plan(payload, metadata, &inputs, file_info)?;
    log_output_plan("preflight", payload, &plan);
    Ok(plan)
}

#[cfg(test)]
pub(crate) fn prepare_execution_plan(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
    file_info: FileListInfo,
) -> Result<ExecutionProcessingPlan> {
    let inputs = build_processing_inputs(payload, true, preview_seconds)?;
    let plan = build_processing_plan(payload, metadata, &inputs, &file_info)?;
    prepare_inspected_execution(payload, InspectedProcessingPlan { plan, file_info })
}

pub(crate) fn prepare_inspected_execution(
    payload: &ProcessPayload,
    inspected: InspectedProcessingPlan,
) -> Result<ExecutionProcessingPlan> {
    let InspectedProcessingPlan {
        mut plan,
        file_info,
    } = inspected;
    let inputs = build_processing_inputs(payload, true, plan.preview_seconds)?;
    // Collision facts may change after review; refresh them without replacing
    // the source inspection or resolving the audio and metadata a second time.
    let sources = file_info
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let mut ledger = OutputPlanLedger::new();
    for job in &mut plan.jobs {
        job.output = ledger.refresh(&job.output, plan.collision_policy, &sources)?;
    }
    plan.plan_signature =
        build_plan_signature(plan.preview_seconds, plan.collision_policy, &plan.jobs);
    log_output_plan("process", payload, &plan);
    enforce_output_plan_review(
        OutputPlanReview {
            expected_signature: payload.preflight_signature.as_deref(),
            current_signature: &plan.plan_signature,
            collision_policy: plan.collision_policy,
        },
        plan.jobs.iter().map(|job| &job.output),
    )?;
    let output_parent_cleanup = ensure_output_parent_dirs(
        &inputs.base_output_dir,
        plan.jobs.iter().map(|job| &job.output),
    )?;
    Ok(ExecutionProcessingPlan {
        plan,
        file_info,
        output_parent_cleanup,
    })
}

fn build_title_processing_jobs(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    inputs: &ProcessingInputs,
    collision_policy: CollisionPolicy,
    output_ledger: &mut OutputPlanLedger,
    file_info: &FileListInfo,
) -> Result<Vec<PlannedProcessingJob>> {
    if payload.input_files.is_empty() {
        return Err(AppError::InvalidInput(
            "No input files provided for processing".to_string(),
        ));
    }
    let validated_input_paths: Vec<PathBuf> = file_info
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect();

    let mut jobs = Vec::new();
    for (index, input) in payload.input_files.iter().enumerate() {
        let path = crate::audio::validate_input_audio_path(Path::new(input))?;
        let source_paths = payload
            .sources_for(index)
            .iter()
            .map(|source| crate::audio::validate_input_audio_path(Path::new(&source.path)))
            .collect::<Result<Vec<_>>>()?;
        let file_patch = metadata.and_then(|map| map.get(input)).cloned();
        let metadata_outcome = plan_metadata_outcome(MetadataOutcomeRequest {
            input_path: Some(&path),
            intent_patch: file_patch.as_ref(),
        })?;
        let metadata_outcome: MetadataOutcomePlan = metadata_outcome;
        let requested_output = build_requested_output_path(
            &inputs.base_output_dir,
            metadata_outcome.naming_metadata.as_ref(),
            inputs.output_naming.clone(),
            Some(&path),
        )?;
        let info = title_file_info(file_info, &source_paths)?;
        let audio_plan = crate::audio::resolve_title_audio(
            &payload.audio_requests[index],
            &info,
            inputs.preview_seconds.is_some(),
        )?;
        let requested_output = requested_output.with_extension(audio_plan.format.extension());
        let output = output_ledger.resolve(
            &requested_output,
            resolve_output_kind(inputs.preview_seconds),
            collision_policy,
            &validated_input_paths,
        )?;
        jobs.push(PlannedProcessingJob {
            input_index: index,
            input_path: path,
            source_paths,
            output,
            source_metadata: metadata_outcome.source_metadata,
            metadata: metadata_outcome.effective_metadata,
            cover_art_passthrough: metadata_outcome.cover_art_passthrough,
            audio_plan,
            metadata_intent: metadata_outcome.write_intent,
        });
    }

    Ok(jobs)
}

pub(super) fn title_file_info(all: &FileListInfo, paths: &[PathBuf]) -> Result<FileListInfo> {
    let mut info = FileListInfo {
        files: Vec::with_capacity(paths.len()),
        total_duration: 0.0,
        total_size: 0.0,
        valid_count: 0,
        invalid_count: 0,
    };
    for path in paths {
        let index = all
            .files
            .iter()
            .position(|file| &file.path == path)
            .ok_or_else(|| AppError::InvalidInput("A title source was not inspected.".into()))?;
        info.files.push(all.files[index].clone());
    }
    info.valid_count = info.files.iter().filter(|file| file.is_valid).count();
    info.invalid_count = info.files.len() - info.valid_count;
    if info.invalid_count > 0 {
        return Err(AppError::InvalidInput("Every source in an output title must be valid. Remove or replace invalid files before processing.".into()));
    }
    info.total_duration = info.files.iter().filter_map(|file| file.duration).sum();
    info.total_size = info.files.iter().filter_map(|file| file.size).sum();
    Ok(info)
}

impl ResolvedProcessingPlan {
    pub(crate) fn to_public(&self) -> ProcessingPreflightPlan {
        let outputs = self
            .jobs
            .iter()
            .map(|job| job.output.to_public(job.input_index, &job.input_path))
            .collect();
        ProcessingPreflightPlan {
            preview_seconds: self.preview_seconds,
            collision_policy: self.collision_policy,
            plan_signature: self.plan_signature.clone(),
            outputs,
            audio_plans: self.jobs.iter().map(|job| job.audio_plan.clone()).collect(),
        }
    }
}
