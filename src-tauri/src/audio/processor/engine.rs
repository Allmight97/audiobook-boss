//! ffmpeg-next engine implementation (single-engine path)

use std::path::Path;
use std::sync::Once;

use ffmpeg_next as ff;

use crate::audio::buffer::SampleAccumulator;
use crate::audio::cleanup::CleanupGuard;
use crate::audio::processor::frame_pipeline::{FramePipelineCtx, PreviewAction, PreviewState};
use crate::audio::processor::plan::{MediaProcessingPlan, MediaProcessor};
use crate::audio::{ProcessingContext, SampleRateConfig};
use crate::errors::Result;

/// ffmpeg-next based processor
pub struct FfmpegNextProcessor;

impl FfmpegNextProcessor {
    /// Processes a single input file through the decode/resample/encode pipeline
    /// Returns PreviewAction to signal adaptive preview transitions
    fn process_input_file(
        input_path: &Path,
        encoder: &mut ff::codec::encoder::audio::Encoder,
        output_context: &mut ff::format::context::Output,
        file_index: usize,
        ctx: &mut FramePipelineCtx,
        accumulator: &mut SampleAccumulator,
    ) -> Result<PreviewAction> {
        use crate::errors::AppError;

        log::info!(
            "🎵 Starting to process input file: {}",
            input_path.display()
        );

        if ctx.context.is_cancelled() {
            log::warn!(
                "Processing was cancelled before processing file: {}",
                input_path.display()
            );
            ctx.emitter.emit_cancelled("Processing was cancelled");
            return Err(AppError::InvalidInput("Processing was cancelled".into()));
        }

        // Initialize per-file preview state if adaptive preview is active
        let file_name = input_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        if let Some(ref mut ps) = ctx.preview_state {
            ps.start_new_file(file_index, file_name, *ctx.running_pts);
            log::info!(
                "Adaptive preview: starting file {} '{}' at pts={}",
                file_index + 1,
                file_name,
                *ctx.running_pts
            );
        }

        log::info!(
            "Setting up decoder and resampler for: {}",
            input_path.display()
        );
        let (mut ictx, mut decoder, mut resampler, stream_index) =
            crate::audio::processor::streams::setup_decoder_and_resampler(input_path, encoder)?;
        log::info!(
            "✓ Decoder and resampler setup complete for stream index: {}",
            stream_index
        );

        // Update context indices for this file (P1.4) then process packets
        ctx.current_file_index = file_index;
        ctx.current_stream_index = stream_index;
        log::info!(
            "Updated context: file_index={}, stream_index={}",
            file_index,
            stream_index
        );

        log::info!("Processing input packets from: {}", input_path.display());
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

        // Flush decoder for this input (noop currently)
        log::info!("✓ Decoder frames flushed successfully (skipped for simplicity)");

        log::info!("✅ Completed processing file: {}", input_path.display());
        Ok(action)
    }
}

