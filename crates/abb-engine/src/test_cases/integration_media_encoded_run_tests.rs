//! Real-media proofs that an encoded run publishes a book whose length, audio
//! and chapters match its sources, or publishes nothing. Fixtures are
//! synthesized at test time (WAV in Rust, other formats through the FFmpeg
//! CLI); no media is committed. Route-by-route library facts live in
//! `integration_media_route_parity_tests.rs`.

use super::media_fixtures::{
    assert_auto_merge_stereo, audio_duration_ts, audio_stream_timing, chapters_of, decode_pcm_f32,
    ffmpeg, ffprobe, find_atom, minimal_jpg_bytes, native_encoder_settings, next_atom, parse_mdhd,
    timed_text_chapter_track, write_sine_wav, write_stereo_sine_wav, MediaLane, SAMPLE_RATE,
};
use crate::audio::{
    execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest, ChannelConfig,
    EncoderSettings, SampleRateConfig,
};
#[cfg(target_os = "macos")]
use crate::audio::{BitrateMode, EncoderType};
use crate::metadata::{read_metadata, AudiobookMetadata, CoverArtPassthroughPolicy};
use crate::processing::{
    AudioHandling, CancellationChecker, JobRegistry, OutputConfig, ProcessingContext,
    ProcessingSession,
};
use crate::AppError;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn native_aac_reprocessing_keeps_the_original_playable_sample_count() {
    for settings in [
        native_encoder_settings(),
        EncoderSettings {
            native_aac_speed: 4,
            faac_profile: crate::audio::FaacProfile::Auto,
            ..native_encoder_settings()
        },
    ] {
        assert_reprocessing_sample_count(settings).await;
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn apple_aac_reprocessing_keeps_the_original_playable_sample_count() {
    assert_reprocessing_sample_count(EncoderSettings {
        encoder_type: EncoderType::AacAt,
        bitrate_mode: BitrateMode::Cvbr,
        ..native_encoder_settings()
    })
    .await;
}

async fn assert_reprocessing_sample_count(settings: EncoderSettings) {
    let lane = MediaLane::with_fixtures(&[1.003]).with_encoder(settings.clone());
    let expected = (1.003 * f64::from(SAMPLE_RATE)) as u64;
    let mut output = lane.process(None).await;
    let mut generations = Vec::new();
    for generation in 0..3 {
        let (duration_ts, time_base) = audio_stream_timing(&output);
        assert_eq!(time_base, "1/44100");
        assert_eq!(
            duration_ts, expected,
            "generation {generation} added playable padding"
        );
        let samples = decode_pcm_f32(&output);
        assert_eq!(
            samples.len() as u64,
            expected,
            "generation {generation} decoded sample count"
        );
        let tail = &samples[samples.len() - 512..];
        let rms = (tail
            .iter()
            .map(|value| f64::from(*value).powi(2))
            .sum::<f64>()
            / tail.len() as f64)
            .sqrt();
        assert!(
            rms > 0.1,
            "generation {generation} lost the audible tail: RMS {rms}"
        );
        if generation < 2 {
            let next = MediaLane::for_inputs(vec![output]).with_encoder(settings.clone());
            output = next.process(None).await;
            generations.push(next);
        }
    }
}

const MAX_ROUTE_DIFFERENCE: f32 = 1e-3;

const AAC_FRAME_SAMPLES: u64 = 1_024;

fn stereo_native_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        channels: ChannelConfig::Stereo,
        bitrate_kbps: 320,
        ..native_encoder_settings()
    }
}

fn decode_to_packed_float_wav(source: &Path, wav: &Path) {
    ffmpeg([
        "-v",
        "error",
        "-xerror",
        "-i",
        source.to_str().expect("utf8 path"),
        "-map",
        "0:a:0",
        "-c:a",
        "pcm_f32le",
        "-y",
        wav.to_str().expect("utf8 path"),
    ]);
}

