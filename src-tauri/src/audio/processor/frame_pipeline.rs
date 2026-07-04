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
    pub(crate) context: &'a crate::processing::context::ProcessingContext,
    pub(crate) emitter: &'a crate::processing::progress::ProgressEmitter,
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
        let percentage = crate::processing::progress::converting_percentage_from_seconds(
            current_seconds,
            ctx.total_duration,
        ) as f64;
        let file_label = if ctx.current_file_name.is_empty() {
            "Unknown file"
        } else {
            ctx.current_file_name.as_str()
        };
        ctx.emitter.emit_converting_progress(
            percentage.min(crate::processing::progress::PROGRESS_CONVERTING_MAX as f64) as f32,
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
        if preview_state.all_files_complete() {
            log::info!(
                "adaptive preview complete: {} file excerpts, total_pts={}",
                preview_state.file_count,
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

/// Single-file preview early-stop check (used when preview_state is None).
/// Communicates purely through the `ctx.early_stop` side effect.
#[inline]
fn check_and_mark_preview_early_stop(ctx: &mut FramePipelineCtx) {
    // Skip if adaptive preview is active (handled by check_per_file_preview_stop)
    if ctx.preview_state.is_some() {
        return;
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
        }
    }
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

const RESAMPLER_FLUSH_CHUNK_SAMPLES: usize = 1024;

fn allocate_resampler_output_frame(
    resampler: &ff::software::resampling::Context,
    samples: usize,
) -> ff::frame::Audio {
    let out_def = *resampler.output();
    let mut out = ff::frame::Audio::empty();
    out.set_format(out_def.format);
    out.set_channel_layout(out_def.channel_layout);
    out.set_rate(out_def.rate);
    out.set_samples(samples);
    unsafe {
        out.alloc(out_def.format, samples, out_def.channel_layout);
    }
    out
}

fn resampler_output_frame_samples(
    resampler: &ff::software::resampling::Context,
    input_samples: usize,
) -> usize {
    let input_rate = u128::from(resampler.input().rate);
    let output_rate = u128::from(resampler.output().rate);
    let delayed_input = resampler
        .delay()
        .map(|delay| delay.input.max(0) as u128)
        .unwrap_or(0);
    let input_samples = input_samples as u128;
    let total_input = delayed_input.saturating_add(input_samples);
    let numerator = total_input.saturating_mul(output_rate);
    let scaled = numerator.saturating_add(input_rate.saturating_sub(1)) / input_rate;
    scaled.max(1).min(i32::MAX as u128) as usize
}

#[cfg(test)]
fn drain_resampler_sample_count(
    resampler: &mut ff::software::resampling::Context,
    chunk: usize,
) -> std::result::Result<usize, ff::Error> {
    let mut samples = 0usize;
    loop {
        let mut out = allocate_resampler_output_frame(resampler, chunk);
        let remaining = resampler.flush(&mut out)?;
        if out.samples() == 0 {
            break;
        }
        samples += out.samples();
        if remaining.is_none() {
            break;
        }
    }
    Ok(samples)
}

/// Encodes the resampler's held-back tail after the decoder is fully drained.
fn flush_resampler_tail(
    resampler: &mut ff::software::resampling::Context,
    encoder: &mut ff::codec::encoder::audio::Encoder,
    output_context: &mut ff::format::context::Output,
    ctx: &mut FramePipelineCtx,
    accumulator: &mut crate::audio::buffer::SampleAccumulator,
) -> Result<()> {
    use crate::errors::AppError;

    let mut frame_count = 0usize;
    let mut sample_count = 0usize;
    loop {
        let mut out = allocate_resampler_output_frame(resampler, RESAMPLER_FLUSH_CHUNK_SAMPLES);
        let remaining = resampler
            .flush(&mut out)
            .map_err(|e| AppError::General(format!("Resampler flush failed: {e}")))?;
        if out.samples() == 0 {
            break;
        }
        frame_count += 1;
        sample_count += out.samples();
        process_and_encode_frame(&out, encoder, output_context, ctx, accumulator)?;
        if remaining.is_none() {
            break;
        }
    }
    if frame_count > 0 {
        log::debug!(
            "Flushed resampler tail: {} frame(s), {} sample(s)",
            frame_count,
            sample_count
        );
    }
    Ok(())
}

fn should_drain_decoder_after_input(ctx: &FramePipelineCtx, action: PreviewAction) -> bool {
    if action != PreviewAction::Continue {
        return false;
    }

    if *ctx.early_stop {
        return false;
    }

    true
}

pub(crate) fn flush_accumulator_tail(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    output_context: &mut ff::format::context::Output,
    ctx: &mut FramePipelineCtx,
    accumulator: &mut crate::audio::buffer::SampleAccumulator,
) -> Result<bool> {
    if let Some(mut tail) = accumulator.flush_tail(true) {
        tail.set_pts(Some(*ctx.running_pts));
        *ctx.running_pts += tail.samples() as i64;
        *ctx.encoded_samples_total += tail.samples() as u64;
        crate::audio::processor::encoder::encode_and_write_frame(
            encoder,
            &tail,
            output_context,
            ctx.output_stream_index,
            ctx.output_time_base,
        )?;
        return Ok(true);
    }

    Ok(false)
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
    // Preview runs prioritize deterministic excerpt boundaries over the decoder-format fast path.
    let fastpath_enabled = !disable_fastpath && ctx.context.preview.is_none();
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

                let mut out = allocate_resampler_output_frame(
                    resampler,
                    resampler_output_frame_samples(resampler, frame.samples()),
                );

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

        send_packet_to_decoder(decoder, &packet, packet_count)?;

        final_action = process_packet_frames(
            decoder,
            encoder,
            resampler,
            output_context,
            ctx,
            accumulator,
            packet_count,
        )?;
        if final_action != PreviewAction::Continue {
            break;
        }

        if *ctx.early_stop {
            if log::log_enabled!(log::Level::Info) {
                log::info!("Preview early-stop marked; exiting packet loop");
            }
            break;
        }
    }

    let should_drain = should_drain_decoder_after_input(ctx, final_action);
    if should_drain {
        log::debug!("Sending decoder EOF after packet processing");
        let _ = decoder.send_eof();
        log::debug!("Draining decoder after EOF");
        let drain_action = process_decoded_frames(
            decoder,
            encoder,
            resampler,
            output_context,
            ctx,
            accumulator,
        )?;
        if drain_action != PreviewAction::Continue {
            final_action = drain_action;
        } else if !*ctx.early_stop {
            // End of this file's real audio: recover the resampler's
            // held-back tail before moving to the next input.
            flush_resampler_tail(resampler, encoder, output_context, ctx, accumulator)?;
        }
    } else {
        log::debug!("Skipping decoder drain after preview boundary");
    }

    log::info!("✓ Processed {} packets total", packet_count);
    Ok(final_action)
}

fn send_packet_to_decoder(
    decoder: &mut ff::codec::decoder::Audio,
    packet: &ff::Packet,
    packet_count: usize,
) -> Result<()> {
    use crate::errors::AppError;

    if log::log_enabled!(log::Level::Debug) {
        log::debug!("Sending packet {} to decoder", packet_count);
    }

    match decoder.send_packet(packet) {
        Ok(()) => {
            log::debug!("✓ Packet {} sent to decoder successfully", packet_count);
            Ok(())
        }
        Err(e) => {
            log::error!("✗ Failed to send packet {} to decoder: {}", packet_count, e);
            Err(AppError::General(format!(
                "Decoder send packet failed: {e}"
            )))
        }
    }
}

fn process_packet_frames(
    decoder: &mut ff::codec::decoder::Audio,
    encoder: &mut ff::codec::encoder::audio::Encoder,
    resampler: &mut ff::software::resampling::Context,
    output_context: &mut ff::format::context::Output,
    ctx: &mut FramePipelineCtx,
    accumulator: &mut crate::audio::buffer::SampleAccumulator,
    packet_count: usize,
) -> Result<PreviewAction> {
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
            Ok(action)
        }
        Err(e) => {
            log::error!(
                "✗ Failed to process decoded frames for packet {}: {}",
                packet_count,
                e
            );
            Err(e)
        }
    }
}

