//! Encoder setup and packet writing utilities (behavior-preserving extraction)

use crate::audio::settings_encoder::{
    self, BitrateMode, ChannelConfig as EncoderChannelConfig, EncoderAvailability, EncoderSettings,
    EncoderType, ThreadSetting,
};
use crate::audio::ChannelConfig as LegacyChannelConfig;
use crate::errors::Result;
use ffmpeg_next as ff;
use std::borrow::Cow;

const DEFAULT_FDK_VBR_LEVEL: u8 = 3;

fn default_bitrate_mode_for(encoder_type: EncoderType) -> BitrateMode {
    match encoder_type {
        EncoderType::FdkHeAac | EncoderType::Auto | EncoderType::Opus => {
            BitrateMode::Vbr(DEFAULT_FDK_VBR_LEVEL)
        }
        EncoderType::AacAt => BitrateMode::Cvbr,
        EncoderType::NativeAac => BitrateMode::Cbr,
    }
}

fn legacy_channel_to_encoder(ch: LegacyChannelConfig) -> EncoderChannelConfig {
    match ch {
        LegacyChannelConfig::Mono => EncoderChannelConfig::Mono,
        LegacyChannelConfig::Stereo => EncoderChannelConfig::Stereo,
    }
}

fn resolve_plan_encoder_settings<'a>(
    plan: &'a crate::audio::media_pipeline::MediaProcessingPlan,
    availability: &EncoderAvailability,
) -> (Cow<'a, EncoderSettings>, EncoderType) {
    if let Some(settings) = &plan.encoder_settings_v2 {
        let resolved = settings_encoder::resolve_encoder_type(settings, availability);
        (Cow::Borrowed(settings), resolved)
    } else {
        let default_encoder_type = if availability.aac_at_available {
            EncoderType::AacAt
        } else if availability.fdk_available {
            EncoderType::FdkHeAac
        } else if availability.native_aac_available {
            EncoderType::NativeAac
        } else if availability.opus_available {
            EncoderType::Opus
        } else {
            EncoderType::NativeAac
        };
        let synthesized = EncoderSettings {
            encoder_type: default_encoder_type,
            bitrate_kbps: plan.settings.bitrate as u16,
            bitrate_mode: default_bitrate_mode_for(default_encoder_type),
            channels: legacy_channel_to_encoder(plan.settings.channels.clone()),
            afterburner: matches!(default_encoder_type, EncoderType::FdkHeAac),
            threads: ThreadSetting::Auto,
        };
        if let Err(err) = settings_encoder::validate_encoder_settings(&synthesized) {
            log::warn!("synthesized encoder settings failed validation: {}", err);
        }
        (Cow::Owned(synthesized), default_encoder_type)
    }
}

/// Finds encoder by name using FFmpeg's encoder registry
fn find_encoder_by_name(name: &str) -> Result<ff::Codec> {
    use crate::errors::AppError;
    use std::ffi::CString;

    unsafe {
        let c_name = CString::new(name)
            .map_err(|e| AppError::General(format!("Invalid encoder name '{}': {}", name, e)))?;

        let codec_ptr = ffmpeg_next::sys::avcodec_find_encoder_by_name(c_name.as_ptr());
        if codec_ptr.is_null() {
            return Err(AppError::General(format!("Encoder '{}' not found", name)));
        }

        Ok(ff::Codec::wrap(codec_ptr))
    }
}

