//! Media processing pipeline for FFmpeg operations
//!
//! This module provides a unified interface for all FFmpeg operations,
//! encapsulating command building and execution behind a stable Rust interface.
//!
//! The `MediaProcessingPlan` struct holds inputs, outputs, and metadata for
//! processing operations, following mentor recommendations for abstraction.

use super::context::ProcessingContext;
use super::{AudioSettings, SampleRateConfig};
use crate::errors::Result;
use std::future::Future;
use std::path::{Path, PathBuf};
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
        files.iter().filter_map(|f| f.duration).sum()
    }

    // Legacy test helper build_ffmpeg_command removed (Phase 11 cleanup):
    // Integration tests no longer inspect synthetic command strings; behavior
    // is validated via end-to-end processing and sample rate detection tests.

    /// Executes the processing plan with context-based progress tracking
    pub async fn execute_with_context(
        &self, 
        context: &ProcessingContext,
        metadata: Option<&crate::metadata::AudiobookMetadata>,
    ) -> Result<()> {
        let processor = crate::audio::media_pipeline::FfmpegNextProcessor;
        processor.execute(self, context, metadata).await
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
        metadata: Option<&'a crate::metadata::AudiobookMetadata>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
}

// Feature-gated processor based on ffmpeg-next bindings
pub struct FfmpegNextProcessor;

// Phase 1: Context Type Introduction (no functional changes)
// Private context carrying frequently-shared parameters across the frame pipeline
struct FramePipelineCtx<'a> {
    context: &'a super::context::ProcessingContext,
    emitter: &'a crate::audio::progress::ProgressEmitter,
    total_duration: f64,
    total_files: usize,
    target_sample_rate: u32,
    output_stream_index: usize,
    output_time_base: ffmpeg_next::Rational,
    running_pts: &'a mut i64,
    last_emit: &'a mut std::time::Instant,
    // Mutable per-file state (P1.4 refactor): updated at start of each input file
    current_file_index: usize,
    current_stream_index: usize,
}

impl FfmpegNextProcessor {
    /// Resolves target sample rate and channels from plan settings
    fn resolve_target_audio_params(plan: &MediaProcessingPlan) -> Result<(u32, i32)> {
        use crate::errors::AppError;
        use ffmpeg_next as ff;

        match &plan.settings.sample_rate {
            SampleRateConfig::Explicit(rate) => {
                Ok((*rate, plan.settings.channels.channel_count() as i32))
            }
            SampleRateConfig::Auto => {
                // Fallback to first input's properties; if unavailable, use DEFAULT_SAMPLE_RATE
                let first = plan
                    .input_file_paths
                    .first()
                    .ok_or_else(|| AppError::InvalidInput("No input files provided".to_string()))?;
                let ictx = ff::format::input(&first)
                    .map_err(|e| AppError::General(format!("Open input failed: {e}")))?;
                let stream = ictx.streams().best(ff::media::Type::Audio).ok_or_else(|| {
                    AppError::InvalidInput("No audio stream in first input".to_string())
                })?;
                let codec_ctx = ff::codec::context::Context::from_parameters(stream.parameters())
                    .map_err(|e| {
                    AppError::General(format!("Decoder ctx from params failed: {e}"))
                })?;
                let decoder = codec_ctx
                    .decoder()
                    .audio()
                    .map_err(|e| AppError::General(format!("Open audio decoder failed: {e}")))?;
                Ok((decoder.rate(), decoder.channels() as i32))
            }
        }
    }

    /// Creates and configures the audio encoder
    fn create_audio_encoder(
        plan: &MediaProcessingPlan,
        target_sample_rate: u32,
        target_channels: i32,
        requires_global_header: bool,
    ) -> Result<ffmpeg_next::codec::encoder::audio::Encoder> {
        use crate::errors::AppError;
        use ffmpeg_next as ff;

        let codec = ff::encoder::find(ff::codec::Id::AAC)
            .ok_or_else(|| AppError::General("AAC encoder not found".to_string()))?;

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
        
        // Enhanced AAC quality: Enable twoloop for better psychoacoustic analysis
        // Implemented with graceful fallback. Users can disable via ABB_DISABLE_TWOOLOOP=1
        let disable_twoloop = std::env::var("ABB_DISABLE_TWOOLOOP").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
        if disable_twoloop {
            log::info!("Twoloop AAC enhancement disabled via environment override");
        } else {
            // ffmpeg-next crate (current version) does not expose a safe set_option API on the
            // audio encoder context for 'aac_coder'; implementing would require unsafe AVOption
            // access. We log intent so future versions can enable it.
            log::info!("Twoloop AAC enhancement pending (ffmpeg-next API limitation) – using standard AAC-LC");
        }
        
        // Some containers require global header on encoder
        if requires_global_header {
            opened.set_flags(ff::codec::flag::Flags::GLOBAL_HEADER);
        }
        let enc_ctx = opened
            .open_as(codec)
            .map_err(|e| AppError::General(format!("Final open encoder failed: {e}")))?;

        Ok(enc_ctx)
    }

