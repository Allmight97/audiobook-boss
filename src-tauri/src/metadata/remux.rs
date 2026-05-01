use super::passthrough::PassthroughMetadata;
use super::{AudiobookMetadata, MetadataWritePlan};
use crate::errors::Result;
use ffmpeg_next as ff;

use super::cover_art::embedding::{
    add_cover_art_stream_pre_header, write_cover_art_packet_post_header,
};
use super::ffi::set_stream_disposition_and_clear_codec_tag;

/// Rewrite metadata and optional passthrough state using ffmpeg-next via remux/stream-copy.
/// - Copies all non-attached_pic streams (audio + chapters handled separately)
/// - Preserves or replaces chapters depending on provided passthrough data
/// - If metadata.cover_art is provided, replaces existing attached_pic with the new one
/// - Otherwise keeps existing cover art unless passthrough cover art is provided
/// - Sets container metadata when provided
/// - Writes to a temp file and atomically replaces the original
pub fn rewrite_metadata_with_ffmpeg(
    input_path: &std::path::Path,
    metadata: Option<&AudiobookMetadata>,
    passthrough: Option<&PassthroughMetadata>,
) -> Result<()> {
    let metadata_plan = metadata.cloned().map(MetadataWritePlan::from_metadata);
    rewrite_metadata_with_ffmpeg_plan(input_path, metadata_plan.as_ref(), passthrough)
}

pub(crate) fn rewrite_metadata_with_ffmpeg_plan(
    input_path: &std::path::Path,
    metadata: Option<&MetadataWritePlan>,
    passthrough: Option<&PassthroughMetadata>,
) -> Result<()> {
    rewrite_metadata_with_ffmpeg_plan_as(input_path, metadata, passthrough, None)
}

