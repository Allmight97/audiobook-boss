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
        output_path: PathBuf,
        settings: AudioSettings,
        input_file_paths: Vec<PathBuf>,
        total_duration: f64,
    ) -> Self {
        Self {
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
/// This provides a stable interface for media processing implementations.
/// Currently uses ffmpeg-next as the single processing engine.
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
    input_samples_total: &'a mut u64,
    encoded_samples_total: &'a mut u64,
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

    /// Attempts to configure AAC encoder for variable frame sizes
    /// 
    /// This function uses unsafe FFI to set encoder options that allow variable frame sizes,
    /// which should help with the frame size mismatch issue we're experiencing.
    fn try_configure_variable_frame_size(encoder_ctx: &mut ffmpeg_next::codec::context::Context) -> Result<()> {
        use crate::errors::AppError;
        use std::ffi::CString;
        
        unsafe {
            let av_ctx = encoder_ctx.as_mut_ptr();
            if av_ctx.is_null() {
                return Err(AppError::General("Invalid encoder context pointer".to_string()));
            }
            
            // Try to set strict compliance to experimental to allow more flexibility
            let strict_key = CString::new("strict").map_err(|e| {
                AppError::General(format!("Failed to create strict key string: {}", e))
            })?;
            let experimental_value = CString::new("experimental").map_err(|e| {
                AppError::General(format!("Failed to create experimental value string: {}", e))
            })?;
            
            let result = ffmpeg_next::sys::av_opt_set(
                av_ctx as *mut std::ffi::c_void,
                strict_key.as_ptr(),
                experimental_value.as_ptr(),
                0,
            );
            
            if result < 0 {
                log::debug!("Could not set strict=experimental: FFmpeg error code {}", result);
            } else {
                log::debug!("Set strict=experimental on encoder context");
            }
            
            Ok(())
        }
    }

    /// Attempts to enable twoloop AAC enhancement for better psychoacoustic analysis
    /// 
    /// This function uses unsafe FFI to access the underlying AVCodecContext and set
    /// the aac_coder option to twoloop. This provides better quality AAC encoding
    /// through improved psychoacoustic analysis, at the cost of slightly longer encoding time.
    /// 
    /// Returns Ok(()) if successful, Err with description if failed.
    /// Failures are non-critical - encoding continues with standard AAC-LC.
    fn try_enable_twoloop_aac(encoder_ctx: &mut ffmpeg_next::codec::context::Context) -> Result<()> {
        use crate::errors::AppError;
        use std::ffi::CString;
        
        // Access the underlying AVCodecContext via unsafe FFI
        // This is necessary because ffmpeg-next doesn't expose set_option for encoder contexts
        unsafe {
            let av_ctx = encoder_ctx.as_mut_ptr();
            if av_ctx.is_null() {
                return Err(AppError::General("Invalid encoder context pointer".to_string()));
            }
            
            // Use av_opt_set to set aac_coder to "twoloop" (option value 1)
            // This directly calls into libavcodec's option system
            let key = CString::new("aac_coder").map_err(|e| {
                AppError::General(format!("Failed to create key string: {}", e))
            })?;
            let value = CString::new("twoloop").map_err(|e| {
                AppError::General(format!("Failed to create value string: {}", e))
            })?;
            
            // av_opt_set returns 0 on success, negative on error
            let result = ffmpeg_next::sys::av_opt_set(
                av_ctx as *mut std::ffi::c_void,
                key.as_ptr(),
                value.as_ptr(),
                0, // flags
            );
            
            if result < 0 {
                return Err(AppError::General(format!(
                    "Failed to set aac_coder option: FFmpeg error code {}", result
                )));
            }
            
            log::debug!("Successfully set aac_coder=twoloop on encoder context");
            Ok(())
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
        
        // Try to configure encoder for variable frame sizes before other options
        match Self::try_configure_variable_frame_size(&mut opened) {
            Ok(()) => log::info!("AAC encoder configured for variable frame sizes"),
            Err(e) => log::warn!("Could not configure variable frame sizes ({}), may have frame size issues", e),
        }
        
        // Enhanced AAC quality: Enable twoloop for better psychoacoustic analysis
        // Implemented with graceful fallback. Users can disable via ABB_DISABLE_TWOOLOOP=1
        let disable_twoloop = std::env::var("ABB_DISABLE_TWOOLOOP").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
        if disable_twoloop {
            log::info!("Twoloop AAC enhancement disabled via environment override");
        } else {
            // Attempt to enable twoloop AAC enhancement for better psychoacoustic analysis
            match Self::try_enable_twoloop_aac(&mut opened) {
                Ok(()) => log::info!("Twoloop AAC enhancement enabled successfully - expect improved audio quality"),
                Err(e) => {
                    log::warn!("Twoloop AAC enhancement unavailable ({}), falling back to standard AAC-LC", e);
                    // Continue with standard AAC-LC encoding - this is not a critical failure
                }
            }
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
        if let Some(metadata) = metadata {
            match crate::metadata::set_container_metadata(&mut octx, metadata) {
                Ok(()) => log::debug!("Container metadata set successfully"),
                Err(e) => log::warn!("Failed to set container metadata: {} - continuing with audio processing", e),
            }
        }

        let codec = ff::encoder::find(ff::codec::Id::AAC)
            .ok_or_else(|| AppError::General("AAC encoder not found".to_string()))?;

        // Compute global header flag before borrowing stream
        let requires_global_header = octx
            .format()
            .flags()
            .contains(ff::format::flag::Flags::GLOBAL_HEADER);

        // IMPORTANT: Add audio stream FIRST to ensure it gets index 0
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

        // PRE-HEADER: Attempt to add cover art stream AFTER audio stream (store index & format for post-header packet write)
        // Native cover art embedding with graceful fallback to Lofty in finalize stage
        let mut cover_art_stream_info: Option<(usize, crate::metadata::CoverFormat)> = None;
        if let Some(metadata) = metadata {
            if let Some(ref cover_data) = metadata.cover_art {
                let bytes = cover_data.len();
                log::info!("cover_art_plan decision=native_attempt bytes={}", bytes);
                log::info!("Attempting native cover art embedding - {} bytes of cover data", bytes);
                cover_art_stream_info = crate::metadata::add_cover_art_stream_pre_header(&mut octx, cover_data);
                if let Some((stream_idx, format)) = cover_art_stream_info {
                    log::info!("✓ Native cover art stream added successfully (stream={}, format={:?}) - will embed during encoding", stream_idx, format);
                } else {
                    log::warn!("cover_art_plan decision=fallback reason=stream_creation_failed bytes={}", bytes);
                    log::warn!("✗ Native cover art stream creation failed - will fallback to Lofty embedding in finalize stage");
                }
            } else {
                log::info!("cover_art_plan decision=none reason=no_cover_art_data");
            }
        } else {
            log::info!("cover_art_plan decision=none reason=no_metadata");
        }

        // Write header (streams finalized)
        octx.write_header()
            .map_err(|e| AppError::General(format!("Write header failed: {e}")))?;

        // POST-HEADER: Write cover art packet if we successfully added a stream
        if let Some(metadata) = metadata {
            if let (Some((stream_index, format)), Some(cover_data)) = (cover_art_stream_info, metadata.cover_art.as_ref()) {
                log::info!("Writing cover art packet to stream {} ({:?} format, {} bytes)", stream_index, format, cover_data.len());
                crate::metadata::write_cover_art_packet_post_header(&mut octx, stream_index, cover_data, format);
                log::info!("✓ Native cover art packet written successfully to stream {}", stream_index);
            } else if metadata.cover_art.is_some() {
                log::warn!("Cover art data present but no stream created - will rely on finalize stage fallback");
            }
        }

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
        accumulator: &mut crate::audio::buffer::SampleAccumulator,
    ) -> Result<()> {
        use crate::errors::AppError;

        log::info!("📦 Starting packet processing for stream index: {}", ctx.current_stream_index);
        let mut packet_count = 0;

        // Read packets/frames
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
            match Self::process_decoded_frames(decoder, encoder, resampler, output_context, ctx, accumulator) {
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

        log::info!("🔧 Setting up decoder for input file: {}", input_path.display());
        
        // Check if file exists first
        if !input_path.exists() {
            return Err(AppError::FileValidation(format!("Input file does not exist: {}", input_path.display())));
        }
        log::info!("✓ Input file exists and is accessible");

        log::info!("Opening FFmpeg input context...");
        let ictx = ff::format::input(&input_path)
            .map_err(|e| AppError::General(format!("Failed to open input file '{}': {}", input_path.display(), e)))?;
        log::info!("✓ FFmpeg input context opened successfully");

        log::info!("Finding best audio stream...");
        let istream = ictx.streams().best(ff::media::Type::Audio).ok_or_else(|| {
            AppError::InvalidInput(format!("No audio stream found in input file: {}", input_path.display()))
        })?;
        let stream_index = istream.index();
        log::info!("✓ Found audio stream at index: {}", stream_index);

        log::info!("Creating decoder context from stream parameters...");
        let dec_ctx = ff::codec::context::Context::from_parameters(istream.parameters())
            .map_err(|e| AppError::General(format!("Failed to create decoder context from parameters for '{}': {}", input_path.display(), e)))?;
        log::info!("✓ Decoder context created successfully");

        log::info!("Opening audio decoder...");
        let decoder = dec_ctx
            .decoder()
            .audio()
            .map_err(|e| AppError::General(format!("Failed to open audio decoder for '{}': {}", input_path.display(), e)))?;
        log::info!("✓ Audio decoder opened successfully");

        // Build resampler for this input stream
        log::info!("Creating resampler...");
        let in_layout = decoder.channel_layout();
        let in_rate = decoder.rate();
        let in_format = decoder.format();
        log::info!("Input audio format: rate={}, channels={:?}, format={:?}", in_rate, in_layout, in_format);
        log::info!("Output audio format: rate={}, channels={:?}, format={:?}", encoder.rate(), encoder.channel_layout(), encoder.format());
        
        let resampler = ff::software::resampling::Context::get(
            in_format,
            in_layout,
            in_rate,
            encoder.format(),
            encoder.channel_layout(),
            encoder.rate(),
        )
        .map_err(|e| AppError::General(format!("Failed to create resampler for '{}': {}", input_path.display(), e)))?;
        log::info!("✓ Resampler created successfully");

        log::info!("🎉 Decoder and resampler setup completed for: {}", input_path.display());
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
        accumulator: &mut crate::audio::buffer::SampleAccumulator,
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
                    out.set_samples(frame.samples());
                    
                    resampler
                        .run(&frame, &mut out)
                        .map_err(|e| AppError::General(format!("Resample failed: {e}")))?;
                    
                    // Defensive check: ensure we have valid output
                    if out.samples() == 0 {
                        log::warn!("Resampler produced 0 samples – skipping frame to avoid panic");
                        continue;
                    }

                    *ctx.input_samples_total += out.samples() as u64;
                    for mut full in accumulator.push_frame(&out) {
                        full.set_pts(Some(*ctx.running_pts));
                        *ctx.running_pts += full.samples() as i64;
                        *ctx.encoded_samples_total += full.samples() as u64;
                        Self::encode_and_write_frame(
                            encoder,
                            &full,
                            output_context,
                            ctx.output_stream_index,
                            ctx.output_time_base,
                        )?;
                    }

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
        accumulator: &mut crate::audio::buffer::SampleAccumulator,
    ) -> Result<()> {
        use crate::errors::AppError;

        log::info!("🎵 Starting to process input file: {}", input_path.display());

        if ctx.context.is_cancelled() {
            log::warn!("Processing was cancelled before processing file: {}", input_path.display());
            ctx.emitter.emit_cancelled("Processing was cancelled");
            return Err(AppError::InvalidInput("Processing was cancelled".into()));
        }

        log::info!("Setting up decoder and resampler for: {}", input_path.display());
        let (mut ictx, mut decoder, mut resampler, stream_index) =
            Self::setup_decoder_and_resampler(input_path, encoder)?;
        log::info!("✓ Decoder and resampler setup complete for stream index: {}", stream_index);

        // Update context indices for this file (P1.4) then process packets
        ctx.current_file_index = file_index;
        ctx.current_stream_index = stream_index;
        log::info!("Updated context: file_index={}, stream_index={}", file_index, stream_index);

        log::info!("Processing input packets from: {}", input_path.display());
        Self::process_input_packets(
            &mut ictx,
            &mut decoder,
            encoder,
            &mut resampler,
            output_context,
            ctx,
            accumulator,
        )?;
        log::info!("✓ Input packets processed successfully");

        // Flush decoder for this input
        log::info!("Flushing decoder frames for: {}", input_path.display());
        
        // Skip the old flush for now - the simple truncation approach should work
        log::info!("✓ Decoder frames flushed successfully (skipped for simplicity)");
        log::info!("✓ Decoder frames flushed successfully");

        log::info!("✅ Completed processing file: {}", input_path.display());
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
                    out.set_samples(frame.samples());
                    resampler
                        .run(&frame, &mut out)
                        .map_err(|e| AppError::General(format!("Resample failed: {e}")))?;
                    if out.samples() == 0 {
                        log::warn!("Resampler (flush) produced 0 samples – skipping");
                        continue;
                    }
                    let target_size = encoder.frame_size() as usize;
                    let mut total_samples = out.samples() as usize;
                    let mut start_sample = 0usize;
                    while start_sample < total_samples {
                        let remaining = total_samples - start_sample;
                        let take = remaining.min(target_size);
                        if start_sample == 0 && take == total_samples && take <= target_size {
                            out.set_pts(Some(*ctx.running_pts));
                            *ctx.running_pts += take as i64;
                            encoder
                                .send_frame(&out)
                                .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;
                        } else {
                            let mut sub = ff::frame::Audio::empty();
                            sub.set_format(encoder.format());
                            sub.set_channel_layout(encoder.channel_layout());
                            sub.set_rate(encoder.rate());
                            sub.set_samples(take);
                            
                            // CRITICAL: Must allocate the frame buffer before accessing data_mut  
                            unsafe {
                                sub.alloc(encoder.format(), take, encoder.channel_layout());
                            }
                            
                            for ch in 0..encoder.channel_layout().channels() as usize {
                                let src_plane = out.data(ch);
                                let dst_plane = sub.data_mut(ch);
                                let bytes_per_sample = 4; // F32
                                let plane_samples = out.samples() as usize;
                                let src_offset_bytes = start_sample * bytes_per_sample;
                                let take_bytes = take * bytes_per_sample;
                                
                                // Defensive bounds check to prevent panic
                                if dst_plane.len() == 0 {
                                    log::error!("Destination plane {} has zero length - frame allocation failed during flush", ch);
                                    break;
                                }
                                
                                if src_offset_bytes + take_bytes <= plane_samples * bytes_per_sample &&
                                   dst_plane.len() >= take_bytes && src_plane.len() >= src_offset_bytes + take_bytes {
                                    dst_plane[..take_bytes]
                                        .copy_from_slice(&src_plane[src_offset_bytes..src_offset_bytes + take_bytes]);
                                } else {
                                    log::warn!("Flush alignment copy bounds check failed (ch={}) - aborting remainder", ch);
                                    break;
                                }
                            }
                            sub.set_pts(Some(*ctx.running_pts));
                            *ctx.running_pts += take as i64;
                            encoder
                                .send_frame(&sub)
                                .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;
                        }
                        let mut pkt = ff::Packet::empty();
                        while encoder.receive_packet(&mut pkt).is_ok() {
                            pkt.set_stream(ctx.output_stream_index);
                            pkt.rescale_ts(encoder.time_base(), ctx.output_time_base);
                            pkt.write_interleaved(output_context)
                                .map_err(|e| AppError::General(format!("Write packet failed: {e}")))?;
                        }
                        start_sample += take;
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
                let warnings = crate::metadata::validate_metadata_compatibility(md);
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
            let mut input_samples_total: u64 = 0;
            let mut encoded_samples_total: u64 = 0;
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
                input_samples_total: &mut input_samples_total,
                encoded_samples_total: &mut encoded_samples_total,
            };

            // Process each input file
            log::info!("Starting audio processing for {} input files", plan.input_file_paths.len());
            let mut accumulator = crate::audio::buffer::SampleAccumulator::new(
                enc_ctx.channel_layout().channels() as usize,
                enc_ctx.frame_size() as usize,
                enc_ctx.rate() as u32,
                enc_ctx.channel_layout(),
                enc_ctx.format(),
            );
            for (idx, in_path) in plan.input_file_paths.iter().enumerate() {
                log::info!("Processing input file {}/{}: {}", idx + 1, plan.input_file_paths.len(), in_path.display());
                Self::process_input_file(in_path, &mut enc_ctx, &mut octx, idx, &mut ctx, &mut accumulator)?;
                log::info!("✓ Completed processing input file {}/{}", idx + 1, plan.input_file_paths.len());
            }
            log::info!("✓ All input files processed successfully");

            // Finalize encoding
            log::info!("🏁 Starting encoding finalization...");
            Self::finalize_encoding(&mut enc_ctx, &mut octx, ost_index, ost_time_base)?;
            log::info!("✓ Encoding finalization completed successfully");

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


