use crate::errors::{AppError, Result};

use super::super::container::Container;
use super::format::{detect_cover_art_format, detect_image_dimensions, CoverFormat};

/// The `comment` tag value FFmpeg's ID3 and FLAC muxers write as picture type 3
/// and its demuxers report for one.
pub(crate) const FRONT_COVER_COMMENT: &str = "Cover (front)";

/// A cover stream added to an output before its header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CoverStream {
    pub(crate) index: usize,
    pub(crate) format: CoverFormat,
    /// A Matroska attachment is written with the header and takes no packet.
    pub(crate) written_with_header: bool,
}

/// Adds the stream that carries `cover_data`: a Matroska attachment, or an
/// attached picture whose packet `write_cover_packet` writes after the header.
pub(crate) fn add_cover_stream(
    output: &mut abb_ffmpeg::Output,
    cover_data: &[u8],
) -> Result<Option<CoverStream>> {
    if cover_data.is_empty() {
        return Ok(None);
    }
    let Some(format) = detect_cover_art_format(cover_data) else {
        return Err(AppError::General(
            "Unsupported cover art format (only JPEG and PNG are supported)".to_string(),
        ));
    };
    let step = abb_ffmpeg::Step::SetTags;
    let container = Container::from_format_name(&output.muxer_name());
    if container == Container::Matroska {
        let png = format == CoverFormat::Png;
        let mut tags = abb_ffmpeg::Tags::default();
        tags.push(
            "filename",
            if png { "cover.png" } else { "cover.jpg" },
            step,
        )?;
        tags.push(
            "mimetype",
            if png { "image/png" } else { "image/jpeg" },
            step,
        )?;
        let index = output.add_attachment_stream(format.codec(), cover_data, &tags)?;
        return Ok(Some(CoverStream {
            index,
            format,
            written_with_header: true,
        }));
    }
    let (width, height) = detect_image_dimensions(cover_data, format).ok_or_else(|| {
        AppError::General(format!(
            "Cannot detect dimensions for {:?} cover art ({} bytes); skipping embedding",
            format,
            cover_data.len()
        ))
    })?;
    let index = output
        .add_attached_picture_stream(format.picture_kind(), width as u32, height as u32)
        .map_err(|e| AppError::General(format!("Failed adding cover art stream: {e}")))?;
    if container == Container::Mp3 || container.uses_vorbis_comments() {
        let mut tags = abb_ffmpeg::Tags::default();
        tags.push("comment", FRONT_COVER_COMMENT, step)?;
        output.set_stream_tags(index, &tags)?;
    }
    log::info!(
        "Added cover art stream with attached_pic disposition (index={}, format={:?}, bytes={})",
        index,
        format,
        cover_data.len()
    );
    Ok(Some(CoverStream {
        index,
        format,
        written_with_header: false,
    }))
}

pub(crate) fn write_cover_packet(
    muxer: &mut abb_ffmpeg::Muxer,
    stream: CoverStream,
    cover_data: &[u8],
) -> Result<()> {
    if cover_data.is_empty() || stream.written_with_header {
        return Ok(());
    }
    let mut packet = abb_ffmpeg::Packet::key_frame_of(cover_data)?;
    packet.set_stream_index(stream.index);
    packet.set_pts(Some(0));
    packet.set_dts(Some(0));
    muxer.write_interleaved(&mut packet)?;
    log::info!(
        "Cover art packet written as attached pic (stream={}, format={:?}, size={} bytes)",
        stream.index,
        stream.format,
        cover_data.len()
    );
    Ok(())
}
