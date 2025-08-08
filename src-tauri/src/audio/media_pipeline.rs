//! Media processing pipeline for FFmpeg operations
//! 
//! This module provides a unified interface for all FFmpeg operations,
//! encapsulating command building and execution behind a stable Rust interface.
//! 
//! The `MediaProcessingPlan` struct holds inputs, outputs, and metadata for
//! processing operations, following mentor recommendations for abstraction.

use super::{AudioSettings, SampleRateConfig};
use super::constants::*;
use super::context::ProcessingContext;
use super::processor::{detect_input_sample_rate, create_session_from_legacy_state};
use super::progress_monitor::{setup_process_execution, monitor_process_with_progress, finalize_process_execution};
use crate::errors::Result;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::future::Future;
use std::pin::Pin;

/// Media processing plan that encapsulates inputs, outputs, and metadata
/// 
/// This struct follows the mentor's recommendation to use a `MediaProcessingPlan`
/// to hold all processing parameters in a structured way.
#[derive(Debug, Clone)]
pub struct MediaProcessingPlan {
    /// Input concat file path
    pub input_concat_file: PathBuf,
    /// Output file path
    pub output_path: PathBuf,
    /// Audio processing settings
    pub settings: AudioSettings,
    /// Input file paths for sample rate detection
    pub input_file_paths: Vec<PathBuf>,
    /// Total duration for progress tracking
    pub total_duration: f64,
}

impl MediaProcessingPlan {
    /// Creates a new media processing plan
    pub fn new(
        input_concat_file: PathBuf,
        output_path: PathBuf,
        settings: AudioSettings,
        input_file_paths: Vec<PathBuf>,
        total_duration: f64,
    ) -> Self {
        Self {
            input_concat_file,
            output_path,
            settings,
            input_file_paths,
            total_duration,
        }
    }

    /// Helper function to calculate total duration from AudioFile list
    /// Handles Option<f64> duration fields properly
    pub fn calculate_total_duration(files: &[super::AudioFile]) -> f64 {
        files.iter()
            .filter_map(|f| f.duration)
            .sum()
    }

    /// Builds FFmpeg command for this processing plan
    pub fn build_ffmpeg_command(&self) -> Result<Command> {
        build_merge_command(
            &self.input_concat_file,
            &self.output_path,
            &self.settings,
            &self.input_file_paths,
        )
    }

    /// Executes the processing plan with context-based progress tracking
    pub async fn execute_with_context(
        &self,
        context: &ProcessingContext,
    ) -> Result<()> {
        let cmd = self.build_ffmpeg_command()?;
        execute_ffmpeg_with_progress_context(cmd, context, self.total_duration).await
    }


}

/// Trait defining a media processor boundary for executing processing plans.
///
/// This allows swapping implementations (e.g., shell-based FFmpeg vs ffmpeg-next)
/// without changing call sites.
pub trait MediaProcessor {
    fn execute<'a>(
        &'a self,
        plan: &'a MediaProcessingPlan,
        context: &'a ProcessingContext,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
}

/// Shell-based FFmpeg processor implementation that delegates to the existing
/// command-building and progress-execution pipeline.
pub struct ShellFFmpegProcessor;

impl MediaProcessor for ShellFFmpegProcessor {
    fn execute<'a>(
        &'a self,
        plan: &'a MediaProcessingPlan,
        context: &'a ProcessingContext,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let cmd = plan.build_ffmpeg_command()?;
            execute_ffmpeg_with_progress_context(cmd, context, plan.total_duration).await
        })
    }
}

// Feature-gated processor based on ffmpeg-next bindings (skeleton)
#[cfg(feature = "safe-ffmpeg")]
pub struct FfmpegNextProcessor;

#[cfg(feature = "safe-ffmpeg")]
impl MediaProcessor for FfmpegNextProcessor {
    fn execute<'a>(
        &'a self,
        plan: &'a MediaProcessingPlan,
        context: &'a ProcessingContext,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        use crate::errors::AppError;
        use ffmpeg_next as ff;
        use std::sync::Once;

