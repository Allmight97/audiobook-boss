use crate::audio;
use crate::audio::file_list::FileListInfo;
use crate::audio::output_path::{
    build_output_path, resolve_collision, resolve_collision_with_claimed,
};
use crate::audio::settings_encoder::validate_encoder_settings;
use crate::commands::audio_types::{
    FilenamePattern, JobType, ProcessCommandResult, ProcessV2Payload,
};
use crate::errors::{AppError, Result};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

struct ProcessingInputs {
    sample_rate: audio::SampleRateConfig,
    use_subdir_pattern: bool,
    filename_pattern: FilenamePattern,
    base_output_dir: PathBuf,
    preview_seconds: Option<f64>,
}

pub async fn process_payload(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    payload: ProcessV2Payload,
    metadata: Option<HashMap<String, crate::metadata::AudiobookMetadata>>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    validate_encoder_settings(&payload.settings)?;
    log_encoder_summary(&payload);

    let inputs = ProcessingInputs {
        sample_rate: resolve_sample_rate(&payload)?,
        use_subdir_pattern: payload.use_subdir_pattern.unwrap_or(true),
        filename_pattern: payload
            .filename_pattern
            .unwrap_or(FilenamePattern::TitleYear),
        base_output_dir: ensure_output_dir(&payload.output_dir)?,
        preview_seconds,
    };
    let job_type = payload.job_type.unwrap_or(JobType::Batch);

    match job_type {
        JobType::Merge => dispatch_merge_job(window, registry, &payload, metadata, &inputs).await,
        JobType::Batch => dispatch_batch_jobs(window, registry, &payload, metadata, &inputs).await,
    }
}

fn log_encoder_summary(payload: &ProcessV2Payload) {
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

fn resolve_sample_rate(payload: &ProcessV2Payload) -> Result<audio::SampleRateConfig> {
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
                base_output_dir.display(),
                e
            ))
        })?;
    }
    Ok(base_output_dir)
}

async fn dispatch_merge_job(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    payload: &ProcessV2Payload,
    metadata: Option<HashMap<String, crate::metadata::AudiobookMetadata>>,
    inputs: &ProcessingInputs,
) -> Result<ProcessCommandResult> {
    let paths: Vec<PathBuf> = payload.input_files.iter().map(PathBuf::from).collect();
    let file_info = audio::get_file_list_info(&paths)?;
    let merge_key = payload.input_files.first().map(|s| s.as_str());
    let merge_metadata =
        merge_key.and_then(|key| metadata.as_ref().and_then(|map| map.get(key).cloned()));
    let output_path = build_output_path(
        &inputs.base_output_dir,
        merge_metadata.as_ref(),
        inputs.use_subdir_pattern,
        inputs.filename_pattern,
        None,
    )?;
    let resolved_output = resolve_collision(&output_path)?;

    run_processing_job(
        window,
        registry,
        payload.settings.clone(),
        inputs.sample_rate.clone(),
        None,
        resolved_output,
        file_info,
        merge_metadata,
        inputs.preview_seconds,
    )
    .await
}

