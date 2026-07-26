//! Owned orchestration for the ffmpeg-next input processing loop.

use ffmpeg_next as ff;

use crate::audio::buffer::SampleAccumulator;
use crate::audio::processor::frame_pipeline::{
    flush_accumulator_tail, FramePipelineCtx, PreviewAction,
};
use crate::audio::processor::plan::MediaProcessingPlan;
use crate::audio::processor::preview_state::PreviewState;
use crate::errors::{sanitize_path_for_display, Result};
use crate::processing::{ProcessingContext, ProgressEmitter};

use super::engine::FfmpegNextProcessor;

pub(crate) struct InputProcessingContext<'a> {
    pub(crate) enc_ctx: &'a mut ff::codec::encoder::audio::Encoder,
    pub(crate) octx: &'a mut ff::format::context::Output,
    pub(crate) ost_index: usize,
    pub(crate) ost_time_base: ff::Rational,
    pub(crate) target_sample_rate: u32,
    pub(crate) samples_per_frame: usize,
    pub(crate) emitter: &'a ProgressEmitter,
}

pub(crate) fn process_input_files(
    plan: &MediaProcessingPlan,
    context: &ProcessingContext,
    io: &mut InputProcessingContext<'_>,
) -> Result<bool> {
    let mut running_pts: i64 = 0;
    let mut last_emit = std::time::Instant::now();
    let mut input_samples_total: u64 = 0;
    let mut encoded_samples_total: u64 = 0;
    let mut preview_early_stop = false;
    let file_count = plan.input_file_paths.len();
    let mut preview_state_storage =
        context
            .preview
            .as_ref()
            .filter(|_| file_count > 1)
            .map(|cfg| {
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
        emitter: io.emitter,
        total_duration: plan.total_duration.max(0.001),
        total_files: file_count,
        target_sample_rate: io.target_sample_rate,
        output_stream_index: io.ost_index,
        output_time_base: io.ost_time_base,
        running_pts: &mut running_pts,
        last_emit: &mut last_emit,
        current_file_index: 0,
        current_stream_index: 0,
        current_file_name: String::new(),
        input_samples_total: &mut input_samples_total,
        encoded_samples_total: &mut encoded_samples_total,
        early_stop: &mut preview_early_stop,
        preview_state: preview_state_storage.as_mut(),
    };

    let mut accumulator = SampleAccumulator::new(
        io.enc_ctx.channel_layout().channels() as usize,
        io.samples_per_frame,
        io.enc_ctx.rate(),
        io.enc_ctx.channel_layout(),
        io.enc_ctx.format(),
    )?;

    log::info!(
        "Starting audio processing for {} input files",
        plan.input_file_paths.len()
    );

    // This runs on the adapter's `spawn_blocking` thread (adapter.rs), so no
    // additional `block_in_place` layer is needed here.
    for (idx, in_path) in plan.input_file_paths.iter().enumerate() {
        let file_label = in_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        ctx.current_file_name = file_label.to_string();
        let path = sanitize_path_for_display(in_path);
        log::info!("Processing input file {}/{}: {}", idx + 1, file_count, path);
        let action = FfmpegNextProcessor::process_input_file(
            in_path,
            io.enc_ctx,
            io.octx,
            idx,
            &mut ctx,
            &mut accumulator,
        )?;
        log::info!(
            "✓ Completed processing input file {}/{}",
            idx + 1,
            file_count
        );

        if *ctx.early_stop {
            log::info!(
                "Preview boundary reached after file {}; stopping further input processing",
                idx + 1
            );
            break;
        }

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
            }
            PreviewAction::Continue => {}
        }
    }

    log::info!("✓ All input files processed successfully");
    flush_accumulator_tail(io.enc_ctx, io.octx, &mut ctx, &mut accumulator)
}
