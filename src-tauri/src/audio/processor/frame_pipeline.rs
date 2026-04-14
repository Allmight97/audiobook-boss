//! Frame processing pipeline (behavior-preserving extraction)

use crate::audio::processor::preview_state::PreviewState;
use crate::errors::Result;
use ffmpeg_next as ff;

/// Progress emission throttle interval (milliseconds)
const PROGRESS_EMIT_INTERVAL_MS: u64 = 1000;

/// Result of checking per-file preview progress
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewAction {
    /// Continue processing current file
    Continue,
    /// Current file excerpt complete, move to next file
    NextFile,
    /// All file excerpts complete, stop processing
    StopAll,
}

pub(crate) struct FramePipelineCtx<'a> {
    pub(crate) context: &'a crate::audio::context::ProcessingContext,
    pub(crate) emitter: &'a crate::audio::progress::ProgressEmitter,
    pub(crate) total_duration: f64,
    pub(crate) total_files: usize,
    pub(crate) target_sample_rate: u32,
    pub(crate) output_stream_index: usize,
    pub(crate) output_time_base: ff::Rational,
    pub(crate) running_pts: &'a mut i64,
    pub(crate) last_emit: &'a mut std::time::Instant,
    pub(crate) current_file_index: usize,
    pub(crate) current_stream_index: usize,
    pub(crate) current_file_name: String,
    pub(crate) input_samples_total: &'a mut u64,
    pub(crate) encoded_samples_total: &'a mut u64,
    pub(crate) early_stop: &'a mut bool,
    /// Adaptive preview state (None when not in preview mode or using single-file preview)
    pub(crate) preview_state: Option<&'a mut PreviewState>,
}

fn emit_progress_update(ctx: &mut FramePipelineCtx) {
    if ctx.last_emit.elapsed() > std::time::Duration::from_millis(PROGRESS_EMIT_INTERVAL_MS) {
        *ctx.last_emit = std::time::Instant::now();
        let current_seconds = *ctx.running_pts as f64 / ctx.target_sample_rate as f64;
        let percentage = crate::audio::progress::converting_percentage_from_seconds(
            current_seconds,
            ctx.total_duration,
        ) as f64;
        let file_label = if ctx.current_file_name.is_empty() {
            "Unknown file"
        } else {
            ctx.current_file_name.as_str()
        };
        ctx.emitter.emit_converting_progress(
            percentage.min(crate::audio::constants::PROGRESS_CONVERTING_MAX as f64) as f32,
            "Converting and merging audio files...",
            Some(format!(
                "{} ({}/{})",
                file_label,
                ctx.current_file_index + 1,
                ctx.total_files
            )),
            None,
        );
    }
}

/// Checks per-file preview progress and returns the appropriate action
#[inline]
fn check_per_file_preview_stop(ctx: &mut FramePipelineCtx) -> PreviewAction {
    let Some(preview_state) = ctx.preview_state.as_mut() else {
        return PreviewAction::Continue;
    };

    let elapsed_seconds =
        preview_state.current_file_elapsed_samples as f64 / ctx.target_sample_rate as f64;

    if elapsed_seconds >= preview_state.per_file_seconds {
        // Record chapter marker for this file excerpt
        preview_state.record_chapter(*ctx.running_pts, ctx.target_sample_rate);

        if preview_state.all_files_complete() {
            log::info!(
                "adaptive preview complete: {} chapters, total_pts={}",
                preview_state.chapter_markers.len(),
                *ctx.running_pts
            );
            *ctx.early_stop = true;
            return PreviewAction::StopAll;
        }

        log::info!(
            "adaptive preview: file {} complete ({:.3}s), moving to next",
            preview_state.current_file_index + 1,
            elapsed_seconds
        );
        return PreviewAction::NextFile;
    }

    PreviewAction::Continue
}