pub(crate) fn rewrite_metadata_with_ffmpeg_plan_as(
    input_path: &std::path::Path,
    metadata: Option<&MetadataWritePlan>,
    passthrough: Option<&PassthroughMetadata>,
    output_format: Option<&str>,
) -> Result<()> {
    use crate::errors::AppError;

    ff::init().map_err(AppError::Ffmpeg)?;
    let mut ictx = ff::format::input(input_path).map_err(AppError::Ffmpeg)?;
    let temp_path = build_temp_output_path(input_path)?;
    let mut octx = match output_format {
        Some(format) => ff::format::output_as(&temp_path, format).map_err(AppError::Ffmpeg)?,
        None => ff::format::output(&temp_path).map_err(AppError::Ffmpeg)?,
    };
    let metadata_value = metadata.map(|plan| &plan.metadata);
    let (stream_mapping, output_time_bases) = copy_streams(&ictx, &mut octx, metadata_value)?;
    copy_chapters(&ictx, &mut octx, passthrough)?;
    copy_container_metadata(&ictx, &mut octx, metadata)?;

    let cover = select_cover_art(metadata_value, passthrough);
    let cover_stream_info = if let Some(selection) = cover {
        match add_cover_art_stream_pre_header(&mut octx, selection.bytes()) {
            Ok(stream_info) => stream_info,
            Err(error) if selection.is_passthrough() => {
                log::warn!(
                    "Could not preserve passthrough cover art during metadata remux: {}",
                    error
                );
                None
            }
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    octx.write_header().map_err(AppError::Ffmpeg)?;

    if let (Some(selection), Some((stream_index, format))) = (cover, cover_stream_info) {
        if let Err(error) =
            write_cover_art_packet_post_header(&mut octx, stream_index, selection.bytes(), format)
        {
            if selection.is_passthrough() {
                log::warn!(
                    "Could not preserve passthrough cover art packet during metadata remux: {}",
                    error
                );
            } else {
                return Err(error);
            }
        }
    }

    stream_copy_packets(&mut ictx, &mut octx, &stream_mapping, &output_time_bases)?;
    octx.write_trailer().map_err(AppError::Ffmpeg)?;
    std::fs::rename(&temp_path, input_path).map_err(AppError::Io)?;
    Ok(())
}

fn build_temp_output_path(input_path: &std::path::Path) -> Result<std::path::PathBuf> {
    use crate::errors::AppError;

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
        .and_then(|value| value.to_str())
        .unwrap_or("m4b");
    let temp_path = parent.join(format!(".abb_meta_{}.{}", uuid::Uuid::new_v4(), ext));
    if temp_path.exists() {
        std::fs::remove_file(&temp_path).map_err(AppError::Io)?;
    }
    Ok(temp_path)
}

fn copy_streams(
    ictx: &ff::format::context::Input,
    octx: &mut ff::format::context::Output,
    metadata: Option<&AudiobookMetadata>,
) -> Result<(Vec<isize>, Vec<Option<ff::Rational>>)> {
    use crate::errors::AppError;

    let stream_len = ictx.streams().len();
    let mut stream_mapping: Vec<isize> = vec![-1; stream_len];
    let mut output_time_bases: Vec<Option<ff::Rational>> = vec![None; stream_len];

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
        if is_attached_pic
            && metadata
                .and_then(|value| value.cover_art.as_ref())
                .is_some()
        {
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

    Ok((stream_mapping, output_time_bases))
}

fn copy_chapters(
    ictx: &ff::format::context::Input,
    octx: &mut ff::format::context::Output,
    passthrough: Option<&PassthroughMetadata>,
) -> Result<()> {
    if let Some(passthrough) = passthrough {
        if let Err(error) =
            crate::metadata::passthrough::add_chapters_to_output(octx, &passthrough.chapters)
        {
            log::warn!(
                "Could not preserve passthrough chapters during metadata remux: {}",
                error
            );
        }
        return Ok(());
    }

    if ictx.nb_chapters() == 0 {
        return Ok(());
    }

    for chapter in ictx.chapters() {
        let title = chapter
            .metadata()
            .get("title")
            .map(|value| value.to_string());
        if let Err(error) = octx.add_chapter(
            chapter.id(),
            chapter.time_base(),
            chapter.start(),
            chapter.end(),
            title.as_deref().unwrap_or(""),
        ) {
            log::warn!("Failed to add chapter id {}: {}", chapter.id(), error);
        }
    }

    Ok(())
}

fn copy_container_metadata(
    ictx: &ff::format::context::Input,
    octx: &mut ff::format::context::Output,
    metadata: Option<&MetadataWritePlan>,
) -> Result<()> {
    if let Some(plan) = metadata {
        let merged_dict =
            super::ffmpeg_dict::merge_metadata_with_plan(ictx.metadata().to_owned(), plan)?;
        octx.set_metadata(merged_dict);
    } else {
        octx.set_metadata(ictx.metadata().to_owned());
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum CoverArtSelection<'a> {
    Explicit(&'a Vec<u8>),
    Passthrough(&'a Vec<u8>),
}

impl<'a> CoverArtSelection<'a> {
    fn bytes(self) -> &'a Vec<u8> {
        match self {
            Self::Explicit(bytes) | Self::Passthrough(bytes) => bytes,
        }
    }

    fn is_passthrough(self) -> bool {
        matches!(self, Self::Passthrough(_))
    }
}

fn select_cover_art<'a>(
    metadata: Option<&'a AudiobookMetadata>,
    passthrough: Option<&'a PassthroughMetadata>,
) -> Option<CoverArtSelection<'a>> {
    metadata
        .and_then(|value| value.cover_art.as_ref())
        .map(CoverArtSelection::Explicit)
        .or_else(|| {
            passthrough
                .and_then(|value| value.cover_art.as_ref())
                .map(CoverArtSelection::Passthrough)
        })
        .filter(|selection| !selection.bytes().is_empty())
}

fn stream_copy_packets(
    ictx: &mut ff::format::context::Input,
    octx: &mut ff::format::context::Output,
    stream_mapping: &[isize],
    output_time_bases: &[Option<ff::Rational>],
) -> Result<()> {
    use crate::errors::AppError;

    for (input_stream, mut packet) in ictx.packets() {
        let in_index = input_stream.index();
        let out_index = *stream_mapping.get(in_index).unwrap_or(&-1);
        if out_index < 0 {
            continue;
        }

        let out_tb = output_time_bases
            .get(out_index as usize)
            .and_then(|time_base| *time_base)
            .unwrap_or(input_stream.time_base());

        packet.set_stream(out_index as usize);
        packet.rescale_ts(input_stream.time_base(), out_tb);
        packet.write_interleaved(octx).map_err(AppError::Ffmpeg)?;
    }

    Ok(())
}
