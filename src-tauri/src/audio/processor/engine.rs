//! ffmpeg-next engine implementation (native in-process path)

use std::path::Path;
use std::sync::Once;
use std::time::Instant;

use ffmpeg_next as ff;

use crate::audio::cleanup::CleanupGuard;
use crate::audio::processor::encoder::{
    append_in_process_encoding_log_best_effort, encoding_log_enabled, InProcessEncoderRunLog,
};
use crate::audio::processor::frame_pipeline::PreviewAction;
use crate::audio::processor::plan::MediaProcessingPlan;
use crate::audio::SampleRateConfig;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::processing::ProcessingContext;

/// ffmpeg-next based processor
pub struct FfmpegNextProcessor;

struct EncodingRunDiagnostics {
    started: Instant,
    opened_encoder: Option<String>,
    encoder_details: Option<String>,
}

impl FfmpegNextProcessor {
    /// Processes a single input file through the decode/resample/encode pipeline
    /// Returns PreviewAction to signal adaptive preview transitions
    pub(crate) fn process_input_file(
        input_path: &Path,
        encoder: &mut super::encoder::EncoderSession,
        file_index: usize,
        ctx: &mut crate::audio::processor::frame_pipeline::FramePipelineCtx,
        accumulator: &mut crate::audio::buffer::SampleAccumulator,
    ) -> Result<PreviewAction> {
        use crate::errors::AppError;

        let input_label = sanitize_path_for_display(input_path);
        log::info!("🎵 Starting to process input file: {}", input_label);

        if ctx.context.is_cancelled() {
            log::warn!(
                "Processing was cancelled before processing file: {}",
                input_label
            );
            ctx.emitter.emit_cancelled("Processing was cancelled");
            return Err(AppError::cancelled());
        }

        // Initialize per-file preview state if adaptive preview is active
        if let Some(ref mut ps) = ctx.preview_state {
            ps.start_new_file(file_index);
            log::info!(
                "Adaptive preview: starting file {} '{}' at pts={}",
                file_index + 1,
                input_label,
                *ctx.running_pts
            );
        }

        log::info!("Setting up decoder and resampler for: {}", input_label);
        let (mut ictx, mut decoder, mut resampler, stream_index) =
            crate::audio::processor::streams::setup_decoder_and_resampler(input_path, encoder)?;
        log::info!(
            "✓ Decoder and resampler setup complete for stream index: {}",
            stream_index
        );

        // Update context indices for this file
        ctx.current_file_index = file_index;
        ctx.current_stream_index = stream_index;
        log::info!(
            "Updated context: file_index={}, stream_index={}",
            file_index,
            stream_index
        );

        log::info!("Processing input packets from: {}", input_label);
        let action = crate::audio::processor::frame_pipeline::process_input_packets(
            &mut ictx,
            &mut decoder,
            encoder,
            &mut resampler,
            ctx,
            accumulator,
        )?;
        log::info!(
            "✓ Input packets processed successfully (action={:?})",
            action
        );

        log::info!("✅ Completed processing file: {}", input_label);
        Ok(action)
    }
}

impl FfmpegNextProcessor {
    /// Executes a media processing plan through the native ffmpeg-next pipeline.
    ///
    /// Synchronous and CPU-bound; the caller offloads this onto a blocking
    /// thread via `spawn_blocking` so it never occupies an async worker.
    pub(crate) fn execute(
        plan: &MediaProcessingPlan,
        context: &ProcessingContext,
        metadata: Option<&crate::metadata::AudiobookMetadata>,
        passthrough: Option<&crate::metadata::PassthroughMetadata>,
    ) -> Result<()> {
        let mut diagnostics = encoding_log_enabled().then(|| EncodingRunDiagnostics {
            started: Instant::now(),
            opened_encoder: None,
            encoder_details: None,
        });
        let result =
            Self::execute_pipeline(plan, context, metadata, passthrough, diagnostics.as_mut());
        if let Some(diagnostics) = diagnostics {
            append_in_process_encoding_run(plan, context, &diagnostics, &result);
        }
        result
    }