/// A stereo 44.1 kHz AAC M4B decodes to float planar stereo, which is exactly
/// what the Native AAC encoder takes, so reprocessing it uses the
/// decoder-format fast path. The same audio as a packed-float WAV differs in
/// sample format, so it goes through the resampler. Both routes must hand the
/// encoder the same audio, left and right channel included.
#[tokio::test]
async fn fast_path_and_resampler_route_produce_the_same_audio() {
    let tmp = TempDir::new().expect("stereo fixture tempdir");
    let wav = tmp.path().join("stereo.wav");
    write_stereo_sine_wav(&wav, 1.5, 440.0, 660.0);
    let source = MediaLane::for_inputs(vec![wav]).with_encoder(stereo_native_encoder_settings());
    let m4b = source.process(None).await;
    let packed_float = tmp.path().join("packed-float.wav");
    decode_to_packed_float_wav(&m4b, &packed_float);

    let fast = MediaLane::for_inputs(vec![m4b]).with_encoder(stereo_native_encoder_settings());
    let fast_output = fast.process(None).await;
    let resampled = MediaLane::for_inputs(vec![packed_float.clone()])
        .with_encoder(stereo_native_encoder_settings());
    let resampled_output = resampled.process(None).await;

    assert_eq!(
        audio_duration_ts(&fast_output),
        audio_duration_ts(&resampled_output),
        "both routes declare the same playable length and padding"
    );
    let fast_samples = decode_pcm_f32(&fast_output);
    let resampled_samples = decode_pcm_f32(&resampled_output);
    assert_eq!(fast_samples.len(), resampled_samples.len());
    let max_difference = fast_samples
        .iter()
        .zip(&resampled_samples)
        .map(|(fast, resampled)| (fast - resampled).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        max_difference < MAX_ROUTE_DIFFERENCE,
        "fast path diverged from the resampler route by {max_difference}"
    );
}

#[tokio::test]
async fn truncated_audio_fails_without_publishing_a_shortened_book() {
    let lane = MediaLane::with_fixtures(&[2.0]);
    let complete = lane.process(None).await;
    let broken = lane.tmp.path().join("truncated.m4b");
    ffmpeg([
        "-v",
        "error",
        "-i",
        complete.to_str().expect("utf8 path"),
        "-map",
        "0:a:0",
        "-c:a",
        "copy",
        "-movflags",
        "+faststart",
        broken.to_str().expect("utf8 path"),
    ]);
    let probe = ffprobe([
        "-v",
        "error",
        "-select_streams",
        "a:0",
        "-show_packets",
        "-of",
        "json",
        broken.to_str().expect("utf8 path"),
    ]);
    let facts: serde_json::Value = serde_json::from_slice(&probe).expect("fixture value");
    let packet = &facts["packets"][30];
    let end = packet["pos"]
        .as_str()
        .expect("fixture value")
        .parse::<u64>()
        .expect("fixture value")
        + packet["size"]
            .as_str()
            .expect("fixture value")
            .parse::<u64>()
            .expect("fixture value");
    fs::OpenOptions::new()
        .write(true)
        .open(&broken)
        .expect("fixture value")
        .set_len(end)
        .expect("fixture value");
    let broken_lane = MediaLane::for_inputs(vec![broken])
        .with_sample_rate(SampleRateConfig::Explicit(SAMPLE_RATE));
    let info = get_file_list_info(&broken_lane.inputs, AacDecoder::Auto).expect("fixture value");
    assert_eq!(
        info.invalid_count, 1,
        "declared packets extend past the end of this MP4"
    );
    assert!(info.files[0]
        .error
        .as_deref()
        .expect("fixture value")
        .contains("incomplete or truncated"));
    let request = AudioExecutionRequest::new(
        broken_lane.context(ProcessingSession::new()),
        info,
        None,
        CoverArtPassthroughPolicy::Preserve,
        broken_lane.handling(),
    );
    let result = execute_audio_engine(request).await;
    assert!(
        result.is_err(),
        "truncated input was reported as successful: {result:?}"
    );
    assert!(!broken_lane.output_path().exists());
    assert!(broken_lane.residual_workspace_dirs().is_empty());
}

const MERGED_SAMPLES: u64 = 110_250;

const SECOND_CHAPTER_START_MS: i64 = 1_500;

const MERGED_END_MS: i64 = 2_500;

const CHAPTER_TOLERANCE_MS: i64 = 2;

fn assert_chapters_at_the_real_joins(chapters: &[(Option<String>, i64, i64)], generation: &str) {
    let expected = [
        ("fixture-0", 0, SECOND_CHAPTER_START_MS),
        ("fixture-1", SECOND_CHAPTER_START_MS, MERGED_END_MS),
    ];
    assert_eq!(chapters.len(), expected.len(), "{generation} chapter count");
    for ((title, start_ms, end_ms), (name, expected_start, expected_end)) in
        chapters.iter().zip(expected)
    {
        assert_eq!(title.as_deref(), Some(name), "{generation} chapter title");
        assert!(
            (start_ms - expected_start).abs() <= CHAPTER_TOLERANCE_MS
                && (end_ms - expected_end).abs() <= CHAPTER_TOLERANCE_MS,
            "{generation} chapter {name} spans {start_ms}..{end_ms} ms, expected {expected_start}..{expected_end} ms"
        );
    }
}

