use crate::errors::{AppError, Result};
use ffmpeg_next as ff;

use super::super::ffi::set_attached_pic_disposition;
use super::format::{detect_cover_art_format, detect_image_dimensions, CoverFormat};

/// Adds a cover art stream prior to header writing.
///
/// For M4B/MP4 containers, this properly sets the `attached_pic` disposition using
/// FFI to ensure the stream is treated as cover art rather than a regular video stream.
pub fn add_cover_art_stream_pre_header(
    octx: &mut ff::format::context::Output,
    cover_data: &[u8],
) -> Result<Option<(usize, CoverFormat)>> {
    if cover_data.is_empty() {
        return Ok(None);
    }
    let Some(format) = detect_cover_art_format(cover_data) else {
        return Err(AppError::General(
            "Unsupported cover art format (only JPEG and PNG are supported)".to_string(),
        ));
    };

    let codec_id = format.codec_id();
    let Some(codec) = ff::encoder::find(codec_id) else {
        return Err(AppError::General(format!(
            "Cover art codec {:?} missing in ffmpeg build",
            format
        )));
    };

    match octx.add_stream(codec) {
        Ok(mut stream) => {
            let idx = stream.index();

            // Configure stream parameters for cover art
            configure_cover_art_stream_parameters(&mut stream, format, cover_data)?;

            // Set the ATTACHED_PIC disposition using FFI
            // This is crucial for M4B/MP4 containers to properly recognize cover art
            set_attached_pic_disposition(octx, idx)?;

            log::info!("Added cover art stream with attached_pic disposition (index={}, format={:?}, bytes={})",
                      idx, format, cover_data.len());
            Ok(Some((idx, format)))
        }
        Err(e) => Err(AppError::General(format!(
            "Failed adding cover art stream: {}",
            e
        ))),
    }
}

/// Writes the cover art packet after header if a stream was added.
///
/// For attached pics in M4B/MP4 containers, this writes a single packet with
/// specific flags that mark it as cover art. The packet should have PTS/DTS of 0
/// and KEY flag to indicate it's a standalone image.
pub fn write_cover_art_packet_post_header(
    octx: &mut ff::format::context::Output,
    stream_index: usize,
    cover_data: &[u8],
    format: CoverFormat,
) -> Result<()> {
    if cover_data.is_empty() {
        return Ok(());
    }

    let mut pkt = ff::Packet::copy(cover_data);
    pkt.set_stream(stream_index);
    pkt.set_flags(ff::packet::flag::Flags::KEY);

    // For attached pics, set PTS and DTS to 0
    // This indicates it's a single frame that should be treated as cover art
    pkt.set_pts(Some(0));
    pkt.set_dts(Some(0));

    pkt.write_interleaved(octx).map_err(AppError::Ffmpeg)?;
    log::info!(
        "Cover art packet written as attached pic (stream={}, format={:?}, size={} bytes)",
        stream_index,
        format,
        cover_data.len()
    );
    Ok(())
}

/// Configures stream parameters for cover art embedding
///
/// This creates a proper codec context for the cover art stream to ensure it
/// is recognized correctly by the container format.
fn configure_cover_art_stream_parameters(
    stream: &mut ff::format::stream::StreamMut,
    format: CoverFormat,
    cover_data: &[u8],
) -> Result<()> {
    use crate::errors::AppError;

    // Create a codec context for the cover art
    let codec_id = format.codec_id();

    let codec = ff::encoder::find(codec_id)
        .ok_or_else(|| AppError::General(format!("Codec {:?} not found", codec_id)))?;

    let mut ctx = ff::codec::context::Context::new()
        .encoder()
        .video()
        .map_err(|e| AppError::General(format!("Failed to create video encoder context: {}", e)))?;

    // Return an error when dimensions cannot be detected; explicit cover-art
    // writes must not be reported as successful if the stream cannot be built.
    // Using a fixed 600×600 here would produce wrong codec parameters.
    let (width, height) = detect_image_dimensions(cover_data, format).ok_or_else(|| {
        AppError::General(format!(
            "Cannot detect dimensions for {:?} cover art ({} bytes); skipping embedding",
            format,
            cover_data.len()
        ))
    })?;

    ctx.set_width(width as u32);
    ctx.set_height(height as u32);

    // Set pixel format for the codec
    ctx.set_format(format.pixel_format());

    // Set time base for single frame
    ctx.set_time_base(ff::Rational(1, 1));

    // Open the encoder to finalize parameters
    let encoder = ctx
        .open_as(codec)
        .map_err(|e| AppError::General(format!("Failed to open cover art encoder: {}", e)))?;

    // Set the stream parameters from the encoder context
    stream.set_parameters(&encoder);

    log::debug!(
        "Configured cover art stream parameters for {:?} format ({}x{})",
        format,
        width,
        height
    );
    Ok(())
}