    fn execute_pipeline(
        plan: &MediaProcessingPlan,
        context: &ProcessingContext,
        metadata: Option<&crate::metadata::AudiobookMetadata>,
        passthrough: Option<&crate::metadata::PassthroughMetadata>,
        mut diagnostics: Option<&mut EncodingRunDiagnostics>,
    ) -> Result<()> {
        // Initialize FFmpeg (idempotent)
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = ff::init();
        });

        // Setup encoder and output context with metadata
        // Skip chapter passthrough in preview mode (chapters won't align with shortened output)
        let skip_chapter_passthrough = context.preview.is_some();
        // The encoder/output handles drop before cleanup can remove their path.
        let mut cleanup_guard = CleanupGuard::new(context.session.id());
        cleanup_guard.add_path(&plan.output_path);
        let mut enc_ctx = crate::audio::processor::encoder::setup_encoder(
            plan,
            metadata,
            skip_chapter_passthrough,
            passthrough,
        )?;

        if let Some(diagnostics) = diagnostics.as_deref_mut() {
            diagnostics.opened_encoder = Some(enc_ctx.name().to_owned());
        }

        // Validate metadata compatibility if provided
        if let Some(md) = metadata {
            let warnings = crate::metadata::validate_metadata_compatibility(md);
            for warning in warnings {
                log::warn!("Metadata compatibility: {}", warning);
            }
        }

        let emitter = context.new_emitter();
        let result = (|| {
            let mut io = super::engine_orchestrator::InputProcessingContext {
                enc_ctx: &mut enc_ctx,
                emitter: &emitter,
            };
            super::engine_orchestrator::process_input_files(plan, context, &mut io)?;
            if context.is_cancelled() {
                return Err(AppError::cancelled());
            }
            enc_ctx.finish()
        })();
        if let Some(diagnostics) = diagnostics {
            diagnostics.encoder_details = Some(enc_ctx.diagnostics());
        }
        result?;

        // Preserve output on success
        let _ = cleanup_guard.remove_path(&plan.output_path);

        if metadata.is_some() {
            log::info!("Audio processing completed with metadata integration");
        } else {
            log::info!("Audio processing completed without metadata");
        }

        Ok(())
    }
}

fn append_in_process_encoding_run(
    plan: &MediaProcessingPlan,
    context: &ProcessingContext,
    diagnostics: &EncodingRunDiagnostics,
    result: &Result<()>,
) {
    let status = match result {
        Ok(()) => "success",
        Err(AppError::Cancellation(_)) => "cancelled",
        Err(_) => "failed",
    };
    let status_detail = match result {
        Ok(()) => None,
        Err(error) => Some(error.to_string()),
    };

    append_in_process_encoding_log_best_effort(&InProcessEncoderRunLog {
        status,
        status_detail: status_detail.as_deref(),
        elapsed: diagnostics.started.elapsed(),
        opened_encoder: diagnostics.opened_encoder.as_deref(),
        encoder_details: diagnostics.encoder_details.as_deref(),
        encoder_settings: &plan.encoder_settings,
        sample_rate: &plan.sample_rate,
        session_id: context.session.id(),
        job_id: context.job_id.as_deref(),
        input_index: context.input_index,
        operation_kind: format!("{:?}", context.operation_kind),
        preview: context.preview.is_some(),
        temp_output: &plan.output_path,
        input_paths: &plan.input_file_paths,
        target_duration_seconds: plan.total_duration,
    });
}

/// Resolves sample rate after the shared Audio boundary has chosen output channels.
pub(crate) fn resolve_target_audio_params(plan: &MediaProcessingPlan) -> Result<(u32, i32)> {
    let target_channels = plan
        .encoder_settings
        .channels
        .forced_channels()
        .map(i32::from)
        .ok_or_else(|| {
            AppError::General("Output channels were not resolved before encoder setup.".to_string())
        })?;
    let target_sample_rate = match plan.sample_rate {
        SampleRateConfig::Explicit(rate) => rate,
        SampleRateConfig::Auto => probe_first_sample_rate(plan)?,
    };

    Ok((target_sample_rate, target_channels))
}

fn probe_first_sample_rate(plan: &MediaProcessingPlan) -> Result<u32> {
    let first = plan
        .input_file_paths
        .first()
        .ok_or_else(|| AppError::InvalidInput("No input files provided".to_string()))?;
    let inspection = crate::audio::processor::streams::inspect_audio_decoder(first)?;
    log::info!(
        "probe_first_input path={} selected_decoder={} rate={} channels={}",
        sanitize_path_for_display(first),
        inspection.selected_decoder,
        inspection.sample_rate,
        inspection.channels
    );
    Ok(inspection.sample_rate)
}