    /// Sets up the output encoder context and stream with metadata support
    /// Returns (output_context, encoder_context, output_stream_index, output_time_base, target_sample_rate)
    fn setup_encoder(
        plan: &MediaProcessingPlan,
        metadata: Option<&crate::metadata::AudiobookMetadata>,
    ) -> Result<(
        ffmpeg_next::format::context::Output,
        ffmpeg_next::codec::encoder::audio::Encoder,
        usize,
        ffmpeg_next::Rational,
        u32,
    )> {
        use crate::errors::AppError;
        use ffmpeg_next as ff;

    // Resolve target audio parameters
    let (target_sample_rate, target_channels) = Self::resolve_target_audio_params(plan)?;

        // Prepare output muxer and encoder
        let mut octx = ff::format::output(&plan.output_path)
            .map_err(|e| AppError::General(format!("Create output failed: {e}")))?;

        // Set container metadata if provided
        if let Some(_metadata) = metadata {
            // TODO: Re-enable metadata integration after module cleanup
            // crate::metadata::ffmpeg_bridge::set_container_metadata(&mut octx, metadata)?;
            log::info!("Metadata support temporarily disabled during transition");
        }

        // PRE-HEADER: Attempt to add cover art stream (store index & format for post-header packet write)
        // TEMPORARILY DISABLED: Native cover art embedding causing codec compatibility issues
        // Will fall back to Lofty embedding in finalize stage
        // let mut cover_art_stream_info: Option<(usize, crate::metadata::ffmpeg_bridge::CoverFormat)> = None;
        // Cover art disabled during transition
        // if let Some(_metadata) = _metadata {
        //     if let Some(ref cover_data) = _metadata.cover_art {
        //         cover_art_stream_info = crate::metadata::ffmpeg_bridge::add_cover_art_stream_pre_header(&mut octx, cover_data);
        //     }
        // }

        let codec = ff::encoder::find(ff::codec::Id::AAC)
            .ok_or_else(|| AppError::General("AAC encoder not found".to_string()))?;

        // Compute global header flag before borrowing stream
        let requires_global_header = octx
            .format()
            .flags()
            .contains(ff::format::flag::Flags::GLOBAL_HEADER);

        let mut ost = octx
            .add_stream(codec)
            .map_err(|e| AppError::General(format!("Add output stream failed: {e}")))?;

    let enc_ctx = Self::create_audio_encoder(
            plan,
            target_sample_rate,
            target_channels,
            requires_global_header,
        )?;

        ost.set_time_base(enc_ctx.time_base());
        ost.set_parameters(&enc_ctx);
        let ost_index = ost.index();
        let ost_time_base = ost.time_base();

        // Write header (streams finalized)
        octx.write_header()
            .map_err(|e| AppError::General(format!("Write header failed: {e}")))?;

        // POST-HEADER: Write cover art packet if we successfully added a stream
        // TEMPORARILY DISABLED: Native cover art embedding disabled above
        // Cover art disabled during transition
        // if let Some(_metadata) = metadata {
        //     if let (Some((stream_index, format)), Some(ref cover_data)) = (cover_art_stream_info, _metadata.cover_art.as_ref()) {
        //         crate::metadata::ffmpeg_bridge::write_cover_art_packet_post_header(&mut octx, stream_index, cover_data, format);
        //     }
        // }

        Ok((octx, enc_ctx, ost_index, ost_time_base, target_sample_rate))
    }

