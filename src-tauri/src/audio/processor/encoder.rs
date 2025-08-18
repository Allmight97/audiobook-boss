//! Encoder setup and packet writing utilities (behavior-preserving extraction)

use crate::errors::Result;
use ffmpeg_next as ff;

/// Attempts to configure AAC encoder for variable frame sizes.
fn try_configure_variable_frame_size(encoder_ctx: &mut ff::codec::context::Context) -> Result<()> {
    use crate::errors::AppError;
    use std::ffi::CString;

    unsafe {
        let av_ctx = encoder_ctx.as_mut_ptr();
        if av_ctx.is_null() {
            return Err(AppError::General("Invalid encoder context pointer".to_string()));
        }

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

/// Attempts to enable twoloop AAC enhancement for better psychoacoustic analysis.
fn try_enable_twoloop_aac(encoder_ctx: &mut ff::codec::context::Context) -> Result<()> {
    use crate::errors::AppError;
    use std::ffi::CString;

    unsafe {
        let av_ctx = encoder_ctx.as_mut_ptr();
        if av_ctx.is_null() {
            return Err(AppError::General("Invalid encoder context pointer".to_string()));
        }

        let key = CString::new("aac_coder").map_err(|e| {
            AppError::General(format!("Failed to create key string: {}", e))
        })?;
        let value = CString::new("twoloop").map_err(|e| {
            AppError::General(format!("Failed to create value string: {}", e))
        })?;

        let result = ffmpeg_next::sys::av_opt_set(
            av_ctx as *mut std::ffi::c_void,
            key.as_ptr(),
            value.as_ptr(),
            0,
        );

        if result < 0 {
            return Err(AppError::General(format!(
                "Failed to set aac_coder option: FFmpeg error code {}",
                result
            )));
        }

        log::debug!("Successfully set aac_coder=twoloop on encoder context");
        Ok(())
    }
}

/// Resolves target sample rate and channels from plan settings or first input file.
fn resolve_target_audio_params(
    plan: &crate::audio::media_pipeline::MediaProcessingPlan,
) -> Result<(u32, i32)> {
    use crate::audio::{SampleRateConfig};
    use crate::errors::AppError;

    match &plan.settings.sample_rate {
        SampleRateConfig::Explicit(rate) => Ok((*rate, plan.settings.channels.channel_count() as i32)),
        SampleRateConfig::Auto => {
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
            Ok((decoder.rate(), decoder.channels() as i32))
        }
    }
}

/// Creates and configures the audio encoder
pub(crate) fn create_audio_encoder(
    plan: &crate::audio::media_pipeline::MediaProcessingPlan,
    target_sample_rate: u32,
    target_channels: i32,
    requires_global_header: bool,
) -> Result<ff::codec::encoder::audio::Encoder> {
    use crate::errors::AppError;

    let codec = ff::encoder::find(ff::codec::Id::AAC)
        .ok_or_else(|| AppError::General("AAC encoder not found".to_string()))?;

    let channel_layout = ff::channel_layout::ChannelLayout::default(target_channels);
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

    match try_configure_variable_frame_size(&mut opened) {
        Ok(()) => log::info!("AAC encoder configured for variable frame sizes"),
        Err(e) => log::warn!("Could not configure variable frame sizes ({}), may have frame size issues", e),
    }

    let disable_twoloop = std::env::var("ABB_DISABLE_TWOOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if disable_twoloop {
        log::info!("Twoloop AAC enhancement disabled via environment override");
    } else {
        match try_enable_twoloop_aac(&mut opened) {
            Ok(()) => log::info!("Twoloop AAC enhancement enabled successfully - expect improved audio quality"),
            Err(e) => log::warn!("Twoloop AAC enhancement unavailable ({}), falling back to standard AAC-LC", e),
        }
    }

    if requires_global_header {
        opened.set_flags(ff::codec::flag::Flags::GLOBAL_HEADER);
    }
    let enc_ctx = opened
        .open_as(codec)
        .map_err(|e| AppError::General(format!("Final open encoder failed: {e}")))?;

    Ok(enc_ctx)
}

/// Sets up the output encoder context and stream with metadata support.
/// Returns (output_context, encoder_context, output_stream_index, output_time_base, target_sample_rate)
pub(crate) fn setup_encoder(
    plan: &crate::audio::media_pipeline::MediaProcessingPlan,
    metadata: Option<&crate::metadata::AudiobookMetadata>,
) -> Result<(
    ff::format::context::Output,
    ff::codec::encoder::audio::Encoder,
    usize,
    ff::Rational,
    u32,
)> {
    use crate::errors::AppError;

    let (target_sample_rate, target_channels) = resolve_target_audio_params(plan)?;

    let mut octx = ff::format::output(&plan.output_path)
        .map_err(|e| AppError::General(format!("Create output failed: {e}")))?;

    if let Some(metadata) = metadata {
        match crate::metadata::set_container_metadata(&mut octx, metadata) {
            Ok(()) => log::debug!("Container metadata set successfully"),
            Err(e) => log::warn!("Failed to set container metadata: {} - continuing with audio processing", e),
        }
    }

    let codec = ff::encoder::find(ff::codec::Id::AAC)
        .ok_or_else(|| AppError::General("AAC encoder not found".to_string()))?;

    let requires_global_header = octx
        .format()
        .flags()
        .contains(ff::format::flag::Flags::GLOBAL_HEADER);

    let mut ost = octx
        .add_stream(codec)
        .map_err(|e| AppError::General(format!("Add output stream failed: {e}")))?;

    let enc_ctx = create_audio_encoder(
        plan,
        target_sample_rate,
        target_channels,
        requires_global_header,
    )?;

    ost.set_time_base(enc_ctx.time_base());
    ost.set_parameters(&enc_ctx);
    let ost_index = ost.index();
    let ost_time_base = ost.time_base();

    // Pre-header cover art stream attempt
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

    // Header
    octx.write_header()
        .map_err(|e| AppError::General(format!("Write header failed: {e}")))?;

    // Post-header cover art packet
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

#[cfg(debug_assertions)]
fn debug_validate_frame_contract(
    frame: &ff::frame::Audio,
    encoder: &ff::codec::encoder::audio::Encoder,
) {
    // Format/layout/rate must match encoder
    debug_assert_eq!(frame.format(), encoder.format(), "Frame format must match encoder format");
    debug_assert_eq!(frame.channel_layout(), encoder.channel_layout(), "Channel layout mismatch");
    debug_assert_eq!(frame.rate(), encoder.rate(), "Sample rate mismatch");

    // Samples must be > 0 and respect encoder frame size if non-zero
    let samples_i64 = frame.samples() as i64;
    debug_assert!(samples_i64 > 0, "Frame must contain at least one sample");
    let enc_frame_size_i64 = encoder.frame_size() as i64;
    if enc_frame_size_i64 > 0 {
        debug_assert!(samples_i64 <= enc_frame_size_i64, "Frame samples exceed encoder.frame_size()");
    }

    // PTS should be set
    debug_assert!(frame.pts().is_some(), "Frame PTS must be set before encoding");
}

/// Encodes frame and writes packets to output
pub(crate) fn encode_and_write_frame(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    frame: &ff::frame::Audio,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
) -> Result<()> {
    use crate::errors::AppError;

    #[cfg(debug_assertions)]
    {
        debug_validate_frame_contract(frame, encoder);
    }

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

/// Flushes the encoder and writes the output trailer
pub(crate) fn finalize_encoding(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
) -> Result<()> {
    use crate::errors::AppError;

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