impl MediaProcessor for FfmpegNextProcessor {
    // EXCEPTION: Orchestration function for media processing pipeline requires cohesive control flow.
    // Breaking into smaller pieces would fragment related preview/encoding logic unnecessarily.
    #[allow(clippy::too_many_lines)]
    fn execute<'a>(
        &'a self,
        plan: &'a MediaProcessingPlan,
        context: &'a ProcessingContext,
        metadata: Option<&'a crate::metadata::AudiobookMetadata>,
        passthrough: Option<&'a crate::metadata::passthrough::PassthroughMetadata>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        // Initialize FFmpeg (idempotent)
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = ff::init();
        });

        Box::pin(async move {
            // Setup encoder and output context with metadata
            // Skip chapter passthrough in preview mode (chapters won't align with shortened output)
            let skip_chapter_passthrough = context.preview.is_some();
            let (mut octx, mut enc_ctx, ost_index, ost_time_base, target_sample_rate) =
                crate::audio::processor::encoder::setup_encoder(
                    plan,
                    metadata,
                    skip_chapter_passthrough,
                    passthrough,
                )?;

            // Validate metadata compatibility if provided (now active post-legacy purge)
            if let Some(md) = metadata {
                let warnings = crate::metadata::validate_metadata_compatibility(md);
                for warning in warnings {
                    log::warn!("Metadata compatibility: {}", warning);
                }
            }

            // Ensure partial outputs are removed on failure or cancellation
            let mut cleanup_guard = CleanupGuard::new(context.session.id());
            cleanup_guard.add_path(&plan.output_path);

            // Initialize processing state
            let mut running_pts: i64 = 0; // in encoder time_base units
            let mut last_emit = std::time::Instant::now();
            let emitter = crate::audio::progress::ProgressEmitter::new(context.window.clone());

            // Construct context struct to reduce parameter passing
            let mut input_samples_total: u64 = 0;
            let mut encoded_samples_total: u64 = 0;
            let mut preview_early_stop = false;
            // Initialize adaptive preview state if preview mode is enabled
            let file_count = plan.input_file_paths.len();
            let mut preview_state_storage = context.preview.as_ref().map(|cfg| {
                let per_file_sec = cfg.per_file_seconds(file_count);
                log::info!(
                    "Adaptive preview: {} files × {:.3}s/file = {:.3}s total",
                    file_count,
                    per_file_sec,
                    per_file_sec * file_count as f64
                );
                PreviewState::new(file_count, per_file_sec)
            });

            let mut ctx = FramePipelineCtx {
                context,
                emitter: &emitter,
                total_duration: plan.total_duration.max(0.001),
                total_files: file_count,
                target_sample_rate,
                output_stream_index: ost_index,
                output_time_base: ost_time_base,
                running_pts: &mut running_pts,
                last_emit: &mut last_emit,
                current_file_index: 0,
                current_stream_index: 0,
                input_samples_total: &mut input_samples_total,
                encoded_samples_total: &mut encoded_samples_total,
                early_stop: &mut preview_early_stop,
                preview_state: preview_state_storage.as_mut(),
            };

            // Process each input file
            log::info!(
                "Starting audio processing for {} input files",
                plan.input_file_paths.len()
            );
            let mut accumulator = SampleAccumulator::new(
                enc_ctx.channel_layout().channels() as usize,
                enc_ctx.frame_size() as usize,
                enc_ctx.rate(),
                enc_ctx.channel_layout(),
                enc_ctx.format(),
            );
            for (idx, in_path) in plan.input_file_paths.iter().enumerate() {
                log::info!(
                    "Processing input file {}/{}: {}",
                    idx + 1,
                    plan.input_file_paths.len(),
                    in_path.display()
                );
                let action = Self::process_input_file(
                    in_path,
                    &mut enc_ctx,
                    &mut octx,
                    idx,
                    &mut ctx,
                    &mut accumulator,
                )?;
                log::info!(
                    "✓ Completed processing input file {}/{}",
                    idx + 1,
                    plan.input_file_paths.len()
                );

                // Handle adaptive preview actions
                match action {
                    PreviewAction::StopAll => {
                        log::info!(
                            "Adaptive preview complete after file {}; stopping further input processing",
                            idx + 1
                        );
                        break;
                    }
                    PreviewAction::NextFile => {
                        log::info!(
                            "Adaptive preview: file {} excerpt complete, continuing to next file",
                            idx + 1
                        );
                        // Continue to next file in loop
                    }
                    PreviewAction::Continue => {
                        // Check legacy early_stop flag for backward compatibility
                        if *ctx.early_stop {
                            log::info!(
                                "Preview early-stop engaged after file {}; stopping further input processing",
                                idx + 1
                            );
                            break;
                        }
                    }
                }
            }
            log::info!("✓ All input files processed successfully");

            // Log preview chapter count for diagnostics
            if let Some(ref ps) = preview_state_storage {
                if !ps.chapter_markers.is_empty() {
                    log::info!(
                        "Adaptive preview: processed {} file excerpts",
                        ps.chapter_markers.len()
                    );
                }
            }

            // Finalize encoding (same path for full encode or preview early-stop)
            log::info!("🏁 Starting encoding finalization...");
            crate::audio::processor::encoder::finalize_encoding_after_preview(
                &mut enc_ctx,
                &mut octx,
                ost_index,
                ost_time_base,
            )?;
            log::info!("✓ Encoding finalization completed successfully");

            // Preserve output on success (Phase 11: re-enabled after legacy purge)
            let _ = cleanup_guard.remove_path(&plan.output_path);

            if metadata.is_some() {
                log::info!("Audio processing completed with metadata integration");
            } else {
                log::info!("Audio processing completed without metadata");
            }

            Ok(())
        })
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
    let ictx = ff::format::input(&first)
        .map_err(|e| AppError::General(format!("Open input failed: {e}")))?;
    let stream = ictx
        .streams()
        .best(ff::media::Type::Audio)
        .ok_or_else(|| AppError::InvalidInput("No audio stream in first input".to_string()))?;
    let codec_ctx = ff::codec::context::Context::from_parameters(stream.parameters())
        .map_err(|e| AppError::General(format!("Decoder ctx from params failed: {e}")))?;
    let decoder = codec_ctx
        .decoder()
        .audio()
        .map_err(|e| AppError::General(format!("Open audio decoder failed: {e}")))?;
    let channels = decoder.channel_layout().channels() as i32;
    Ok((decoder.rate(), channels.max(1)))
}
