use super::plan_signature;
use crate::audio::FileListInfo;
use crate::errors::{AppError, Result};
use crate::metadata::{
    plan_metadata_outcome, CoverArtPassthroughPolicy, MetadataOutcomePlan, NamingMetadata,
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

/// What decides where a title's output lands besides its metadata.
pub(crate) struct NamingInputs<'a> {
    pub(crate) base_dir: &'a Path,
    pub(crate) naming: &'a OutputNamingConfig,
    pub(crate) anchor: &'a Path,
}

/// The requested output path of a title, before collision handling. The plan
/// and the title's output record both name through this.
pub(crate) fn output_path_for(
    inputs: NamingInputs<'_>,
    metadata: &NamingMetadata,
    extension: &str,
) -> Result<PathBuf> {
    build_output_path_preview(
        inputs.base_dir,
        Some(metadata),
        inputs.naming.clone(),
        Some(inputs.anchor),
        extension,
    )
}

struct ProcessingInputs {
    output_naming: OutputNamingConfig,
    base_output_dir: PathBuf,
    output_kind: OutputKind,
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedProcessingJob {
    pub(crate) input_index: usize,
    pub(crate) input_path: PathBuf,
    pub(crate) source_paths: Vec<PathBuf>,
    pub(crate) output: ResolvedOutputPlan,
    pub(crate) metadata: Option<crate::metadata::AudiobookMetadata>,
    /// The anchor's own tags as read for this plan.
    pub(crate) source_metadata: crate::metadata::AudiobookMetadata,
    pub(crate) cover_art_passthrough: CoverArtPassthroughPolicy,
    pub(crate) audio_plan: crate::audio::TitleAudioPlan,
    pub(crate) metadata_intent: Option<crate::metadata::MetadataIntentPatch>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedProcessingPlan {
    pub(crate) collision_policy: CollisionPolicy,
    pub(crate) plan_signature: String,
    pub(crate) jobs: Vec<PlannedProcessingJob>,
}

pub(crate) struct ExecutionProcessingPlan {
    pub(crate) plan: ResolvedProcessingPlan,
    pub(crate) file_info: FileListInfo,
    pub(crate) output_parent_cleanup: OutputParentDirCleanup,
}

pub(crate) struct InspectedProcessingPlan {
    pub(crate) plan: ResolvedProcessingPlan,
    pub(crate) file_info: FileListInfo,
}

fn resolve_output_dir(output_dir: &str, allow_missing: bool) -> Result<PathBuf> {
    let base_output_dir = PathBuf::from(output_dir);
    crate::audio::check_output_directory(&base_output_dir, allow_missing)?;
    Ok(base_output_dir)
}

fn resolve_collision_policy(payload: &ProcessPayload) -> CollisionPolicy {
    payload.collision_policy.unwrap_or(CollisionPolicy::Fail)
}

fn resolve_output_kind(preview: bool) -> OutputKind {
    if preview {
        OutputKind::Preview
    } else {
        OutputKind::Final
    }
}

fn build_processing_inputs(payload: &ProcessPayload, preview: bool) -> Result<ProcessingInputs> {
    Ok(ProcessingInputs {
        output_naming: payload.output_naming.clone().unwrap_or_default(),
        base_output_dir: resolve_output_dir(&payload.output_dir, false)?,
        output_kind: resolve_output_kind(preview),
    })
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

    let plan_signature = plan_signature::build(
        inputs.output_kind,
        collision_policy,
        payload.aac_decoder,
        &jobs,
    );

    Ok(ResolvedProcessingPlan {
        collision_policy,
        plan_signature,
        jobs,
    })
}

pub(super) fn collision_kind_label(output: &ResolvedOutputPlan) -> String {
    output
        .collision
        .as_ref()
        .map(|value| format!("{:?}", value.kind))
        .unwrap_or_else(|| "none".to_string())
}

pub(super) fn collision_path_label(output: &ResolvedOutputPlan) -> String {
    output
        .collision
        .as_ref()
        .and_then(|value| value.conflicting_path.as_ref())
        .map(|value| value.display().to_string())
        .unwrap_or_default()
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
            collision_kind_label(&job.output),
            collision_path_label(&job.output),
        );
    }
}

pub(crate) fn resolve_processing_plan(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview: bool,
    file_info: &FileListInfo,
) -> Result<ResolvedProcessingPlan> {
    let inputs = build_processing_inputs(payload, preview)?;
    let plan = build_processing_plan(payload, metadata, &inputs, file_info)?;
    log_output_plan("preflight", payload, &plan);
    Ok(plan)
}

