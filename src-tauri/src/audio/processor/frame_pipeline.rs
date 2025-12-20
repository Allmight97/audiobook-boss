// REFACTOR: Module exceeds 400 LOC (553). Consider splitting before adding new code.
//! Frame processing pipeline (behavior-preserving extraction)

use crate::errors::Result;
use ffmpeg_next as ff;

/// Chapter marker for preview output (embedded in M4B)
#[derive(Debug, Clone)]
pub struct ChapterMarker {
    /// Chapter start time in milliseconds
    pub start_ms: i64,
    /// Chapter end time in milliseconds
    pub end_ms: i64,
    /// Chapter title (sanitized filename)
    pub title: String,
}

/// Extended state for adaptive multi-file preview
#[derive(Debug)]
pub struct PreviewState {
    /// Total number of files being processed
    pub file_count: usize,
    /// Calculated per-file duration (seconds)
    pub per_file_seconds: f64,
    /// Current file index (0-based)
    pub current_file_index: usize,
    /// PTS at start of current file excerpt
    pub current_file_start_pts: i64,
    /// Samples processed for current file excerpt
    pub current_file_elapsed_samples: u64,
    /// Collected chapter markers for embedding
    pub chapter_markers: Vec<ChapterMarker>,
    /// Current file name for chapter title
    pub current_file_name: String,
}

impl PreviewState {
    /// Creates a new PreviewState for adaptive preview
    pub fn new(file_count: usize, per_file_seconds: f64) -> Self {
        Self {
            file_count,
            per_file_seconds,
            current_file_index: 0,
            current_file_start_pts: 0,
            current_file_elapsed_samples: 0,
            chapter_markers: Vec::with_capacity(file_count),
            current_file_name: String::new(),
        }
    }

    /// Resets per-file counters when switching to a new file
    pub fn start_new_file(&mut self, file_index: usize, file_name: &str, current_pts: i64) {
        self.current_file_index = file_index;
        self.current_file_name = file_name.to_string();
        self.current_file_start_pts = current_pts;
        self.current_file_elapsed_samples = 0;
    }

    /// Records a chapter marker for the current file excerpt
    pub fn record_chapter(&mut self, end_pts: i64, sample_rate: u32) {
        let start_ms = (self.current_file_start_pts * 1000) / sample_rate as i64;
        let end_ms = (end_pts * 1000) / sample_rate as i64;
        let title = sanitize_chapter_title(&self.current_file_name);
        self.chapter_markers.push(ChapterMarker {
            start_ms,
            end_ms,
            title,
        });
    }

    /// Returns true if all files have been processed
    pub fn all_files_complete(&self) -> bool {
        self.current_file_index + 1 >= self.file_count
    }
}

/// Sanitize filename for FFMETADATA chapter title
pub fn sanitize_chapter_title(filename: &str) -> String {
    // Remove extension
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    // Replace FFMETADATA special characters
    stem.chars()
        .map(|c| match c {
            '=' | '[' | ']' | '#' | ';' | '\\' | '\n' | '\r' => '_',
            _ => c,
        })
        .collect()
}

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
    /// Adaptive preview state (None when not in preview mode or using legacy single-file preview)
    pub(crate) preview_state: Option<&'a mut PreviewState>,
}

