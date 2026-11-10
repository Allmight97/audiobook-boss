//! Packet-copy joining for matching AAC sources with complete, contiguous frames.
//! Interior priming, padding and decoder resets require re-encoding.
use super::workspace::Workspace;
use crate::audio::AudioFile;
use crate::errors::{AppError, Result};
use crate::execution::ProcessingContext;
use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy};
use abb_ffmpeg::{CodecId, Input, Medium, Packet, Rational, StreamFacts};
use std::path::{Path, PathBuf};

fn incompatible(detail: &str) -> AppError {
    AppError::InvalidInput(format!("Pass-through is unavailable: {detail}."))
}

#[derive(PartialEq, Eq)]
struct Configuration {
    codec: CodecId,
    extradata: Vec<u8>,
    sample_rate: Option<u32>,
    channels: Option<u32>,
    time_base: Rational,
}

fn open_source(file: &AudioFile) -> Result<(Input, StreamFacts, Configuration)> {
    crate::audio::validate_preservation_source(file)?;
    let path = crate::audio::validate_input_audio_path(&file.path)?;
    let input = crate::metadata::open_input(&path)?;
    let streams = input.streams();
    if streams
        .iter()
        .filter(|stream| stream.medium == Medium::Audio)
        .count()
        != 1
    {
        return Err(incompatible(
            "each source must contain exactly one audio stream",
        ));
    }
    let encoder_tag = input.tags().get("encoder").map(abb_ffmpeg::Text::display);
    if super::he_timing::is_abb_faac_he(encoder_tag.as_deref()) {
        return Err(incompatible(
            "a source has encoder priming that must be removed at a join",
        ));
    }
    let stream = input
        .best_audio_stream()
        .and_then(|index| streams.into_iter().nth(index))
        .ok_or_else(|| incompatible("an audio stream is missing"))?;
    if stream.codec != CodecId::AAC {
        return Err(incompatible(&format!(
            "merging without re-encoding currently supports AAC; '{}' contains {}",
            crate::errors::sanitize_path_for_display(&path),
            stream.codec.name().to_uppercase(),
        )));
    }
    let configuration = Configuration {
        codec: stream.codec,
        extradata: input
            .stream_extradata(stream.index)
            .ok_or_else(|| incompatible("AAC configuration is missing"))?,
        sample_rate: file.sample_rate,
        channels: file.channels,
        time_base: stream.time_base,
    };
    Ok((input, stream, configuration))
}

fn visit_packets(
    input: &mut Input,
    stream: &StreamFacts,
    mut visit: impl FnMut(&mut Packet) -> Result<()>,
) -> Result<i64> {
    let duration = stream.duration.unwrap_or(i64::MIN);
    if stream.start_time != Some(0) || duration <= 0 {
        return Err(incompatible(
            "a source has a shifted or unknown playable interval",
        ));
    }
    let mut end = 0_i64;
    let mut frame_duration = None;
    let mut packet = Packet::new()?;
    while input.read_packet(&mut packet)? {
        if packet.stream_index() != stream.index {
            continue;
        }
        if packet.dts() != Some(end) || packet.pts() != Some(end) || packet.duration() <= 0 {
            return Err(incompatible(
                "a source has priming, gaps, or overlapping timestamps",
            ));
        }
        let step = *frame_duration.get_or_insert(packet.duration());
        let trimmed = !matches!(
            packet.skip_samples(),
            Ok(None
                | Some(abb_ffmpeg::SkipSamples {
                    start: 0,
                    end: 0,
                    ..
                }))
        );
        if step != packet.duration() || trimmed {
            return Err(incompatible("a source has trimmed or padded audio frames"));
        }
        end = end
            .checked_add(step)
            .ok_or_else(|| incompatible("the audio timeline is too long"))?;
        visit(&mut packet)?;
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
        let (mut input, stream, configuration) = open_source(file)?;
        if expected
            .as_ref()
            .is_some_and(|value| value != &configuration)
        {
            return Err(incompatible(
                "Audio configurations, sample rates, or channel layouts differ",
            ));
        }
        expected = Some(configuration);
        visit_packets(&mut input, &stream, |_| Ok(()))?;
    }
    Ok(())
}

