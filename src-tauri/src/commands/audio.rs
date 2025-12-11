use crate::audio;
use crate::audio::file_list::FileListInfo;
use crate::audio::job_registry::JobId;
use crate::audio::settings_encoder::{
    detect_available_encoders, validate_encoder_settings, EncoderAvailability, EncoderSettings,
};
use crate::errors::{AppError, Result};
use chrono::{Datelike, Utc};
use std::path::{Path, PathBuf};
// removed duplicate PathBuf import

/// Validates that all provided file paths exist and are files
/// Accepts an array of file paths and checks file existence
#[tauri::command]
pub fn validate_files(file_paths: Vec<String>) -> Result<String> {
    if file_paths.is_empty() {
        return Err(AppError::InvalidInput(
            "No files provided for validation".to_string(),
        ));
    }

    let mut validated_count = 0;
    let mut validation_errors = Vec::new();

    for path_str in file_paths {
        let path = PathBuf::from(&path_str);

        match audio::path_validation::validate_input_audio_path(&path) {
            Ok(_canonical_path) => {
                validated_count += 1;
            }
            Err(e) => {
                validation_errors.push(e.to_string());
            }
        }
    }

    if !validation_errors.is_empty() {
        return Err(AppError::FileValidation(validation_errors.join("; ")));
    }

    Ok(format!("Successfully validated {validated_count} files"))
}

/// Validates and analyzes a list of audio files
/// Returns comprehensive file information including duration and size
#[tauri::command]
pub fn analyze_audio_files(file_paths: Vec<String>) -> Result<FileListInfo> {
    let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    audio::get_file_list_info(&paths)
}

/// Validates encoder settings (no side effects)
#[tauri::command]
pub fn validate_encoder_settings_cmd(settings: EncoderSettings) -> Result<String> {
    validate_encoder_settings(&settings)?;
    Ok("Encoder settings are valid".to_string())
}

/// Lists runtime encoder availability so the UI can surface guidance.
#[tauri::command]
pub fn list_available_encoders() -> EncoderAvailability {
    log::info!("🔍 list_available_encoders command invoked");
    let result = detect_available_encoders();
    log::info!("🔍 Returning encoder availability: {:?}", result);
    result
}

/// Returns the current maximum concurrent jobs setting
#[tauri::command]
pub fn get_max_concurrent_jobs(registry: tauri::State<'_, crate::ManagedJobRegistry>) -> usize {
    registry.max_concurrent()
}