// EXCEPTION: inline test pins the swresample tail invariant this pipeline
// depends on; it needs no decoder/encoder scaffolding.
#[cfg(test)]
mod tests {
    use super::*;

    /// With sample-rate conversion active, swresample withholds a filter tail
    /// that `run()` alone never emits — the audio the pipeline used to drop at
    /// every file boundary. The resampler drain must recover it so that
    /// run + flush accounts for the full converted duration.
    #[test]
    fn rate_converting_resampler_tail_is_recovered_by_drain() {
        ff::init().expect("ffmpeg init");
        let layout = ff::ChannelLayout::default(1);
        let fmt = ff::format::Sample::F32(ff::format::sample::Type::Planar);
        let mut resampler =
            ff::software::resampling::Context::get(fmt, layout, 44_100, fmt, layout, 22_050)
                .expect("build 44.1k->22.05k resampler");

        let input_samples = 4096usize;
        let mut input = ff::frame::Audio::empty();
        input.set_format(fmt);
        input.set_channel_layout(layout);
        input.set_rate(44_100);
        input.set_samples(input_samples);
        unsafe {
            input.alloc(fmt, input_samples, layout);
        }
        for v in input.plane_mut::<f32>(0) {
            *v = 0.25;
        }

        // Mirror the pipeline's run() usage: pre-allocated output frame.
        let mut out = allocate_resampler_output_frame(&resampler, input_samples);
        resampler.run(&input, &mut out).expect("resample run");
        let converted = out.samples();

        let expected_total = input_samples / 2;
        assert!(
            converted < expected_total,
            "precondition: swr withholds a tail under rate conversion (run emitted {converted} of {expected_total})"
        );

        let tail = drain_resampler_sample_count(&mut resampler, 1024).expect("drain resampler");
        assert!(tail > 0, "drain must recover the held-back tail");

        let total = converted + tail;
        assert!(
            (total as i64 - expected_total as i64).abs() <= 32,
            "run + drain must account for the full converted duration (got {total}, want ~{expected_total})"
        );
    }