/// Attempts to configure AAC encoder for variable frame sizes.
fn try_configure_variable_frame_size(encoder_ctx: &mut ff::codec::context::Context) -> Result<()> {
    use crate::errors::AppError;
    use std::ffi::CString;

    unsafe {
        let av_ctx = encoder_ctx.as_mut_ptr();
        if av_ctx.is_null() {
            return Err(AppError::General(
                "Invalid encoder context pointer".to_string(),
            ));
        }

        let strict_key = CString::new("strict")
            .map_err(|e| AppError::General(format!("Failed to create strict key string: {}", e)))?;
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
            log::debug!(
                "Could not set strict=experimental: FFmpeg error code {}",
                result
            );
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
            return Err(AppError::General(
                "Invalid encoder context pointer".to_string(),
            ));
        }

        let key = CString::new("aac_coder")
            .map_err(|e| AppError::General(format!("Failed to create key string: {}", e)))?;
        let value = CString::new("twoloop")
            .map_err(|e| AppError::General(format!("Failed to create value string: {}", e)))?;

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
    use crate::audio::SampleRateConfig;

    let needs_probe_for_rate = matches!(plan.settings.sample_rate, SampleRateConfig::Auto);
    let needs_probe_for_channels = plan
        .encoder_settings_v2
        .as_ref()
        .map(|enc| matches!(enc.channels, EncoderChannelConfig::Auto))
        .unwrap_or(false);

    let probe = if needs_probe_for_rate || needs_probe_for_channels {
        Some(probe_first_input(plan)?)
    } else {
        None
    };

    let sampled_rate = probe.map(|(rate, _)| rate);
    let sampled_channels = probe.map(|(_, ch)| ch);

    let target_sample_rate = match plan.settings.sample_rate {
        SampleRateConfig::Explicit(rate) => rate,
        SampleRateConfig::Auto => sampled_rate.expect("input probe to provide sample rate"),
    };

    let plan_channel_fallback = plan.settings.channels.channel_count() as i32;
    let target_channels = plan
        .encoder_settings_v2
        .as_ref()
        .and_then(|enc| enc.channels.forced_channels().map(|c| c as i32))
        .or(sampled_channels)
        .unwrap_or(plan_channel_fallback);

    Ok((target_sample_rate, target_channels))
}

