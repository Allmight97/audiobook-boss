use super::container::Container;
use super::passthrough::PassthroughMetadata;
use super::{AudiobookMetadata, MetadataWritePlan};
use crate::errors::{AppError, Result};
use abb_ffmpeg::{
    CodecId, Disposition, Input, Medium, Muxer, Output, Packet, Rational, StreamFacts, Tags, Text,
};
use abb_metadata_core::PatchOp;
use std::time::Instant;

use super::cover_art::embedding::{add_cover_stream, write_cover_packet};
use super::cover_art::ResolvedCover;
use crate::file_replace::SiblingTemp;

/// Rewrite metadata and optional passthrough state by remux (stream copy).
/// - Copies all streams except data streams; attached pictures stay unless a
///   cover is written
/// - Preserves or replaces chapters depending on provided passthrough data
/// - An explicit or passthrough cover replaces the source pictures
/// - Sets container metadata when provided
/// - Writes to a sibling temp and replaces the original only when complete
pub fn rewrite_metadata_with_ffmpeg(
    input_path: &std::path::Path,
    metadata: Option<&AudiobookMetadata>,
    passthrough: Option<&PassthroughMetadata>,
) -> Result<()> {
    let metadata_plan = metadata.map(MetadataWritePlan::from_metadata);
    rewrite_metadata_with_ffmpeg_plan_as(input_path, metadata_plan.as_ref(), passthrough, None)
}

pub(crate) fn rewrite_metadata_with_ffmpeg_plan_as(
    input_path: &std::path::Path,
    metadata: Option<&MetadataWritePlan>,
    passthrough: Option<&PassthroughMetadata>,
    output_format: Option<&Container>,
) -> Result<()> {
    let temp = meta_temp(input_path)?;
    remux_into(
        input_path,
        temp.path(),
        metadata,
        passthrough,
        output_format,
    )?;
    crate::diagnostics::stage("metadata_replace", input_path, || {
        temp.commit(input_path).map_err(AppError::Io)
    })
}

/// The sibling `.abb_meta_<uuid>.<ext>` a rewrite of `input_path` is written
/// into; its extension lets FFmpeg pick the muxer when no format is named.
pub(super) fn meta_temp(input_path: &std::path::Path) -> Result<SiblingTemp> {
    let ext = input_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("m4b");
    SiblingTemp::beside(input_path, "meta", &format!(".{ext}")).map_err(AppError::Io)
}

pub(super) fn remux_into(
    input_path: &std::path::Path,
    temp_path: &std::path::Path,
    metadata: Option<&MetadataWritePlan>,
    passthrough: Option<&PassthroughMetadata>,
    output_format: Option<&Container>,
) -> Result<()> {
    use crate::diagnostics::stage;
    let started = Instant::now();

    let mut ictx = stage("metadata_open_input", input_path, || {
        crate::metadata::open_input(input_path)
    })?;

    log::info!(
        "media_handoff stage=remux_create source_artifact={} destination_artifact={}",
        crate::diagnostics::artifact_id(input_path),
        crate::diagnostics::artifact_id(temp_path)
    );

    let mut octx = stage("metadata_create_output", temp_path, || {
        match output_format {
            Some(container) => Output::create(temp_path, container.muxer_name()),
            None if temp_path
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(Container::from_extension)
                .is_some_and(|container| container.is_mp4_family()) =>
            {
                Output::create(temp_path, Container::Mp4.muxer_name())
            }
            None => Output::create_for_file_name(temp_path),
        }
        .map_err(AppError::Ffmpeg)
    })?;
    let route = Route {
        input: Container::of_input(&ictx),
        output: Container::of_output(&octx),
    };

    if let Some(plan) = metadata {
        super::plan_log::log_write_plan(
            plan,
            "ffmpeg_remux",
            &crate::diagnostics::artifact_id(input_path),
        );
    }
    log::info!("metadata_route artifact={} input_format={} writer=ffmpeg_remux output_format={} chapters={}",
        crate::diagnostics::artifact_id(input_path), ictx.format_name(), octx.muxer_name(),
        passthrough.map_or(0, |value| value.chapters.len()));

    let cover_intent = metadata.and_then(|plan| plan.cover.as_ref());
    let cover = ResolvedCover::select(cover_intent, passthrough);
    let replace_cover = cover.is_some() || cover_intent.is_some();
    let stream_mapping = stage("metadata_copy_streams", temp_path, || {
        copy_streams(&ictx, &mut octx, &route, replace_cover)
    })?;

    stage("metadata_copy_chapters", temp_path, || {
        copy_chapters(&ictx, &mut octx, passthrough)
    })?;
    stage("metadata_copy_tags", temp_path, || {
        copy_container_metadata(&ictx, &mut octx, &route, metadata)
    })?;

    log_cover_selection(input_path, cover, cover_intent);
    let cover_stream = if let Some(selection) = cover {
        match add_cover_stream(&mut octx, selection.bytes()) {
            Ok(stream) => stream,
            Err(error) => {
                selection.handle_embed_failure(error, "cover art")?;
                None
            }
        }
    } else {
        None
    };

    let mut muxer = stage("metadata_write_header", temp_path, || {
        write_container_header(&ictx, octx, &route)
    })?;
    if let (Some(selection), Some(stream)) = (cover, cover_stream) {
        if let Err(error) = write_cover_packet(&mut muxer, stream, selection.bytes()) {
            selection.handle_embed_failure(error, "cover art packet")?;
        }
    }
    let delivered = stage("metadata_copy_packets", temp_path, || {
        stream_copy_packets(&mut ictx, &mut muxer, &route, &stream_mapping)
    })?;
    if route.input.is_mp4_family() && route.output.is_mp4_family() {
        stage("metadata_copy_hidden_units", temp_path, || {
            copy_hidden_trailing_units(input_path, &mut muxer, &stream_mapping, delivered)
        })?;
    }
    stage("metadata_write_trailer", temp_path, || {
        muxer.finish().map_err(AppError::Ffmpeg)
    })?;

    log::info!(
        "metadata_remux status=ok elapsed_ms={} path={}",
        started.elapsed().as_millis(),
        input_path.display()
    );
    Ok(())
}

