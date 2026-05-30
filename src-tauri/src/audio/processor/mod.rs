//! Audio processor module.
//!
//! Staged modules:
//!   - prepare.rs   : validation and workspace setup
//!   - execute.rs   : merge / ffmpeg execution
//!   - finalize.rs  : metadata writing, output move, cleanup
//!   - staging.rs   : destination-adjacent temp output directories
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
use crate::metadata::passthrough::merge_passthrough_cover_art;
use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy};
use crate::processing::{ProcessingContext, ProcessingStage, ProgressReporter};
use std::time::Duration;

// Submodules
mod adapter;
mod encoder;
mod engine;
mod execute;
mod external_fdk;
mod finalize;
mod frame_pipeline;
mod plan;
mod prepare;
mod preview_state;
mod selection;
mod staging;
mod streams;

pub(in crate::audio) use streams::inspect_audio_decoder;
pub use streams::{
    detect_aac_decoder_availability, preferred_aac_decoder_order_labels, AacDecoderAvailability,
};

pub struct AudioExecutionRequest {
    context: ProcessingContext,
    file_info: FileListInfo,
    metadata: Option<AudiobookMetadata>,
    cover_art_passthrough: CoverArtPassthroughPolicy,
    encoder_settings: EncoderSettings,
}

impl AudioExecutionRequest {
    pub fn new(
        context: ProcessingContext,
        file_info: FileListInfo,
        metadata: Option<AudiobookMetadata>,
        cover_art_passthrough: CoverArtPassthroughPolicy,
        encoder_settings: EncoderSettings,
    ) -> Self {
        Self {
            context,
            file_info,
            metadata,
            cover_art_passthrough,
            encoder_settings,
        }
    }
}

pub fn validate_audio_engine_inputs(
    encoder_settings: &EncoderSettings,
    file_info: &FileListInfo,
) -> Result<()> {
    let adapter = adapter::resolve_processor_adapter(encoder_settings)?;
    adapter.validate_inputs(file_info)
}

pub async fn execute_audio_engine(request: AudioExecutionRequest) -> Result<String> {
    let FileListInfo {
        files,
        selected_decoders,
        ..
    } = request.file_info;
    let adapter = adapter::resolve_processor_adapter(&request.encoder_settings)?;
    log::info!(
        "audio engine adapter: kind={:?} requested_encoder={:?}",
        adapter.kind(),
        request.encoder_settings.encoder_type,
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

    /// Accessor helpers (kept small / inline for clarity).
    pub(crate) fn total_duration(&self) -> f64 {
        self.total_duration
    }
}

/// Main audiobook processing entrypoint.
///
/// Coordinates the three-stage processing pipeline:
/// 1. Validate & Prepare
/// 2. Execute Processing
/// 3. Finalize Processing
async fn process_audiobook_with_context(
    context: ProcessingContext,
    files: Vec<AudioFile>,
    metadata: Option<AudiobookMetadata>,
    cover_art_passthrough: CoverArtPassthroughPolicy,
) -> Result<String> {
    let mut reporter = ProgressReporter::new(files.len());
    let mut metrics = ProcessingMetrics::new();

    // Stage 1: Validate + Prepare (from prepare module)
    reporter.set_stage(ProcessingStage::Analyzing);
    let workflow = prepare::validate_and_prepare(&context, &files)?;
    let workflow_temp_dir = workflow.temp_dir.clone();
    let mut workflow_cleanup = CleanupGuard::new(context.session.id());
    workflow_cleanup.add_path(&workflow_temp_dir);

    // Extract passthrough metadata (chapters, original cover art) from all valid files.
    let passthrough_metadata = cover_art_passthrough.apply_to_passthrough(
        crate::metadata::passthrough::extract_passthrough_metadata(&files).into_option(),
    );

    let effective_metadata = merge_passthrough_cover_art(metadata, passthrough_metadata.as_ref());

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
        &mut reporter,
    )
    .await?;

    // Stage 3: Finalize
    let result = finalize::finalize_processing(
        &context,
        workflow,
        merged_output,
        effective_metadata,
        passthrough_metadata,
        &mut reporter,
    )
    .await?;
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
