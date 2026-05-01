use crate::audio;
use crate::audio::output_path::{
    action_requires_output_write, build_output_path_preview, plan_is_hard_block, CollisionPolicy,
    OutputKind, OutputPlanLedger, PlannedOutputAction, ResolvedOutputPlan,
};
use crate::commands::audio_types::{
    JobType, OutputNamingConfig, ProcessPayload, ProcessingPreflightPlan,
};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::run::{
    resolve_effective_processing_metadata, resolve_naming_metadata, validate_batch_input_path,
};

pub(crate) struct ProcessingInputs {
    pub(crate) output_naming: OutputNamingConfig,
    pub(crate) base_output_dir: PathBuf,
    pub(crate) preview_seconds: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedProcessingJob {
    pub(crate) input_index: Option<usize>,
    pub(crate) input_path: Option<PathBuf>,
    pub(crate) output: ResolvedOutputPlan,
    pub(crate) metadata: Option<crate::metadata::AudiobookMetadata>,
    pub(crate) allow_passthrough_cover_art: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedProcessingPlan {
    pub(crate) job_type: JobType,
    pub(crate) preview_seconds: Option<f64>,
    pub(crate) collision_policy: CollisionPolicy,
    pub(crate) plan_signature: String,
    pub(crate) jobs: Vec<PlannedProcessingJob>,
}

pub(crate) fn resolve_output_dir(output_dir: &str, create_if_missing: bool) -> Result<PathBuf> {
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

fn action_is_hard_block(job: &PlannedProcessingJob) -> bool {
    plan_is_hard_block(&job.output)
}

pub(crate) fn ensure_output_parent_dirs(plan: &ResolvedProcessingPlan) -> Result<()> {
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

pub(crate) fn enforce_plan_review(
    payload: &ProcessPayload,
    plan: &ResolvedProcessingPlan,
) -> Result<()> {
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

pub(crate) fn build_processing_plan(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    inputs: &ProcessingInputs,
) -> Result<ResolvedProcessingPlan> {
    let job_type = payload.job_type.unwrap_or(JobType::Batch);
    let collision_policy = resolve_collision_policy(payload);
    let output_kind = resolve_output_kind(inputs.preview_seconds);
    let mut output_ledger = OutputPlanLedger::new();
    let mut jobs = Vec::new();

    match job_type {
        JobType::Merge => jobs.push(build_merge_processing_job(
            payload,
            metadata,
            inputs,
            output_kind,
            collision_policy,
            &mut output_ledger,
        )?),
        JobType::Batch => jobs.extend(build_batch_processing_jobs(
            payload,
            metadata,
            inputs,
            output_kind,
            collision_policy,
            &mut output_ledger,
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

pub(crate) fn log_output_plan(
    phase: &str,
    payload: &ProcessPayload,
    plan: &ResolvedProcessingPlan,
) {
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
    output_ledger: &mut OutputPlanLedger,
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
    let allow_passthrough_cover_art = !merge_patch
        .as_ref()
        .is_some_and(crate::metadata::MetadataIntentPatch::clears_cover_art);
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
    let output = output_ledger.resolve(
        &requested_output,
        output_kind,
        collision_policy,
        &source_paths,
    )?;

    Ok(PlannedProcessingJob {
        input_index: None,
        input_path: None,
        output,
        metadata: merge_metadata,
        allow_passthrough_cover_art,
    })
}

fn build_batch_processing_jobs(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    inputs: &ProcessingInputs,
    output_kind: OutputKind,
    collision_policy: CollisionPolicy,
    output_ledger: &mut OutputPlanLedger,
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
        let allow_passthrough_cover_art = !file_patch
            .as_ref()
            .is_some_and(crate::metadata::MetadataIntentPatch::clears_cover_art);
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
        let output = output_ledger.resolve(
            &requested_output,
            output_kind,
            collision_policy,
            &validated_input_paths,
        )?;
        jobs.push(PlannedProcessingJob {
            input_index: Some(index),
            input_path: Some(path),
            output,
            metadata: effective_metadata,
            allow_passthrough_cover_art,
        });
    }

    Ok(jobs)
}

impl ResolvedProcessingPlan {
    pub(crate) fn to_public(&self) -> ProcessingPreflightPlan {
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

#[cfg(test)]
mod tests {
    use super::{
        build_processing_plan, enforce_plan_review, ProcessingInputs, ResolvedProcessingPlan,
    };
    use crate::audio::output_path::{
        CollisionPolicy, OutputCollision, OutputCollisionKind, OutputKind, PlannedOutputAction,
        ResolvedOutputPlan,
    };
    use crate::audio::settings_encoder::{
        BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
    };
    use crate::commands::audio_types::{JobType, NamingPreset, OutputNamingConfig, ProcessPayload};
    use crate::metadata::{MetadataIntentPatch, PatchOp};
    use std::collections::HashMap;
    use std::path::PathBuf;
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
            output_dir: "/tmp/out".to_string(),
            settings: encoder_settings(),
            external_toolchain: None,
            sample_rate: None,
            job_type: Some(JobType::Batch),
            output_naming: None,
            collision_policy: None,
            preflight_signature: None,
        };
        overrides(&mut payload);
        payload
    }

    fn resolved_plan(plan_signature: &str) -> ResolvedProcessingPlan {
        ResolvedProcessingPlan {
            job_type: JobType::Batch,
            preview_seconds: None,
            collision_policy: CollisionPolicy::Fail,
            plan_signature: plan_signature.to_string(),
            jobs: Vec::new(),
        }
    }

    #[test]
    fn enforce_plan_review_rejects_stale_signature() {
        let payload = process_payload(|payload| {
            payload.preflight_signature = Some("old-signature".to_string());
        });
        let plan = resolved_plan("new-signature");

        let err = enforce_plan_review(&payload, &plan).expect_err("stale signature should fail");

        assert!(
            err.to_string().contains("collision state changed"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn enforce_plan_review_rejects_policy_without_signature() {
        let payload = process_payload(|payload| {
            payload.collision_policy = Some(CollisionPolicy::RenameNew);
        });
        let plan = resolved_plan("review-signature");

        let err =
            enforce_plan_review(&payload, &plan).expect_err("unreviewed collision policy fails");

        assert!(
            err.to_string()
                .contains("Collision policy selections require a reviewed preflight plan"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn enforce_plan_review_rejects_source_overlap_hard_block_even_with_signature() {
        let payload = process_payload(|payload| {
            payload.preflight_signature = Some("review-signature".to_string());
        });
        let source_path = PathBuf::from("/books/input.m4b");
        let plan = ResolvedProcessingPlan {
            job_type: JobType::Batch,
            preview_seconds: None,
            collision_policy: CollisionPolicy::Fail,
            plan_signature: "review-signature".to_string(),
            jobs: vec![super::PlannedProcessingJob {
                input_index: Some(0),
                input_path: Some(source_path.clone()),
                output: ResolvedOutputPlan {
                    kind: OutputKind::Final,
                    requested_path: source_path.clone(),
                    resolved_path: source_path.clone(),
                    rename_candidate: None,
                    collision: Some(OutputCollision {
                        kind: OutputCollisionKind::SourceDestinationOverlap,
                        conflicting_path: Some(source_path),
                        detail: Some("Output path resolves to an input source file.".to_string()),
                    }),
                    action: PlannedOutputAction::ReviewRequired,
                },
                metadata: None,
                allow_passthrough_cover_art: true,
            }],
        };

        let err = enforce_plan_review(&payload, &plan).expect_err("hard block should fail");

        assert!(
            err.to_string().contains("targets an input source file"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn build_processing_plan_does_not_create_output_parent_dirs() {
        let temp_dir = TempDir::new().expect("temp dir");
        let source_fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("media")
            .join("media_20sec.mp3");
        let input_path = temp_dir.path().join("input.m4b");
        std::fs::copy(&source_fixture, &input_path).expect("copy fixture");
        let mut metadata = HashMap::new();
        metadata.insert(
            input_path.to_string_lossy().to_string(),
            MetadataIntentPatch {
                title: PatchOp::Set("Book".to_string()),
                ..Default::default()
            },
        );
        let planned_parent = temp_dir.path().join("Nested");
        let payload = process_payload(|payload| {
            payload.input_files = vec![input_path.to_string_lossy().to_string()];
            payload.output_dir = temp_dir.path().to_string_lossy().to_string();
            payload.output_naming = Some(OutputNamingConfig {
                preset: NamingPreset::CustomTemplate,
                include_year: false,
                custom_template: Some("Nested/{title}".to_string()),
            });
        });
        let inputs = ProcessingInputs {
            output_naming: payload.output_naming.clone().unwrap_or_default(),
            base_output_dir: temp_dir.path().to_path_buf(),
            preview_seconds: None,
        };

        let plan = build_processing_plan(&payload, Some(&metadata), &inputs).expect("build plan");

        assert_eq!(plan.jobs.len(), 1);
        assert!(plan.jobs[0]
            .output
            .requested_path
            .ends_with("Nested/Book.m4b"));
        assert!(
            !planned_parent.exists(),
            "preflight planning should not create output parent directories"
        );
    }

    #[test]
    fn build_processing_plan_suppresses_passthrough_cover_after_explicit_clear() {
        let temp_dir = TempDir::new().expect("temp dir");
        let source_fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("media")
            .join("media_20sec.mp3");
        let input_path = temp_dir.path().join("input.m4b");
        std::fs::copy(&source_fixture, &input_path).expect("copy fixture");
        let mut metadata = HashMap::new();
        metadata.insert(
            input_path.to_string_lossy().to_string(),
            MetadataIntentPatch {
                cover_art: PatchOp::Clear,
                ..Default::default()
            },
        );
        let payload = process_payload(|payload| {
            payload.input_files = vec![input_path.to_string_lossy().to_string()];
            payload.output_dir = temp_dir.path().to_string_lossy().to_string();
        });
        let inputs = ProcessingInputs {
            output_naming: payload.output_naming.clone().unwrap_or_default(),
            base_output_dir: temp_dir.path().to_path_buf(),
            preview_seconds: None,
        };

        let plan = build_processing_plan(&payload, Some(&metadata), &inputs).expect("build plan");

        assert_eq!(plan.jobs.len(), 1);
        assert!(
            !plan.jobs[0].allow_passthrough_cover_art,
            "explicit cover clear must not be refilled from source passthrough"
        );
    }
}