/// The containers a remux reads and writes.
struct Route {
    input: Container,
    output: Container,
}

fn stream_tag(stream: &StreamFacts, key: &str) -> String {
    stream.tags.get(key).map(Text::display).unwrap_or_default()
}

/// Maps each input stream to its output stream, or `None` for one the remux
/// leaves out.
fn copy_streams(
    ictx: &Input,
    octx: &mut Output,
    route: &Route,
    replace_cover: bool,
) -> Result<Vec<Option<usize>>> {
    let octx_is_mp4 = route.output.is_mp4_family();
    let streams = ictx.streams();
    let mut stream_mapping = vec![None; streams.len()];
    for istream in &streams {
        if istream.medium == Medium::Data {
            log::info!(
                "Skipping data stream {} (codec: {}) during metadata remux",
                istream.index,
                istream.codec.name()
            );
            continue;
        }
        let is_attached_pic = istream.disposition.contains(Disposition::ATTACHED_PIC);
        let is_cover_attachment = istream.medium == Medium::Attachment
            && super::matroska_cover::is_cover(
                &stream_tag(istream, "filename"),
                &stream_tag(istream, "mimetype"),
            );
        if (is_attached_pic || is_cover_attachment) && replace_cover {
            log::info!("Skipping source attached_pic stream in favor of new cover art");
            continue;
        }
        if is_attached_pic && route.output == Container::Matroska {
            // The Matroska muxer drops attached-picture streams; its demuxer
            // turned the source's picture attachments into them.
            if let Some(bytes) = ictx.attached_picture(istream.index) {
                octx.add_attachment_stream(istream.codec, &bytes, &istream.tags)?;
            }
            continue;
        }
        let index = copy_stream(ictx, istream, octx, octx_is_mp4)?;
        stream_mapping[istream.index] = Some(index);
    }
    Ok(stream_mapping)
}

/// Adds the output stream that copies `istream` and returns its index.
fn copy_stream(
    ictx: &Input,
    istream: &StreamFacts,
    octx: &mut Output,
    octx_is_mp4: bool,
) -> Result<usize> {
    let index = octx.add_stream_copied_from(ictx, istream.index)?;
    octx.set_stream_time_base(index, istream.time_base)?;
    if istream.codec == CodecId::OPUS && octx_is_mp4 {
        // FFmpeg MOV needs a frame size to honor final discard padding.
        // Opus packets can vary in duration; use its smallest legal frame.
        octx.set_stream_frame_size(index, 120)?;
        octx.set_stream_time_base(index, Rational::new(1, 48_000))?;
    }
    octx.set_stream_tags(index, &istream.tags)?;
    octx.set_stream_disposition_and_clear_codec_tag(index, istream.disposition)?;
    if istream.disposition.contains(Disposition::ATTACHED_PIC) {
        set_missing_picture_dimensions(ictx, istream.index, octx, index)?;
    }
    Ok(index)
}