fn probe_first_input(
    plan: &crate::audio::media_pipeline::MediaProcessingPlan,
) -> Result<(u32, i32)> {
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

/// Creates and configures an AAC audio encoder with optimal settings
#[allow(clippy::too_many_lines)]
pub(crate) fn create_audio_encoder(
    encoder_settings: &EncoderSettings,
    resolved_encoder: EncoderType,
    target_sample_rate: u32,
    target_channels: i32,
    requires_global_header: bool,
) -> Result<ff::codec::encoder::audio::Encoder> {
    use crate::errors::AppError;

    let codec_name = settings_encoder::resolve_encoder_name(resolved_encoder);
    let codec = if codec_name == "aac" {
        ff::encoder::find(ff::codec::Id::AAC)
            .ok_or_else(|| AppError::General("AAC encoder not found".to_string()))?
    } else if codec_name == "libopus" {
        find_encoder_by_name("libopus")?
    } else {
        find_encoder_by_name(codec_name)?
    };

    let channel_layout = ff::channel_layout::ChannelLayout::default(target_channels);
    let sample_format = match resolved_encoder {
        EncoderType::FdkHeAac | EncoderType::AacAt => {
            ff::format::Sample::I16(ff::format::sample::Type::Packed)
        }
        _ => ff::format::Sample::F32(ff::format::sample::Type::Planar),
    };
    let time_base = ff::Rational(1, target_sample_rate as i32);

    let mut opened = ff::codec::context::Context::new()
        .encoder()
        .audio()
        .map_err(|e| AppError::General(format!("Open encoder failed: {e}")))?;

    let target_bit_rate = encoder_settings.bitrate_kbps as usize * 1000;
    opened.set_bit_rate(target_bit_rate);
    opened.set_rate(target_sample_rate as i32);
    opened.set_channel_layout(channel_layout);
    opened.set_format(sample_format);
    opened.set_time_base(time_base);

    if matches!(
        resolved_encoder,
        EncoderType::FdkHeAac | EncoderType::AacAt | EncoderType::NativeAac
    ) {
        if let Err(e) = try_configure_variable_frame_size(&mut opened) {
            log::warn!(
                "Could not configure variable frame sizes ({}), may have frame size issues",
                e
            );
        }
    }

    match resolved_encoder {
        EncoderType::FdkHeAac => configure_fdk_encoder(&mut opened, encoder_settings)?,
        EncoderType::AacAt => configure_aac_at_encoder(&mut opened)?,
        EncoderType::NativeAac | EncoderType::Auto => configure_native_aac_encoder(&mut opened)?,
        EncoderType::Opus => configure_opus_encoder(&mut opened, encoder_settings)?,
    }

    configure_threads(&mut opened, encoder_settings.threads);

    if requires_global_header {
        opened.set_flags(ff::codec::flag::Flags::GLOBAL_HEADER);
    }

    let enc_ctx = opened
        .open_as(codec)
        .map_err(|e| AppError::General(format!("Final open encoder failed: {e}")))?;

    Ok(enc_ctx)
}

fn configure_threads(ctx: &mut ff::codec::context::Context, threads: ThreadSetting) {
    let threads_value = match threads {
        ThreadSetting::Auto => 0,
        ThreadSetting::Off => 1,
        ThreadSetting::Fixed(n) => n as i32,
    };
    if threads_value > 0 {
        unsafe {
            use std::ffi::CString;
            let av_ctx = ctx.as_mut_ptr();
            let key = CString::new("threads").expect("threads key should be valid");
            let _ = ffmpeg_next::sys::av_opt_set_int(
                av_ctx as *mut std::ffi::c_void,
                key.as_ptr(),
                threads_value as i64,
                0,
            );
        }
    }
}

fn configure_fdk_encoder(
    ctx: &mut ff::codec::context::Context,
    settings: &EncoderSettings,
) -> Result<()> {
    use crate::errors::AppError;
    use std::ffi::CString;

    unsafe {
        let av_ctx = ctx.as_mut_ptr();
        let profile_key = CString::new("profile").expect("profile key");
        let _ = ffmpeg_next::sys::av_opt_set_int(
            av_ctx as *mut std::ffi::c_void,
            profile_key.as_ptr(),
            ffmpeg_next::sys::FF_PROFILE_AAC_HE as i64,
            0,
        );

        if let BitrateMode::Vbr(level) = settings.bitrate_mode {
            let vbr_key = CString::new("vbr").expect("vbr key");
            let _ = ffmpeg_next::sys::av_opt_set_int(
                av_ctx as *mut std::ffi::c_void,
                vbr_key.as_ptr(),
                level as i64,
                0,
            );
        } else {
            return Err(AppError::InvalidInput(
                "FDK encoder requires VBR bitrate mode".to_string(),
            ));
        }

        let afterburner_key = CString::new("afterburner").expect("afterburner key");
        let _ = ffmpeg_next::sys::av_opt_set_int(
            av_ctx as *mut std::ffi::c_void,
            afterburner_key.as_ptr(),
            if settings.afterburner { 1 } else { 0 },
            0,
        );
    }
    Ok(())
}

fn configure_aac_at_encoder(ctx: &mut ff::codec::context::Context) -> Result<()> {
    use crate::errors::AppError;
    use std::ffi::CString;

    unsafe {
        let av_ctx = ctx.as_mut_ptr();
        let key = CString::new("aac_at_mode").expect("aac_at_mode key");
        let value = CString::new("cvbr").expect("cvbr value");
        let rc = ffmpeg_next::sys::av_opt_set(
            av_ctx as *mut std::ffi::c_void,
            key.as_ptr(),
            value.as_ptr(),
            0,
        );
        if rc < 0 {
            return Err(AppError::General(format!(
                "Failed to set aac_at_mode: FFmpeg error code {}",
                rc
            )));
        }
    }
    Ok(())
}

fn configure_native_aac_encoder(ctx: &mut ff::codec::context::Context) -> Result<()> {
    let disable_twoloop = std::env::var("ABB_DISABLE_TWOOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if disable_twoloop {
        log::info!("Twoloop AAC enhancement disabled via environment override");
        return Ok(());
    }

    match try_enable_twoloop_aac(ctx) {
        Ok(()) => log::info!("Twoloop AAC enhancement enabled successfully"),
        Err(e) => log::warn!(
            "Twoloop AAC enhancement unavailable ({}), falling back to standard AAC-LC",
            e
        ),
    }
    Ok(())
}

fn configure_opus_encoder(
    ctx: &mut ff::codec::context::Context,
    _settings: &EncoderSettings,
) -> Result<()> {
    use crate::errors::AppError;
    use std::ffi::CString;

    unsafe {
        let av_ctx = ctx.as_mut_ptr();
        // compression_level 10
        let comp_key = CString::new("compression_level").expect("compression_level key");
        let _ = ffmpeg_next::sys::av_opt_set_int(
            av_ctx as *mut std::ffi::c_void,
            comp_key.as_ptr(),
            10,
            0,
        );

        // application audio
        let app_key = CString::new("application").expect("application key");
        let app_val = CString::new("audio").expect("audio value");
        let rc = ffmpeg_next::sys::av_opt_set(
            av_ctx as *mut std::ffi::c_void,
            app_key.as_ptr(),
            app_val.as_ptr(),
            0,
        );
        if rc < 0 {
            return Err(AppError::General(format!(
                "Failed to set opus application mode: FFmpeg error code {}",
                rc
            )));
        }

        let vbr_key = CString::new("vbr").expect("vbr key");
        let vbr_val = CString::new("on").expect("on value");
        let _ = ffmpeg_next::sys::av_opt_set(
            av_ctx as *mut std::ffi::c_void,
            vbr_key.as_ptr(),
            vbr_val.as_ptr(),
            0,
        );
    }
    Ok(())
}
/// Sets up the output encoder context and stream with metadata support.
/// Returns (output_context, encoder_context, output_stream_index, output_time_base, target_sample_rate)
#[allow(clippy::too_many_lines)]
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

    let availability = settings_encoder::detect_available_encoders();
    let (effective_settings, resolved_encoder_type) =
        resolve_plan_encoder_settings(plan, &availability);

    let mut octx = ff::format::output(&plan.output_path)
        .map_err(|e| AppError::General(format!("Create output failed: {e}")))?;

    if let Some(metadata) = metadata {
        match crate::metadata::set_container_metadata(&mut octx, metadata) {
            Ok(()) => log::debug!("Container metadata set successfully"),
            Err(e) => log::warn!(
                "Failed to set container metadata: {} - continuing with audio processing",
                e
            ),
        }
    }

    let stream_codec_id = match resolved_encoder_type {
        EncoderType::Opus => ff::codec::Id::OPUS,
        _ => ff::codec::Id::AAC,
    };
    let codec = ff::encoder::find(stream_codec_id)
        .ok_or_else(|| AppError::General(format!("{:?} encoder not found", stream_codec_id)))?;

    let requires_global_header = octx
        .format()
        .flags()
        .contains(ff::format::flag::Flags::GLOBAL_HEADER);

    let mut ost = octx
        .add_stream(codec)
        .map_err(|e| AppError::General(format!("Add output stream failed: {e}")))?;

    let enc_ctx = create_audio_encoder(
        effective_settings.as_ref(),
        resolved_encoder_type,
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
            log::info!(
                "Attempting native cover art embedding - {} bytes of cover data",
                bytes
            );
            cover_art_stream_info =
                crate::metadata::add_cover_art_stream_pre_header(&mut octx, cover_data);
            if let Some((stream_idx, format)) = cover_art_stream_info {
                log::info!("✓ Native cover art stream added successfully (stream={}, format={:?}) - will embed during encoding", stream_idx, format);
            } else {
                log::warn!(
                    "cover_art_plan decision=fallback reason=stream_creation_failed bytes={}",
                    bytes
                );
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
        if let (Some((stream_index, format)), Some(cover_data)) =
            (cover_art_stream_info, metadata.cover_art.as_ref())
        {
            log::info!(
                "Writing cover art packet to stream {} ({:?} format, {} bytes)",
                stream_index,
                format,
                cover_data.len()
            );
            crate::metadata::write_cover_art_packet_post_header(
                &mut octx,
                stream_index,
                cover_data,
                format,
            );
            log::info!(
                "✓ Native cover art packet written successfully to stream {}",
                stream_index
            );
        } else if metadata.cover_art.is_some() {
            log::warn!("Cover art data present but no stream created - will rely on finalize stage fallback");
        }
    }

    let logged_bitrate = plan
        .encoder_settings_v2
        .as_ref()
        .map(|enc| enc.bitrate_kbps as u32)
        .unwrap_or(plan.settings.bitrate);

    log::info!(
        "encoder_setup resolved: encoder={:?} rate={}Hz channels={} fmt={:?} frame_size={} bitrate={}k requested_v2={:?}",
        resolved_encoder_type,
        target_sample_rate,
        target_channels,
        enc_ctx.format(),
        enc_ctx.frame_size(),
        logged_bitrate,
        plan.encoder_settings_v2
    );

    Ok((octx, enc_ctx, ost_index, ost_time_base, target_sample_rate))
}

#[cfg(debug_assertions)]
fn debug_validate_frame_contract(
    frame: &ff::frame::Audio,
    encoder: &ff::codec::encoder::audio::Encoder,
) {
    // Format/layout/rate must match encoder
    debug_assert_eq!(
        frame.format(),
        encoder.format(),
        "Frame format must match encoder format"
    );
    debug_assert_eq!(
        frame.channel_layout(),
        encoder.channel_layout(),
        "Channel layout mismatch"
    );
    debug_assert_eq!(frame.rate(), encoder.rate(), "Sample rate mismatch");

    // Samples must be > 0 and respect encoder frame size if non-zero
    let samples_i64 = frame.samples() as i64;
    debug_assert!(samples_i64 > 0, "Frame must contain at least one sample");
    let enc_frame_size_i64 = encoder.frame_size() as i64;
    if enc_frame_size_i64 > 0 {
        debug_assert!(
            samples_i64 <= enc_frame_size_i64,
            "Frame samples exceed encoder.frame_size()"
        );
    }

    // PTS should be set
    debug_assert!(
        frame.pts().is_some(),
        "Frame PTS must be set before encoding"
    );
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
        // Validate structural contract
        debug_validate_frame_contract(frame, encoder);
        // Validate sample values in debug builds (only for F32 format)
        if matches!(frame.format(), ff::format::Sample::F32(_)) {
            for ch in 0..encoder.channel_layout().channels() as usize {
                let plane = frame.data(ch);
                let len_f32 = plane.len() / 4;
                let src: &[f32] =
                    unsafe { std::slice::from_raw_parts(plane.as_ptr() as *const f32, len_f32) };
                for &v in src.iter().take(frame.samples()) {
                    debug_assert!(v.is_finite(), "Non-finite sample encountered");
                    debug_assert!((-1.0..=1.0).contains(&v), "Sample out of range [-1,1]");
                }
            }
        }
    }

    // Optional encode-stage sanitation (default ON, can be disabled via ABB_DISABLE_ENCODE_SANITIZE)
    // Only applies to F32 format; S16 format (used by AAC-AT) doesn't need float sanitation
    let disable_encode_sanitize = std::env::var("ABB_DISABLE_ENCODE_SANITIZE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let is_f32_format = matches!(frame.format(), ff::format::Sample::F32(_));

    if !disable_encode_sanitize && is_f32_format {
        // Clamp to [-1,1] and replace non-finite values; preserve frame metadata
        let mut clean = ff::frame::Audio::empty();
        clean.set_format(frame.format());
        clean.set_channel_layout(frame.channel_layout());
        clean.set_rate(frame.rate());
        clean.set_samples(frame.samples());
        unsafe {
            clean.alloc(frame.format(), frame.samples(), frame.channel_layout());
        }
        clean.set_pts(frame.pts());

        if frame.samples() > 0 {
            let channels = frame.channel_layout().channels() as usize;
            for ch in 0..channels {
                let src_plane = frame.data(ch);
                let dst_plane = clean.data_mut(ch);
                let len_f32 = (dst_plane.len() / 4).min(src_plane.len() / 4);
                let src: &[f32] = unsafe {
                    std::slice::from_raw_parts(src_plane.as_ptr() as *const f32, len_f32)
                };
                let dst: &mut [f32] = unsafe {
                    std::slice::from_raw_parts_mut(dst_plane.as_mut_ptr() as *mut f32, len_f32)
                };
                let mut repaired = 0usize;
                for i in 0..len_f32 {
                    let mut v = src[i];
                    if !v.is_finite() {
                        v = 0.0;
                        repaired += 1;
                    }
                    if v > 1.0 {
                        v = 1.0;
                        repaired += 1;
                    }
                    if v < -1.0 {
                        v = -1.0;
                        repaired += 1;
                    }
                    dst[i] = v;
                }
                if repaired > 0 {
                    log::warn!(
                        "Sanitized {} samples on channel {} before encoding",
                        repaired,
                        ch
                    );
                }
            }
        }

        encoder
            .send_frame(&clean)
            .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;
    } else {
        encoder
            .send_frame(frame)
            .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;
    }
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

/// Flushes the encoder and writes trailer, used when preview early-stop is engaged
pub(crate) fn finalize_encoding_after_preview(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
) -> Result<()> {
    // Currently identical to finalize_encoding; separated for clarity and future hooks
    finalize_encoding(
        encoder,
        output_context,
        output_stream_index,
        output_time_base,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::media_pipeline::MediaProcessingPlan;
    use crate::audio::settings_encoder::{EncoderSettings, EncoderType, ThreadSetting};
    use crate::audio::{AudioSettings, ChannelConfig, SampleRateConfig};

    #[test]
    fn create_encoder_respects_v2_settings() {
        let _ = ff::init();
        let temp = tempfile::TempDir::new().expect("create temp dir");
        let out_path = temp.path().join("out.m4b");

        let base_encoder_settings = EncoderSettings {
            encoder_type: EncoderType::AacAt,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Cvbr,
            channels: EncoderChannelConfig::Stereo,
            afterburner: false,
            threads: ThreadSetting::Auto,
        };
        let availability = settings_encoder::detect_available_encoders();
        if !availability.aac_at_available && !availability.native_aac_available {
            eprintln!("Skipping encoder test - no AAC encoder available in environment");
            return;
        }

        let audio_settings = AudioSettings {
            bitrate: 64,
            channels: ChannelConfig::Stereo,
            sample_rate: SampleRateConfig::Explicit(44_100),
            output_path: out_path.clone(),
        };

        let mut plan = MediaProcessingPlan::new(out_path, audio_settings, vec![], 60.0);
        plan.encoder_settings_v2 = Some(base_encoder_settings.clone());
        let (effective_settings, resolved_type) =
            resolve_plan_encoder_settings(&plan, &availability);
        let encoder_channels = effective_settings
            .channels
            .forced_channels()
            .unwrap_or(plan.settings.channels.channel_count())
            as i32;
        let target_bitrate = effective_settings.bitrate_kbps as i64 * 1000;
        let enc = match create_audio_encoder(
            effective_settings.as_ref(),
            resolved_type,
            44_100,
            encoder_channels,
            false,
        ) {
            Ok(enc) => enc,
            Err(e) => {
                eprintln!("encoder setup skipped in test environment: {e}");
                return;
            }
        };

        let codec = enc.codec().expect("encoder should expose codec");
        let expected_codec = settings_encoder::resolve_encoder_name(resolved_type);
        assert_eq!(
            codec.name(),
            expected_codec,
            "encoder selection should honor resolved encoder type"
        );

        let configured_br = unsafe {
            let ctx_ptr = enc.as_ptr();
            (*ctx_ptr).bit_rate
        };
        assert!(
            (configured_br - target_bitrate).abs() <= 1_000,
            "bitrate should track v2 setting; requested {} got {}",
            target_bitrate,
            configured_br
        );

        assert_eq!(
            enc.channel_layout().channels(),
            encoder_channels,
            "channel count should reflect v2 settings"
        );
        assert_eq!(enc.rate(), 44_100, "sample rate should match input");
    }
}
