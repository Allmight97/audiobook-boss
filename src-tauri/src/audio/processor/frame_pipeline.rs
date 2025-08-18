//! Frame processing pipeline (behavior-preserving extraction)

use crate::errors::Result;
use ffmpeg_next as ff;

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
    pub(crate) input_samples_total: &'a mut u64,
    pub(crate) encoded_samples_total: &'a mut u64,
}

fn emit_progress_update(ctx: &mut FramePipelineCtx) {
    if ctx.last_emit.elapsed() > std::time::Duration::from_millis(200) {
        *ctx.last_emit = std::time::Instant::now();
        let current_seconds = *ctx.running_pts as f64 / ctx.target_sample_rate as f64;
        let percentage = crate::audio::progress::converting_percentage_from_seconds(
            current_seconds,
            ctx.total_duration,
        ) as f64;
        ctx.emitter.emit_converting_progress(
            percentage.min(crate::audio::constants::PROGRESS_CONVERTING_MAX as f64) as f32,
            "Converting and merging audio files...",
            Some(format!("Input {} of {}", ctx.current_file_index + 1, ctx.total_files)),
            None,
        );
    }
}

/// Processes audio frames from decoder through resample and encode pipeline
pub(crate) fn process_decoded_frames(
    decoder: &mut ff::codec::decoder::Audio,
    encoder: &mut ff::codec::encoder::audio::Encoder,
    resampler: &mut ff::software::resampling::Context,
    output_context: &mut ff::format::context::Output,
    ctx: &mut FramePipelineCtx,
    accumulator: &mut crate::audio::buffer::SampleAccumulator,
) -> Result<()> {
    use crate::errors::AppError;

    loop {
        if ctx.context.is_cancelled() {
            let _ = encoder.send_eof();
            ctx.emitter.emit_cancelled("Processing was cancelled");
            return Err(AppError::InvalidInput("Processing was cancelled".into()));
        }
        let mut frame = ff::frame::Audio::empty();
        match decoder.receive_frame(&mut frame) {
            Ok(()) => {
                let decoder_matches_encoder =
                    frame.format() == encoder.format()
                        && frame.channel_layout() == encoder.channel_layout()
                        && frame.rate() == encoder.rate();
                let disable_fastpath = std::env::var("ABB_DISABLE_FASTPATH")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);

                if decoder_matches_encoder && !disable_fastpath {
                    log::debug!("Fast-path: decoder frame matches encoder format – skipping resampler");
                    if frame.samples() == 0 {
                        log::warn!("Decoder produced 0 samples – skipping frame");
                        continue;
                    }
                    *ctx.input_samples_total += frame.samples() as u64;
                    for mut full in accumulator.push_frame(&frame) {
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
                    continue;
                }

                let mut out = ff::frame::Audio::empty();
                out.set_format(encoder.format());
                out.set_channel_layout(encoder.channel_layout());
                out.set_rate(encoder.rate());
                out.set_samples(frame.samples());
                unsafe { out.alloc(encoder.format(), frame.samples(), encoder.channel_layout()); }

                resampler
                    .run(&frame, &mut out)
                    .map_err(|e| AppError::General(format!("Resample failed: {e}")))?;

                if out.samples() == 0 {
                    log::warn!("Resampler produced 0 samples – skipping frame to avoid panic");
                    continue;
                }

                *ctx.input_samples_total += out.samples() as u64;
                for mut full in accumulator.push_frame(&out) {
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
            }
            Err(ff::Error::Other { .. }) | Err(ff::Error::Eof) => break,
            Err(e) => return Err(AppError::General(format!("Decoder receive failed: {e}"))),
        }
    }
    Ok(())
}

/// Processes packets from input stream through decoder pipeline
pub(crate) fn process_input_packets(
    ictx: &mut ff::format::context::Input,
    decoder: &mut ff::codec::decoder::Audio,
    encoder: &mut ff::codec::encoder::audio::Encoder,
    resampler: &mut ff::software::resampling::Context,
    output_context: &mut ff::format::context::Output,
    ctx: &mut FramePipelineCtx,
    accumulator: &mut crate::audio::buffer::SampleAccumulator,
) -> Result<()> {
    use crate::errors::AppError;

    log::info!("📦 Starting packet processing for stream index: {}", ctx.current_stream_index);
    let mut packet_count = 0;
    log::info!("Starting packet iteration...");
    for (si, packet) in ictx.packets() {
        log::debug!("Processing packet from stream {}", si.index());
        if ctx.context.is_cancelled() {
            log::warn!("Processing was cancelled during packet processing");
            ctx.emitter.emit_cancelled("Processing was cancelled");
            return Err(AppError::InvalidInput("Processing was cancelled".into()));
        }
        if si.index() != ctx.current_stream_index {
            log::debug!("Skipping packet from stream {} (expecting {})", si.index(), ctx.current_stream_index);
            continue;
        }

        packet_count += 1;
        if packet_count % 100 == 0 {
            log::info!("Processed {} packets so far", packet_count);
        }

        log::debug!("Sending packet {} to decoder", packet_count);
        match decoder.send_packet(&packet) {
            Ok(()) => log::debug!("✓ Packet {} sent to decoder successfully", packet_count),
            Err(e) => {
                log::error!("✗ Failed to send packet {} to decoder: {}", packet_count, e);
                return Err(AppError::General(format!("Decoder send packet failed: {}", e)));
            }
        }

        log::debug!("Processing decoded frames for packet {}", packet_count);
        match process_decoded_frames(decoder, encoder, resampler, output_context, ctx, accumulator) {
            Ok(()) => log::debug!("✓ Decoded frames processed successfully for packet {}", packet_count),
            Err(e) => {
                log::error!("✗ Failed to process decoded frames for packet {}: {}", packet_count, e);
                return Err(e);
            }
        }
    }
    log::info!("✓ Processed {} packets total", packet_count);
    Ok(())
}