/// The bundled FFmpeg has no PNG, GIF, BMP or WebP decoder, so such a picture
/// probes without dimensions and every muxer but FLAC's refuses to copy it;
/// the image header supplies them.
fn set_missing_picture_dimensions(
    ictx: &Input,
    input_index: usize,
    octx: &mut Output,
    output_index: usize,
) -> Result<()> {
    let sized = octx
        .streams()
        .get(output_index)
        .is_some_and(|stream| stream.width > 0 && stream.height > 0);
    if sized {
        return Ok(());
    }
    let Some(bytes) = ictx.attached_picture(input_index) else {
        return Ok(());
    };
    if let Some((width, height)) = super::cover_art::format::picture_dimensions(&bytes) {
        octx.set_stream_picture_size(output_index, width, height)?;
    }
    Ok(())
}

fn copy_chapters(
    ictx: &Input,
    octx: &mut Output,
    passthrough: Option<&PassthroughMetadata>,
) -> Result<()> {
    if let Some(passthrough) = passthrough {
        crate::metadata::add_chapters(octx, &passthrough.chapters)?;
        return Ok(());
    }

    for chapter in ictx.chapters() {
        let copied = super::passthrough::titled_chapter(&chapter)
            .and_then(|chapter| octx.add_chapter(&chapter));
        if let Err(error) = copied {
            log::warn!("Failed to add chapter id {}: {}", chapter.id, error);
        }
    }
    Ok(())
}

fn copy_container_metadata(
    ictx: &Input,
    octx: &mut Output,
    route: &Route,
    metadata: Option<&MetadataWritePlan>,
) -> Result<()> {
    let existing = ictx.tags();
    let mut tags = match metadata {
        Some(plan) => super::ffmpeg_dict::merge_metadata_with_plan(&existing, plan, &route.output)?,
        None => existing.clone(),
    };
    if route.output.is_mp4_family() {
        if let Some(tool) = existing.get("encoder") {
            let key = Text::new("encoding_tool", abb_ffmpeg::Step::SetTags)?;
            tags.insert(key, tool.clone());
        }
    }
    octx.set_tags(&tags)?;
    Ok(())
}

/// The last audio access unit the edit-aware read delivered for a stream: its
/// byte position and the timestamp after it, in the input time base.
#[derive(Clone, Copy)]
struct LastUnit {
    position: i64,
    end: i64,
}

fn stream_copy_packets(
    ictx: &mut Input,
    muxer: &mut Muxer,
    route: &Route,
    stream_mapping: &[Option<usize>],
) -> Result<Vec<Option<LastUnit>>> {
    let exact_opus_clock = route.input == Container::Matroska && route.output.is_mp4_family();
    let streams = ictx.streams();
    let mut opus_positions = vec![None; streams.len()];
    let mut delivered = vec![None; streams.len()];
    let opus_clock = Rational::new(1, 48_000);
    let mut packet = Packet::new()?;
    while ictx.read_packet(&mut packet)? {
        let in_index = packet.stream_index();
        let input_stream = streams
            .get(in_index)
            .ok_or_else(|| AppError::General("Remux input stream is missing.".into()))?;
        let Some(out_index) = stream_mapping.get(in_index).copied().flatten() else {
            continue;
        };
        let out_tb = muxer
            .stream_time_base(out_index)
            .map_err(|_| AppError::General("Remux audio stream is missing.".into()))?;
        packet.set_stream_index(out_index);
        if exact_opus_clock && input_stream.codec == CodecId::OPUS {
            // Matroska timestamps round to milliseconds. Recover the sample clock
            // from packet framing, preserving priming and final discard padding.
            let position =
                opus_positions[in_index].get_or_insert(-i64::from(input_stream.initial_padding));
            let pts = packet.pts().ok_or_else(|| {
                AppError::InvalidInput(
                    "Opus source timestamps are missing. Use encoding settings for this title."
                        .into(),
                )
            })?;
            let source_pts = Rational::rescale(pts, input_stream.time_base, opus_clock);
            if source_pts.abs_diff(*position) > 48 {
                return Err(AppError::InvalidInput("Opus source has a discontinuous timeline. Use encoding settings for this title.".into()));
            }
            let duration = opus_packet_samples(packet.data())?;
            packet.set_pts(Some(*position));
            packet.set_dts(Some(*position));
            packet.set_duration(duration);
            *position += duration;
            packet.rescale_ts(opus_clock, out_tb);
        } else {
            if input_stream.medium == Medium::Audio {
                if let Some(position) = packet.position() {
                    delivered[in_index] = packet.pts().map(|pts| LastUnit {
                        position,
                        end: pts + packet.duration(),
                    });
                }
            }
            packet.rescale_ts(input_stream.time_base, out_tb);
        }
        muxer.write_interleaved(&mut packet)?;
    }
    Ok(delivered)
}

