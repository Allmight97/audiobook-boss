//! Audio processor module.
//!
//! Staged modules:
//!   - prepare.rs   : validation and workspace setup
//!   - execute.rs   : merge / ffmpeg execution
//!   - finalize.rs  : metadata writing, output move, cleanup
//!   - staging.rs   : app-cache local processing workspace directories
//!   - adapter.rs   : native vs external processor adapter resolution
//!
//! The default path uses in-process ffmpeg-next (`FfmpegNextProcessor`).
//! FDK HE-AAC routes through an external FFmpeg/libfdk_aac adapter when selected.

// Imports for orchestrator function
use crate::audio::cleanup::CleanupGuard;
use crate::audio::file_list::FileListInfo;
use crate::audio::metrics::ProcessingMetrics;
use crate::audio::settings_encoder::EncoderSettings;
use crate::audio::AudioFile;
use crate::errors::Result;
use crate::metadata::{
    extract_passthrough_metadata, prepare_output_cover_art, AudiobookMetadata,
    CoverArtPassthroughPolicy, PassthroughSource,
};
use crate::processing::AudioHandling;
use crate::processing::ProcessingContext;
use std::time::Duration;

// Submodules
mod adapter;
mod encoder;
mod engine;
mod engine_orchestrator;
mod execute;
mod external_fdk;
mod faac_timing;
mod finalize;
mod frame_pipeline;
mod plan;
mod prepare;
mod preserve;
mod preview_state;
mod run_diagnostics;
mod staging;
mod streams;

pub(crate) use streams::assess_preservation;
pub(in crate::audio) use streams::inspect_audio_decoder;
pub use streams::{
    detect_aac_decoder_availability, preferred_aac_decoder_order_labels, AacDecoderAvailability,
};

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
    ) -> Self {
        Self {
            context,
            file_info,
            metadata,
            cover_art_passthrough,
            handling: AudioHandling::Encode,
            metadata_intent: None,
        }
    }

    pub fn with_handling(mut self, handling: AudioHandling) -> Self {
        self.handling = handling;
        self
    }

    pub fn with_metadata_intent(
        mut self,
        metadata_intent: Option<crate::metadata::MetadataIntentPatch>,
    ) -> Self {
        self.metadata_intent = metadata_intent;
        self
    }
}

