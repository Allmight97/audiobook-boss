use super::AudiobookMetadata;
use crate::errors::Result;
use ffmpeg_next as ff;

use super::cover_art::embedding::{
    add_cover_art_stream_pre_header, write_cover_art_packet_post_header,
};
use super::ffi::set_stream_disposition_and_clear_codec_tag;
use super::ffmpeg_dict::merge_metadata;

/// Rewrite metadata (and optional cover) using ffmpeg-next via remux/stream-copy.
/// - Copies all non-attached_pic streams (audio + chapters handled separately)
/// - Copies chapters
/// - If metadata.cover_art is provided, replaces existing attached_pic with the new one
/// - Sets container metadata from AudiobookMetadata merged with existing tags
/// - Writes to a temp file and atomically replaces the original
pub fn rewrite_metadata_with_ffmpeg(
    input_path: &std::path::Path,
    metadata: &AudiobookMetadata,
) -> Result<()> {
    use crate::errors::AppError;

    ff::init().map_err(AppError::Ffmpeg)?;

    let mut ictx = ff::format::input(input_path).map_err(AppError::Ffmpeg)?;

    let parent = input_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| {
            AppError::FileValidation(format!(
                "Input path has no parent directory: {}",
                input_path.display()
            ))
        })?;
    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("m4b");
    let temp_path = parent.join(format!(".abb_meta_{}.{}", uuid::Uuid::new_v4(), ext));

    // Ensure temp path is free
    if temp_path.exists() {
        std::fs::remove_file(&temp_path).map_err(AppError::Io)?;
    }

    let mut octx = ff::format::output(&temp_path).map_err(AppError::Ffmpeg)?;

    let stream_len = ictx.streams().len();
    let mut stream_mapping: Vec<isize> = vec![-1; stream_len];
    let mut output_time_bases: Vec<Option<ff::Rational>> = vec![None; stream_len];

    // Copy streams (skip attached_pic if replacing cover art)
    for (index, istream) in ictx.streams().enumerate() {
        let medium = istream.parameters().medium();
        if medium == ff::media::Type::Data {
            log::info!(
                "Skipping data stream {} (codec: {:?}) during metadata remux",
                index,
                istream.parameters().id()
            );
            continue;
        }

        let in_disposition = istream.disposition();
        let is_attached_pic =
            in_disposition.contains(ff::format::stream::Disposition::ATTACHED_PIC);

        if is_attached_pic && metadata.cover_art.is_some() {
            log::info!("Skipping source attached_pic stream in favor of new cover art");
            continue;
        }

        let codec_ctx = ff::codec::context::Context::from_parameters(istream.parameters())
            .map_err(AppError::Ffmpeg)?;
        let mut ostream = octx.add_stream_with(&codec_ctx).map_err(AppError::Ffmpeg)?;

        ostream.set_time_base(istream.time_base());
        ostream.set_metadata(istream.metadata().to_owned());

        set_stream_disposition_and_clear_codec_tag(&mut ostream, in_disposition);

        stream_mapping[index] = ostream.index() as isize;
        output_time_bases[ostream.index()] = Some(ostream.time_base());
    }

    // Copy chapters before header write
    if ictx.nb_chapters() > 0 {
        for chapter in ictx.chapters() {
            let title = chapter.metadata().get("title").map(|s| s.to_string());
            match octx.add_chapter(
                chapter.id(),
                chapter.time_base(),
                chapter.start(),
                chapter.end(),
                title.as_deref().unwrap_or(""),
            ) {
                Ok(_out_chapter) => {}
                Err(e) => {
                    log::warn!("Failed to add chapter id {}: {}", chapter.id(), e);
                }
            }
        }
    }

    // Merge container metadata: start from existing then overlay requested values
    let merged_dict = merge_metadata(ictx.metadata().to_owned(), metadata)?;
    octx.set_metadata(merged_dict);

    // Add cover art stream if provided
    let cover = metadata.cover_art.as_ref();
    let cover_stream_info =
        cover.and_then(|bytes| add_cover_art_stream_pre_header(&mut octx, bytes));

    octx.write_header().map_err(AppError::Ffmpeg)?;

    if let (Some(bytes), Some((stream_index, format))) = (cover, cover_stream_info) {
        write_cover_art_packet_post_header(&mut octx, stream_index, bytes, format);
    }

    // Stream-copy all packets respecting mapping
    for (input_stream, mut packet) in ictx.packets() {
        let in_index = input_stream.index();
        let out_index = *stream_mapping.get(in_index).unwrap_or(&-1);
        if out_index < 0 {
            continue;
        }

        let out_tb = output_time_bases
            .get(out_index as usize)
            .and_then(|tb| *tb)
            .unwrap_or(input_stream.time_base());

        packet.set_stream(out_index as usize);
        packet.rescale_ts(input_stream.time_base(), out_tb);
        packet
            .write_interleaved(&mut octx)
            .map_err(AppError::Ffmpeg)?;
    }

    octx.write_trailer().map_err(AppError::Ffmpeg)?;

    // Atomic replace original
    std::fs::rename(&temp_path, input_path).map_err(AppError::Io)?;

    Ok(())
}
