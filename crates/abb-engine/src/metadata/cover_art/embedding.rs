use crate::errors::{AppError, Result};
use ffmpeg_next as ff;

use super::super::container::Container;
use super::super::ffi::set_attached_pic_disposition;
use super::format::{detect_cover_art_format, detect_image_dimensions, CoverFormat};

/// The `comment` tag value FFmpeg's ID3 and FLAC muxers write as picture type 3
/// and its demuxers report for one.
pub(crate) const FRONT_COVER_COMMENT: &str = "Cover (front)";

/// Adds a cover art stream before the header is written. MP4-family streams get
/// the `attached_pic` disposition.
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

    let container = Container::of_output(octx);
    if container == Container::Matroska {
        return add_matroska_cover(octx, cover_data, format).map(Some);
    }
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

            configure_cover_art_stream_parameters(&mut stream, format, cover_data)?;
            if container == Container::Mp3 || container.uses_vorbis_comments() {
                let mut tags = ff::Dictionary::new();
                tags.set("comment", FRONT_COVER_COMMENT);
                stream.set_metadata(tags);
            }

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

/// Writes the cover packet after the header when a stream was added.
pub fn write_cover_art_packet_post_header(
    octx: &mut ff::format::context::Output,
    stream_index: usize,
    cover_data: &[u8],
    format: CoverFormat,
) -> Result<()> {
    if cover_data.is_empty() {
        return Ok(());
    }

    if octx
        .stream(stream_index)
        .is_some_and(|stream| stream.parameters().medium() == ff::media::Type::Attachment)
    {
        return Ok(()); // Matroska wrote the cover attachment with the header.
    }
    let mut pkt = ff::Packet::copy(cover_data);
    pkt.set_stream(stream_index);
    pkt.set_flags(ff::packet::flag::Flags::KEY);

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

fn configure_cover_art_stream_parameters(
    stream: &mut ff::format::stream::StreamMut,
    format: CoverFormat,
    cover_data: &[u8],
) -> Result<()> {
    use crate::errors::AppError;

    let codec_id = format.codec_id();

    let codec = ff::encoder::find(codec_id)
        .ok_or_else(|| AppError::General(format!("Codec {:?} not found", codec_id)))?;

    let mut ctx = ff::codec::context::Context::new()
        .encoder()
        .video()
        .map_err(|e| AppError::General(format!("Failed to create video encoder context: {}", e)))?;

    let (width, height) = detect_image_dimensions(cover_data, format).ok_or_else(|| {
        AppError::General(format!(
            "Cannot detect dimensions for {:?} cover art ({} bytes); skipping embedding",
            format,
            cover_data.len()
        ))
    })?;

    ctx.set_width(width as u32);
    ctx.set_height(height as u32);

    ctx.set_format(format.pixel_format());

    ctx.set_time_base(ff::Rational(1, 1));

    let encoder = ctx
        .open_as(codec)
        .map_err(|e| AppError::General(format!("Failed to open cover art encoder: {}", e)))?;

    // Parameters first, then the stream time_base. An unset cover time_base
    // (num=0) makes the ipod muxer default the stream to 1/90000, LCM with
    // audio 1/44100 into movie timescale 4410000, and overflow QT chapter packets.
    stream.set_parameters(&encoder);
    stream.set_time_base(ff::Rational(1, 1));

    log::debug!(
        "Configured cover art stream parameters for {:?} format ({}x{})",
        format,
        width,
        height
    );
    Ok(())
}

fn add_matroska_cover(
    octx: &mut ff::format::context::Output,
    bytes: &[u8],
    format: CoverFormat,
) -> Result<(usize, CoverFormat)> {
    let mut tags = ff::Dictionary::new();
    let png = format.codec_id() == ff::codec::Id::PNG;
    tags.set("filename", if png { "cover.png" } else { "cover.jpg" });
    tags.set("mimetype", if png { "image/png" } else { "image/jpeg" });
    let index = add_matroska_attachment(octx, bytes, format.codec_id(), tags)?;
    Ok((index, format))
}

/// Adds `bytes` as a Matroska attachment, the only form in which that muxer
/// keeps a picture.
pub(crate) fn add_matroska_attachment(
    octx: &mut ff::format::context::Output,
    bytes: &[u8],
    codec_id: ff::codec::Id,
    tags: ff::Dictionary,
) -> Result<usize> {
    let size = i32::try_from(bytes.len())
        .map_err(|_| AppError::InvalidInput("Cover art is too large.".into()))?;
    let mut parameters = ff::codec::Parameters::new();
    // SAFETY: parameters owns the fresh allocation and the padded av_mallocz buffer.
    // FFmpeg frees that extradata when the parameters/stream are dropped.
    unsafe {
        let raw = &mut *parameters.as_mut_ptr();
        raw.codec_type = ff::sys::AVMediaType::AVMEDIA_TYPE_ATTACHMENT;
        raw.codec_id = codec_id.into();
        raw.extradata =
            ff::sys::av_mallocz(bytes.len() + ff::sys::AV_INPUT_BUFFER_PADDING_SIZE as usize)
                .cast();
        if raw.extradata.is_null() {
            return Err(AppError::General(
                "Cannot allocate cover attachment.".into(),
            ));
        }
        raw.extradata_size = size;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), raw.extradata, bytes.len());
    }
    let mut stream = octx.add_stream(None)?;
    stream.set_parameters(parameters);
    stream.set_metadata(tags);
    Ok(stream.index())
}