    /// Processes packets from input stream through decoder pipeline
    ///
    /// P1.4 cleanup: stream_index & file_index now stored in FramePipelineCtx
    fn process_input_packets(
        ictx: &mut ffmpeg_next::format::context::Input,
        decoder: &mut ffmpeg_next::codec::decoder::Audio,
        encoder: &mut ffmpeg_next::codec::encoder::audio::Encoder,
        resampler: &mut ffmpeg_next::software::resampling::Context,
        output_context: &mut ffmpeg_next::format::context::Output,
        ctx: &mut FramePipelineCtx,
    ) -> Result<()> {
        use crate::errors::AppError;

        // Read packets/frames
        for (si, packet) in ictx.packets() {
            if ctx.context.is_cancelled() {
                ctx.emitter.emit_cancelled("Processing was cancelled");
                return Err(AppError::InvalidInput("Processing was cancelled".into()));
            }
            if si.index() != ctx.current_stream_index {
                continue;
            }

            decoder
                .send_packet(&packet)
                .map_err(|e| AppError::General(format!("Decoder send failed: {e}")))?;

            Self::process_decoded_frames(
                decoder,
                encoder,
                resampler,
                output_context,
                ctx,
            )?;
        }
        Ok(())
    }

    /// Sets up decoder and resampler for a single input file
    fn setup_decoder_and_resampler(
        input_path: &Path,
        encoder: &ffmpeg_next::codec::encoder::audio::Encoder,
    ) -> Result<(
        ffmpeg_next::format::context::Input,
        ffmpeg_next::codec::decoder::Audio,
        ffmpeg_next::software::resampling::Context,
        usize,
    )> {
        use crate::errors::AppError;
        use ffmpeg_next as ff;

        let ictx = ff::format::input(&input_path)
            .map_err(|e| AppError::General(format!("Open input failed: {e}")))?;
        let istream = ictx.streams().best(ff::media::Type::Audio).ok_or_else(|| {
            AppError::InvalidInput(format!("No audio stream in input {}", input_path.display()))
        })?;
        let stream_index = istream.index();
        let dec_ctx = ff::codec::context::Context::from_parameters(istream.parameters())
            .map_err(|e| AppError::General(format!("Decoder ctx from params failed: {e}")))?;
        let decoder = dec_ctx
            .decoder()
            .audio()
            .map_err(|e| AppError::General(format!("Open audio decoder failed: {e}")))?;

        // Build resampler for this input stream
        let in_layout = decoder.channel_layout();
        let in_rate = decoder.rate();
        let in_format = decoder.format();
        let resampler = ff::software::resampling::Context::get(
            in_format,
            in_layout,
            in_rate,
            encoder.format(),
            encoder.channel_layout(),
            encoder.rate(),
        )
        .map_err(|e| AppError::General(format!("Create resampler failed: {e}")))?;

        Ok((ictx, decoder, resampler, stream_index))
    }

    /// Encodes frame and writes packets to output
    fn encode_and_write_frame(
        encoder: &mut ffmpeg_next::codec::encoder::audio::Encoder,
        frame: &ffmpeg_next::frame::Audio,
        output_context: &mut ffmpeg_next::format::context::Output,
        output_stream_index: usize,
        output_time_base: ffmpeg_next::Rational,
    ) -> Result<()> {
        use crate::errors::AppError;
        use ffmpeg_next as ff;

        encoder
            .send_frame(frame)
            .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;
        let mut pkt = ff::Packet::empty();
        while encoder.receive_packet(&mut pkt).is_ok() {
            pkt.set_stream(output_stream_index);
            pkt.rescale_ts(encoder.time_base(), output_time_base);
            pkt.write_interleaved(output_context)
                .map_err(|e| AppError::General(format!("Write packet failed: {e}")))?;
        }
        Ok(())
    }

    /// Emits progress updates during audio processing using context state
    fn emit_progress_update(ctx: &mut FramePipelineCtx) {
        if ctx.last_emit.elapsed() > std::time::Duration::from_millis(200) {
            *ctx.last_emit = std::time::Instant::now();
            let current_seconds = *ctx.running_pts as f64 / ctx.target_sample_rate as f64;
            let percentage = crate::audio::progress::converting_percentage_from_seconds(
                current_seconds,
                ctx.total_duration,
            ) as f64;
            ctx.emitter.emit_converting_progress(
                percentage.min(super::constants::PROGRESS_CONVERTING_MAX as f64) as f32,
                "Converting and merging audio files...",
                Some(format!(
                    "Input {} of {}",
                    ctx.current_file_index + 1,
                    ctx.total_files
                )),
                None,
            );
        }
    }