    #[test]
    fn upsample_resampler_output_capacity_prevents_bulk_eof_backlog() {
        ff::init().expect("ffmpeg init");
        let layout = ff::ChannelLayout::default(1);
        let fmt = ff::format::Sample::F32(ff::format::sample::Type::Planar);
        let mut resampler =
            ff::software::resampling::Context::get(fmt, layout, 22_050, fmt, layout, 44_100)
                .expect("build 22.05k->44.1k resampler");

        let input_samples = 4096usize;
        let mut input = ff::frame::Audio::empty();
        input.set_format(fmt);
        input.set_channel_layout(layout);
        input.set_rate(22_050);
        input.set_samples(input_samples);
        unsafe {
            input.alloc(fmt, input_samples, layout);
        }
        for v in input.plane_mut::<f32>(0) {
            *v = 0.25;
        }

        let output_capacity = resampler_output_frame_samples(&resampler, input_samples);
        assert!(
            output_capacity >= input_samples * 2,
            "upsample output frame must be sized for the output-rate sample count"
        );
        let mut out = allocate_resampler_output_frame(&resampler, output_capacity);
        resampler.run(&input, &mut out).expect("resample run");

        let expected_total = input_samples * 2;
        let tail = drain_resampler_sample_count(&mut resampler, RESAMPLER_FLUSH_CHUNK_SAMPLES)
            .expect("drain resampler");
        assert!(
            tail < RESAMPLER_FLUSH_CHUNK_SAMPLES,
            "EOF flush should only carry filter delay, not bulk upsample backlog (tail={tail})"
        );
        assert!(
            (out.samples() + tail).abs_diff(expected_total) <= 64,
            "run + drain must account for the upsampled duration"
        );
    }
}
