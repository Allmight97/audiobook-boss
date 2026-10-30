use crate::audio::cleanup::CleanupGuard;
use crate::audio::file_list::FileListInfo;
use crate::audio::settings_encoder::EncoderSettings;
use crate::audio::AudioFile;
use crate::errors::Result;
use crate::metadata::{
    extract_passthrough_metadata, prepare_output_cover_art, AudiobookMetadata,
    CoverArtPassthroughPolicy, PassthroughSource,
};
use crate::processing::AudioHandling;
use crate::processing::ProcessingContext;

mod encoder;
mod engine;
mod engine_orchestrator;
mod execute;
mod finalize;
mod frame_pipeline;
mod he_timing;
mod plan;
mod prepare;
mod preserve;
mod preserve_merge;

pub(crate) fn validate_preserved_title(files: &[AudioFile]) -> Result<()> {
    for file in files {
        crate::audio::validate_preservation_source(file)?;
    }
    if files.len() > 1 {
        preserve_merge::validate(files)?;
    }
    Ok(())
}
mod preview_state;
pub(in crate::audio) mod resolution;
mod run_diagnostics;
mod staging;
mod streams;

pub(crate) use streams::assess_preservation;
pub(in crate::audio) use streams::inspect_audio_decoder;
pub(crate) use streams::probe_builtin_audio_decoder;

/// A finished title: its success message and display-only warnings, such as a
/// source cover that was left off.
#[derive(Debug)]
pub struct AudioRunOutcome {
    pub message: String,
    pub warnings: Vec<String>,
}

pub struct AudioExecutionRequest {
    context: ProcessingContext,
    file_info: FileListInfo,
    metadata: Option<AudiobookMetadata>,
    cover_art_passthrough: CoverArtPassthroughPolicy,
    handling: AudioHandling,
    metadata_intent: Option<crate::metadata::MetadataIntentPatch>,
}

impl AudioExecutionRequest {
    pub fn new(
        context: ProcessingContext,
        file_info: FileListInfo,
        metadata: Option<AudiobookMetadata>,
        cover_art_passthrough: CoverArtPassthroughPolicy,
        handling: AudioHandling,
    ) -> Self {
        Self {
            context,
            file_info,
            metadata,
            cover_art_passthrough,
            handling,
            metadata_intent: None,
        }
    }

    pub fn with_metadata_intent(
        mut self,
        metadata_intent: Option<crate::metadata::MetadataIntentPatch>,
    ) -> Self {
        self.metadata_intent = metadata_intent;
        self
    }
}

pub(in crate::audio) fn validate_resolved_audio_inputs(
    encoder_type: crate::audio::EncoderType,
    encoder_settings: &EncoderSettings,
    titles: &[FileListInfo],
    sample_rate: &crate::audio::SampleRateConfig,
) -> Result<()> {
    crate::audio::settings_encoder::validate_encoder_settings(encoder_settings)?;
    crate::audio::settings::validate_encoder_sample_rate(
        encoder_type,
        encoder_settings.faac_profile,
        sample_rate,
    )?;
    for title in titles {
        resolution::resolve_output_channels(encoder_settings.channels, &title.files)?;
        if matches!(
            encoder_type,
            crate::audio::EncoderType::NativeAac | crate::audio::EncoderType::Faac
        ) {
            validate_output_target(encoder_settings, encoder_type, sample_rate, &title.files)?;
        }
    }
    Ok(())
}

fn validate_output_target(
    settings: &EncoderSettings,
    encoder: crate::audio::EncoderType,
    sample_rate: &crate::audio::SampleRateConfig,
    files: &[AudioFile],
) -> Result<()> {
    let channels = resolution::resolve_output_channels(settings.channels, files)?
        .forced_channels()
        .expect("output channels are resolved");
    let rate = sample_rate
        .explicit_rate()
        .or_else(|| {
            files
                .iter()
                .find(|file| file.is_valid)
                .and_then(|file| file.sample_rate)
                .map(|rate| {
                    crate::audio::settings::automatic_sample_rate(
                        encoder,
                        settings.faac_profile,
                        rate,
                    )
                })
        })
        .ok_or_else(|| {
            crate::errors::AppError::InvalidInput(format!(
                "Could not determine {encoder} sample rate; choose it explicitly."
            ))
        })?;
    crate::audio::settings::validate_encoder_sample_rate(
        encoder,
        settings.faac_profile,
        &crate::audio::SampleRateConfig::Explicit(rate),
    )?;
    match encoder {
        crate::audio::EncoderType::Faac => {
            encoder::validate_faac_configuration(settings, rate, u32::from(channels))
        }
        _ => crate::audio::settings_encoder::validate_native_target_bitrate(
            settings.bitrate_kbps,
            rate,
            u32::from(channels),
        ),
    }
}

pub(crate) fn processing_workspace_root(cache_dir: &std::path::Path) -> std::path::PathBuf {
    staging::workspace_root_for_app_cache(cache_dir)
}

pub(crate) fn cleanup_abandoned_processing_workspaces(cache_dir: &std::path::Path) -> Result<()> {
    let root = processing_workspace_root(cache_dir);
    staging::cleanup_abandoned_processing_sessions(&root)
}

pub(crate) fn passthrough_sources_from_audio_files(files: &[AudioFile]) -> Vec<PassthroughSource> {
    files
        .iter()
        .map(|file| PassthroughSource {
            path: file.path.clone(),
            duration: file.duration,
            is_valid: file.is_valid,
            chapters: Some(
                file.chapter_plan
                    .as_ref()
                    .map_or_else(|| file.chapters.clone(), |plan| plan.chapters.clone()),
            ),
        })
        .collect()
}