        // Initialize FFmpeg (idempotent)
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = ff::init();
        });

        Box::pin(async move {
            // Resolve target audio parameters
            let (target_sample_rate, target_channels) = match &plan.settings.sample_rate {
                SampleRateConfig::Explicit(rate) => (*rate, plan.settings.channels.channel_count() as i32),
                SampleRateConfig::Auto => {
                    // Fallback to first input's properties; if unavailable, use DEFAULT_SAMPLE_RATE
                    let first = plan.input_file_paths.first()
                        .ok_or_else(|| AppError::InvalidInput("No input files provided".to_string()))?;
                    let ictx = ff::format::input(&first).map_err(|e| AppError::General(format!("Open input failed: {e}")))?;
                    let stream = ictx.streams()
                        .best(ff::media::Type::Audio)
                        .ok_or_else(|| AppError::InvalidInput("No audio stream in first input".to_string()))?;
                    let codec_ctx = ff::codec::context::Context::from_parameters(stream.parameters())
                        .map_err(|e| AppError::General(format!("Decoder ctx from params failed: {e}")))?;
                    let decoder = codec_ctx.decoder().audio()
                        .map_err(|e| AppError::General(format!("Open audio decoder failed: {e}")))?;
                    (decoder.rate(), decoder.channels() as i32)
                }
            };

            // Prepare output muxer and encoder
            let mut octx = ff::format::output(&plan.output_path)
                .map_err(|e| AppError::General(format!("Create output failed: {e}")))?;

            let codec = ff::encoder::find(ff::codec::Id::AAC)
                .ok_or_else(|| AppError::General("AAC encoder not found".to_string()))?;

            // Compute global header flag before borrowing stream
            let requires_global_header = octx.format().flags().contains(ff::format::flag::Flags::GLOBAL_HEADER);

            let mut ost = octx.add_stream(codec)
                .map_err(|e| AppError::General(format!("Add output stream failed: {e}")))?;

            let channel_layout = ff::channel_layout::ChannelLayout::default(target_channels);

            // Choose a reasonable sample format (fallback to planar f32)
            let sample_format = ff::format::Sample::F32(ff::format::sample::Type::Planar);

            let time_base = ff::Rational(1, target_sample_rate as i32);

            let mut opened = ff::codec::context::Context::new()
                .encoder()
                .audio()
                .map_err(|e| AppError::General(format!("Open encoder failed: {e}")))?;
            opened.set_bit_rate(((plan.settings.bitrate as i64) * 1000) as usize);
            opened.set_rate(target_sample_rate as i32);
            opened.set_channel_layout(channel_layout);
            opened.set_format(sample_format);
            opened.set_time_base(time_base);
            // Some containers require global header on encoder
            if requires_global_header {
                opened.set_flags(ff::codec::flag::Flags::GLOBAL_HEADER);
            }
            let mut enc_ctx = opened.open_as(codec)
                .map_err(|e| AppError::General(format!("Final open encoder failed: {e}")))?;

            ost.set_time_base(enc_ctx.time_base());
            ost.set_parameters(&enc_ctx);
            let ost_index = ost.index();
            let ost_time_base = ost.time_base();
            drop(ost);

            octx.write_header().map_err(|e| AppError::General(format!("Write header failed: {e}")))?;

            // Resampler from input-decoder fmt → encoder fmt
            let mut running_pts: i64 = 0; // in encoder time_base units
            let total_duration = plan.total_duration.max(0.001);
            let mut last_emit = std::time::Instant::now();

            // Progress emitter
            let emitter = crate::audio::progress::ProgressEmitter::new(context.window.clone());

            for (idx, in_path) in plan.input_file_paths.iter().enumerate() {
                if context.is_cancelled() {
                    return Err(AppError::InvalidInput("Processing was cancelled".into()));
                }

                let mut ictx = ff::format::input(&in_path)
                    .map_err(|e| AppError::General(format!("Open input failed: {e}")))?;
                let istream = ictx.streams()
                    .best(ff::media::Type::Audio)
                    .ok_or_else(|| AppError::InvalidInput(format!("No audio stream in input {}", in_path.display())))?;
                let stream_index = istream.index();
                let dec_ctx = ff::codec::context::Context::from_parameters(istream.parameters())
                    .map_err(|e| AppError::General(format!("Decoder ctx from params failed: {e}")))?;
                let mut decoder = dec_ctx.decoder().audio()
                    .map_err(|e| AppError::General(format!("Open audio decoder failed: {e}")))?;

                // Build resampler for this input stream
                let in_layout = decoder.channel_layout();
                let in_rate = decoder.rate();
                let in_format = decoder.format();
                let mut resampler = ff::software::resampling::Context::get(
                    in_format,
                    in_layout,
                    in_rate,
                    enc_ctx.format(),
                    enc_ctx.channel_layout(),
                    enc_ctx.rate(),
                ).map_err(|e| AppError::General(format!("Create resampler failed: {e}")))?;

                // Read packets/frames
                for (si, packet) in ictx.packets() {
                    if context.is_cancelled() {
                        return Err(AppError::InvalidInput("Processing was cancelled".into()));
                    }
                    if si.index() != stream_index { continue; }

                    decoder.send_packet(&packet)
                        .map_err(|e| AppError::General(format!("Decoder send failed: {e}")))?;
                    loop {
                        let mut frame = ff::frame::Audio::empty();
                        match decoder.receive_frame(&mut frame) {
                            Ok(()) => {
                                // Resample to encoder format
                                let mut out = ff::frame::Audio::empty();
                                out.set_format(enc_ctx.format());
                                out.set_channel_layout(enc_ctx.channel_layout());
                                out.set_rate(enc_ctx.rate());
                                resampler.run(&frame, &mut out)
                                    .map_err(|e| AppError::General(format!("Resample failed: {e}")))?;

                                // Set PTS in encoder time_base
                                out.set_pts(Some(running_pts));
                                running_pts += out.samples() as i64;

                                // Encode and write
                                enc_ctx.send_frame(&out)
                                    .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;
                                let mut pkt = ff::Packet::empty();
                                while enc_ctx.receive_packet(&mut pkt).is_ok() {
                                    pkt.set_stream(ost_index);
                                    pkt.rescale_ts(enc_ctx.time_base(), ost_time_base);
                                    pkt.write_interleaved(&mut octx)
                                        .map_err(|e| AppError::General(format!("Write packet failed: {e}")))?;
                                }

                                // Progress emit every ~200ms
                                if last_emit.elapsed() > std::time::Duration::from_millis(200) {
                                    last_emit = std::time::Instant::now();
                                    let current_seconds = running_pts as f64 / target_sample_rate as f64;
                                    let file_progress = (current_seconds / total_duration).clamp(0.0, 1.0);
                                    let percentage = super::constants::PROGRESS_CONVERTING_START as f64 + (file_progress * super::constants::PROGRESS_RANGE_MULTIPLIER);
                                    emitter.emit_converting_progress(
                                        percentage.min(super::constants::PROGRESS_CONVERTING_MAX as f64) as f32,
                                        "Converting and merging audio files...",
                                        Some(format!("Input {} of {}", idx + 1, plan.input_file_paths.len())),
                                        None,
                                    );
                                }
                            }
                            Err(ff::Error::Other { .. }) | Err(ff::Error::Eof) => break,
                            Err(e) => return Err(AppError::General(format!("Decoder receive failed: {e}"))),
                        }
                    }
                }

                // Flush decoder for this input
                decoder.send_eof().ok();
                loop {
                    let mut frame = ff::frame::Audio::empty();
                    match decoder.receive_frame(&mut frame) {
                        Ok(()) => {
                            let mut out = ff::frame::Audio::empty();
                            out.set_format(enc_ctx.format());
                            out.set_channel_layout(enc_ctx.channel_layout());
                            out.set_rate(enc_ctx.rate());
                            resampler.run(&frame, &mut out)
                                .map_err(|e| AppError::General(format!("Resample failed: {e}")))?;
                            out.set_pts(Some(running_pts));
                            running_pts += out.samples() as i64;
                            enc_ctx.send_frame(&out)
                                .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;
                            let mut pkt = ff::Packet::empty();
                            while enc_ctx.receive_packet(&mut pkt).is_ok() {
                                pkt.set_stream(ost_index);
                                pkt.rescale_ts(enc_ctx.time_base(), ost_time_base);
                                pkt.write_interleaved(&mut octx)
                                    .map_err(|e| AppError::General(format!("Write packet failed: {e}")))?;
                            }
                        }
                        Err(ff::Error::Eof) | Err(ff::Error::Other { .. }) => break,
                        Err(e) => return Err(AppError::General(format!("Decoder flush failed: {e}"))),
                    }
                }
            }

            // Flush encoder and write remaining packets
            enc_ctx.send_eof().ok();
            let mut pkt = ff::Packet::empty();
            while enc_ctx.receive_packet(&mut pkt).is_ok() {
                pkt.set_stream(ost_index);
                pkt.rescale_ts(enc_ctx.time_base(), ost_time_base);
                pkt.write_interleaved(&mut octx)
                    .map_err(|e| AppError::General(format!("Write packet failed: {e}")))?;
            }

            octx.write_trailer().map_err(|e| AppError::General(format!("Write trailer failed: {e}")))?;
            Ok(())
        })
    }
}