/// Cover art is a second output stream. If its time_base stays 0/0, FFmpeg's
/// ipod muxer defaults it to 1/90000, LCMs with audio 1/44100 into movie
/// timescale 4410000, and the QuickTime chapter packets overflow. Apple Books
/// reads that broken timed-text track; Nero `chpl` stays correct.
fn assert_quicktime_chapter_track_spans_the_book(path: &Path, generation: &str) {
    let (duration_secs, text_timescale, audio_timescale) = timed_text_chapter_track(path);
    assert_eq!(
        text_timescale, audio_timescale,
        "{generation} timed-text timescale must match audio"
    );
    let one_aac_frame_secs = AAC_FRAME_SAMPLES as f64 / f64::from(SAMPLE_RATE);
    assert!(
        (duration_secs - MERGED_END_MS as f64 / 1_000.0).abs() <= one_aac_frame_secs,
        "{generation} timed-text track lasts {duration_secs}s, expected the whole book"
    );
}

fn assert_merged_book(output: &Path, title: &str, cover: &[u8], generation: &str) {
    assert_eq!(
        audio_duration_ts(output),
        MERGED_SAMPLES,
        "{generation} declared length"
    );
    assert_eq!(
        decode_pcm_f32(output).len() as u64,
        MERGED_SAMPLES,
        "{generation} decoded length"
    );

    let analysis = get_file_list_info(&[output], AacDecoder::Auto).expect("analyze the M4B");
    assert_eq!(analysis.valid_count, 1, "{generation} output probes valid");
    assert!((analysis.total_duration - MERGED_END_MS as f64 / 1_000.0).abs() < 0.001);
    let analyzed: Vec<_> = analysis.files[0]
        .chapters
        .iter()
        .map(|chapter| (chapter.title.clone(), chapter.start_ms, chapter.end_ms))
        .collect();
    assert_chapters_at_the_real_joins(&analyzed, &format!("{generation} analysis"));
    assert_chapters_at_the_real_joins(&chapters_of(output), &format!("{generation} chpl"));
    assert_quicktime_chapter_track_spans_the_book(output, generation);

    let tags = read_metadata(output).expect("re-read tags from the artifact");
    assert_eq!(tags.title.as_deref(), Some(title), "{generation} title");
    assert_eq!(
        tags.cover_art.as_deref(),
        Some(cover),
        "{generation} cover bytes"
    );
}

#[tokio::test]
async fn merged_title_keeps_every_sample_chapter_and_cover_through_reprocessing() {
    let lane = MediaLane::with_fixtures(&[1.5, 1.0]);
    let cover = minimal_jpg_bytes();
    let mut metadata = AudiobookMetadata::new();
    metadata.title = Some("Golden Book".to_string());
    metadata.artist = Some("Lane Narrator".to_string());
    metadata.cover_art = Some(cover.clone());

    let message =
        execute_audio_engine(lane.execution_request(ProcessingSession::new(), Some(metadata)))
            .await
            .expect("native processing of generated WAV fixtures succeeds")
            .message;

    let merged = lane.output_path();
    assert!(
        message.ends_with(merged.to_str().expect("utf8 temp path")),
        "engine success message names the committed final path, got: {message}"
    );
    assert_merged_book(&merged, "Golden Book", &cover, "merged");
    assert_eq!(
        read_metadata(&merged)
            .expect("re-read tags")
            .artist
            .as_deref(),
        Some("Lane Narrator")
    );
    assert!(lane.residual_workspace_dirs().is_empty());

    let reprocess_lane = MediaLane::for_inputs(vec![merged]);
    let mut retitle = AudiobookMetadata::new();
    retitle.title = Some("Second Pass".to_string());
    let reprocessed = reprocess_lane.process(Some(retitle)).await;
    assert_merged_book(&reprocessed, "Second Pass", &cover, "reprocessed");
    assert!(reprocess_lane.residual_workspace_dirs().is_empty());
}

