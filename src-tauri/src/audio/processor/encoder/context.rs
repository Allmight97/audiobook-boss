//! Encoder context creation and output stream setup.

use crate::audio::settings_encoder::{self, EncoderSettings, EncoderType};
use crate::errors::Result;
use ffmpeg_next as ff;

use super::common::{
    configure_threads, encoder_log, find_encoder_by_name, resolve_plan_encoder_settings,
    try_configure_variable_frame_size, EncoderFramePlan,
};
use super::options::{build_apple_options, build_fdk_options, build_native_options};

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

    // Build encoder-specific options Dictionary
    // Options are passed to avcodec_open2 via open_as_with, which is how FFmpeg CLI does it
    let opts = match resolved_encoder {
        EncoderType::FdkHeAac => build_fdk_options(encoder_settings)?,
        EncoderType::AacAt => build_apple_options(&mut opened, encoder_settings),
        EncoderType::NativeAac => build_native_options(&mut opened, encoder_settings),
        EncoderType::Auto => {
            unreachable!("create_audio_encoder requires a resolved encoder type")
        }
    };

    configure_threads(&mut opened, encoder_settings.threads);

    let raw_bit_rate = unsafe { (*opened.as_mut_ptr()).bit_rate };
    encoder_log(&format!(
        "encoder_config resolved={:?} bitrate_mode={:?} bit_rate_field={} fmt={:?} channels={} rate={} afterburner={} opts={:?}",
        resolved_encoder,
        encoder_settings.bitrate_mode,
        raw_bit_rate,
        opened.format(),
        opened.channel_layout().channels(),
        opened.rate(),
        encoder_settings.afterburner,
        opts.iter().collect::<Vec<_>>()
    ));

    if requires_global_header {
        opened.set_flags(ff::codec::flag::Flags::GLOBAL_HEADER);
    }

    // Open encoder with codec AND options dictionary
    // This passes options to avcodec_open2, which correctly handles encoder-private options
    let enc_ctx = opened
        .open_as_with(codec, opts)
        .map_err(|e| AppError::General(format!("Final open encoder failed: {e}")))?;

    Ok(enc_ctx)
}

/// Sets up the output encoder context and stream with metadata support.
/// Returns (output_context, encoder_context, output_stream_index, output_time_base, target_sample_rate, frame_plan)
///
/// When `skip_chapter_passthrough` is false and input files have chapters, they are copied
/// to the output context before the header is written (#66).
#[allow(clippy::too_many_lines)]
pub(crate) fn setup_encoder(
    plan: &crate::audio::processor::plan::MediaProcessingPlan,
    metadata: Option<&crate::metadata::AudiobookMetadata>,
    skip_chapter_passthrough: bool,
    passthrough: Option<&crate::metadata::passthrough::PassthroughMetadata>,
) -> Result<(
    ff::format::context::Output,
    ff::codec::encoder::audio::Encoder,
    usize,
    ff::Rational,
    u32,
    EncoderFramePlan,
)> {
    use crate::errors::AppError;

    let (target_sample_rate, target_channels) =
        crate::audio::processor::engine::resolve_target_audio_params(plan)?;

    let availability = crate::audio::detect_encoder_availability(None);
    let (effective_settings, resolved_encoder_type) =
        resolve_plan_encoder_settings(plan, &availability);

    let mut octx = ff::format::output(&plan.output_path)
        .map_err(|e| AppError::General(format!("Create output failed: {e}")))?;

    if let Some(metadata) = metadata {
        crate::metadata::set_container_metadata(&mut octx, metadata)
            .map_err(|e| AppError::General(format!("Failed to set container metadata: {e}")))?;
        log::debug!("Container metadata set successfully");
    }

    let stream_codec_id = ff::codec::Id::AAC;
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
    // Prefer user-provided cover art; otherwise reuse passthrough cover art without reprocessing.
    let selected_cover = metadata
        .and_then(|m| m.cover_art.as_ref().map(|data| (data, "user")))
        .or_else(|| {
            passthrough
                .and_then(|p| p.cover_art.as_ref())
                .map(|data| (data, "passthrough"))
        });

    let selected_cover = selected_cover.filter(|(cover_data, _)| !cover_data.is_empty());

    let cover_art_stream_info = match selected_cover {
        Some((cover_data, source)) => {
            let bytes = cover_data.len();
            log::info!(
                "cover_art_plan decision=native_attempt source={} bytes={}",
                source,
                bytes
            );
            let cover_art_stream_info =
                match crate::metadata::add_cover_art_stream_pre_header(&mut octx, cover_data) {
                    Ok(stream_info) => stream_info,
                    Err(error) if source == "passthrough" => {
                        log::warn!(
                            "Could not preserve passthrough cover art during native encoding: {}",
                            error
                        );
                        None
                    }
                    Err(error) => return Err(error),
                };
            if let Some((stream_idx, format)) = cover_art_stream_info {
                log::info!("✓ Native cover art stream added successfully (stream={}, format={:?}) - will embed during encoding", stream_idx, format);
            }
            cover_art_stream_info
        }
        None => {
            log::info!("cover_art_plan decision=none reason=no_cover_art_data");
            None
        }
    };

    // Chapter passthrough: copy chapters from source inputs (#66)
    // Skip in preview mode since chapters won't align with shortened output
    if !skip_chapter_passthrough {
        if let Some(p) = passthrough {
            match crate::metadata::passthrough::add_chapters_to_output(&mut octx, &p.chapters) {
                Ok(count) if count > 0 => {
                    log::info!("✓ Copied {} chapters from source files", count);
                }
                Ok(_) => {
                    log::debug!("No chapters found to copy from source files");
                }
                Err(error) => {
                    log::warn!(
                        "Could not preserve passthrough chapters during native encoding: {}",
                        error
                    );
                }
            }
        }
    } else {
        log::debug!("Chapter passthrough skipped (preview mode)");
    }

    // Header
    octx.write_header()
        .map_err(|e| AppError::General(format!("Write header failed: {e}")))?;

    // Post-header cover art packet
    if let Some((stream_index, format)) = cover_art_stream_info {
        if let Some((cover_data, source)) = selected_cover {
            log::info!(
                "Writing cover art packet to stream {} ({:?} format, {} bytes, source={})",
                stream_index,
                format,
                cover_data.len(),
                source
            );
            if let Err(error) = crate::metadata::write_cover_art_packet_post_header(
                &mut octx,
                stream_index,
                cover_data,
                format,
            ) {
                if source == "passthrough" {
                    log::warn!(
                        "Could not preserve passthrough cover art packet during native encoding: {}",
                        error
                    );
                } else {
                    return Err(error);
                }
            } else {
                log::info!(
                    "✓ Native cover art packet written successfully to stream {}",
                    stream_index
                );
            }
        } else {
            log::warn!("Cover art stream exists but no cover bytes were available for writing");
        }
    }

    log::info!(
        "encoder_setup resolved: encoder={:?} rate={}Hz channels={} fmt={:?} frame_size={} bitrate={}k requested={:?}",
        resolved_encoder_type,
        target_sample_rate,
        target_channels,
        enc_ctx.format(),
        enc_ctx.frame_size(),
        plan.encoder_settings.bitrate_kbps,
        plan.encoder_settings
    );
    let frame_plan = EncoderFramePlan::from_opened_encoder(&enc_ctx, resolved_encoder_type)?;

    Ok((
        octx,
        enc_ctx,
        ost_index,
        ost_time_base,
        target_sample_rate,
        frame_plan,
    ))
}