/// An MP4 edit list can end before the last access units, and the edit-aware
/// read drops them. They are copied after the delivered units, flagged
/// discard, so the new edit list keeps the playable length and a decoder
/// that reads every unit still finds them.
fn copy_hidden_trailing_units(
    input_path: &std::path::Path,
    muxer: &mut Muxer,
    stream_mapping: &[Option<usize>],
    mut delivered: Vec<Option<LastUnit>>,
) -> Result<()> {
    let mut options = abb_ffmpeg::SecretOptions::default();
    options.push("ignore_editlist", "1")?;
    let mut raw = Input::open(input_path, &options)?.input;
    let time_bases: Vec<Rational> = raw
        .streams()
        .iter()
        .map(|stream| stream.time_base)
        .collect();
    let mut restored = 0_u32;
    let mut packet = Packet::new()?;
    while raw.read_packet(&mut packet)? {
        let in_index = packet.stream_index();
        let Some(Some(last)) = delivered.get_mut(in_index) else {
            continue;
        };
        let position = packet.position().unwrap_or(-1);
        if position <= last.position {
            continue;
        }
        let out_index = stream_mapping
            .get(in_index)
            .copied()
            .flatten()
            .ok_or_else(|| AppError::General("Remux audio stream is missing.".into()))?;
        let in_tb = time_bases
            .get(in_index)
            .copied()
            .ok_or_else(|| AppError::General("Remux input stream is missing.".into()))?;
        let out_tb = muxer
            .stream_time_base(out_index)
            .map_err(|_| AppError::General("Remux audio stream is missing.".into()))?;
        packet.set_pts(Some(last.end));
        packet.set_dts(Some(last.end));
        last.end += packet.duration();
        last.position = position;
        packet.mark_discard();
        packet.set_stream_index(out_index);
        packet.rescale_ts(in_tb, out_tb);
        muxer.write_interleaved(&mut packet)?;
        restored += 1;
    }
    if restored > 0 {
        log::info!("metadata_remux hidden_trailing_units={restored}");
    }
    Ok(())
}

// RFC 6716 §3.1–3.2: TOC configuration gives frame duration; the packing
// code gives frame count. This reads framing only, without decoding audio.
fn opus_packet_samples(data: &[u8]) -> Result<i64> {
    let invalid = || crate::errors::AppError::InvalidInput("Invalid Opus packet framing.".into());
    let toc = *data.first().ok_or_else(invalid)?;
    let config = toc >> 3;
    let frame_samples = match config {
        0..=11 => [480, 960, 1920, 2880][usize::from(config & 3)],
        12..=15 => 480 << (config & 1),
        _ => 120 << (config & 3),
    };
    let frames = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => i64::from(*data.get(1).ok_or_else(invalid)? & 63),
    };
    let samples = frame_samples * frames;
    if !(120..=5760).contains(&samples) {
        return Err(invalid());
    }
    Ok(samples)
}

/// A new Xing header would invent decoder-delay trimming for a source that
/// has none; existing gapless timing stays when it is present.
fn write_container_header(ictx: &Input, octx: Output, route: &Route) -> Result<Muxer> {
    let mut options = Tags::default();
    let starts_at_zero = ictx
        .best_audio_stream()
        .and_then(|index| ictx.streams().into_iter().nth(index))
        .is_some_and(|stream| stream.start_time == Some(0));
    if route.output == Container::Mp3 && starts_at_zero {
        options.push("write_xing", "0", abb_ffmpeg::Step::WriteHeader)?;
    }
    crate::audio::write_output_header(octx, &options)
}

fn log_cover_selection(
    input_path: &std::path::Path,
    cover: Option<ResolvedCover<'_>>,
    cover_intent: Option<&PatchOp<Vec<u8>>>,
) {
    log::info!(
        "metadata_cover artifact={} writer=ffmpeg_remux source={} bytes={} format={:?} decision={}",
        crate::diagnostics::artifact_id(input_path),
        cover.map_or("none", ResolvedCover::source),
        cover.map_or(0, |s| s.bytes().len()),
        cover.and_then(|s| super::cover_art::detect_cover_art_format(s.bytes())),
        if cover.is_some() {
            "embed"
        } else if cover_intent.is_some() {
            "explicit_clear"
        } else {
            "no_cover_data"
        }
    );
}
