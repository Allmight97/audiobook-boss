use crate::audio;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::{
    plan_metadata_outcome, CoverArtPassthroughPolicy, MetadataOutcomePlan, MetadataOutcomeRequest,
    NamingMetadata,
};
use crate::output_artifact::{
    build_output_path_preview, enforce_output_plan_review, ensure_output_parent_dirs,
    CollisionPolicy, OutputKind, OutputPlanLedger, OutputPlanReview, PlannedOutputAction,
    ResolvedOutputPlan,
};
use crate::processing::{JobType, OutputNamingConfig, ProcessPayload, ProcessingPreflightPlan};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct ProcessingInputs {
    output_naming: OutputNamingConfig,
    base_output_dir: PathBuf,
    preview_seconds: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedProcessingJob {
    pub(crate) input_index: Option<usize>,
    pub(crate) input_path: Option<PathBuf>,
    pub(crate) output: ResolvedOutputPlan,
    pub(crate) metadata: Option<crate::metadata::AudiobookMetadata>,
    pub(crate) cover_art_passthrough: CoverArtPassthroughPolicy,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedProcessingPlan {
    pub(crate) job_type: JobType,
    pub(crate) preview_seconds: Option<f64>,
    pub(crate) collision_policy: CollisionPolicy,
    pub(crate) plan_signature: String,
    pub(crate) jobs: Vec<PlannedProcessingJob>,
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
    create_output_dir: bool,
    preview_seconds: Option<f64>,
) -> Result<ProcessingInputs> {
    Ok(ProcessingInputs {
        output_naming: payload.output_naming.clone().unwrap_or_default(),
        base_output_dir: resolve_output_dir(&payload.output_dir, create_output_dir)?,
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

pub(crate) fn resolve_preflight_plan(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> Result<ProcessingPreflightPlan> {
    let inputs = build_processing_inputs(payload, false, preview_seconds)?;
    let plan = build_processing_plan(payload, metadata, &inputs)?;
    log_output_plan("preflight", payload, &plan);
    Ok(plan.to_public())
}

pub(crate) fn prepare_execution_plan(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> Result<ResolvedProcessingPlan> {
    let inputs = build_processing_inputs(payload, true, preview_seconds)?;
    let plan = build_processing_plan(payload, metadata, &inputs)?;
    log_output_plan("process", payload, &plan);
    enforce_output_plan_review(
        OutputPlanReview {
            expected_signature: payload.preflight_signature.as_deref(),
            current_signature: &plan.plan_signature,
            collision_policy: plan.collision_policy,
        },
        plan.jobs.iter().map(|job| &job.output),
    )?;
    ensure_output_parent_dirs(plan.jobs.iter().map(|job| &job.output))?;
    Ok(plan)
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
    let metadata_outcome = plan_metadata_outcome(MetadataOutcomeRequest {
        input_path: Some(&merge_source_path),
        intent_patch: merge_patch.as_ref(),
    })?;
    let metadata_outcome: MetadataOutcomePlan = metadata_outcome;
    let requested_output = build_requested_output_path(
        &inputs.base_output_dir,
        metadata_outcome.naming_metadata.as_ref(),
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
        metadata: metadata_outcome.effective_metadata,
        cover_art_passthrough: metadata_outcome.cover_art_passthrough,
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
        .map(|input| audio::path_validation::validate_input_audio_path(&PathBuf::from(input)))
        .collect::<Result<_>>()?;

    let mut jobs = Vec::new();
    for (index, path) in validated_input_paths.iter().cloned().enumerate() {
        let input = &payload.input_files[index];
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
            metadata: metadata_outcome.effective_metadata,
            cover_art_passthrough: metadata_outcome.cover_art_passthrough,
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
    use super::{prepare_execution_plan, resolve_preflight_plan};
    use crate::audio::settings_encoder::{
        BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
    };
    use crate::metadata::{CoverArtPassthroughPolicy, MetadataIntentPatch, PatchOp};
    use crate::output_artifact::{CollisionPolicy, OutputKind};
    use crate::processing::{JobType, NamingPreset, OutputNamingConfig, ProcessPayload};
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

    fn copy_audio_fixture(temp_dir: &TempDir, name: &str) -> PathBuf {
        let source_fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("media")
            .join("media_20sec.mp3");
        let input_path = temp_dir.path().join(name);
        std::fs::copy(&source_fixture, &input_path).expect("copy fixture");
        input_path
    }

    #[test]
    fn preflight_plan_only_uses_explicit_positive_preview_request() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = copy_audio_fixture(&temp_dir, "input.m4b");
        let payload = process_payload(|payload| {
            payload.input_files = vec![input_path.to_string_lossy().to_string()];
            payload.output_dir = temp_dir.path().to_string_lossy().to_string();
        });

        let default_plan = resolve_preflight_plan(&payload, None, None).expect("default preflight");
        let preview_plan =
            resolve_preflight_plan(&payload, None, Some(30.0)).expect("preview preflight");
        let negative_plan =
            resolve_preflight_plan(&payload, None, Some(-1.0)).expect("negative preflight");

        assert_eq!(default_plan.preview_seconds, None);
        assert_eq!(preview_plan.preview_seconds, Some(30.0));
        assert_eq!(negative_plan.preview_seconds, None);
        assert_eq!(default_plan.outputs[0].kind, OutputKind::Final);
        assert_eq!(preview_plan.outputs[0].kind, OutputKind::Preview);
        assert_eq!(negative_plan.outputs[0].kind, OutputKind::Final);
    }

    #[test]
    fn batch_preflight_rejects_symlink_before_metadata_projection() {
        let temp_dir = TempDir::new().expect("temp dir");
        let source = temp_dir.path().join("source.m4b");
        std::fs::write(&source, b"not audio, but enough for path validation")
            .expect("write source");
        let symlink = temp_dir.path().join("source-link.m4b");

        #[cfg(unix)]
        std::os::unix::fs::symlink(&source, &symlink).expect("create symlink");
        #[cfg(not(unix))]
        std::os::windows::fs::symlink_file(&source, &symlink).expect("create symlink");

        let payload = process_payload(|payload| {
            payload.input_files = vec![symlink.to_string_lossy().to_string()];
            payload.output_dir = temp_dir.path().to_string_lossy().to_string();
        });

        let err = resolve_preflight_plan(&payload, None, None)
            .expect_err("symlink should be rejected before metadata projection");

        assert!(
            err.to_string().contains("Symlinks are not supported"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn execution_plan_rejects_stale_review_signature() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = copy_audio_fixture(&temp_dir, "input.m4b");
        let payload = process_payload(|payload| {
            payload.input_files = vec![input_path.to_string_lossy().to_string()];
            payload.output_dir = temp_dir.path().to_string_lossy().to_string();
        });
        let preflight = resolve_preflight_plan(&payload, None, None).expect("preflight plan");
        let payload = process_payload(|payload| {
            payload.input_files = vec![input_path.to_string_lossy().to_string()];
            payload.output_dir = temp_dir.path().to_string_lossy().to_string();
            payload.preflight_signature = Some("old-signature".to_string());
        });

        assert_ne!(
            payload.preflight_signature.as_deref(),
            Some(preflight.plan_signature.as_str())
        );

        let err =
            prepare_execution_plan(&payload, None, None).expect_err("stale signature should fail");

        assert!(
            err.to_string().contains("collision state changed"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn execution_plan_rejects_policy_without_signature() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = copy_audio_fixture(&temp_dir, "input.m4b");
        let payload = process_payload(|payload| {
            payload.input_files = vec![input_path.to_string_lossy().to_string()];
            payload.output_dir = temp_dir.path().to_string_lossy().to_string();
            payload.collision_policy = Some(CollisionPolicy::RenameNew);
        });

        let err = prepare_execution_plan(&payload, None, None)
            .expect_err("unreviewed collision policy fails");

        assert!(
            err.to_string()
                .contains("Collision policy selections require a reviewed preflight plan"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn execution_plan_rejects_source_overlap_hard_block_even_with_signature() {
        let temp_dir = TempDir::new().expect("temp dir");
        let alpha_path = copy_audio_fixture(&temp_dir, "alpha.m4b");
        let beta_path = copy_audio_fixture(&temp_dir, "beta.m4b");
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
        let payload = process_payload(|payload| {
            payload.input_files = vec![
                alpha_path.to_string_lossy().to_string(),
                beta_path.to_string_lossy().to_string(),
            ];
            payload.output_dir = temp_dir.path().to_string_lossy().to_string();
            payload.output_naming = Some(OutputNamingConfig {
                preset: NamingPreset::CustomTemplate,
                include_year: false,
                custom_template: Some("{title}".to_string()),
            });
            payload.collision_policy = Some(CollisionPolicy::ReplaceExisting);
        });
        let preflight =
            resolve_preflight_plan(&payload, Some(&metadata), None).expect("preflight plan");
        let payload = process_payload(|payload| {
            payload.input_files = vec![
                alpha_path.to_string_lossy().to_string(),
                beta_path.to_string_lossy().to_string(),
            ];
            payload.output_dir = temp_dir.path().to_string_lossy().to_string();
            payload.output_naming = Some(OutputNamingConfig {
                preset: NamingPreset::CustomTemplate,
                include_year: false,
                custom_template: Some("{title}".to_string()),
            });
            payload.collision_policy = Some(CollisionPolicy::ReplaceExisting);
            payload.preflight_signature = Some(preflight.plan_signature);
        });

        let err = prepare_execution_plan(&payload, Some(&metadata), None)
            .expect_err("hard block should fail");

        assert!(
            err.to_string().contains("targets an input source file"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn preflight_plan_does_not_create_output_parent_dirs() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = copy_audio_fixture(&temp_dir, "input.m4b");
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

        let plan = resolve_preflight_plan(&payload, Some(&metadata), None).expect("build plan");

        assert_eq!(plan.outputs.len(), 1);
        assert!(PathBuf::from(&plan.outputs[0].requested_path).ends_with("Nested/Book.m4b"));
        assert!(
            !planned_parent.exists(),
            "preflight planning should not create output parent directories"
        );
    }

    #[test]
    fn execution_plan_creates_output_parent_dirs() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = copy_audio_fixture(&temp_dir, "input.m4b");
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

        let plan = prepare_execution_plan(&payload, Some(&metadata), None).expect("execution plan");

        assert_eq!(plan.jobs.len(), 1);
        assert!(plan.jobs[0]
            .output
            .requested_path
            .ends_with("Nested/Book.m4b"));
        assert!(
            planned_parent.exists(),
            "execution planning should create output parent directories"
        );
    }

    #[test]
    fn execution_plan_suppresses_passthrough_cover_after_explicit_clear() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = copy_audio_fixture(&temp_dir, "input.m4b");
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

        let plan = prepare_execution_plan(&payload, Some(&metadata), None).expect("execution plan");

        assert_eq!(plan.jobs.len(), 1);
        assert!(
            matches!(
                plan.jobs[0].cover_art_passthrough,
                CoverArtPassthroughPolicy::SuppressAfterExplicitClear
            ),
            "explicit cover clear must not be refilled from source passthrough"
        );
    }
}