/// Single-file preview early-stop check (used when preview_state is None)
#[inline]
fn check_and_mark_preview_early_stop(ctx: &mut FramePipelineCtx) -> bool {
    // Skip if adaptive preview is active (handled by check_per_file_preview_stop)
    if ctx.preview_state.is_some() {
        return false;
    }

    if let Some(preview) = ctx.context.preview.as_ref() {
        let elapsed_seconds = *ctx.running_pts as f64 / ctx.target_sample_rate as f64;
        if elapsed_seconds >= preview.total_seconds {
            if log::log_enabled!(log::Level::Info) {
                log::info!(
                    "preview early-stop reached elapsed={:.3}s target={:.3}s",
                    elapsed_seconds,
                    preview.total_seconds
                );
            }
            *ctx.early_stop = true;
            return true;
        }
    }
    false
}

fn process_and_encode_frame(
    frame: &ff::frame::Audio,
    encoder: &mut ff::codec::encoder::audio::Encoder,
    output_context: &mut ff::format::context::Output,
    ctx: &mut FramePipelineCtx,
    accumulator: &mut crate::audio::buffer::SampleAccumulator,
) -> Result<PreviewAction> {
    let samples_count = frame.samples() as u64;
    *ctx.input_samples_total += samples_count;

    if let Some(ref mut ps) = ctx.preview_state {
        ps.current_file_elapsed_samples += samples_count;
    }

    for mut full in accumulator.push_frame(frame) {
        full.set_pts(Some(*ctx.running_pts));
        *ctx.running_pts += full.samples() as i64;
        *ctx.encoded_samples_total += full.samples() as u64;
        crate::audio::processor::encoder::encode_and_write_frame(
            encoder,
            &full,
            output_context,
            ctx.output_stream_index,
            ctx.output_time_base,
        )?;
    }

    emit_progress_update(ctx);
    let action = check_per_file_preview_stop(ctx);
    if action != PreviewAction::Continue {
        return Ok(action);
    }

    check_and_mark_preview_early_stop(ctx);
    Ok(PreviewAction::Continue)
}

/// Processes audio frames from decoder through resample and encode pipeline
/// Returns PreviewAction to signal adaptive preview file transitions
pub(crate) fn process_decoded_frames(
    decoder: &mut ff::codec::decoder::Audio,
    encoder: &mut ff::codec::encoder::audio::Encoder,
    resampler: &mut ff::software::resampling::Context,
    output_context: &mut ff::format::context::Output,
    ctx: &mut FramePipelineCtx,
    accumulator: &mut crate::audio::buffer::SampleAccumulator,
) -> Result<PreviewAction> {
    use crate::errors::AppError;

    let mut action = PreviewAction::Continue;
    let disable_fastpath = std::env::var("ABB_DISABLE_FASTPATH")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let fastpath_enabled = !disable_fastpath;
    let encoder_format = encoder.format();
    let encoder_channel_layout = encoder.channel_layout();
    let encoder_sample_rate = encoder.rate();

    loop {
        if ctx.context.is_cancelled() {
            let _ = encoder.send_eof();
            ctx.emitter.emit_cancelled("Processing was cancelled");
            return Err(AppError::cancelled());
        }
        let mut frame = ff::frame::Audio::empty();
        match decoder.receive_frame(&mut frame) {
            Ok(()) => {
                let decoder_matches_encoder = fastpath_enabled
                    && frame.format() == encoder_format
                    && frame.channel_layout() == encoder_channel_layout
                    && frame.rate() == encoder_sample_rate;

                if decoder_matches_encoder {
                    log::debug!(
                        "Fast-path: decoder frame matches encoder format – skipping resampler"
                    );
                    if frame.samples() == 0 {
                        log::warn!("Decoder produced 0 samples – skipping frame");
                        continue;
                    }
                    action = process_and_encode_frame(
                        &frame,
                        encoder,
                        output_context,
                        ctx,
                        accumulator,
                    )?;
                    if action != PreviewAction::Continue || *ctx.early_stop {
                        break;
                    }
                    continue;
                }

                let mut out = ff::frame::Audio::empty();
                out.set_format(encoder_format);
                out.set_channel_layout(encoder_channel_layout);
                out.set_rate(encoder_sample_rate);
                out.set_samples(frame.samples());
                unsafe {
                    out.alloc(encoder_format, frame.samples(), encoder_channel_layout);
                }

                resampler
                    .run(&frame, &mut out)
                    .map_err(|e| AppError::General(format!("Resample failed: {e}")))?;

                if out.samples() == 0 {
                    log::warn!("Resampler produced 0 samples – skipping frame to avoid panic");
                    continue;
                }

                action = process_and_encode_frame(&out, encoder, output_context, ctx, accumulator)?;
                if action != PreviewAction::Continue || *ctx.early_stop {
                    break;
                }
            }
            Err(ff::Error::Other { .. }) | Err(ff::Error::Eof) => break,
            Err(e) => return Err(AppError::General(format!("Decoder receive failed: {e}"))),
        }
    }
    Ok(action)
}