#[tokio::test]
async fn a_pre_cancelled_run_publishes_nothing_and_leaves_the_source_alone() {
    let tmp = TempDir::new().expect("cancel fixture tempdir");
    let wav = tmp.path().join("source.wav");
    write_sine_wav(&wav, 1.0, 440.0);
    let m4b_lane = MediaLane::with_fixtures(&[1.0]);
    let m4b = m4b_lane.process(None).await;
    let registry = JobRegistry::new(1);
    for (source, name, handling) in [
        (
            wav,
            "cancelled.m4b",
            AudioHandling::Encode {
                settings: native_encoder_settings(),
            },
        ),
        (m4b, "cancelled-kept.m4b", AudioHandling::Preserve),
    ] {
        let source_bytes = fs::read(&source).expect("read cancellation source");
        let destination = tmp.path().join(name);
        let workspace = tmp.path().join(format!("workspace-{name}"));
        let (job_id, _permit) = registry.register_job().await.expect("register job");
        let checker = CancellationChecker::new(Some(crate::processing::cancelled_token()));
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::with_cancellation(job_id.0, checker)),
            SAMPLE_RATE,
            OutputConfig::new(&destination),
            workspace.clone(),
        );
        let info = get_file_list_info(std::slice::from_ref(&source), AacDecoder::Auto)
            .expect("probe source");
        let error = execute_audio_engine(AudioExecutionRequest::new(
            context,
            info,
            None,
            CoverArtPassthroughPolicy::Preserve,
            handling,
        ))
        .await
        .expect_err("a cancelled run must not report success");
        assert!(
            matches!(error, AppError::Cancellation(_)),
            "{name}: {error:?}"
        );
        assert!(!destination.exists(), "{name}: nothing is published");
        assert_eq!(
            fs::read(&source).expect("source remains"),
            source_bytes,
            "{name}: the source is untouched"
        );
        assert!(
            !workspace.exists()
                || fs::read_dir(&workspace)
                    .expect("workspace")
                    .next()
                    .is_none(),
            "{name}: no staging residue"
        );
    }
}

/// Apple AAC (AudioToolbox) is the second in-process encoder route and is
/// present on every macOS machine, so it earns a deterministic lane test.
/// macOS-only: `aac_at` does not exist elsewhere, so the lane skips it on
/// Linux/Windows agents rather than failing.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn apple_aac_encoder_route_produces_valid_m4b_with_metadata() {
    let lane = MediaLane::with_fixtures(&[1.5]).with_encoder(EncoderSettings {
        encoder_type: EncoderType::AacAt,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cvbr,
        channels: ChannelConfig::Mono,
        native_aac_speed: 0,
        faac_profile: crate::audio::FaacProfile::Auto,
    });
    let mut metadata = AudiobookMetadata::new();
    metadata.title = Some("Apple AAC Route".to_string());

    let output = lane.process(Some(metadata)).await;

    let probe = get_file_list_info(&[&output], AacDecoder::Auto).expect("re-probe aac_at output");
    assert_eq!(probe.valid_count, 1, "aac_at output probes as valid audio");
    let source_samples = (1.5 * f64::from(SAMPLE_RATE)) as u64;
    assert_eq!(
        audio_duration_ts(&output),
        source_samples,
        "declared length"
    );
    assert_eq!(
        decode_pcm_f32(&output).len() as u64,
        source_samples,
        "decoded length"
    );

    let reread = read_metadata(&output).expect("re-read tags from aac_at artifact");
    assert_eq!(reread.title.as_deref(), Some("Apple AAC Route"));
}

#[tokio::test]
async fn rate_converted_merge_keeps_truthful_duration_and_rate() {
    let lane =
        MediaLane::with_fixtures(&[1.5, 1.0]).with_sample_rate(SampleRateConfig::Explicit(22_050));
    let converted_samples = 55_125;

    execute_audio_engine(lane.execution_request(ProcessingSession::new(), None))
        .await
        .expect("rate-converted native processing succeeds");

    let output = lane.output_path();
    let probe =
        get_file_list_info(&[&output], AacDecoder::Auto).expect("re-probe rate-converted M4B");
    assert_eq!(probe.valid_count, 1, "output M4B probes as valid audio");
    assert_eq!(
        probe.files[0].sample_rate,
        Some(22_050),
        "output carries the requested explicit sample rate"
    );
    assert_eq!(
        audio_duration_ts(&output),
        converted_samples,
        "declared length"
    );
    assert_eq!(
        decode_pcm_f32(&output).len() as u64,
        converted_samples,
        "decoded length"
    );
}

