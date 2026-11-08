use crate::audio::settings_encoder::{self, EncoderSettings, EncoderType};
use crate::errors::{AppError, Result};
use ffmpeg_next as ff;

use super::super::run_diagnostics::with_encoding_log_file;
use super::options::{build_apple_options, build_native_options, validate_native_options};
use super::{
    faac::FaacEncoder,
    session::{Backend, EncoderSession},
};

pub(crate) fn create_audio_encoder(
    encoder_settings: &EncoderSettings,
    target_sample_rate: u32,
    target_channels: i32,
    requires_global_header: bool,
) -> Result<ff::codec::encoder::audio::Encoder> {
    let resolved_encoder = encoder_settings.encoder_type;

    if resolved_encoder == EncoderType::Faac {
        return Err(AppError::InvalidInput(
            "FAAC uses its own encoder path and cannot be opened as an FFmpeg encoder.".to_string(),
        ));
    }

    let codec_name = settings_encoder::resolve_encoder_name(resolved_encoder);
    let codec = ff::encoder::find_by_name(codec_name)
        .ok_or_else(|| AppError::General(format!("Encoder '{codec_name}' not found")))?;
    if resolved_encoder == EncoderType::NativeAac {
        settings_encoder::validate_native_target_bitrate(
            encoder_settings.bitrate_kbps,
            target_sample_rate,
            target_channels as u32,
        )?;
    }

    let channel_layout = ff::channel_layout::ChannelLayout::default(target_channels);
    let sample_format = match resolved_encoder {
        EncoderType::Opus => ff::format::Sample::F32(ff::format::sample::Type::Packed),
        EncoderType::AacAt => ff::format::Sample::I16(ff::format::sample::Type::Packed),
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

    let opts = match resolved_encoder {
        EncoderType::Opus => {
            opened.set_bit_rate(usize::from(encoder_settings.bitrate_kbps) * 1000);
            opened.set_compression(Some(10));
            let mut options = ff::Dictionary::new();
            options.set("vbr", "on");
            options.set("application", "audio");
            options
        }
        EncoderType::AacAt => build_apple_options(&mut opened, encoder_settings),
        EncoderType::NativeAac => build_native_options(&mut opened, encoder_settings),
        EncoderType::Faac | EncoderType::Auto => {
            unreachable!("create_audio_encoder requires a resolved in-process encoder type")
        }
    };

    // SAFETY: `opened` owns a live encoder context, so its pointer is valid for this scalar read.
    let raw_bit_rate = unsafe { (*opened.as_mut_ptr()).bit_rate };
    encoder_log(&format!(
        "encoder_config resolved={:?} bitrate_mode={:?} bit_rate_field={} fmt={:?} channels={} rate={} opts={:?}",
        resolved_encoder,
        encoder_settings.bitrate_mode,
        raw_bit_rate,
        opened.format(),
        opened.channel_layout().channels(),
        opened.rate(),
        opts.iter().collect::<Vec<_>>()
    ));

    if requires_global_header {
        opened.set_flags(ff::codec::flag::Flags::GLOBAL_HEADER);
    }

    let enc_ctx = opened
        .open_as_with(codec, opts)
        .map_err(|e| AppError::General(format!("Final open encoder failed: {e}")))?;

    if resolved_encoder == EncoderType::NativeAac {
        validate_native_options(&enc_ctx, encoder_settings)?;
    }
    Ok(enc_ctx)
}

/// Sets up the output encoder context and stream with metadata support.
/// The returned session owns the codec, output handle, and sample timeline.
///
/// Accepted chapters are written to the output context before the header (#66).
#[expect(
    clippy::too_many_lines,
    reason = "Sequential codec/container/cover/chapter setup with fallible handoffs"
)]
pub(crate) fn setup_encoder(
    plan: &crate::audio::processor::plan::MediaProcessingPlan,
    metadata: Option<&crate::metadata::AudiobookMetadata>,
    passthrough: Option<&crate::metadata::PassthroughMetadata>,
) -> Result<EncoderSession> {
    let (target_sample_rate, target_channels) =
        crate::audio::processor::engine::resolve_target_audio_params(plan)?;

    let resolved_encoder_type = plan.encoder_settings.encoder_type;
    settings_encoder::validate_encoder_settings(&plan.encoder_settings)?;

    let muxer = plan
        .output_path
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(crate::audio::format_for_extension)
        .ok_or_else(|| AppError::InvalidInput("Unsupported audiobook container.".into()))?
        .muxer;
    let mut octx = ff::format::output_as(&plan.output_path, muxer.ffmpeg_name())
        .map_err(|e| AppError::General(format!("Create output failed: {e}")))?;

    if let Some(metadata) = metadata {
        crate::metadata::set_container_metadata(&mut octx, metadata);
        log::debug!("Container metadata set successfully");
    }

    let stream_codec_id = crate::audio::codec_family_of(resolved_encoder_type).codec_id();
    let codec = ff::encoder::find(stream_codec_id)
        .ok_or_else(|| AppError::General(format!("{:?} encoder not found", stream_codec_id)))?;

    let requires_global_header = octx
        .format()
        .flags()
        .contains(ff::format::flag::Flags::GLOBAL_HEADER);

    let mut ost = octx
        .add_stream(codec)
        .map_err(|e| AppError::General(format!("Add output stream failed: {e}")))?;

    let enc_ctx = if resolved_encoder_type == EncoderType::Faac {
        Backend::Faac(FaacEncoder::open(
            target_sample_rate,
            target_channels,
            &plan.encoder_settings,
        )?)
    } else {
        Backend::Ffmpeg(create_audio_encoder(
            &plan.encoder_settings,
            target_sample_rate,
            target_channels,
            requires_global_header,
        )?)
    };

    encoder_log(&format!("encoder_effective artifact={} encoder={:?} rate={} channels={} format={:?} frame_samples={} time_base={:?}",
        crate::diagnostics::artifact_id(&plan.output_path), resolved_encoder_type,
        enc_ctx.rate(), enc_ctx.channel_layout().channels(), enc_ctx.format(), enc_ctx.frame_size(), enc_ctx.time_base()));

    ost.set_time_base(enc_ctx.time_base());
    ost.set_parameters(enc_ctx.parameters()?);
    let ost_index = ost.index();

    let selected_cover = crate::metadata::ResolvedCover::select_for_encode(metadata, passthrough);

    let cover_art_stream_info = match selected_cover {
        Some(cover) => {
            log::info!(
                "cover_art_plan decision=native_attempt source={} bytes={}",
                cover.source(),
                cover.bytes().len()
            );
            let cover_art_stream_info =
                match crate::metadata::add_cover_art_stream_pre_header(&mut octx, cover.bytes()) {
                    Ok(stream_info) => stream_info,
                    Err(error) => {
                        cover.handle_embed_failure(error, "native cover art")?;
                        None
                    }
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

    if let Some(p) = passthrough {
        let count = crate::metadata::add_chapters_to_output(&mut octx, &p.chapters)?;
        log::debug!("Copied {count} accepted chapters");
    }

    octx.write_header()
        .map_err(|e| AppError::General(format!("Write header failed: {e}")))?;
    if let Backend::Faac(encoder) = &enc_ctx {
        let tool = std::ffi::CString::new(encoder.encoding_tool()).expect("static tool name");
        // SAFETY: the live output owns this dictionary; av_dict_set copies the
        // strings and updates its allocation without replacing/leaking other tags.
        let status = unsafe {
            ff::sys::av_dict_set(
                &mut (*octx.as_mut_ptr()).metadata,
                c"encoding_tool".as_ptr(),
                tool.as_ptr(),
                0,
            )
        };
        if status < 0 {
            return Err(AppError::General(
                "Cannot record FAAC output provenance.".into(),
            ));
        }
    }

    if let (Some((stream_index, format)), Some(cover)) = (cover_art_stream_info, selected_cover) {
        log::info!(
            "Writing cover art packet to stream {} ({:?} format, {} bytes, source={})",
            stream_index,
            format,
            cover.bytes().len(),
            cover.source()
        );
        match crate::metadata::write_cover_art_packet_post_header(
            &mut octx,
            stream_index,
            cover.bytes(),
            format,
        ) {
            Ok(()) => log::info!(
                "✓ Native cover art packet written successfully to stream {}",
                stream_index
            ),
            Err(error) => cover.handle_embed_failure(error, "native cover art packet")?,
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
    let frame_samples = samples_per_frame(enc_ctx.frame_size() as usize, resolved_encoder_type)?;

    let ost_time_base = octx
        .stream(ost_index)
        .expect("created audio stream")
        .time_base();
    Ok(EncoderSession::new(
        enc_ctx,
        octx,
        ost_index,
        ost_time_base,
        frame_samples,
        resolved_encoder_type,
    ))
}

fn encoder_log(message: &str) {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "the log file is best-effort; the same message goes to the application log"
    )]
    let _ = with_encoding_log_file(|file| {
        use std::io::Write as _;
        writeln!(file, "{message}")
    });
    log::info!("{message}");
}

const AAC_FRAME_QUANTUM_SAMPLES: usize = 1024;

/// Encoders reporting frame size 0 accept variable frames; AAC still takes 1024-sample quanta.
fn samples_per_frame(frame_size: usize, resolved_encoder: EncoderType) -> Result<usize> {
    match (frame_size, resolved_encoder) {
        (0, EncoderType::AacAt | EncoderType::NativeAac) => Ok(AAC_FRAME_QUANTUM_SAMPLES),
        (0, _) => Err(AppError::General(format!(
            "{resolved_encoder} reported no frame size and has no fixed frame quantum."
        ))),
        (reported, _) => Ok(reported),
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