/// Builds FFmpeg command for merging audio files
/// 
/// This function encapsulates all FFmpeg command construction logic,
/// providing a stable interface for audio processing operations.
pub fn build_merge_command(
    concat_file: &Path,
    output: &Path,
    settings: &AudioSettings,
    file_paths: &[PathBuf],
) -> Result<Command> {
    let ffmpeg_path = crate::ffmpeg::locate_ffmpeg()?;
    
    // Resolve sample rate (auto-detect if needed)
    let sample_rate = match &settings.sample_rate {
        SampleRateConfig::Explicit(rate) => *rate,
        SampleRateConfig::Auto => detect_input_sample_rate(file_paths)?,
    };
    
    // Log the resolved FFmpeg path once per invocation (helps debug env issues)
    log::info!("Using FFmpeg binary: {}", ffmpeg_path.display());

    let mut cmd = Command::new(&ffmpeg_path);
    cmd.args([
        "-f", FFMPEG_CONCAT_FORMAT,
        "-safe", FFMPEG_CONCAT_SAFE_MODE,
        "-i", &concat_file.to_string_lossy(),
        "-vn",  // Disable video processing (ignore album artwork)
        "-map", "0:a",  // Only map audio streams
        "-map_metadata", "0",  // Preserve metadata from first input
        "-c:a", FFMPEG_AUDIO_CODEC,
        "-b:a", &format!("{}k", settings.bitrate),
        "-ar", &sample_rate.to_string(),
        "-ac", &settings.channels.channel_count().to_string(),
        "-progress", FFMPEG_PROGRESS_PIPE,  // Enable progress output to stderr
        "-nostats",  // Disable normal stats output to avoid interference
        "-y",  // Overwrite output file
        &output.to_string_lossy(),
    ]);
    
    cmd.stderr(Stdio::piped());
    cmd.stdout(Stdio::piped());

    // Emit a debug-friendly preview of the command that can be copy-pasted
    let cmd_preview = format!(
        "{} -f {} -safe {} -i {} -vn -map 0:a -map_metadata 0 -c:a {} -b:a {}k -ar {} -ac {} -progress {} -nostats -y {}",
        ffmpeg_path.display(),
        FFMPEG_CONCAT_FORMAT,
        FFMPEG_CONCAT_SAFE_MODE,
        concat_file.to_string_lossy(),
        FFMPEG_AUDIO_CODEC,
        settings.bitrate,
        sample_rate,
        settings.channels.channel_count(),
        FFMPEG_PROGRESS_PIPE,
        output.to_string_lossy()
    );
    log::info!("FFmpeg command preview: {cmd_preview}");
    
    Ok(cmd)
}

