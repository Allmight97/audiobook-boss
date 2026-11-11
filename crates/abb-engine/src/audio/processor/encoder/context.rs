use crate::audio::settings_encoder::{self, EncoderSettings, EncoderType};
use crate::errors::{AppError, Result};
use abb_ffmpeg::{ChannelLayout, SampleFormat};

use super::super::run_diagnostics::with_encoding_log_file;
use super::options::{
    build_apple_options, build_native_options, validate_native_options, EncoderOptions,
};
use super::{
    faac::FaacEncoder,
    session::{Backend, EncoderSession},
};

pub(crate) fn create_audio_encoder(
    encoder_settings: &EncoderSettings,
    target_sample_rate: u32,
    target_channels: i32,
    requires_global_header: bool,
) -> Result<abb_ffmpeg::AudioEncoder> {
    let resolved_encoder = encoder_settings.encoder_type;

    if resolved_encoder == EncoderType::Faac {
        return Err(AppError::InvalidInput(
            "FAAC uses its own encoder path and cannot be opened as an FFmpeg encoder.".to_string(),
        ));
    }

    let codec_name = settings_encoder::resolve_encoder_name(resolved_encoder);
    if !abb_ffmpeg::has_encoder(codec_name) {
        return Err(AppError::General(format!(
            "Encoder '{codec_name}' not found"
        )));
    }
    if resolved_encoder == EncoderType::NativeAac {
        settings_encoder::validate_native_target_bitrate(
            encoder_settings.bitrate_kbps,
            target_sample_rate,
            target_channels as u32,
        )?;
    }

    let layout = ChannelLayout::default_for(target_channels as u16);
    let format = match resolved_encoder {
        EncoderType::Opus => SampleFormat::F32,
        EncoderType::AacAt => SampleFormat::S16,
        _ => SampleFormat::F32_PLANAR,
    };

    let options = match resolved_encoder {
        EncoderType::Opus => EncoderOptions {
            bit_rate: i64::from(encoder_settings.bitrate_kbps) * 1000,
            compression_level: Some(10),
            private: vec![("vbr", "on".into()), ("application", "audio".into())],
        },
        EncoderType::AacAt => build_apple_options(encoder_settings),
        EncoderType::NativeAac => build_native_options(encoder_settings),
        EncoderType::Faac | EncoderType::Auto => {
            unreachable!("create_audio_encoder requires a resolved in-process encoder type")
        }
    };

    encoder_log(&format!(
        "encoder_config resolved={:?} bitrate_mode={:?} bit_rate_field={} fmt={:?} channels={} rate={} opts={:?}",
        resolved_encoder,
        encoder_settings.bitrate_mode,
        options.bit_rate,
        format,
        layout.channels(),
        target_sample_rate,
        options.private
    ));

    let open_failure = |e| AppError::General(format!("Final open encoder failed: {e}"));
    let mut private = abb_ffmpeg::Tags::default();
    for (name, value) in &options.private {
        private
            .push(name, value, abb_ffmpeg::Step::OpenEncoder)
            .map_err(open_failure)?;
    }
    let opened = abb_ffmpeg::AudioEncoder::open(&abb_ffmpeg::EncoderRequest {
        codec_name,
        rate: target_sample_rate,
        layout,
        format,
        bit_rate: options.bit_rate,
        compression_level: options.compression_level,
        global_header: requires_global_header,
        options: &private,
    })
    .map_err(open_failure)?;
    if !opened.unused_options.is_empty() {
        log::warn!(
            "encoder_config encoder={codec_name} options_not_taken={}",
            opened.unused_options.join(",")
        );
    }

    if resolved_encoder == EncoderType::NativeAac {
        validate_native_options(&opened.encoder, encoder_settings)?;
    }
    Ok(opened.encoder)
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

    let muxer = crate::audio::muxer_for_path(&plan.output_path)?;
    let mut octx = abb_ffmpeg::Output::create(&plan.output_path, muxer.ffmpeg_name())
        .map_err(|e| AppError::General(format!("Create output failed: {e}")))?;

    if let Some(metadata) = metadata {
        crate::metadata::set_container_metadata(&mut octx, metadata)?;
        log::debug!("Container metadata set successfully");
    }

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
            octx.wants_global_header(),
        )?)
    };
    let layout = enc_ctx.channel_layout()?;

    encoder_log(&format!("encoder_effective artifact={} encoder={:?} rate={} channels={} format={:?} frame_samples={} time_base={:?}",
        crate::diagnostics::artifact_id(&plan.output_path), resolved_encoder_type,
        enc_ctx.rate(), layout.channels(), enc_ctx.format(), enc_ctx.frame_size(), enc_ctx.time_base()));

    let ost_index = enc_ctx.add_stream_to(&mut octx)?;
    octx.set_stream_time_base(ost_index, enc_ctx.time_base())?;

    let selected_cover = crate::metadata::ResolvedCover::select_for_encode(metadata, passthrough);

    let cover_stream = match selected_cover {
        Some(cover) => {
            log::info!(
                "cover_art_plan decision=native_attempt source={} bytes={}",
                cover.source(),
                cover.bytes().len()
            );
            let cover_stream = match crate::metadata::add_cover_stream(&mut octx, cover.bytes()) {
                Ok(stream) => stream,
                Err(error) => {
                    cover.handle_embed_failure(error, "native cover art")?;
                    None
                }
            };
            if let Some(stream) = cover_stream {
                log::info!("✓ Native cover art stream added successfully (stream={}, format={:?}) - will embed during encoding", stream.index, stream.format);
            }
            cover_stream
        }
        None => {
            log::info!("cover_art_plan decision=none reason=no_cover_art_data");
            None
        }
    };

    if let Some(p) = passthrough {
        let count = crate::metadata::add_chapters(&mut octx, &p.chapters)?;
        log::debug!("Copied {count} accepted chapters");
    }

    let mut muxer = crate::audio::write_output_header(octx, &abb_ffmpeg::Tags::default())
        .map_err(|e| AppError::General(format!("Write header failed: {e}")))?;
    if let Backend::Faac(encoder) = &enc_ctx {
        muxer
            .set_tag("encoding_tool", encoder.encoding_tool())
            .map_err(|_| AppError::General("Cannot record FAAC output provenance.".into()))?;
    }

    if let (Some(stream), Some(cover)) = (cover_stream, selected_cover) {
        log::info!(
            "Writing cover art packet to stream {} ({:?} format, {} bytes, source={})",
            stream.index,
            stream.format,
            cover.bytes().len(),
            cover.source()
        );
        match crate::metadata::write_cover_packet(&mut muxer, stream, cover.bytes()) {
            Ok(()) => log::info!(
                "✓ Native cover art packet written successfully to stream {}",
                stream.index
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

    let ost_time_base = muxer.stream_time_base(ost_index)?;
    Ok(EncoderSession::new(
        enc_ctx,
        layout,
        muxer,
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