async fn dispatch_batch_jobs(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    payload: &ProcessV2Payload,
    metadata: Option<HashMap<String, crate::metadata::AudiobookMetadata>>,
    inputs: &ProcessingInputs,
) -> Result<ProcessCommandResult> {
    if payload.input_files.is_empty() {
        return Err(AppError::InvalidInput(
            "No input files provided for batch processing".to_string(),
        ));
    }

    let mut tasks = Vec::new();
    let mut claimed_paths: HashSet<PathBuf> = HashSet::new();
    for (index, input) in payload.input_files.iter().enumerate() {
        let path = PathBuf::from(input);
        let file_metadata = metadata.as_ref().and_then(|map| map.get(input));
        let output_path = build_output_path(
            &inputs.base_output_dir,
            file_metadata,
            inputs.use_subdir_pattern,
            inputs.filename_pattern,
            Some(&path),
        )?;
        let resolved_output = resolve_collision_with_claimed(&output_path, &claimed_paths)?;
        claimed_paths.insert(resolved_output.clone());

        let window_cloned = window.clone();
        let registry_cloned = registry.clone();
        let settings_cloned = payload.settings.clone();
        let sr_cloned = inputs.sample_rate.clone();
        let md_cloned = file_metadata.cloned();
        let input_index = Some(index);
        let preview_cloned = inputs.preview_seconds;

        tasks.push(tokio::spawn(async move {
            let file_info = audio::get_file_list_info(std::slice::from_ref(&path))?;
            run_processing_job(
                window_cloned,
                registry_cloned,
                settings_cloned,
                sr_cloned,
                input_index,
                resolved_output,
                file_info,
                md_cloned,
                preview_cloned,
            )
            .await
        }));
    }

    let mut last_ok: Option<ProcessCommandResult> = None;
    for task in tasks {
        match task.await {
            Ok(Ok(res)) => last_ok = Some(res),
            Ok(Err(e)) => return Err(e),
            Err(join_err) => {
                return Err(AppError::General(format!(
                    "Batch task join error: {join_err}"
                )))
            }
        }
    }

    last_ok
        .ok_or_else(|| AppError::InvalidInput("Batch processing produced no results".to_string()))
}

#[allow(clippy::too_many_arguments)]
async fn run_processing_job(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    encoder_settings: audio::settings_encoder::EncoderSettings,
    sample_rate: audio::SampleRateConfig,
    input_index: Option<usize>,
    output_path: PathBuf,
    file_info: FileListInfo,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    // Register job with the registry (blocks if at capacity)
    let (job_id, _permit) = registry.register_job().await?;
    log::info!(
        "Job {} started for output: {}",
        job_id,
        output_path.display()
    );

    // Get cancellation checker for this job
    let cancellation_checker = registry.cancellation_checker(job_id).await;

    // Validate derived settings (sample_rate/output path)
    audio::settings::validate_output_path(&output_path)?;

    // Process the audiobook with progress events
    let result = {
        // Create session using job registry for cancellation
        let session =
            audio::session::ProcessingSession::from_job_registry(job_id.0, cancellation_checker);
        let final_output_path = output_path.clone();
        let output_config = audio::OutputConfig::new(final_output_path.clone());
        let mut context = audio::ProcessingContext::new(
            window,
            std::sync::Arc::new(session),
            encoder_settings.clone(),
            sample_rate,
            output_config,
        );

        // Set job_id on context for progress emission
        context.job_id = Some(job_id.to_string());
        context.input_index = input_index;

        // Resolve preview seconds
        let mut preview_seconds_resolved: Option<f64> = None;
        if let Some(sec) = preview_seconds.or_else(|| {
            std::env::var("ABB_PREVIEW_SECONDS")
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
        }) {
            if sec.is_finite() && sec > 0.0 {
                context.preview = Some(crate::audio::context::PreviewConfig::new(sec));
                log::info!("Preview requested: total_seconds={:.3}", sec);
                preview_seconds_resolved = Some(sec);
            }
        }
        let is_preview = context.preview.is_some();

        // Execute processing
        let process_result =
            audio::processor::process_audiobook_with_context(context, file_info.files, metadata)
                .await;

        process_result.map(|msg| {
            let preview_path = if is_preview {
                let final_output = &final_output_path;
                let parent = final_output
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."));
                let stem = final_output
                    .file_stem()
                    .map(|s| s.to_string_lossy())
                    .unwrap_or_else(|| Cow::from("output"));
                let p = parent.join(format!("{}.preview.m4b", stem));
                Some(p.display().to_string())
            } else {
                None
            };
            (msg, preview_path, preview_seconds_resolved)
        })
    };

    // Complete or fail the job in registry
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

    Ok(ProcessCommandResult {
        message,
        preview_file_path: preview_path_opt,
        preview_actual_seconds: preview_seconds_used,
        job_id: job_id.to_string(),
    })
}