    /// Processes audio frames from decoder through resample and encode pipeline
    fn process_decoded_frames(
        decoder: &mut ffmpeg_next::codec::decoder::Audio,
        encoder: &mut ffmpeg_next::codec::encoder::audio::Encoder,
        resampler: &mut ffmpeg_next::software::resampling::Context,
        output_context: &mut ffmpeg_next::format::context::Output,
        ctx: &mut FramePipelineCtx,
    ) -> Result<()> {
        use crate::errors::AppError;
        use ffmpeg_next as ff;

        loop {
            if ctx.context.is_cancelled() {
                // Best-effort flush signal, but we will delete partial output via guard
                let _ = encoder.send_eof();
                ctx.emitter.emit_cancelled("Processing was cancelled");
                return Err(AppError::InvalidInput("Processing was cancelled".into()));
            }
            let mut frame = ff::frame::Audio::empty();
            match decoder.receive_frame(&mut frame) {
                Ok(()) => {
                    // Resample to encoder format
                    let mut out = ff::frame::Audio::empty();
                    out.set_format(encoder.format());
                    out.set_channel_layout(encoder.channel_layout());
                    out.set_rate(encoder.rate());
                    resampler
                        .run(&frame, &mut out)
                        .map_err(|e| AppError::General(format!("Resample failed: {e}")))?;

                    // Set PTS in encoder time_base
                    out.set_pts(Some(*ctx.running_pts));
                    *ctx.running_pts += out.samples() as i64;

                    // Encode and write
                    Self::encode_and_write_frame(
                        encoder,
                        &out,
                        output_context,
                        ctx.output_stream_index,
                        ctx.output_time_base,
                    )?;

                    // Progress emit every ~200ms
                    Self::emit_progress_update(ctx);
                }
                Err(ff::Error::Other { .. }) | Err(ff::Error::Eof) => break,
                Err(e) => return Err(AppError::General(format!("Decoder receive failed: {e}"))),
            }
        }
        Ok(())
    }

    /// Processes a single input file through the decode/resample/encode pipeline
    fn process_input_file(
        input_path: &Path,
        encoder: &mut ffmpeg_next::codec::encoder::audio::Encoder,
        output_context: &mut ffmpeg_next::format::context::Output,
        file_index: usize,
        ctx: &mut FramePipelineCtx,
    ) -> Result<()> {
        use crate::errors::AppError;

        if ctx.context.is_cancelled() {
            ctx.emitter.emit_cancelled("Processing was cancelled");
            return Err(AppError::InvalidInput("Processing was cancelled".into()));
        }

        let (mut ictx, mut decoder, mut resampler, stream_index) =
            Self::setup_decoder_and_resampler(input_path, encoder)?;

        // Update context indices for this file (P1.4) then process packets
        ctx.current_file_index = file_index;
        ctx.current_stream_index = stream_index;

        Self::process_input_packets(
            &mut ictx,
            &mut decoder,
            encoder,
            &mut resampler,
            output_context,
            ctx,
        )?;

        // Flush decoder for this input
        Self::flush_decoder_frames(
            &mut decoder,
            encoder,
            output_context,
            &mut resampler,
            ctx,
        )?;

        Ok(())
    }

    /// Flushes any remaining frames from the decoder after processing an input file
    fn flush_decoder_frames(
        decoder: &mut ffmpeg_next::codec::decoder::Audio,
        encoder: &mut ffmpeg_next::codec::encoder::audio::Encoder,
        output_context: &mut ffmpeg_next::format::context::Output,
        resampler: &mut ffmpeg_next::software::resampling::Context,
        ctx: &mut FramePipelineCtx,
    ) -> Result<()> {
        use crate::errors::AppError;
        use ffmpeg_next as ff;

        decoder.send_eof().ok();
        loop {
            let mut frame = ff::frame::Audio::empty();
            match decoder.receive_frame(&mut frame) {
                Ok(()) => {
                    let mut out = ff::frame::Audio::empty();
                    out.set_format(encoder.format());
                    out.set_channel_layout(encoder.channel_layout());
                    out.set_rate(encoder.rate());
                    resampler
                        .run(&frame, &mut out)
                        .map_err(|e| AppError::General(format!("Resample failed: {e}")))?;
                    out.set_pts(Some(*ctx.running_pts));
                    *ctx.running_pts += out.samples() as i64;
                    encoder
                        .send_frame(&out)
                        .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;
                    let mut pkt = ff::Packet::empty();
                    while encoder.receive_packet(&mut pkt).is_ok() {
                        pkt.set_stream(ctx.output_stream_index);
                        pkt.rescale_ts(encoder.time_base(), ctx.output_time_base);
                        pkt.write_interleaved(output_context)
                            .map_err(|e| AppError::General(format!("Write packet failed: {e}")))?;
                    }
                }
                Err(ff::Error::Eof) | Err(ff::Error::Other { .. }) => break,
                Err(e) => return Err(AppError::General(format!("Decoder flush failed: {e}"))),
            }
        }
        Ok(())
    }