fn remux(files: &[AudioFile], destination: &Path, context: &ProcessingContext) -> Result<Vec<f64>> {
    let first = files
        .first()
        .ok_or_else(|| incompatible("there are no sources"))?;
    let (input, stream, expected) = open_source(first)?;
    let mut output = abb_ffmpeg::Output::create_for_file_name(destination)?;
    output.add_stream_copied_from(&input, stream.index)?;
    output.set_stream_time_base(0, expected.time_base)?;
    drop(input);
    let mut muxer = crate::audio::write_output_header(output, &abb_ffmpeg::Tags::default())?;
    let time_base = muxer.stream_time_base(0)?;
    let mut offset = 0_i64;
    let mut durations = Vec::with_capacity(files.len());
    for (source_index, file) in files.iter().enumerate() {
        let (mut input, stream, configuration) = open_source(file)?;
        if configuration != expected {
            return Err(incompatible("Audio configurations differ"));
        }
        let duration = visit_packets(&mut input, &stream, |packet| {
            if context.is_cancelled() {
                return Err(AppError::cancelled());
            }
            let timestamp = packet
                .dts()
                .and_then(|value| value.checked_add(offset))
                .ok_or_else(|| incompatible("the audio timeline is too long"))?;
            packet.set_pts(Some(timestamp));
            packet.set_dts(Some(timestamp));
            packet.set_stream_index(0);
            packet.clear_position();
            packet.rescale_ts(configuration.time_base, time_base);
            muxer.write_interleaved(packet)?;
            Ok(())
        })?;
        let seconds_per_tick = f64::from(configuration.time_base.numerator)
            / f64::from(configuration.time_base.denominator);
        durations.push(duration as f64 * seconds_per_tick);
        offset = offset
            .checked_add(duration)
            .ok_or_else(|| incompatible("the audio timeline is too long"))?;
        super::preserve::emit_progress(
            context,
            0.5 + 0.5 * (source_index + 1) as f32 / files.len() as f32,
            "Joining original audio without re-encoding...",
        );
    }
    muxer.finish()?;
    Ok(durations)
}

pub(super) fn write_fresh_mux(
    context: &ProcessingContext,
    workspace: &Workspace,
    mut copies: Vec<AudioFile>,
    metadata: Option<AudiobookMetadata>,
    cover_policy: CoverArtPassthroughPolicy,
) -> Result<(PathBuf, Vec<String>)> {
    let staged: PathBuf = workspace
        .dir()
        .join("merged")
        .with_extension(context.output.final_path().extension().unwrap_or_default());
    super::preserve::emit_progress(
        context,
        0.5,
        "Joining original audio without re-encoding...",
    );
    let durations = remux(&copies, &staged, context)?;
    for (file, duration) in copies.iter_mut().zip(durations) {
        file.duration = Some(duration);
    }
    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }
    let sources = super::passthrough_sources_from_audio_files(&copies);
    let mut extracted = crate::metadata::extract_passthrough_metadata(&sources);
    let passthrough_warnings = std::mem::take(&mut extracted.warnings);
    let passthrough = cover_policy.apply_to_passthrough(extracted.into_option());
    let (metadata, passthrough, cover_warning) =
        crate::metadata::prepare_output_cover_art(metadata, passthrough, cover_policy)?;
    context
        .new_emitter()
        .emit_metadata_start("Writing title metadata and chapters...");
    crate::metadata::finalize_artifact_metadata(&staged, metadata.as_ref(), passthrough.as_ref())?;
    Ok((
        staged,
        passthrough_warnings
            .into_iter()
            .chain(cover_warning)
            .collect(),
    ))
}

#[cfg(test)]
#[path = "preserve_merge_tests.rs"]
mod tests;
