//! Packet-copy joining for matching AAC sources with complete, contiguous frames.
//! Interior priming, padding and edit lists need re-encoding; a single MP4 audio
//! track cannot express those per-source decoder resets through this mux path.
use crate::audio::{AudioFile, CleanupGuard, FileListInfo};
use crate::errors::{AppError, Result};
use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy};
use crate::processing::ProcessingContext;
use ffmpeg_next as ff;
use std::path::{Path, PathBuf};

fn incompatible(detail: &str) -> AppError {
    AppError::InvalidInput(format!("Cannot keep original audio in this title: {detail}. Re-encode this title or keep its files as separate titles."))
}

#[derive(PartialEq, Eq)]
struct Configuration {
    extradata: Vec<u8>,
    sample_rate: Option<u32>,
    channels: Option<u32>,
    time_base: ff::Rational,
}

fn open_source(file: &AudioFile) -> Result<(ff::format::context::Input, usize, Configuration)> {
    crate::audio::validate_preservation_source(file)?;
    let path = crate::audio::validate_input_audio_path(&file.path)?;
    let input = ff::format::input(&path)?;
    if input
        .streams()
        .filter(|stream| stream.parameters().medium() == ff::media::Type::Audio)
        .count()
        != 1
    {
        return Err(incompatible(
            "each source must contain exactly one audio stream",
        ));
    }
    if super::faac_timing::FaacDecodeWindow::from_input(&input)?.is_some() {
        return Err(incompatible(
            "a source has encoder priming that must be removed at a join",
        ));
    }
    let stream = input
        .streams()
        .best(ff::media::Type::Audio)
        .ok_or_else(|| incompatible("an audio stream is missing"))?;
    let params = stream.parameters();
    if params.id() != ff::codec::Id::AAC {
        return Err(incompatible("a merged M4B requires AAC sources"));
    }
    let configuration = Configuration {
        extradata: super::streams::read_codec_extradata(&params)
            .ok_or_else(|| incompatible("AAC configuration is missing"))?,
        sample_rate: file.sample_rate,
        channels: file.channels,
        time_base: stream.time_base(),
    };
    let index = stream.index();
    Ok((input, index, configuration))
}

/// Scan without decoding, requiring an exact frame timeline before accepting a
/// stream-copy join. Execution repeats this against immutable staged copies.
fn visit_packets(
    input: &mut ff::format::context::Input,
    stream_index: usize,
    mut visit: impl FnMut(ff::Packet) -> Result<()>,
) -> Result<i64> {
    let stream = input
        .stream(stream_index)
        .ok_or_else(|| incompatible("an audio stream is missing"))?;
    let duration = stream.duration();
    if stream.start_time() != 0 || duration <= 0 {
        return Err(incompatible(
            "a source has a shifted or unknown playable interval",
        ));
    }
    let mut end = 0_i64;
    let mut frame_duration = None;
    loop {
        let mut packet = ff::Packet::empty();
        match packet.read(input) {
            Ok(()) => {}
            Err(ff::Error::Eof) => break,
            Err(error) => return Err(error.into()),
        }
        if packet.stream() != stream_index {
            continue;
        }
        if packet.dts() != Some(end) || packet.pts() != Some(end) || packet.duration() <= 0 {
            return Err(incompatible(
                "a source has priming, gaps, or overlapping timestamps",
            ));
        }
        let step = *frame_duration.get_or_insert(packet.duration());
        if step != packet.duration()
            || packet.side_data().any(|side| {
                side.kind() == ff::packet::side_data::Type::SkipSamples
                    && side.data().iter().take(8).any(|byte| *byte != 0)
            })
        {
            return Err(incompatible("a source has trimmed or padded audio frames"));
        }
        end = end
            .checked_add(step)
            .ok_or_else(|| incompatible("the audio timeline is too long"))?;
        visit(packet)?;
    }
    if end != duration {
        return Err(incompatible(
            "a source's packet duration differs from its playable duration",
        ));
    }
    Ok(end)
}

pub(super) fn validate(files: &[AudioFile]) -> Result<()> {
    let mut expected = None;
    for file in files {
        let (mut input, index, configuration) = open_source(file)?;
        if expected
            .as_ref()
            .is_some_and(|value| value != &configuration)
        {
            return Err(incompatible(
                "AAC configurations, sample rates, or channel layouts differ",
            ));
        }
        expected = Some(configuration);
        visit_packets(&mut input, index, |_| Ok(()))?;
    }
    Ok(())
}