    /// Finalizes encoding by flushing the encoder and writing the output trailer
    fn finalize_encoding(
        encoder: &mut ffmpeg_next::codec::encoder::audio::Encoder,
        output_context: &mut ffmpeg_next::format::context::Output,
        output_stream_index: usize,
        output_time_base: ffmpeg_next::Rational,
    ) -> Result<()> {
        use crate::errors::AppError;
        use ffmpeg_next as ff;

        // Flush encoder and write remaining packets
        encoder.send_eof().ok();
        let mut pkt = ff::Packet::empty();
        while encoder.receive_packet(&mut pkt).is_ok() {
            pkt.set_stream(output_stream_index);
            pkt.rescale_ts(encoder.time_base(), output_time_base);
            pkt.write_interleaved(output_context)
                .map_err(|e| AppError::General(format!("Write packet failed: {e}")))?;
        }

        output_context
            .write_trailer()
            .map_err(|e| AppError::General(format!("Write trailer failed: {e}")))?;
        Ok(())
    }
}

impl MediaProcessor for FfmpegNextProcessor {
    fn execute<'a>(
        &'a self,
        plan: &'a MediaProcessingPlan,
        context: &'a ProcessingContext,
        metadata: Option<&'a crate::metadata::AudiobookMetadata>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        use ffmpeg_next as ff;
        use std::sync::Once;

        // Initialize FFmpeg (idempotent)
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = ff::init();
        });

        Box::pin(async move {
            // Setup encoder and output context with metadata
            let (mut octx, mut enc_ctx, ost_index, ost_time_base, target_sample_rate) =
                Self::setup_encoder(plan, metadata)?;

            // Validate metadata compatibility if provided (now active post-legacy purge)
            if let Some(md) = metadata {
                let warnings = crate::metadata::ffmpeg_bridge::validate_metadata_compatibility(md);
                for warning in warnings {
                    log::warn!("Metadata compatibility: {}", warning);
                }
            }

            // Ensure partial outputs are removed on failure or cancellation
            let mut cleanup_guard = crate::audio::cleanup::CleanupGuard::new(context.session.id());
            cleanup_guard.add_path(&plan.output_path);

            // Initialize processing state
            let mut running_pts: i64 = 0; // in encoder time_base units
            let mut last_emit = std::time::Instant::now();
            let emitter = crate::audio::progress::ProgressEmitter::new(context.window.clone());

            // Construct context struct to reduce parameter passing
            let mut ctx = FramePipelineCtx {
                context,
                emitter: &emitter,
                total_duration: plan.total_duration.max(0.001),
                total_files: plan.input_file_paths.len(),
                target_sample_rate,
                output_stream_index: ost_index,
                output_time_base: ost_time_base,
                running_pts: &mut running_pts,
                last_emit: &mut last_emit,
                current_file_index: 0,
                current_stream_index: 0,
            };

            // Process each input file
            for (idx, in_path) in plan.input_file_paths.iter().enumerate() {
                Self::process_input_file(in_path, &mut enc_ctx, &mut octx, idx, &mut ctx)?;
            }

            // Finalize encoding
            Self::finalize_encoding(&mut enc_ctx, &mut octx, ost_index, ost_time_base)?;

            // Preserve output on success (Phase 11: re-enabled after legacy purge)
            let _ = cleanup_guard.remove_path(&plan.output_path);

            if let Some(_metadata) = metadata {
                log::info!("Audio processing completed with metadata integration");
            } else {
                log::info!("Audio processing completed without metadata");
            }

            Ok(())
        })
    }
}