#[expect(clippy::disallowed_methods, reason = "joined by execute_preserved")]
async fn execute_preserved(request: AudioExecutionRequest) -> Result<AudioRunOutcome> {
    tokio::task::spawn_blocking(move || {
        if request.file_info.files.len() > 1 {
            return preserve_merge::execute(
                request.context,
                request.file_info,
                request.metadata,
                request.cover_art_passthrough,
            );
        }
        preserve::execute_preserved_audio(
            request.context,
            request.file_info,
            request.metadata_intent,
        )
        .map(|message| AudioRunOutcome {
            message,
            warnings: Vec::new(),
        })
    })
    .await
    .map_err(|error| {
        crate::errors::AppError::General(format!("audio preservation task failed: {error}"))
    })?
}

#[expect(clippy::disallowed_methods, reason = "joined by execute_audio_engine")]
pub async fn execute_audio_engine(request: AudioExecutionRequest) -> Result<AudioRunOutcome> {
    let mut settings = match request.handling {
        AudioHandling::Encode { settings } => settings,
        AudioHandling::Preserve => return execute_preserved(request).await,
    };
    let requested_channels = settings.channels;
    let resolved_channels =
        resolution::resolve_output_channels(requested_channels, &request.file_info.files)?;
    settings.channels = resolved_channels;
    log::info!(
        "audio output channels: requested={:?} resolved={:?}",
        requested_channels,
        resolved_channels,
    );
    for file in request.file_info.files.iter().filter(|file| file.is_valid) {
        if file.cue_source.as_ref().is_some_and(|cue| {
            matches!(
                cue.status,
                crate::metadata::CueStatus::NeedsConfirmation | crate::metadata::CueStatus::Invalid
            )
        }) {
            return Err(crate::errors::AppError::InvalidInput(
                "Review or ignore the CUE before processing.".into(),
            ));
        }
        if request.file_info.files.len() > 1
            && file.chapter_plan.as_ref().is_some_and(|plan| plan.from_cue)
        {
            return Err(crate::errors::AppError::InvalidInput(
                "CUE chapters require a single-source output.".into(),
            ));
        }
    }
    let FileListInfo { files, .. } = request.file_info;
    let context = request.context;
    let encoder_type = resolution::resolve_linked_encoder(&settings)?;
    log::info!(
        "audio engine encoder: operation_id={} job_id={} input_index={} requested_encoder={:?} resolved_encoder={encoder_type:?}",
        context.operation_id.as_deref().unwrap_or("foreground"),
        context.job_id.as_deref().unwrap_or("none"),
        context
            .input_index
            .map_or_else(|| "none".to_string(), |index| index.to_string()),
        settings.encoder_type,
    );
    settings.resolve_encoder(encoder_type);
    let metadata = request.metadata;
    let cover_art_passthrough = request.cover_art_passthrough;
    // The pipeline (prepare -> encode -> finalize) is synchronous, CPU-bound
    // work. Run it on a blocking thread so it never occupies an async runtime
    // worker; progress emission and the atomic cancel flag work off the runtime.
    tokio::task::spawn_blocking(move || {
        process_audiobook_with_context(context, settings, files, metadata, cover_art_passthrough)
    })
    .await
    .map_err(|join_error| {
        crate::errors::AppError::General(format!("audio processing task failed: {join_error}"))
    })?
}

pub(crate) struct ProcessingWorkflow {
    pub(crate) temp_dir: std::path::PathBuf,
    /// Total duration (seconds) of all valid input files (pre‑computed)
    pub(crate) total_duration: f64,
}

impl ProcessingWorkflow {
    pub(crate) fn new(temp_dir: std::path::PathBuf, total_duration: f64) -> Self {
        Self {
            temp_dir,
            total_duration,
        }
    }
}

fn process_audiobook_with_context(
    context: ProcessingContext,
    settings: EncoderSettings,
    files: Vec<AudioFile>,
    metadata: Option<AudiobookMetadata>,
    cover_art_passthrough: CoverArtPassthroughPolicy,
) -> Result<AudioRunOutcome> {
    let mut workflow_cleanup = CleanupGuard::new(context.session.id());
    let workflow = prepare::validate_and_prepare(&context, &files, &mut workflow_cleanup)?;
    let workflow_temp_dir = workflow.temp_dir.clone();

    let passthrough_sources = passthrough_sources_from_audio_files(&files);
    let passthrough_metadata = cover_art_passthrough
        .apply_to_passthrough(extract_passthrough_metadata(&passthrough_sources).into_option());

    let (effective_metadata, passthrough_metadata, cover_warning) =
        prepare_output_cover_art(metadata, passthrough_metadata, cover_art_passthrough)?;

    let merged_output = execute::execute_processing(
        &context,
        &settings,
        &workflow,
        &files,
        effective_metadata.as_ref(),
        passthrough_metadata.as_ref(),
    )?;

    let message = finalize::finalize_processing(
        &context,
        workflow,
        merged_output,
        effective_metadata,
        passthrough_metadata.as_ref(),
    )?;
    let _ = workflow_cleanup.remove_path(&workflow_temp_dir);
    Ok(AudioRunOutcome {
        message,
        warnings: cover_warning.into_iter().collect(),
    })
}