/// Executes FFmpeg command with context-based progress tracking
/// 
/// This function provides a unified interface for executing FFmpeg commands
/// with proper progress monitoring and cancellation support.
pub async fn execute_ffmpeg_with_progress_context(
    cmd: Command,
    context: &ProcessingContext,
    total_duration: f64,
) -> Result<()> {
    log::debug!("Starting FFmpeg execution with progress tracking");
    
    // Set up process execution
    let mut execution = setup_process_execution(cmd, context)?;
    
    // Monitor process with progress updates
    monitor_process_with_progress(&mut execution, context, total_duration)?;
    
    // Finalize and check exit status
    finalize_process_execution(execution, context)?;
    
    log::debug!("FFmpeg execution completed successfully");
    Ok(())
}

/// ADAPTER: Executes command with progress tracking (legacy compatibility)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility by converting parameters
/// to use the new context-based approach internally.
#[deprecated = "Use execute_ffmpeg_with_progress_context for new code - this adapter maintains compatibility"]
#[allow(dead_code)]
pub async fn execute_with_progress_events(
    cmd: Command,
    window: &tauri::Window,
    state: &tauri::State<'_, crate::ProcessingState>,
    total_duration: f64,
) -> Result<()> {
    // Convert legacy parameters to context-based approach
    let session = create_session_from_legacy_state(state)?;
    let context = ProcessingContext::new(window.clone(), session, AudioSettings::default());
    // Note: We use default settings here since they're not available in the legacy adapter
    
    execute_ffmpeg_with_progress_context(cmd, &context, total_duration).await
}

/// ADAPTER: Builds merge command (legacy compatibility)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility for existing code
/// that calls build_merge_command directly.
#[deprecated = "Use MediaProcessingPlan::build_ffmpeg_command for new code - this adapter maintains compatibility"]
#[allow(dead_code)]
pub fn build_merge_command_legacy(
    concat_file: &Path,
    output: &Path,
    settings: &AudioSettings,
    file_paths: &[PathBuf],
) -> Result<Command> {
    build_merge_command(concat_file, output, settings, file_paths)
}