pub(crate) fn prepare_inspected_execution(
    payload: &ProcessPayload,
    inspected: InspectedProcessingPlan,
) -> Result<ExecutionProcessingPlan> {
    let InspectedProcessingPlan {
        mut plan,
        file_info,
    } = inspected;
    let base_output_dir = resolve_output_dir(&payload.output_dir, true)?;
    let sources = file_info
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let mut ledger = OutputPlanLedger::new();
    for job in &mut plan.jobs {
        job.output = ledger.refresh(&job.output, plan.collision_policy, &sources)?;
        check_companion_pdf_name(payload, job.input_index, &job.output)?;
    }
    plan.plan_signature = plan_signature::build(
        plan.output_kind(),
        plan.collision_policy,
        payload.aac_decoder,
        &plan.jobs,
    );
    log_output_plan("process", payload, &plan);
    enforce_output_plan_review(
        OutputPlanReview {
            expected_signature: payload.preflight_signature.as_deref(),
            current_signature: &plan.plan_signature,
            collision_policy: plan.collision_policy,
        },
        plan.jobs.iter().map(|job| &job.output),
    )?;
    let output_parent_cleanup =
        ensure_output_parent_dirs(&base_output_dir, plan.jobs.iter().map(|job| &job.output))?;
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
        let metadata_outcome = plan_metadata_outcome(&path, file_patch.as_ref())?;
        let metadata_outcome: MetadataOutcomePlan = metadata_outcome;
        let requested_output = output_path_for(
            NamingInputs {
                base_dir: &inputs.base_output_dir,
                naming: &inputs.output_naming,
                anchor: &path,
            },
            &metadata_outcome.naming_metadata,
            payload.audio_requests[index].format().extension(),
        )?;
        let info = title_file_info(file_info, &source_paths)?;
        let audio_plan = crate::audio::resolve_title_audio(
            &payload.audio_requests[index],
            &info,
            inputs.output_kind == OutputKind::Preview,
        )?;
        let output = output_ledger.resolve(
            &requested_output,
            inputs.output_kind,
            collision_policy,
            &validated_input_paths,
        )?;
        check_companion_pdf_name(payload, index, &output)?;
        jobs.push(PlannedProcessingJob {
            input_index: index,
            input_path: path,
            source_paths,
            output,
            source_metadata: metadata_outcome.source_metadata,
            metadata: Some(metadata_outcome.effective_metadata),
            cover_art_passthrough: metadata_outcome.cover_art_passthrough,
            audio_plan,
            metadata_intent: metadata_outcome.write_intent,
        });
    }

    Ok(jobs)
}

fn check_companion_pdf_name(
    payload: &ProcessPayload,
    index: usize,
    output: &ResolvedOutputPlan,
) -> Result<()> {
    if output.kind != OutputKind::Final
        || super::run::supplemental_assets_for_input(payload, index).is_empty()
    {
        return Ok(());
    }
    abb_output_artifact_core::check_supplemental_pdf_name(&output.resolved_path).map_err(Into::into)
}

pub(super) fn title_file_info(all: &FileListInfo, paths: &[PathBuf]) -> Result<FileListInfo> {
    let files = paths
        .iter()
        .map(|path| {
            all.files
                .iter()
                .find(|file| &file.path == path)
                .cloned()
                .ok_or_else(|| AppError::InvalidInput("A title source was not inspected.".into()))
        })
        .collect::<Result<Vec<_>>>()?;
    let info = FileListInfo::from_files(files);
    if info.invalid_count > 0 {
        return Err(AppError::InvalidInput("Every source in an output title must be valid. Remove or replace invalid files before processing.".into()));
    }
    Ok(info)
}

impl ResolvedProcessingPlan {
    pub(crate) fn output_kind(&self) -> OutputKind {
        self.jobs
            .first()
            .map_or(OutputKind::Final, |job| job.output.kind)
    }

    pub(crate) fn to_public(&self) -> ProcessingPreflightPlan {
        let outputs = self
            .jobs
            .iter()
            .map(|job| job.output.to_public(job.input_index, &job.input_path))
            .collect();
        ProcessingPreflightPlan {
            preview: self.output_kind() == OutputKind::Preview,
            collision_policy: self.collision_policy,
            plan_signature: self.plan_signature.clone(),
            outputs,
            audio_plans: self.jobs.iter().map(|job| job.audio_plan.clone()).collect(),
        }
    }
}
