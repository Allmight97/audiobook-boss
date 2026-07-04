//! ffmpeg-next engine implementation (native in-process path)

use std::path::Path;
use std::sync::Once;

use ffmpeg_next as ff;

use crate::audio::cleanup::CleanupGuard;
use crate::audio::processor::frame_pipeline::PreviewAction;
use crate::audio::processor::plan::MediaProcessingPlan;
use crate::audio::SampleRateConfig;
use crate::errors::{sanitize_path_for_display, Result};
use crate::processing::ProcessingContext;

/// ffmpeg-next based processor
pub struct FfmpegNextProcessor;

impl FfmpegNextProcessor {
    /// Processes a single input file through the decode/resample/encode pipeline
    /// Returns PreviewAction to signal adaptive preview transitions
    pub(crate) fn process_input_file(
        input_path: &Path,
        encoder: &mut ff::codec::encoder::audio::Encoder,
        output_context: &mut ff::format::context::Output,
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
            output_context,
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
        // Initialize FFmpeg (idempotent)
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = ff::init();
        });

        // Setup encoder and output context with metadata
        // Skip chapter passthrough in preview mode (chapters won't align with shortened output)
        let skip_chapter_passthrough = context.preview.is_some();
        let (mut octx, mut enc_ctx, ost_index, ost_time_base, target_sample_rate, frame_plan) =
            crate::audio::processor::encoder::setup_encoder(
                plan,
                metadata,
                skip_chapter_passthrough,
                passthrough,
            )?;

        // Validate metadata compatibility if provided
        if let Some(md) = metadata {
            let warnings = crate::metadata::validate_metadata_compatibility(md);
            for warning in warnings {
                log::warn!("Metadata compatibility: {}", warning);
            }
        }

        // Ensure partial outputs are removed on failure or cancellation
        let mut cleanup_guard = CleanupGuard::new(context.session.id());
        cleanup_guard.add_path(&plan.output_path);

        let emitter = context.new_emitter();
        let mut io = super::engine_orchestrator::InputProcessingContext {
            enc_ctx: &mut enc_ctx,
            octx: &mut octx,
            ost_index,
            ost_time_base,
            target_sample_rate,
            samples_per_frame: frame_plan.samples_per_frame(),
            emitter: &emitter,
        };
        if super::engine_orchestrator::process_input_files(plan, context, &mut io)? {
            log::info!("✓ Flushed final accumulator tail frame");
        }

        // Finalize encoding (same path for full encode or preview early-stop)
        log::info!("🏁 Starting encoding finalization...");
        crate::audio::processor::encoder::finalize_encoding(
            &mut enc_ctx,
            &mut octx,
            ost_index,
            ost_time_base,
        )?;
        log::info!("✓ Encoding finalization completed successfully");

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

/// Helper to determine the target sample rate and channel count based on plan settings and input probe.
pub(crate) fn resolve_target_audio_params(plan: &MediaProcessingPlan) -> Result<(u32, i32)> {
    let needs_probe_for_rate = matches!(plan.sample_rate, SampleRateConfig::Auto);
    let needs_probe_for_channels = matches!(
        plan.encoder_settings.channels,
        crate::audio::settings_encoder::ChannelConfig::Auto
    );

    let probe = if needs_probe_for_rate || needs_probe_for_channels {
        Some(probe_first_input(plan)?)
    } else {
        None
    };

    let sampled_rate = probe.map(|(rate, _)| rate);
    let sampled_channels = probe.map(|(_, ch)| ch);

    use crate::errors::AppError;

    let target_sample_rate = match plan.sample_rate {
        SampleRateConfig::Explicit(rate) => rate,
        SampleRateConfig::Auto => sampled_rate.ok_or_else(|| {
            AppError::InvalidInput(
                "Could not determine sample rate from inputs; please specify an explicit sample rate"
                    .to_string(),
            )
        })?,
    };

    let target_channels = plan
        .encoder_settings
        .channels
        .forced_channels()
        .map(|c| c as i32)
        .or(sampled_channels)
        .ok_or_else(|| {
            AppError::InvalidInput(
                "Could not determine channel count from inputs; please specify channels explicitly"
                    .to_string(),
            )
        })?;

    Ok((target_sample_rate, target_channels))
}

pub(crate) fn probe_first_input(plan: &MediaProcessingPlan) -> Result<(u32, i32)> {
    use crate::errors::AppError;

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
    Ok((inspection.sample_rate, (inspection.channels as i32).max(1)))
}