/// Processes packets from input stream through decoder pipeline
/// Returns PreviewAction to signal adaptive preview file transitions
pub(crate) fn process_input_packets(
    ictx: &mut ff::format::context::Input,
    decoder: &mut ff::codec::decoder::Audio,
    encoder: &mut ff::codec::encoder::audio::Encoder,
    resampler: &mut ff::software::resampling::Context,
    output_context: &mut ff::format::context::Output,
    ctx: &mut FramePipelineCtx,
    accumulator: &mut crate::audio::buffer::SampleAccumulator,
) -> Result<PreviewAction> {
    use crate::errors::AppError;

    if log::log_enabled!(log::Level::Info) {
        log::info!(
            "Starting packet processing for stream index: {}",
            ctx.current_stream_index
        );
    }
    let mut packet_count = 0;
    let mut final_action = PreviewAction::Continue;
    for (si, packet) in ictx.packets() {
        if log::log_enabled!(log::Level::Debug) {
            log::debug!("Processing packet from stream {}", si.index());
        }
        if ctx.context.is_cancelled() {
            if log::log_enabled!(log::Level::Warn) {
                log::warn!("Processing was cancelled during packet processing");
            }
            ctx.emitter.emit_cancelled("Processing was cancelled");
            return Err(AppError::cancelled());
        }
        if si.index() != ctx.current_stream_index {
            log::debug!(
                "Skipping packet from stream {} (expecting {})",
                si.index(),
                ctx.current_stream_index
            );
            continue;
        }

        packet_count += 1;
        if packet_count % 100 == 0 && log::log_enabled!(log::Level::Debug) {
            log::debug!("Processed {} packets so far", packet_count);
        }

        if log::log_enabled!(log::Level::Debug) {
            log::debug!("Sending packet {} to decoder", packet_count);
        }
        match decoder.send_packet(&packet) {
            Ok(()) => log::debug!("✓ Packet {} sent to decoder successfully", packet_count),
            Err(e) => {
                log::error!("✗ Failed to send packet {} to decoder: {}", packet_count, e);
                return Err(AppError::General(format!(
                    "Decoder send packet failed: {}",
                    e
                )));
            }
        }

        log::debug!("Processing decoded frames for packet {}", packet_count);
        match process_decoded_frames(
            decoder,
            encoder,
            resampler,
            output_context,
            ctx,
            accumulator,
        ) {
            Ok(action) => {
                log::debug!(
                    "✓ Decoded frames processed successfully for packet {} (action={:?})",
                    packet_count,
                    action
                );
                if action != PreviewAction::Continue {
                    final_action = action;
                    break;
                }
            }
            Err(e) => {
                log::error!(
                    "✗ Failed to process decoded frames for packet {}: {}",
                    packet_count,
                    e
                );
                return Err(e);
            }
        }

        if *ctx.early_stop {
            if log::log_enabled!(log::Level::Info) {
                log::info!("Preview early-stop marked; exiting packet loop");
            }
            break;
        }
    }
    log::info!("✓ Processed {} packets total", packet_count);
    Ok(final_action)
}
