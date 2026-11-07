use crate::audio::FileListInfo;
use crate::errors::{AppError, Result};
use crate::metadata::{plan_metadata_outcome, CoverArtPassthroughPolicy, MetadataOutcomePlan};
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
    preview: bool,
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
    pub(crate) preview: bool,
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

fn build_plan_signature(
    preview: bool,
    collision_policy: CollisionPolicy,
    aac_decoder: Option<crate::audio::AacDecoder>,
    jobs: &[PlannedProcessingJob],
) -> String {
    let mut lines = vec![
        format!("preview={preview}"),
        format!("collision_policy={collision_policy:?}"),
        format!("aac_decoder={aac_decoder:?}"),
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
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
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
            if job.audio_plan.handling.preserves() {
                "Preserve"
            } else {
                "Encode"
            },
        ));
    }

    lines.join("\n")
}

fn build_processing_inputs(
    payload: &ProcessPayload,
    allow_missing_output_dir: bool,
    preview: bool,
) -> Result<ProcessingInputs> {
    Ok(ProcessingInputs {
        output_naming: payload.output_naming.clone().unwrap_or_default(),
        base_output_dir: resolve_output_dir(&payload.output_dir, allow_missing_output_dir)?,
        preview,
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

    let plan_signature =
        build_plan_signature(inputs.preview, collision_policy, payload.aac_decoder, &jobs);

    Ok(ResolvedProcessingPlan {
        preview: inputs.preview,
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

pub(crate) fn resolve_processing_plan(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview: bool,
    file_info: &FileListInfo,
) -> Result<ResolvedProcessingPlan> {
    let inputs = build_processing_inputs(payload, false, preview)?;
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
    let inputs = build_processing_inputs(payload, true, plan.preview)?;
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
    plan.plan_signature = build_plan_signature(
        plan.preview,
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
        let metadata_outcome = plan_metadata_outcome(&path, file_patch.as_ref())?;
        let metadata_outcome: MetadataOutcomePlan = metadata_outcome;
        let requested_output = build_output_path_preview(
            &inputs.base_output_dir,
            Some(&metadata_outcome.naming_metadata),
            inputs.output_naming.clone(),
            Some(&path),
            payload.audio_requests[index].format().extension(),
        )?;
        let info = title_file_info(file_info, &source_paths)?;
        let audio_plan = crate::audio::resolve_title_audio(
            &payload.audio_requests[index],
            &info,
            inputs.preview,
        )?;
        let output = output_ledger.resolve(
            &requested_output,
            resolve_output_kind(inputs.preview),
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
    pub(crate) fn to_public(&self) -> ProcessingPreflightPlan {
        let outputs = self
            .jobs
            .iter()
            .map(|job| job.output.to_public(job.input_index, &job.input_path))
            .collect();
        ProcessingPreflightPlan {
            preview: self.preview,
            collision_policy: self.collision_policy,
            plan_signature: self.plan_signature.clone(),
            outputs,
            audio_plans: self.jobs.iter().map(|job| job.audio_plan.clone()).collect(),
        }
    }
}
