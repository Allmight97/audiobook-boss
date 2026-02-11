//! Decoder and resampler setup (behavior-preserving extraction)

use crate::errors::{sanitize_path_for_display, Result};
use ffmpeg_next as ff;
use std::path::Path;

/// Sets up decoder and resampler for a single input file
pub(crate) fn setup_decoder_and_resampler(
    input_path: &Path,
    encoder: &ff::codec::encoder::audio::Encoder,
) -> Result<(
    ff::format::context::Input,
    ff::codec::decoder::Audio,
    ff::software::resampling::Context,
    usize,
)> {
    use crate::errors::AppError;

    log::info!(
        "🔧 Setting up decoder for input file: {}",
        input_path.display()
    );

    if !input_path.exists() {
        return Err(AppError::FileValidation(format!(
            "Input file does not exist: {}",
            sanitize_path_for_display(input_path)
        )));
    }
    log::info!("✓ Input file exists and is accessible");

    log::info!("Opening FFmpeg input context...");
    let ictx = ff::format::input(&input_path).map_err(|e| {
        AppError::General(format!(
            "Failed to open input file '{}': {}",
            sanitize_path_for_display(input_path),
            e
        ))
    })?;
    log::info!("✓ FFmpeg input context opened successfully");

    log::info!("Finding best audio stream...");
    let istream = ictx.streams().best(ff::media::Type::Audio).ok_or_else(|| {
        AppError::InvalidInput(format!(
            "No audio stream found in input file: {}",
            sanitize_path_for_display(input_path)
        ))
    })?;
    let stream_index = istream.index();
    log::info!("✓ Found audio stream at index: {}", stream_index);

    log::info!("Creating decoder context from stream parameters...");
    let dec_ctx =
        ff::codec::context::Context::from_parameters(istream.parameters()).map_err(|e| {
            AppError::General(format!(
                "Failed to create decoder context from parameters for '{}': {}",
                sanitize_path_for_display(input_path),
                e
            ))
        })?;
    log::info!("✓ Decoder context created successfully");

    log::info!("Opening audio decoder...");
    let decoder = dec_ctx.decoder().audio().map_err(|e| {
        AppError::General(format!(
            "Failed to open audio decoder for '{}': {}",
            sanitize_path_for_display(input_path),
            e
        ))
    })?;
    log::info!("✓ Audio decoder opened successfully");

    // Build resampler for this input stream
    log::info!("Creating resampler...");
    let in_layout = decoder.channel_layout();
    let in_rate = decoder.rate();
    let in_format = decoder.format();
    log::info!(
        "Input audio format: rate={}, channels={:?}, format={:?}",
        in_rate,
        in_layout,
        in_format
    );
    log::info!(
        "Output audio format: rate={}, channels={:?}, format={:?}",
        encoder.rate(),
        encoder.channel_layout(),
        encoder.format()
    );

    let resampler = ff::software::resampling::Context::get(
        in_format,
        in_layout,
        in_rate,
        encoder.format(),
        encoder.channel_layout(),
        encoder.rate(),
    )
    .map_err(|e| {
        AppError::General(format!(
            "Failed to create resampler for '{}': {}",
            sanitize_path_for_display(input_path),
            e
        ))
    })?;
    log::info!("✓ Resampler created successfully");

    log::info!(
        "🎉 Decoder and resampler setup completed for: {}",
        input_path.display()
    );
    Ok((ictx, decoder, resampler, stream_index))
}