#[tokio::test]
async fn native_auto_merge_preserves_distinct_stereo_in_either_input_order() {
    assert_auto_merge_stereo(native_encoder_settings()).await;
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn apple_auto_merge_preserves_distinct_stereo_in_either_input_order() {
    assert_auto_merge_stereo(EncoderSettings {
        encoder_type: EncoderType::AacAt,
        bitrate_mode: BitrateMode::Cvbr,
        ..native_encoder_settings()
    })
    .await;
}

fn assert_quicktime_chapter_offset(path: &Path, start_ms: u32, first_duration_ms: u32) {
    let bytes = fs::read(path).expect("read QuickTime chapter timing");
    let moov = find_atom(&bytes, 0, bytes.len(), *b"moov").expect("moov");
    let mvhd = find_atom(&bytes, moov.0, moov.1, *b"mvhd").expect("mvhd");
    let (movie_timescale, _) = parse_mdhd(&bytes, mvhd.0, mvhd.1);
    let mut offset = moov.0;
    while let Some((start, end, size)) = next_atom(&bytes, offset, moov.1) {
        offset += size;
        if &bytes[start + 4..start + 8] != b"trak" {
            continue;
        }
        let mdia = find_atom(&bytes, start + 8, end, *b"mdia").expect("track media");
        let hdlr = find_atom(&bytes, mdia.0, mdia.1, *b"hdlr").expect("handler");
        if &bytes[hdlr.0 + 8..hdlr.0 + 12] != b"text" {
            continue;
        }
        let mdhd = find_atom(&bytes, mdia.0, mdia.1, *b"mdhd").expect("media header");
        let (text_timescale, _) = parse_mdhd(&bytes, mdhd.0, mdhd.1);
        let edts = find_atom(&bytes, start + 8, end, *b"edts").expect("chapter edits");
        let elst = find_atom(&bytes, edts.0, edts.1, *b"elst").expect("edit list");
        let read_u32 =
            |at| u32::from_be_bytes(bytes[at..at + 4].try_into().expect("fixture value"));
        assert_eq!(
            bytes[elst.0], 0,
            "small fixture uses edit-list version zero"
        );
        assert_eq!(read_u32(elst.0 + 4), 2, "empty edit precedes chapter media");
        assert_eq!(read_u32(elst.0 + 12), u32::MAX, "first edit is empty");
        assert_eq!(
            u64::from(read_u32(elst.0 + 8)) * 1_000,
            u64::from(start_ms) * u64::from(movie_timescale),
            "QuickTime chapter presentation begins after the chapterless prefix"
        );
        let minf = find_atom(&bytes, mdia.0, mdia.1, *b"minf").expect("media information");
        let stbl = find_atom(&bytes, minf.0, minf.1, *b"stbl").expect("sample table");
        let stts = find_atom(&bytes, stbl.0, stbl.1, *b"stts").expect("sample timing");
        assert_eq!(read_u32(stts.0 + 8), 1, "first chapter sample");
        assert_eq!(
            u64::from(read_u32(stts.0 + 12)) * 1_000,
            u64::from(first_duration_ms) * u64::from(text_timescale),
            "first chapter does not absorb the chapterless prefix"
        );
        return;
    }
    panic!("QuickTime chapter track missing");
}

#[tokio::test]
async fn prepending_chapterless_audio_preserves_later_embedded_chapter_positions() {
    let chaptered_lane = MediaLane::with_fixtures(&[0.4, 0.3]);
    let chaptered = chaptered_lane.process(None).await;
    let mut expected = chapters_of(&chaptered);
    let tmp = TempDir::new().expect("chapterless prefix fixture");
    let prefix = tmp.path().join("prefix.wav");
    write_sine_wav(&prefix, 1.25, 330.0);
    for chapter in &mut expected {
        chapter.1 += 1_250;
        chapter.2 += 1_250;
    }

    let lane = MediaLane::for_inputs(vec![prefix, chaptered]);
    let output = lane.process(None).await;
    assert_eq!(chapters_of(&output), expected);
    assert_quicktime_chapter_offset(&output, 1_250, 400);
    assert!(lane.residual_workspace_dirs().is_empty());
}

#[tokio::test]
async fn encoding_rejects_a_non_anchor_source_replaced_after_planning() {
    let lane = MediaLane::with_fixtures(&[0.2, 0.2]);
    let request = lane.execution_request(ProcessingSession::new(), None);
    write_sine_wav(&lane.inputs[1], 0.4, 880.0);
    let replacement = fs::read(&lane.inputs[1]).expect("read replacement source");
    let error = execute_audio_engine(request)
        .await
        .expect_err("reject stale source identity");
    assert!(
        error.to_string().contains("changed since inspection"),
        "{error}"
    );
    assert!(!lane.output_path().exists());
    assert!(lane.residual_workspace_dirs().is_empty());
    assert_eq!(
        fs::read(&lane.inputs[1]).expect("source retained"),
        replacement
    );
}