fn emit_progress_update(ctx: &mut FramePipelineCtx) {
    if ctx.last_emit.elapsed() > std::time::Duration::from_millis(200) {
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

/// Legacy single-file preview early-stop check (used when preview_state is None)
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

    loop {
        if ctx.context.is_cancelled() {
            let _ = encoder.send_eof();
            ctx.emitter.emit_cancelled("Processing was cancelled");
            return Err(AppError::InvalidInput("Processing was cancelled".into()));
        }
        let mut frame = ff::frame::Audio::empty();
        match decoder.receive_frame(&mut frame) {
            Ok(()) => {
                let decoder_matches_encoder = frame.format() == encoder.format()
                    && frame.channel_layout() == encoder.channel_layout()
                    && frame.rate() == encoder.rate();
                let disable_fastpath = std::env::var("ABB_DISABLE_FASTPATH")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);

                if decoder_matches_encoder && !disable_fastpath {
                    log::debug!(
                        "Fast-path: decoder frame matches encoder format – skipping resampler"
                    );
                    if frame.samples() == 0 {
                        log::warn!("Decoder produced 0 samples – skipping frame");
                        continue;
                    }
                    let samples_count = frame.samples() as u64;
                    *ctx.input_samples_total += samples_count;

                    // Track samples for adaptive preview
                    if let Some(ref mut ps) = ctx.preview_state {
                        ps.current_file_elapsed_samples += samples_count;
                    }

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

                    // Check adaptive preview first, then legacy
                    action = check_per_file_preview_stop(ctx);
                    if action != PreviewAction::Continue {
                        break;
                    }
                    if check_and_mark_preview_early_stop(ctx) {
                        break;
                    }
                    continue;
                }

                let mut out = ff::frame::Audio::empty();
                out.set_format(encoder.format());
                out.set_channel_layout(encoder.channel_layout());
                out.set_rate(encoder.rate());
                out.set_samples(frame.samples());
                unsafe {
                    out.alloc(encoder.format(), frame.samples(), encoder.channel_layout());
                }

                resampler
                    .run(&frame, &mut out)
                    .map_err(|e| AppError::General(format!("Resample failed: {e}")))?;

                if out.samples() == 0 {
                    log::warn!("Resampler produced 0 samples – skipping frame to avoid panic");
                    continue;
                }

                let samples_count = out.samples() as u64;
                *ctx.input_samples_total += samples_count;

                // Track samples for adaptive preview
                if let Some(ref mut ps) = ctx.preview_state {
                    ps.current_file_elapsed_samples += samples_count;
                }

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

                // Check adaptive preview first, then legacy
                action = check_per_file_preview_stop(ctx);
                if action != PreviewAction::Continue {
                    break;
                }
                if check_and_mark_preview_early_stop(ctx) {
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
            "📦 Starting packet processing for stream index: {}",
            ctx.current_stream_index
        );
    }
    let mut packet_count = 0;
    let mut final_action = PreviewAction::Continue;
    log::info!("Starting packet iteration...");
    for (si, packet) in ictx.packets() {
        if log::log_enabled!(log::Level::Debug) {
            log::debug!("Processing packet from stream {}", si.index());
        }
        if ctx.context.is_cancelled() {
            if log::log_enabled!(log::Level::Warn) {
                log::warn!("Processing was cancelled during packet processing");
            }
            ctx.emitter.emit_cancelled("Processing was cancelled");
            return Err(AppError::InvalidInput("Processing was cancelled".into()));
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
        if packet_count % 100 == 0 && log::log_enabled!(log::Level::Info) {
            log::info!("Processed {} packets so far", packet_count);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_chapter_title_removes_extension() {
        assert_eq!(sanitize_chapter_title("Chapter 01.mp3"), "Chapter 01");
        assert_eq!(sanitize_chapter_title("file.m4a"), "file");
        assert_eq!(sanitize_chapter_title("audio.flac"), "audio");
    }

    #[test]
    fn sanitize_chapter_title_handles_no_extension() {
        assert_eq!(sanitize_chapter_title("Chapter 01"), "Chapter 01");
    }

    #[test]
    fn sanitize_chapter_title_replaces_special_chars() {
        // FFMETADATA special characters should be replaced with underscore
        assert_eq!(sanitize_chapter_title("chapter=1.mp3"), "chapter_1");
        assert_eq!(sanitize_chapter_title("chapter[1].mp3"), "chapter_1_");
        assert_eq!(sanitize_chapter_title("chapter#1.mp3"), "chapter_1");
        assert_eq!(sanitize_chapter_title("ch;1.mp3"), "ch_1");
        assert_eq!(sanitize_chapter_title("ch\\1.mp3"), "ch_1");
    }

    #[test]
    fn sanitize_chapter_title_preserves_unicode() {
        assert_eq!(sanitize_chapter_title("第一章.mp3"), "第一章");
        assert_eq!(sanitize_chapter_title("Капитул 1.mp3"), "Капитул 1");
    }

    #[test]
    fn sanitize_chapter_title_handles_multiple_dots() {
        assert_eq!(sanitize_chapter_title("01.Chapter.mp3"), "01.Chapter");
        assert_eq!(
            sanitize_chapter_title("Track.01.Audio.m4a"),
            "Track.01.Audio"
        );
    }

    #[test]
    fn sanitize_chapter_title_handles_full_path() {
        // file_stem extracts just the filename without extension
        assert_eq!(
            sanitize_chapter_title("/path/to/Chapter 01.mp3"),
            "Chapter 01"
        );
    }

    #[test]
    fn preview_state_new_initializes_correctly() {
        let state = PreviewState::new(5, 10.0);
        assert_eq!(state.file_count, 5);
        assert!((state.per_file_seconds - 10.0).abs() < 0.001);
        assert_eq!(state.current_file_index, 0);
        assert_eq!(state.current_file_start_pts, 0);
        assert_eq!(state.current_file_elapsed_samples, 0);
        assert!(state.chapter_markers.is_empty());
    }

    #[test]
    fn preview_state_start_new_file_resets_counters() {
        let mut state = PreviewState::new(5, 10.0);
        state.current_file_elapsed_samples = 48000;
        state.start_new_file(2, "Track03.mp3", 144000);

        assert_eq!(state.current_file_index, 2);
        assert_eq!(state.current_file_name, "Track03.mp3");
        assert_eq!(state.current_file_start_pts, 144000);
        assert_eq!(state.current_file_elapsed_samples, 0);
    }

    #[test]
    fn preview_state_all_files_complete() {
        let mut state = PreviewState::new(3, 10.0);
        assert!(!state.all_files_complete());

        state.current_file_index = 1;
        assert!(!state.all_files_complete());

        state.current_file_index = 2;
        assert!(state.all_files_complete());
    }

    #[test]
    fn preview_state_record_chapter() {
        let mut state = PreviewState::new(3, 10.0);
        state.current_file_name = "Chapter 01.mp3".to_string();
        state.current_file_start_pts = 0;

        // Record chapter at 48000 samples @ 48000 Hz = 1000ms
        state.record_chapter(48000, 48000);

        assert_eq!(state.chapter_markers.len(), 1);
        assert_eq!(state.chapter_markers[0].start_ms, 0);
        assert_eq!(state.chapter_markers[0].end_ms, 1000);
        assert_eq!(state.chapter_markers[0].title, "Chapter 01");
    }

    #[test]
    fn preview_action_enum_values() {
        // Verify enum variants exist and can be compared
        assert_eq!(PreviewAction::Continue, PreviewAction::Continue);
        assert_ne!(PreviewAction::Continue, PreviewAction::NextFile);
        assert_ne!(PreviewAction::NextFile, PreviewAction::StopAll);
    }

    #[test]
    fn chapter_marker_struct() {
        let marker = ChapterMarker {
            start_ms: 0,
            end_ms: 5000,
            title: "Intro".to_string(),
        };
        assert_eq!(marker.start_ms, 0);
        assert_eq!(marker.end_ms, 5000);
        assert_eq!(marker.title, "Intro");
    }
}