fn remux(files: &[AudioFile], destination: &Path, context: &ProcessingContext) -> Result<()> {
    let first = files
        .first()
        .ok_or_else(|| incompatible("there are no sources"))?;
    let (input, index, expected) = open_source(first)?;
    let parameters = input
        .stream(index)
        .expect("selected stream")
        .parameters()
        .clone();
    drop(input);
    let mut output = ff::format::output(destination)?;
    let mut stream = output.add_stream(ff::encoder::find(ff::codec::Id::AAC))?;
    stream.set_parameters(parameters);
    stream.set_time_base(expected.time_base);
    output.write_header()?;
    let time_base = output.stream(0).expect("created stream").time_base();
    let mut offset = 0_i64;
    for (source_index, file) in files.iter().enumerate() {
        let (mut input, index, configuration) = open_source(file)?;
        if configuration != expected {
            return Err(incompatible("AAC configurations differ"));
        }
        let duration = visit_packets(&mut input, index, |mut packet| {
            if context.is_cancelled() {
                return Err(AppError::cancelled());
            }
            let timestamp = packet
                .dts()
                .and_then(|value| value.checked_add(offset))
                .ok_or_else(|| incompatible("the audio timeline is too long"))?;
            packet.set_pts(Some(timestamp));
            packet.set_dts(Some(timestamp));
            packet.set_stream(0);
            packet.set_position(-1);
            packet.rescale_ts(configuration.time_base, time_base);
            packet.write_interleaved(&mut output)?;
            Ok(())
        })?;
        offset = offset
            .checked_add(duration)
            .ok_or_else(|| incompatible("the audio timeline is too long"))?;
        super::preserve::emit_progress(
            context,
            0.5 + 0.5 * (source_index + 1) as f32 / files.len() as f32,
            "Joining original audio without re-encoding...",
        );
    }
    output.write_trailer()?;
    Ok(())
}

pub(super) fn execute(
    context: ProcessingContext,
    file_info: FileListInfo,
    metadata: Option<AudiobookMetadata>,
    cover_policy: CoverArtPassthroughPolicy,
) -> Result<String> {
    if context.preview.is_some() {
        return Err(AppError::InvalidInput(
            "Preserve audio cannot be used for a preview.".into(),
        ));
    }
    let workspace = super::staging::create_processing_workspace_dir(
        context.session.uuid(),
        context.processing_workspace_root(),
    )?;
    let mut cleanup = CleanupGuard::new(context.session.id());
    cleanup.add_path(&workspace);
    context
        .new_emitter()
        .emit_converting_start("Preserving original audio...");
    let mut copies = Vec::new();
    for (index, file) in file_info.files.iter().enumerate() {
        let fingerprint = file
            .chapter_plan
            .as_ref()
            .ok_or_else(|| incompatible("source identity is missing"))?
            .source_fingerprint
            .as_str();
        let path = workspace.join(format!("source-{index}.m4b"));
        cleanup.add_path(&path);
        super::preserve::copy_with_cancellation(
            &file.path,
            fingerprint,
            &path,
            &context,
            (0.5 * index as f32 / file_info.files.len() as f32)
                ..(0.5 * (index + 1) as f32 / file_info.files.len() as f32),
        )?;
        let mut copy = file.clone();
        copy.path = path;
        copies.push(copy);
    }
    let staged: PathBuf = workspace.join("merged.m4b");
    cleanup.add_path(&staged);
    super::preserve::emit_progress(
        &context,
        0.5,
        "Joining original audio without re-encoding...",
    );
    remux(&copies, &staged, &context)?;
    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }
    let mut sources = super::passthrough_sources_from_audio_files(&copies);
    if sources
        .iter()
        .all(|source| source.chapters.as_ref().is_none_or(Vec::is_empty))
    {
        for (source, original) in sources.iter_mut().zip(&file_info.files) {
            source.chapters = Some(vec![crate::metadata::ChapterSpec {
                title: original
                    .path
                    .file_stem()
                    .map(|name| name.to_string_lossy().into_owned()),
                start_ms: 0,
                end_ms: (source.duration.unwrap_or(0.0) * 1000.0).round() as i64,
            }]);
        }
    }
    let passthrough = cover_policy.apply_to_passthrough(
        crate::metadata::extract_passthrough_metadata(&sources).into_option(),
    );
    let (metadata, passthrough) = crate::metadata::prepare_output_cover_art(metadata, passthrough)?;
    context
        .new_emitter()
        .emit_metadata_start("Writing title metadata and chapters...");
    crate::metadata::finalize_artifact_metadata(&staged, metadata.as_ref(), passthrough.as_ref())?;
    super::finalize::complete_staged_output(&context, staged, &mut cleanup)
}