pub fn validate_audio_engine_inputs(
    encoder_settings: &EncoderSettings,
    file_info: &FileListInfo,
    sample_rate: &crate::audio::SampleRateConfig,
    merge_inputs: bool,
) -> Result<()> {
    adapter::resolve_output_channels(encoder_settings.channels, &file_info.files)?;
    let adapter = adapter::resolve_processor_adapter(encoder_settings)?;
    adapter.validate_inputs(file_info)?;
    if let adapter::ResolvedProcessorAdapter::NativeFfmpegNext { encoder_type } = adapter {
        crate::audio::settings::validate_encoder_sample_rate(
            encoder_type,
            encoder_settings.faac_profile,
            sample_rate,
        )?;
        if matches!(
            encoder_type,
            crate::audio::EncoderType::NativeAac | crate::audio::EncoderType::Faac
        ) {
            if merge_inputs {
                validate_output_target(
                    encoder_settings,
                    encoder_type,
                    sample_rate,
                    &file_info.files,
                )?;
            } else {
                for file in file_info.files.iter().filter(|file| file.is_valid) {
                    validate_output_target(
                        encoder_settings,
                        encoder_type,
                        sample_rate,
                        std::slice::from_ref(file),
                    )?;
                }
            }
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
    let channels = adapter::resolve_output_channels(settings.channels, files)?
        .forced_channels()
        .expect("output channels are resolved");
    let rate = sample_rate
        .explicit_rate()
        .or_else(|| {
            files
                .iter()
                .find(|file| file.is_valid)
                .and_then(|file| file.sample_rate)
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

pub async fn execute_audio_engine(mut request: AudioExecutionRequest) -> Result<String> {
    if request.handling == AudioHandling::Preserve {
        return tokio::task::spawn_blocking(move || {
            preserve::execute_preserved_audio(
                request.context,
                request.file_info,
                request.metadata_intent,
            )
        })
        .await
        .map_err(|error| {
            crate::errors::AppError::General(format!("audio preservation task failed: {error}"))
        })?;
    }
    let mut settings = request.context.required_encoder_settings()?.clone();
    let requested_channels = settings.channels;
    let resolved_channels =
        adapter::resolve_output_channels(requested_channels, &request.file_info.files)?;
    settings.channels = resolved_channels;
    request.context.encoder_settings = Some(settings);
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
    let FileListInfo {
        files,
        selected_decoders,
        ..
    } = request.file_info;
    let encoder_settings = request.context.required_encoder_settings()?;
    let adapter = adapter::resolve_processor_adapter(encoder_settings)?;
    let adapter_label = match &adapter {
        adapter::ResolvedProcessorAdapter::NativeFfmpegNext { .. } => "native_ffmpeg_next",
        adapter::ResolvedProcessorAdapter::ExternalFdk { .. } => "external_fdk",
    };
    let operation_id = request
        .context
        .operation_id
        .as_deref()
        .unwrap_or("foreground");
    let job_id = request.context.job_id.as_deref().unwrap_or("none");
    let input_index = request
        .context
        .input_index
        .map_or_else(|| "none".to_string(), |index| index.to_string());
    log::info!(
        "audio engine adapter: operation_id={operation_id} job_id={job_id} input_index={input_index} kind={adapter_label} requested_encoder={:?}",
        encoder_settings.encoder_type,
    );
    adapter
        .execute(
            request.context,
            files,
            selected_decoders,
            request.metadata,
            request.cover_art_passthrough,
        )
        .await
}

/// Internal workflow state passed between processing stages.
///
/// This replaces ad-hoc tuples and keeps intermediate artifacts cohesive.
/// Fields are intentionally minimal; additional items should only be added if
/// required across stage boundaries to avoid hidden coupling.
pub(crate) struct ProcessingWorkflow {
    /// Session-scoped temporary working directory
    pub(crate) temp_dir: std::path::PathBuf,
    /// Total duration (seconds) of all valid input files (pre‑computed)
    pub(crate) total_duration: f64,
}

impl ProcessingWorkflow {
    /// Constructor helper to keep instantiation explicit at call sites.
    pub(crate) fn new(temp_dir: std::path::PathBuf, total_duration: f64) -> Self {
        Self {
            temp_dir,
            total_duration,
        }
    }
}

/// Native (in-process ffmpeg-next) processing entrypoint; the external FDK
/// adapter bypasses this and owns its own staging/finalize handoff.
///
/// Coordinates the three-stage processing pipeline:
/// 1. Validate & Prepare
/// 2. Execute Processing
/// 3. Finalize Processing
fn process_audiobook_with_context(
    context: ProcessingContext,
    files: Vec<AudioFile>,
    metadata: Option<AudiobookMetadata>,
    cover_art_passthrough: CoverArtPassthroughPolicy,
) -> Result<String> {
    let mut metrics = ProcessingMetrics::new();

    // Stage 1: Validate + Prepare (from prepare module)
    let mut workflow_cleanup = CleanupGuard::new(context.session.id());
    let workflow = prepare::validate_and_prepare(&context, &files, &mut workflow_cleanup)?;
    let workflow_temp_dir = workflow.temp_dir.clone();

    // Extract passthrough metadata (chapters, original cover art) from all valid files.
    let passthrough_sources = passthrough_sources_from_audio_files(&files);
    let passthrough_metadata = cover_art_passthrough
        .apply_to_passthrough(extract_passthrough_metadata(&passthrough_sources).into_option());

    let (effective_metadata, passthrough_metadata) =
        prepare_output_cover_art(metadata, passthrough_metadata)?;

    // Metrics accumulation (estimates)
    for file in &files {
        if file.is_valid {
            if let Some(duration) = file.duration {
                let estimated_bytes =
                    (duration * context.effective_bitrate_kbps() as f64 * 125.0) as usize;
                metrics.update_file_processed(Duration::from_secs_f64(duration), estimated_bytes);
            }
        }
    }

    // Stage 2: Execute (execute module)
    let merged_output = execute::execute_processing(
        &context,
        &workflow,
        &files,
        effective_metadata.as_ref(),
        passthrough_metadata.as_ref(),
    )?;

    // Stage 3: Finalize
    let result = finalize::finalize_processing(
        &context,
        workflow,
        merged_output,
        effective_metadata,
        passthrough_metadata.as_ref(),
    )?;
    let _ = workflow_cleanup.remove_path(&workflow_temp_dir);

    // Suppress full-run metrics summary during preview; log preview-specific stats instead
    if context.preview.is_some() {
        // In preview mode, skip the full metrics summary (not representative).
        // The UI receives precise seconds via the command payload.
        log::info!("Preview run completed (metrics summary suppressed for preview mode)");
    } else {
        log::info!("{}", metrics.format_summary());
    }
    Ok(result)
}