/// Updates the maximum concurrent jobs setting (requires idle state)
#[tauri::command]
pub async fn set_max_concurrent_jobs(
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    max_concurrent: Option<usize>,
) -> Result<usize> {
    let desired = max_concurrent.unwrap_or(crate::audio::JobRegistry::default_max());
    registry.update_max_concurrent(desired).await
}

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JobType {
    Merge,
    Batch,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilenamePattern {
    TitleYear,
    AuthorTitle,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessV2Payload {
    pub input_files: Vec<String>,
    pub output_dir: String,
    pub settings: EncoderSettings,
    /// Sample rate from frontend (optional, defaults to Auto)
    pub sample_rate: Option<audio::SampleRateConfig>,
    pub job_type: Option<JobType>,
    /// Optional toggle for metadata-based subdirectory generation (default true)
    pub use_subdir_pattern: Option<bool>,
    /// Optional filename pattern (default title-year)
    pub filename_pattern: Option<FilenamePattern>,
}

/// Processes files using the encoder settings payload (`process_audiobook_files_v2` command name retained for compatibility).
///
/// This command now supports parallel batch processing via the JobRegistry.
/// Multiple invocations can run concurrently up to the configured limit.
// EXCEPTION: Function exceeds 100 lines due to job registry integration for parallel processing.
// Refactoring into smaller functions would obscure the linear flow and complicate error handling.
#[allow(clippy::too_many_lines)]
#[tauri::command]
pub async fn process_audiobook_files_v2(
    window: tauri::Window,
    state: tauri::State<'_, crate::ProcessingState>,
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    payload: ProcessV2Payload,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    // Validate encoder settings
    validate_encoder_settings(&payload.settings)?;
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

    let sample_rate = payload.sample_rate.unwrap_or(audio::SampleRateConfig::Auto);
    audio::settings::validate_sample_rate_config(&sample_rate)?;

    let use_subdir_pattern = payload.use_subdir_pattern.unwrap_or(true);
    let filename_pattern = payload
        .filename_pattern
        .unwrap_or(FilenamePattern::TitleYear);
    let base_output_dir = PathBuf::from(&payload.output_dir);

    if !base_output_dir.exists() {
        std::fs::create_dir_all(&base_output_dir).map_err(|e| {
            AppError::FileValidation(format!(
                "Failed to create output directory '{}': {}",
                base_output_dir.display(),
                e
            ))
        })?;
    }

    let job_type = payload.job_type.unwrap_or(JobType::Merge);

    // Legacy processing flag: set true for the duration of this command
    {
        let mut is_processing = state
            .is_processing
            .lock()
            .map_err(|_| AppError::InvalidInput("Failed to acquire processing lock".to_string()))?;
        *is_processing = true;

        let mut is_cancelled = state.is_cancelled.lock().map_err(|_| {
            AppError::InvalidInput("Failed to acquire cancellation lock".to_string())
        })?;
        *is_cancelled = false;
    }

    let result = match job_type {
        JobType::Merge => {
            let paths: Vec<PathBuf> = payload.input_files.iter().map(PathBuf::from).collect();
            let file_info = audio::get_file_list_info(&paths)?;
            let output_path = build_output_path(
                &base_output_dir,
                metadata.as_ref(),
                use_subdir_pattern,
                filename_pattern,
                None,
            )?;
            let resolved_output = resolve_collision(&output_path)?;

            run_processing_job(
                window,
                registry.inner().clone(),
                payload.settings.clone(),
                sample_rate.clone(),
                resolved_output,
                file_info,
                metadata,
                preview_seconds,
            )
            .await
        }
        JobType::Batch => {
            if payload.input_files.is_empty() {
                return Err(AppError::InvalidInput(
                    "No input files provided for batch processing".to_string(),
                ));
            }

            let mut tasks = Vec::new();
            for input in &payload.input_files {
                let path = PathBuf::from(input);
                let output_path = build_output_path(
                    &base_output_dir,
                    metadata.as_ref(),
                    use_subdir_pattern,
                    filename_pattern,
                    Some(&path),
                )?;
                let resolved_output = resolve_collision(&output_path)?;

                let window_cloned = window.clone();
                let registry_cloned = registry.inner().clone();
                let settings_cloned = payload.settings.clone();
                let sr_cloned = sample_rate.clone();
                let md_cloned = metadata.clone();
                let preview_cloned = preview_seconds;

                tasks.push(tokio::spawn(async move {
                    let file_info = audio::get_file_list_info(std::slice::from_ref(&path))?;
                    run_processing_job(
                        window_cloned,
                        registry_cloned,
                        settings_cloned,
                        sr_cloned,
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

            last_ok.ok_or_else(|| {
                AppError::InvalidInput("Batch processing produced no results".to_string())
            })
        }
    };

    // Reset legacy processing state
    {
        let mut is_processing = state
            .is_processing
            .lock()
            .map_err(|_| AppError::InvalidInput("Failed to acquire processing lock".to_string()))?;
        *is_processing = false;
    }

    result
}

/// Processes multiple audio files into a single M4B audiobook
/// Merges files with specified settings and optional metadata
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessCommandResult {
    pub message: String,
    pub preview_file_path: Option<String>,
    pub preview_actual_seconds: Option<f64>,
    pub job_id: String,
}

fn sanitize_component(input: &str) -> String {
    input
        .replace([','], "_")
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
        .trim()
        .to_string()
}

fn default_year() -> i32 {
    Utc::now().year()
}

pub(crate) fn build_output_path(
    base_dir: &Path,
    metadata: Option<&crate::metadata::AudiobookMetadata>,
    use_subdir_pattern: bool,
    filename_pattern: FilenamePattern,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    let title = sanitize_component(metadata.and_then(|m| m.title.as_deref()).unwrap_or_else(
        || {
            source_path
                .and_then(|p| p.file_stem().and_then(|s| s.to_str()))
                .unwrap_or("Untitled")
        },
    ));
    let author = sanitize_component(
        metadata
            .and_then(|m| m.artist.as_deref())
            .unwrap_or("Unknown Author"),
    );
    let series = metadata
        .and_then(|m| m.series.as_deref())
        .map(sanitize_component);
    let year = metadata
        .and_then(|m| m.date.map(|d| d as i32))
        .unwrap_or_else(default_year);

    let mut dir = base_dir.to_path_buf();
    if use_subdir_pattern {
        dir = dir.join(&author);
        if let Some(series) = &series {
            if !series.is_empty() {
                dir = dir.join(series);
            }
        }
        dir = dir.join(&title);
    }

    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            AppError::FileValidation(format!(
                "Failed to create output directory '{}': {}",
                dir.display(),
                e
            ))
        })?;
    }

    let filename = match filename_pattern {
        FilenamePattern::AuthorTitle => format!("{author} - {title}.m4b"),
        FilenamePattern::TitleYear => format!("{title} ({year}).m4b"),
    };

    let full_path = dir.join(filename);
    audio::settings::validate_output_path(&full_path)?;
    Ok(full_path)
}

pub(crate) fn resolve_collision(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Ok(path.to_path_buf());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| AppError::InvalidInput("Invalid output filename".to_string()))?;
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("m4b");

    for idx in 1..=99 {
        let candidate = parent.join(format!("{stem}-{idx}.{ext}"));
        if !candidate.exists() {
            audio::settings::validate_output_path(&candidate)?;
            return Ok(candidate);
        }
    }
    Err(AppError::FileValidation(
        "Could not find collision-free output filename after 99 attempts".to_string(),
    ))
}

#[allow(clippy::too_many_arguments)]
async fn run_processing_job(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    encoder_settings: EncoderSettings,
    sample_rate: audio::SampleRateConfig,
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
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
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

/// Cancels all active audio processing operations
/// Sets the global cancellation flag in the job registry
#[tauri::command]
pub async fn cancel_processing(
    state: tauri::State<'_, crate::ProcessingState>,
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    job_id: Option<String>,
) -> Result<String> {
    if let Some(id) = job_id {
        let parsed = JobId::parse(&id)?;
        registry.cancel_job(parsed).await?;
        Ok(format!("Cancellation requested for job {}", id))
    } else {
        // Cancel all jobs in the registry
        registry.cancel_all();

        // Also set legacy state for backward compatibility
        let mut is_cancelled = state.is_cancelled.lock().map_err(|_| {
            AppError::InvalidInput("Failed to acquire cancellation lock".to_string())
        })?;
        *is_cancelled = true;

        Ok("All processing jobs cancellation requested".to_string())
    }
}

// Removed legacy merge_audio_files command and shell-based implementation during nuclear cleanup.
