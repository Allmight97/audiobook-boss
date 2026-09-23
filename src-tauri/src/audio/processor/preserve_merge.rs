//! Packet-copy joining for matching AAC or MP3 sources with complete, contiguous frames.
//! Interior priming, padding and decoder resets require re-encoding.
use crate::audio::{AudioFile, CleanupGuard, FileListInfo};
use crate::errors::{AppError, Result};
use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy};
use crate::processing::ProcessingContext;
use ffmpeg_next as ff;
use std::path::{Path, PathBuf};

fn incompatible(detail: &str) -> AppError {
    AppError::InvalidInput(format!("Pass-through is unavailable: {detail}."))
}

#[derive(PartialEq, Eq)]
struct Configuration {
    codec: ff::codec::Id,
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
    if !matches!(params.id(), ff::codec::Id::AAC | ff::codec::Id::MP3) {
        return Err(incompatible(&format!(
            "merging without re-encoding currently supports AAC or MP3; '{}' contains {}",
            crate::errors::sanitize_path_for_display(&path),
            params.id().name().to_uppercase(),
        )));
    }
    let configuration = Configuration {
        codec: params.id(),
        extradata: if params.id() == ff::codec::Id::MP3 {
            Vec::new()
        } else {
            super::streams::read_codec_extradata(&params)
                .ok_or_else(|| incompatible("AAC configuration is missing"))?
        },
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
    let mp3 = stream.parameters().id() == ff::codec::Id::MP3;
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
        if mp3 && end == 0 {
            validate_mp3_start(packet.data().unwrap_or_default())?;
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
    if !mp3 && end != duration {
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
                "Audio configurations, sample rates, or channel layouts differ",
            ));
        }
        expected = Some(configuration);
        visit_packets(&mut input, index, |_| Ok(()))?;
    }
    Ok(())
}

fn remux(files: &[AudioFile], destination: &Path, context: &ProcessingContext) -> Result<Vec<f64>> {
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
    let mut stream = output.add_stream(ff::encoder::find(expected.codec))?;
    stream.set_parameters(parameters);
    stream.set_time_base(expected.time_base);
    let mut options = ff::Dictionary::new();
    if expected.codec == ff::codec::Id::MP3 {
        options.set("write_xing", "0");
    }
    output.write_header_with(options)?;
    let time_base = output.stream(0).expect("created stream").time_base();
    let mut offset = 0_i64;
    let mut durations = Vec::with_capacity(files.len());
    for (source_index, file) in files.iter().enumerate() {
        let (mut input, index, configuration) = open_source(file)?;
        if configuration != expected {
            return Err(incompatible("Audio configurations differ"));
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
        durations.push(duration as f64 * f64::from(configuration.time_base));
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
    Ok(durations)
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
        let path = workspace
            .join(format!("source-{index}"))
            .with_extension(file.path.extension().unwrap_or_default());
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
    let staged: PathBuf = workspace
        .join("merged")
        .with_extension(context.output.final_path().extension().unwrap_or_default());
    cleanup.add_path(&staged);
    super::preserve::emit_progress(
        &context,
        0.5,
        "Joining original audio without re-encoding...",
    );
    let durations = remux(&copies, &staged, &context)?;
    for (file, duration) in copies.iter_mut().zip(durations) {
        file.duration = Some(duration);
    }
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

fn validate_mp3_start(frame: &[u8]) -> Result<()> {
    if frame.len() < 8 || frame[0] != 0xff || frame[1] & 0xe6 != 0xe2 {
        return Err(incompatible(
            "an MP3 source begins with an incomplete frame",
        ));
    }
    let side = 4 + if frame[1] & 1 == 0 { 2 } else { 0 };
    let reservoir = if frame[1] & 0x18 == 0x18 {
        u16::from(frame[side]) * 2 + u16::from(frame[side + 1] >> 7)
    } else {
        u16::from(frame[side])
    };
    if reservoir != 0 {
        return Err(incompatible(
            "an MP3 source depends on audio data before its first frame",
        ));
    }
    Ok(())
}
