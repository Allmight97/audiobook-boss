//! Real-media proofs for audio execution and metadata writes on files ABB
//! produced. Fixtures are synthesized at test time (WAV in Rust, other formats
//! through the FFmpeg CLI); no media is committed. Route-by-route library
//! facts live in `integration_media_route_parity_tests.rs`.

use super::integration_faac_execution_tests::faac_encoder_settings;
use super::integration_session_desk::Desk;
use super::media_fixtures::{
    audio_packet_bytes, duration_ms, ffprobe_tag_ci, minimal_jpg_bytes, oversized_jpeg,
    picture_codecs, preflight_payload, run_planned_title, write_two_picture_fixture,
};
use abb_engine::audio::{
    execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest, BitrateMode,
    ChannelConfig, EncoderSettings, EncoderType, SampleRateConfig,
};
use abb_engine::processing::{AudioHandling, OutputConfig, ProcessingContext, ProcessingSession};
use abb_engine::processing::{CancellationChecker, JobRegistry};
use abb_engine::session::SessionIntent;
use abb_engine::{
    extract_passthrough_metadata, finalize_artifact_metadata, read_audio_cover_thumbnail,
    read_metadata, save_metadata_intent, AlbumSortPatchOp, AppError, AudiobookMetadata,
    CoverArtPassthroughPolicy, MetadataIntentPatch, PassthroughSource, PatchOp, ShutdownOutcome,
};
use image::{GenericImageView, ImageFormat};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tempfile::TempDir;

/// External-reader tag truth: ffprobe's view of the container's format tags.
/// Requires an `ffprobe` binary on PATH (or via `ABB_FFPROBE`); the media lane
/// environments provide one (`scripts/AGENTS.md`).
pub(crate) fn ffprobe_format_tags(path: &Path) -> serde_json::Map<String, serde_json::Value> {
    let binary = std::env::var("ABB_FFPROBE").unwrap_or_else(|_| "ffprobe".to_string());
    let output = Command::new(&binary)
        .args(["-v", "quiet", "-print_format", "json", "-show_format"])
        .arg(path)
        .output()
        .unwrap_or_else(|error| {
            panic!("ffprobe (FFmpeg CLI) must be on PATH or set via ABB_FFPROBE for external-reader tag proof; spawning `{binary}` failed: {error}")
        });
    assert!(
        output.status.success(),
        "ffprobe failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse ffprobe JSON");
    parsed
        .get("format")
        .and_then(|format| format.get("tags"))
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn assert_ffprobe_tag(
    tags: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    expected: &str,
) {
    let actual = ffprobe_tag_ci(tags, key);
    assert_eq!(
        actual.as_deref(),
        Some(expected),
        "ffprobe tag `{key}` should be `{expected}`; full tags: {tags:?}"
    );
}

pub(crate) const SAMPLE_RATE: u32 = 44_100;

#[tokio::test]
async fn native_aac_reprocessing_keeps_the_original_playable_sample_count() {
    for settings in [
        native_encoder_settings(),
        EncoderSettings {
            native_aac_speed: 4,
            faac_profile: abb_engine::audio::FaacProfile::Auto,
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

/// Stream `duration_ts` of the first audio stream: playable length including
/// any padding the encoder or muxer declared.
pub(crate) fn audio_duration_ts(path: &Path) -> u64 {
    audio_stream_timing(path).0
}

fn audio_stream_timing(path: &Path) -> (u64, String) {
    let binary = std::env::var("ABB_FFPROBE").unwrap_or_else(|_| "ffprobe".to_string());
    let probe = Command::new(binary)
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=duration_ts,time_base",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .expect("ffprobe stream timing");
    assert!(
        probe.status.success(),
        "ffprobe failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&probe.stderr)
    );
    let facts: serde_json::Value = serde_json::from_slice(&probe.stdout).expect("fixture value");
    let stream = &facts["streams"][0];
    (
        stream["duration_ts"].as_u64().expect("audio duration_ts"),
        stream["time_base"]
            .as_str()
            .expect("audio time_base")
            .to_string(),
    )
}

const MAX_ROUTE_DIFFERENCE: f32 = 1e-3;
const AAC_FRAME_SAMPLES: u64 = 1_024;
const PREVIEW_REFUSAL: &str = "Encoding previews are available with M4B or Opus output";

fn stereo_native_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        channels: ChannelConfig::Stereo,
        bitrate_kbps: 320,
        ..native_encoder_settings()
    }
}

fn decode_to_packed_float_wav(source: &Path, wav: &Path) {
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let decoded = Command::new(binary)
        .args(["-v", "error", "-xerror", "-i"])
        .arg(source)
        .args(["-map", "0:a:0", "-c:a", "pcm_f32le", "-y"])
        .arg(wav)
        .output()
        .expect("decode source to packed float WAV");
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
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

pub(crate) fn decode_pcm_f32(path: &Path) -> Vec<f32> {
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let decoded = Command::new(binary)
        .args(["-v", "error", "-xerror", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-f",
            "f32le",
            "-c:a",
            "pcm_f32le",
            "pipe:1",
        ])
        .output()
        .expect("decode artifact PCM");
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    decoded
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| f32::from_le_bytes(*bytes))
        .collect()
}

#[tokio::test]
async fn truncated_audio_fails_without_publishing_a_shortened_book() {
    let lane = MediaLane::with_fixtures(&[2.0]);
    let complete = lane.process(None).await;
    let broken = lane.tmp.path().join("truncated.m4b");
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let remux = Command::new(binary)
        .args(["-v", "error", "-i"])
        .arg(complete)
        .args(["-map", "0:a:0", "-c:a", "copy", "-movflags", "+faststart"])
        .arg(&broken)
        .output()
        .expect("fixture value");
    assert!(remux.status.success());
    let binary = std::env::var("ABB_FFPROBE").unwrap_or_else(|_| "ffprobe".to_string());
    let probe = Command::new(binary)
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_packets",
            "-of",
            "json",
        ])
        .arg(&broken)
        .output()
        .expect("fixture value");
    assert!(probe.status.success());
    let facts: serde_json::Value = serde_json::from_slice(&probe.stdout).expect("fixture value");
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
    let broken_lane = MediaLane::for_inputs(vec![broken]);
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

#[tokio::test]
async fn repeated_mp4_contributors_remain_visible_and_survive_unrelated_edits() {
    let lane = MediaLane::with_fixtures(&[0.3]);
    let output = lane.process(None).await;
    let mut tag = mp4ameta::Tag::read_from_path(&output).expect("fixture value");
    tag.set_artists(["First Author".to_string(), "Second Author".to_string()]);
    tag.set_album_artists([
        "First Album Author".to_string(),
        "Second Album Author".to_string(),
    ]);
    tag.set_composers(["First Narrator".to_string(), "Second Narrator".to_string()]);
    tag.write_to_path(&output).expect("fixture value");

    let metadata = read_metadata(&output).expect("fixture value");
    assert_eq!(
        metadata.artist.as_deref(),
        Some("First Author;Second Author")
    );
    assert_eq!(
        metadata.composer.as_deref(),
        Some("First Narrator;Second Narrator")
    );
    let imported = get_file_list_info(&[&output], AacDecoder::Auto).expect("fixture value");
    assert_eq!(imported.files[0].tag_artist, metadata.artist);

    save_metadata_intent(
        &output,
        &MetadataIntentPatch {
            title: Some(PatchOp::Set("Retitled".into())),
            ..Default::default()
        },
    )
    .expect("fixture value");
    let mut tag = mp4ameta::Tag::read_from_path(&output).expect("fixture value");
    assert_eq!(
        tag.artists().collect::<Vec<_>>(),
        ["First Author", "Second Author"]
    );
    assert_eq!(
        tag.album_artists().collect::<Vec<_>>(),
        ["First Album Author", "Second Album Author"]
    );
    assert_eq!(
        tag.composers().collect::<Vec<_>>(),
        ["First Narrator", "Second Narrator"]
    );

    tag.remove_artists();
    tag.write_to_path(&output).expect("fixture value");
    assert_eq!(
        read_metadata(&output)
            .expect("fixture value")
            .artist
            .as_deref(),
        Some("First Album Author;Second Album Author")
    );

    let reprocess = MediaLane::for_inputs(vec![output]);
    let finalized = reprocess.process(Some(metadata)).await;
    let reread = read_metadata(&finalized).expect("fixture value");
    assert_eq!(reread.artist.as_deref(), Some("First Author;Second Author"));
    assert_eq!(
        reread.composer.as_deref(),
        Some("First Narrator;Second Narrator")
    );
}

pub(crate) fn write_sine_wav(path: &Path, seconds: f64, freq_hz: f64) {
    let total_samples = (seconds * f64::from(SAMPLE_RATE)) as u32;
    let data_len = total_samples * 2;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
    bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for n in 0..total_samples {
        let t = f64::from(n) / f64::from(SAMPLE_RATE);
        let sample =
            (0.3 * (2.0 * std::f64::consts::PI * freq_hz * t).sin() * f64::from(i16::MAX)) as i16;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(path, bytes).expect("write WAV fixture");
}

pub(crate) fn native_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: ChannelConfig::Mono,
        native_aac_speed: 0,
        faac_profile: abb_engine::audio::FaacProfile::Auto,
    }
}

pub(crate) struct MediaLane {
    pub(crate) tmp: TempDir,
    pub(crate) inputs: Vec<PathBuf>,
    encoder_settings: EncoderSettings,
    sample_rate: SampleRateConfig,
    aac_decoder: AacDecoder,
}

impl MediaLane {
    pub(crate) fn with_fixtures(durations_secs: &[f64]) -> Self {
        let tmp = TempDir::new().expect("create media lane tempdir");
        let inputs = durations_secs
            .iter()
            .enumerate()
            .map(|(index, seconds)| {
                let path = tmp.path().join(format!("fixture-{index}.wav"));
                write_sine_wav(&path, *seconds, 440.0 + 110.0 * index as f64);
                path
            })
            .collect();
        Self {
            tmp,
            inputs,
            encoder_settings: native_encoder_settings(),
            sample_rate: SampleRateConfig::Auto,
            aac_decoder: AacDecoder::Auto,
        }
    }

    pub(crate) fn for_inputs(inputs: Vec<PathBuf>) -> Self {
        let tmp = TempDir::new().expect("create media lane tempdir");
        Self {
            tmp,
            inputs,
            encoder_settings: native_encoder_settings(),
            sample_rate: SampleRateConfig::Auto,
            aac_decoder: AacDecoder::Auto,
        }
    }

    pub(crate) fn with_aac_decoder(mut self, aac_decoder: AacDecoder) -> Self {
        self.aac_decoder = aac_decoder;
        self
    }

    pub(crate) fn with_encoder(mut self, encoder_settings: EncoderSettings) -> Self {
        self.encoder_settings = encoder_settings;
        self
    }

    pub(crate) fn with_sample_rate(mut self, sample_rate: SampleRateConfig) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    pub(crate) fn output_path(&self) -> PathBuf {
        self.tmp.path().join("out").join("lane-output.m4b")
    }

    pub(crate) fn workspace_root(&self) -> PathBuf {
        self.tmp.path().join("workspace")
    }

    pub(crate) fn handling(&self) -> AudioHandling {
        AudioHandling::Encode {
            settings: self.encoder_settings.clone(),
        }
    }

    pub(crate) fn context(&self, session: ProcessingSession) -> ProcessingContext {
        let output_dir = self.output_path();
        fs::create_dir_all(output_dir.parent().expect("output parent")).expect("create output dir");
        let mut context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(session),
            self.sample_rate,
            OutputConfig::new(self.output_path()),
            self.workspace_root(),
        );
        context.aac_decoder = self.aac_decoder;
        context
    }

    pub(crate) fn execution_request(
        &self,
        session: ProcessingSession,
        metadata: Option<AudiobookMetadata>,
    ) -> AudioExecutionRequest {
        let file_info =
            get_file_list_info(&self.inputs, self.aac_decoder).expect("probe WAV fixtures");
        assert_eq!(
            file_info.invalid_count, 0,
            "generated fixtures must probe as valid audio"
        );
        AudioExecutionRequest::new(
            self.context(session),
            file_info,
            metadata,
            CoverArtPassthroughPolicy::Preserve,
            self.handling(),
        )
    }

    pub(crate) async fn process(&self, metadata: Option<AudiobookMetadata>) -> PathBuf {
        execute_audio_engine(self.execution_request(ProcessingSession::new(), metadata))
            .await
            .expect("native processing succeeds");
        self.output_path()
    }

    pub(crate) fn residual_workspace_dirs(&self) -> Vec<PathBuf> {
        let root = self.workspace_root();
        if !root.exists() {
            return Vec::new();
        }
        fs::read_dir(root)
            .expect("read workspace root")
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect()
    }
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
async fn metadata_saved_during_processing_rereads_from_output_artifact() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let mut metadata = AudiobookMetadata::new();
    metadata.title = Some("Media Lane Title".to_string());
    metadata.artist = Some("Lane Narrator".to_string());
    metadata.album = Some("Lane Album".to_string());
    metadata.genre = Some("Audiobook".to_string());
    // The MP4 muxer drops these keys; they reach the artifact only through the
    // finalized tag write.
    metadata.series = Some("Lane Saga".to_string());
    metadata.series_part = Some("2".to_string());
    metadata.subseries = Some("Lane Arc".to_string());
    metadata.subseries_part = Some("1".to_string());

    execute_audio_engine(lane.execution_request(ProcessingSession::new(), Some(metadata)))
        .await
        .expect("processing with metadata succeeds");

    let reread = read_metadata(lane.output_path()).expect("re-read tags from committed artifact");
    assert_eq!(reread.title.as_deref(), Some("Media Lane Title"));
    assert_eq!(reread.artist.as_deref(), Some("Lane Narrator"));
    assert_eq!(reread.album.as_deref(), Some("Lane Album"));
    assert_eq!(reread.genre.as_deref(), Some("Audiobook"));
    assert_eq!(reread.series.as_deref(), Some("Lane Saga"));
    assert_eq!(reread.series_part.as_deref(), Some("2"));
    assert_eq!(reread.subseries.as_deref(), Some("Lane Arc"));
    assert_eq!(reread.subseries_part.as_deref(), Some("1"));
}

#[test]
fn failed_remux_save_leaves_source_bytes_and_no_temporary_residue() {
    let dir = TempDir::new().expect("temp dir");
    let source = dir.path().join("book.wav");
    write_sine_wav(&source, 0.5, 440.0);
    let before = fs::read(&source).expect("read source");

    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Retitled".to_string())),
        cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
        ..Default::default()
    };
    save_metadata_intent(&source, &patch).expect_err("WAV cannot carry explicit cover art");

    assert_eq!(fs::read(&source).expect("reread source"), before);
    let residue: Vec<_> = fs::read_dir(dir.path())
        .expect("list dir")
        .map(|entry| entry.expect("entry").file_name())
        .filter(|name| name != "book.wav")
        .collect();
    assert!(residue.is_empty(), "unexpected residue: {residue:?}");
}

#[tokio::test]
async fn a_pre_cancelled_run_publishes_nothing_and_leaves_the_source_alone() {
    let tmp = TempDir::new().expect("cancel fixture tempdir");
    let wav = tmp.path().join("source.wav");
    write_sine_wav(&wav, 1.0, 440.0);
    let mp3 = tmp.path().join("source.mp3");
    write_sine_mp3(&mp3, 1.0, 440.0);
    let registry = JobRegistry::new(1);
    for (source, name, handling) in [
        (
            wav,
            "cancelled.m4b",
            AudioHandling::Encode {
                settings: native_encoder_settings(),
            },
        ),
        (mp3, "cancelled.mp3", AudioHandling::Preserve),
    ] {
        let source_bytes = fs::read(&source).expect("read cancellation source");
        let destination = tmp.path().join(name);
        let workspace = tmp.path().join(format!("workspace-{name}"));
        let (job_id, _permit) = registry.register_job().await.expect("register job");
        let checker =
            CancellationChecker::new(Some(Arc::new(std::sync::atomic::AtomicBool::new(true))));
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::with_cancellation(job_id.0, checker)),
            SampleRateConfig::Auto,
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

#[tokio::test]
async fn artifact_fields_survive_normal_saves_and_clear_only_by_explicit_intent() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let mut metadata = AudiobookMetadata::new();
    metadata.title = Some("Artifact Book".to_string());
    metadata.album_sort = Some("Lane Series 01 - Artifact Book".to_string());
    metadata.comment = Some("Provenance note".to_string());
    metadata.track = Some((7, Some(42)));
    metadata.disk = Some((1, Some(2)));

    execute_audio_engine(lane.execution_request(ProcessingSession::new(), Some(metadata)))
        .await
        .expect("processing with artifact metadata succeeds");
    let output = lane.output_path();

    let written = read_metadata(&output).expect("artifact metadata written");
    assert_eq!(
        written.album_sort.as_deref(),
        Some("Lane Series 01 - Artifact Book")
    );
    assert_eq!(written.comment.as_deref(), Some("Provenance note"));
    assert_eq!(written.track, Some((7, Some(42))));
    assert_eq!(written.disk, Some((1, Some(2))));

    let title_only = MetadataIntentPatch {
        title: Some(PatchOp::Set("Renamed Artifact Book".to_string())),
        ..Default::default()
    };
    save_metadata_intent(&output, &title_only).expect("title-only save");
    let preserved = read_metadata(&output).expect("re-read after title-only save");
    assert_eq!(preserved.title.as_deref(), Some("Renamed Artifact Book"));
    assert_eq!(
        preserved.album_sort.as_deref(),
        Some("Lane Series 01 - Artifact Book"),
        "normal saves must preserve album_sort"
    );
    assert_eq!(preserved.comment.as_deref(), Some("Provenance note"));
    assert_eq!(preserved.track, Some((7, Some(42))));
    assert_eq!(preserved.disk, Some((1, Some(2))));

    let clear_artifacts = MetadataIntentPatch {
        album_sort: Some(AlbumSortPatchOp::Clear),
        comment: Some(PatchOp::Clear),
        track: Some(PatchOp::Clear),
        disk: Some(PatchOp::Clear),
        ..Default::default()
    };
    save_metadata_intent(&output, &clear_artifacts).expect("artifact clear save");
    let cleared = read_metadata(&output).expect("re-read after artifact clear");
    assert_eq!(cleared.album_sort, None, "album_sort cleared");
    assert_eq!(cleared.comment, None, "comment cleared");
    assert_eq!(cleared.track, None, "track cleared");
    assert_eq!(cleared.disk, None, "disk cleared");
    assert_eq!(
        cleared.title.as_deref(),
        Some("Renamed Artifact Book"),
        "primary fields untouched by artifact clears"
    );
}

#[tokio::test]
async fn metadata_save_writes_external_ffprobe_visible_mp4_tags() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let output = lane.process(None).await;

    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("External Probe Title".to_string())),
        artist: Some(PatchOp::Set("External Probe Author".to_string())),
        album: Some(PatchOp::Set("External Probe Album".to_string())),
        composer: Some(PatchOp::Set("External Probe Composer".to_string())),
        genre: Some(PatchOp::Set("Audiobook".to_string())),
        date: Some(PatchOp::Set("2024-05-06".to_string())),
        description: Some(PatchOp::Set("External reader proof".to_string())),
        series: Some(PatchOp::Set("Probe Series".to_string())),
        series_part: Some(PatchOp::Set("2".to_string())),
        subseries: Some(PatchOp::Set("Probe Subseries".to_string())),
        subseries_part: Some(PatchOp::Set("7".to_string())),
        comment: Some(PatchOp::Set("Probe Comment".to_string())),
        track: Some(PatchOp::Set((3, Some(9)))),
        disk: Some(PatchOp::Set((1, Some(2)))),
        ..Default::default()
    };
    #[cfg(unix)]
    let inode_before = std::os::unix::fs::MetadataExt::ino(&fs::metadata(&output).expect("stat"));
    save_metadata_intent(&output, &patch).expect("metadata save through mp4ameta path");
    #[cfg(unix)]
    assert_ne!(
        std::os::unix::fs::MetadataExt::ino(&fs::metadata(&output).expect("stat")),
        inode_before
    );

    let tags = ffprobe_format_tags(&output);
    assert_ffprobe_tag(&tags, "title", "External Probe Title");
    assert_ffprobe_tag(&tags, "artist", "External Probe Author");
    assert_ffprobe_tag(&tags, "album_artist", "External Probe Author");
    assert_ffprobe_tag(&tags, "album", "External Probe Album");
    assert_ffprobe_tag(&tags, "composer", "External Probe Composer");
    assert_ffprobe_tag(&tags, "genre", "Audiobook");
    assert_ffprobe_tag(&tags, "date", "2024-05");
    assert_ffprobe_tag(&tags, "description", "External reader proof");
    assert_ffprobe_tag(&tags, "comment", "Probe Comment");
    // mp4ameta writes iTunes freeform series atoms; ffprobe exposes those
    // atoms by freeform name, not as an exhaustive atom inventory.
    assert_ffprobe_tag(&tags, "SERIES", "Probe Series; Probe Subseries");
    assert_ffprobe_tag(&tags, "SERIES-PART", "2; 7");
    assert_ffprobe_tag(&tags, "track", "3/9");
    assert_ffprobe_tag(&tags, "disc", "1/2");
}

/// The iTunes freeform names are MP4 atoms; in an ID3 file they would be
/// stray TXXX frames that no reader maps to a series.
#[test]
fn mp3_save_writes_series_without_mp4_freeform_names() {
    let tmp = TempDir::new().expect("MP3 save workspace");
    let mp3 = tmp.path().join("book.mp3");
    write_sine_mp3(&mp3, 0.5, 440.0);
    save_metadata_intent(
        &mp3,
        &MetadataIntentPatch {
            series: Some(PatchOp::Set("Dune".into())),
            series_part: Some(PatchOp::Set("1".into())),
            ..Default::default()
        },
    )
    .expect("MP3 series save");
    let tags = ffprobe_format_tags(&mp3);
    assert_ffprobe_tag(&tags, "series", "Dune");
    assert_ffprobe_tag(&tags, "series-part", "1");
    assert!(
        !tags.keys().any(|key| key.contains("com.apple.iTunes")),
        "{tags:?}"
    );
    assert_eq!(
        read_metadata(&mp3)
            .expect("read saved MP3")
            .series
            .as_deref(),
        Some("Dune")
    );
}

#[test]
fn a_save_without_cover_intent_keeps_every_attached_picture() {
    let dir = TempDir::new().expect("temp dir");
    let both = [ffmpeg_next::codec::Id::MJPEG, ffmpeg_next::codec::Id::PNG];
    let retitle = MetadataIntentPatch {
        title: Some(PatchOp::Set("Retitled".to_string())),
        ..Default::default()
    };
    for extension in ["mp3", "flac", "mka"] {
        let path = write_two_picture_fixture(dir.path(), extension);
        assert_eq!(picture_codecs(&path), both, "{extension} fixture");

        save_metadata_intent(&path, &retitle).expect("title-only save");

        assert_eq!(picture_codecs(&path), both, "{extension} save");
        let reread = read_metadata(&path).expect("reread");
        assert_eq!(reread.title.as_deref(), Some("Retitled"), "{extension}");
    }

    let m4b = write_two_picture_fixture(dir.path(), "m4b");
    assert_eq!(picture_codecs(&m4b), both, "m4b fixture");
    let text_only = AudiobookMetadata {
        title: Some("Retitled".to_string()),
        ..Default::default()
    };
    finalize_artifact_metadata(&m4b, Some(&text_only), None).expect("m4b remux finalize");
    assert_eq!(picture_codecs(&m4b), both, "m4b remux");

    let opus = write_two_picture_fixture(dir.path(), "opus");
    let before = fs::read(&opus).expect("read opus");
    // The Ogg muxer refuses picture streams, so this Save fails as it did
    // before; the original must stay intact.
    save_metadata_intent(&opus, &retitle).expect_err("Ogg cannot carry a picture stream");
    assert_eq!(fs::read(&opus).expect("reread opus"), before);
}

pub(crate) fn chapters_of(path: &Path) -> Vec<(Option<String>, i64, i64)> {
    let passthrough = extract_passthrough_metadata(&[PassthroughSource {
        chapters: None,
        path: path.to_path_buf(),
        duration: None,
        is_valid: true,
    }]);
    passthrough
        .chapters
        .into_iter()
        .map(|chapter| (chapter.title, chapter.start_ms, chapter.end_ms))
        .collect()
}

/// Apple Books reads the QuickTime `text` trak, not Nero `chpl`.
/// FFmpeg's chapter API only sees `chpl`, so a green chpl probe does not prove
/// the player-visible track. Walk the `text` handler's `mdhd`.
fn timed_text_chapter_track(path: &Path) -> (f64, u32, u32) {
    let bytes = fs::read(path).expect("read artifact for timed-text atom probe");
    let moov = find_atom(&bytes, 0, bytes.len(), *b"moov").expect("moov atom");
    let mut offset = moov.0;
    let end = moov.1;
    let mut audio_timescale = None;
    let mut text = None;
    while offset + 8 <= end {
        let (trak_start, trak_end, size) = match next_atom(&bytes, offset, end) {
            Some(atom) => atom,
            None => break,
        };
        offset += size;
        if bytes[trak_start + 4..trak_start + 8] != *b"trak" {
            continue;
        }
        let Some(mdia) = find_atom(&bytes, trak_start + 8, trak_end, *b"mdia") else {
            continue;
        };
        let Some(hdlr) = find_atom(&bytes, mdia.0, mdia.1, *b"hdlr") else {
            continue;
        };
        // hdlr payload: version/flags (4) + component type (4) + handler (4)
        let subtype_at = hdlr.0 + 8;
        if subtype_at + 4 > hdlr.1 {
            continue;
        }
        let handler = &bytes[subtype_at..subtype_at + 4];
        let Some(mdhd) = find_atom(&bytes, mdia.0, mdia.1, *b"mdhd") else {
            continue;
        };
        let (timescale, duration) = parse_mdhd(&bytes, mdhd.0, mdhd.1);
        if handler == b"soun" {
            audio_timescale = Some(timescale);
        } else if handler == b"text" {
            assert!(timescale > 0, "text trak timescale is zero");
            text = Some((duration as f64 / f64::from(timescale), timescale));
        }
    }
    let (duration_secs, text_timescale) =
        text.expect("QuickTime timed-text chapter trak not found");
    (
        duration_secs,
        text_timescale,
        audio_timescale.expect("audio trak timescale"),
    )
}

fn next_atom(bytes: &[u8], start: usize, end: usize) -> Option<(usize, usize, usize)> {
    if start + 8 > end {
        return None;
    }
    let mut size = u32::from_be_bytes(bytes[start..start + 4].try_into().ok()?) as usize;
    let mut header = 8;
    if size == 1 {
        if start + 16 > end {
            return None;
        }
        size = u64::from_be_bytes(bytes[start + 8..start + 16].try_into().ok()?) as usize;
        header = 16;
    } else if size == 0 {
        size = end - start;
    }
    if size < header || start + size > end {
        return None;
    }
    Some((start, start + size, size))
}

fn find_atom(bytes: &[u8], start: usize, end: usize, fourcc: [u8; 4]) -> Option<(usize, usize)> {
    let mut offset = start;
    while offset + 8 <= end {
        let (atom_start, atom_end, size) = next_atom(bytes, offset, end)?;
        if bytes[atom_start + 4..atom_start + 8] == fourcc {
            let header =
                if u32::from_be_bytes(bytes[atom_start..atom_start + 4].try_into().ok()?) == 1 {
                    16
                } else {
                    8
                };
            return Some((atom_start + header, atom_end));
        }
        offset += size;
    }
    None
}

fn parse_mdhd(bytes: &[u8], start: usize, end: usize) -> (u32, u64) {
    assert!(start + 4 <= end, "mdhd too small");
    let version = bytes[start];
    if version == 1 {
        assert!(start + 28 <= end, "mdhd v1 too small");
        let timescale = u32::from_be_bytes(
            bytes[start + 20..start + 24]
                .try_into()
                .expect("fixture value"),
        );
        let duration = u64::from_be_bytes(
            bytes[start + 24..start + 32]
                .try_into()
                .expect("fixture value"),
        );
        (timescale, duration)
    } else {
        assert!(start + 20 <= end, "mdhd v0 too small");
        let timescale = u32::from_be_bytes(
            bytes[start + 12..start + 16]
                .try_into()
                .expect("fixture value"),
        );
        let duration = u32::from_be_bytes(
            bytes[start + 16..start + 20]
                .try_into()
                .expect("fixture value"),
        ) as u64;
        (timescale, duration)
    }
}

#[tokio::test]
async fn embedded_cover_thumbnail_is_bounded_jpeg_and_coverless_artifact_returns_none() {
    let covered_lane = MediaLane::with_fixtures(&[1.0]);
    let mut metadata = AudiobookMetadata::new();
    metadata.cover_art = Some(minimal_jpg_bytes());
    let covered_output = covered_lane.process(Some(metadata)).await;

    let thumbnail = read_audio_cover_thumbnail(&covered_output)
        .expect("read embedded cover thumbnail")
        .expect("covered artifact should return a thumbnail");
    let decoded = image::load_from_memory(&thumbnail).expect("thumbnail should decode");
    assert_eq!(
        image::guess_format(&thumbnail).expect("thumbnail format should be detectable"),
        ImageFormat::Jpeg
    );
    let (width, height) = decoded.dimensions();
    assert!(
        width <= 128 && height <= 128,
        "thumbnail dimensions {width}x{height} exceed the 128px bound"
    );

    let coverless_lane = MediaLane::with_fixtures(&[1.0]);
    let coverless_output = coverless_lane.process(None).await;
    assert_eq!(
        read_audio_cover_thumbnail(&coverless_output).expect("read coverless artifact thumbnail"),
        None
    );
}

pub(crate) fn write_sine_mp3(path: &Path, seconds: f64, freq_hz: f64) {
    write_sine_mp3_with(path, seconds, freq_hz, &["-b:a", "64k"]);
}

pub(crate) fn write_sine_mp3_with(path: &Path, seconds: f64, freq_hz: f64, encoder_args: &[&str]) {
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let source = format!("sine=frequency={freq_hz}:sample_rate={SAMPLE_RATE}");
    let duration = seconds.to_string();
    let output = Command::new(&binary)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            &source,
            "-t",
            &duration,
            "-ac",
            "1",
            "-codec:a",
            "libmp3lame",
        ])
        .args(encoder_args)
        .arg("-y")
        .arg(path)
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "ffmpeg must be on PATH or set via ABB_FFMPEG for MP3 fixture synthesis; spawning `{binary}` failed: {error}"
            )
        });
    assert!(
        output.status.success(),
        "ffmpeg failed to synthesize {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_chaptered_sine_mp3(path: &Path, seconds: f64, freq_hz: f64) {
    let audio = path.with_file_name("chapter-source.mp3");
    write_sine_mp3(&audio, seconds, freq_hz);
    mux_chapters(&audio, &[("Opening", 0, 450), ("Closing", 450, 900)], path);
}

pub(crate) fn mux_chapters(audio: &Path, chapters: &[(&str, i64, i64)], path: &Path) {
    let metadata = path.with_extension("ffmetadata");
    let mut text = ";FFMETADATA1\n".to_string();
    for (title, start, end) in chapters {
        text.push_str(&format!(
            "[CHAPTER]\nTIMEBASE=1/1000\nSTART={start}\nEND={end}\ntitle={title}\n"
        ));
    }
    fs::write(&metadata, text).expect("write chapter metadata");
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let mut command = Command::new(&binary);
    command
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(audio)
        .args(["-f", "ffmetadata", "-i"])
        .arg(&metadata)
        .args([
            "-map",
            "0",
            "-map_metadata",
            "1",
            "-map_chapters",
            "1",
            "-c",
            "copy",
        ]);
    if path.extension().is_some_and(|ext| ext == "mp3") {
        command.args(["-id3v2_version", "3"]);
    }
    let output = command
        .arg("-y")
        .arg(path)
        .output()
        .unwrap_or_else(|error| {
            panic!("ffmpeg must be on PATH or set via ABB_FFMPEG for chapter fixtures; spawning `{binary}` failed: {error}")
        });
    assert!(
        output.status.success(),
        "ffmpeg failed to add chapters to {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn id3_language_comments_round_trip_through_unrelated_set_and_clear_intent() {
    let tmp = TempDir::new().expect("fixture value");
    let path = tmp.path().join("comments.mp3");
    write_sine_mp3(&path, 0.1, 440.0);
    let audio = fs::read(&path).expect("fixture value");
    let offset = if audio.starts_with(b"ID3") {
        10 + audio[6..10]
            .iter()
            .fold(0usize, |size, byte| (size << 7) | usize::from(*byte))
    } else {
        0
    };
    let mut frames = Vec::new();
    for (language, descriptor, text) in [
        ("eng", "", "English comment"),
        ("fra", "comment", "French comment"),
        ("eng", "iTunNORM", "technical normalization"),
    ] {
        let mut payload = vec![0];
        payload.extend_from_slice(language.as_bytes());
        payload.extend_from_slice(descriptor.as_bytes());
        payload.push(0);
        payload.extend_from_slice(text.as_bytes());
        frames.extend_from_slice(b"COMM");
        frames.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frames.extend_from_slice(&[0, 0]);
        frames.extend_from_slice(&payload);
    }
    let size = frames.len();
    let mut tagged = b"ID3\x03\x00\x00".to_vec();
    for shift in [21, 14, 7, 0] {
        tagged.push(((size >> shift) & 0x7f) as u8);
    }
    tagged.extend(frames);
    tagged.extend_from_slice(&audio[offset..]);
    fs::write(&path, tagged).expect("fixture value");
    assert_eq!(
        read_metadata(&path)
            .expect("fixture value")
            .comment
            .as_deref(),
        Some("English comment")
    );
    save_metadata_intent(
        &path,
        &MetadataIntentPatch {
            genre: Some(PatchOp::Set("Audiobook".into())),
            ..Default::default()
        },
    )
    .expect("fixture value");
    {
        let input = ffmpeg_next::format::input(&path).expect("fixture value");
        let tags = input.metadata();
        assert_eq!(tags.get("comment-eng"), Some("English comment"));
        assert_eq!(tags.get("comment-comment-fra"), Some("French comment"));
    }
    save_metadata_intent(
        &path,
        &MetadataIntentPatch {
            comment: Some(PatchOp::Set("New comment".into())),
            ..Default::default()
        },
    )
    .expect("fixture value");
    assert_eq!(
        read_metadata(&path)
            .expect("fixture value")
            .comment
            .as_deref(),
        Some("New comment")
    );
    save_metadata_intent(
        &path,
        &MetadataIntentPatch {
            comment: Some(PatchOp::Clear),
            ..Default::default()
        },
    )
    .expect("fixture value");
    assert_eq!(read_metadata(&path).expect("fixture value").comment, None);
    let input = ffmpeg_next::format::input(&path).expect("fixture value");
    assert_eq!(
        input.metadata().get("comment-iTunNORM-eng"),
        Some("technical normalization")
    );
}

#[tokio::test]
async fn preserve_copies_supported_m4b_and_mp3_without_mutating_source_bytes() {
    let m4b_lane = MediaLane::with_fixtures(&[1.0]);
    let m4b_source = m4b_lane.process(None).await;
    let mp3_tmp = TempDir::new().expect("mp3 preserve fixture tempdir");
    let mp3_source = mp3_tmp.path().join("source.mp3");
    write_sine_mp3(&mp3_source, 1.0, 440.0);

    for (source, extension) in [(m4b_source, "m4b"), (mp3_source, "mp3")] {
        let lane = MediaLane::for_inputs(vec![source.clone()]);
        let destination = lane.tmp.path().join(format!("preserved.{extension}"));
        let source_bytes = fs::read(&source).expect("read preserve source");
        let info = get_file_list_info(std::slice::from_ref(&source), AacDecoder::Auto)
            .expect("probe source");
        assert!(
            info.files[0]
                .preservation
                .expect("preservation facts")
                .can_preserve
        );
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::new()),
            SampleRateConfig::Auto,
            OutputConfig::new(&destination),
            lane.workspace_root(),
        );
        execute_audio_engine(AudioExecutionRequest::new(
            context,
            info,
            None,
            CoverArtPassthroughPolicy::Preserve,
            abb_engine::processing::AudioHandling::Preserve,
        ))
        .await
        .expect("preserve execution succeeds");
        assert_eq!(
            fs::read(&destination).expect("read preserved output"),
            source_bytes
        );
        assert_eq!(fs::read(&source).expect("re-read source"), source_bytes);
        assert!(lane.residual_workspace_dirs().is_empty());
    }
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "Sequential source/output readbacks prove preservation of audio and cover together"
)]
async fn preserve_applies_metadata_and_cover_without_touching_source_audio() {
    let source_lane = MediaLane::with_fixtures(&[0.8, 0.7]).with_encoder(EncoderSettings {
        bitrate_kbps: 128,
        ..native_encoder_settings()
    });
    let source = source_lane.process(None).await;
    let source_bytes = fs::read(&source).expect("read chaptered source");
    let source_pcm = decode_pcm_f32(&source);
    let source_chapters = chapters_of(&source);
    let lane = MediaLane::for_inputs(vec![source.clone()]);
    let destination = lane.tmp.path().join("retagged.m4b");
    let metadata = MetadataIntentPatch {
        title: Some(PatchOp::Set("Preserved title".into())),
        cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
        ..Default::default()
    };
    let info =
        get_file_list_info(std::slice::from_ref(&source), AacDecoder::Auto).expect("probe source");
    assert!(info.files[0].bitrate.expect("fixture value") > 72_000);
    let plan = abb_engine::audio::resolve_title_audio(
        &title_audio_request(
            abb_engine::audio::AudioIntent::Preserve,
            native_encoder_settings(),
        ),
        &info,
        false,
    )
    .expect("explicit Keep overrides the size-reduction recommendation");
    assert_eq!(plan.handling, AudioHandling::Preserve);
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        SampleRateConfig::Auto,
        OutputConfig::new(&destination),
        lane.workspace_root(),
    );
    execute_audio_engine(
        AudioExecutionRequest::new(
            context,
            info,
            None,
            CoverArtPassthroughPolicy::Preserve,
            plan.handling,
        )
        .with_metadata_intent(Some(metadata)),
    )
    .await
    .expect("preserve retagging succeeds");
    let reread = read_metadata(&destination).expect("read preserved metadata");
    assert_eq!(reread.title.as_deref(), Some("Preserved title"));
    assert_eq!(reread.cover_art, Some(minimal_jpg_bytes()));
    assert_eq!(chapters_of(&destination), source_chapters);
    assert_eq!(decode_pcm_f32(&destination), source_pcm);
    assert_eq!(
        audio_packet_bytes(&destination),
        audio_packet_bytes(&source)
    );
    assert_eq!(
        fs::read(&source).expect("source remains unchanged"),
        source_bytes
    );

    let mp3_tmp = TempDir::new().expect("mp3 metadata fixture tempdir");
    let mp3_source = mp3_tmp.path().join("source.mp3");
    write_chaptered_sine_mp3(&mp3_source, 0.9, 523.0);
    let mp3_source_bytes = fs::read(&mp3_source).expect("read MP3 source");
    let mp3_pcm = decode_pcm_f32(&mp3_source);
    let mp3_source_chapters = chapters_of(&mp3_source);
    assert_eq!(
        mp3_source_chapters,
        vec![
            (Some("Opening".into()), 0, 450),
            (Some("Closing".into()), 450, 900),
        ],
        "fixture must expose its independent FFmpeg-authored MP3 chapters"
    );
    let mp3_lane = MediaLane::for_inputs(vec![mp3_source.clone()]);
    let mp3_destination = mp3_lane.tmp.path().join("retagged.mp3");
    let mp3_info =
        get_file_list_info(std::slice::from_ref(&mp3_source), AacDecoder::Auto).expect("probe MP3");
    let mp3_context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        SampleRateConfig::Auto,
        OutputConfig::new(&mp3_destination),
        mp3_lane.workspace_root(),
    );
    execute_audio_engine(
        AudioExecutionRequest::new(
            mp3_context,
            mp3_info,
            None,
            CoverArtPassthroughPolicy::Preserve,
            abb_engine::processing::AudioHandling::Preserve,
        )
        .with_metadata_intent(Some(MetadataIntentPatch {
            title: Some(PatchOp::Set("Preserved MP3 title".into())),
            cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
            ..Default::default()
        })),
    )
    .await
    .expect("preserve MP3 retagging succeeds");
    assert_eq!(
        read_metadata(&mp3_destination)
            .expect("read preserved MP3 metadata")
            .title
            .as_deref(),
        Some("Preserved MP3 title")
    );
    assert_eq!(
        read_metadata(&mp3_destination)
            .expect("fixture value")
            .cover_art,
        Some(minimal_jpg_bytes())
    );
    assert_eq!(chapters_of(&mp3_destination), mp3_source_chapters);
    assert_eq!(decode_pcm_f32(&mp3_destination), mp3_pcm);
    assert_eq!(
        fs::read(&mp3_source).expect("MP3 source remains unchanged"),
        mp3_source_bytes
    );
}

#[tokio::test]
async fn mixed_preservation_preflight_applies_encoder_constraints_only_to_encoded_books() {
    use abb_engine::audio::AudioIntent::{Encode, Preserve};
    use abb_engine::processing::ProcessPayload;
    let source_lane =
        MediaLane::with_fixtures(&[0.5]).with_sample_rate(SampleRateConfig::Explicit(22_050));
    let source = source_lane.process(None).await;
    let larger = MediaLane::with_fixtures(&[0.5, 0.6]);
    let tmp = TempDir::new().expect("fixture value");
    let output = tmp.path().join("out");
    fs::create_dir(&output).expect("fixture value");
    let mut paths = Vec::new();
    for name in ["prey1.m4b", "prey2.m4a", "prey3.m4b"] {
        let path = tmp.path().join(name);
        fs::copy(&source, &path).expect("fixture value");
        paths.push(path.to_string_lossy().to_string());
    }
    paths.extend(
        larger
            .inputs
            .iter()
            .map(|path| path.to_string_lossy().to_string()),
    );
    let payload = ProcessPayload {
        title_sources: None,
        input_files: paths.clone(),
        input_ids: None,
        chapter_plans: None,
        output_dir: output.to_string_lossy().to_string(),
        audio_requests: [Preserve, Preserve, Preserve, Encode, Encode]
            .map(|intent| title_audio_request(intent, faac_encoder_settings()))
            .to_vec(),
        output_naming: None,
        collision_policy: None,
        preflight_signature: None,
        supplemental_assets_by_input_id: None,
        aac_decoder: Some(AacDecoder::Auto),
    };
    let metadata: std::collections::HashMap<_, _> = paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            (
                path.clone(),
                MetadataIntentPatch {
                    title: Some(PatchOp::Set(format!("Library book {i}"))),
                    ..Default::default()
                },
            )
        })
        .collect();
    let plan =
        preflight_payload(payload.clone(), Some(metadata.clone()), false).expect("fixture value");
    assert_eq!(plan.outputs.len(), 5);
    for (index, extension) in ["m4b", "m4b", "m4b", "m4b", "m4b"].iter().enumerate() {
        assert_eq!(
            std::path::Path::new(&plan.outputs[index].resolved_path)
                .extension()
                .expect("fixture value"),
            *extension
        );
        assert!(plan.outputs[index]
            .resolved_path
            .contains(&format!("Library book {index}")));
    }
    let mut encode_low_rate = payload.clone();
    encode_low_rate.audio_requests[0].intent = abb_engine::audio::AudioIntent::Encode;
    assert!(preflight_payload(encode_low_rate.clone(), Some(metadata.clone()), false).is_ok());
    encode_low_rate.audio_requests[0].sample_rate = SampleRateConfig::Explicit(22_050);
    assert!(preflight_payload(encode_low_rate, Some(metadata.clone()), false).is_err());
    let mut all_preserve = payload.clone();
    all_preserve.input_files.truncate(3);
    all_preserve.audio_requests.truncate(3);
    for request in &mut all_preserve.audio_requests {
        request.sample_rate = SampleRateConfig::Explicit(1); // irrelevant while passing through
    }
    let original_plan = preflight_payload(all_preserve.clone(), Some(metadata.clone()), false)
        .expect("fixture value");
    let refusal = preflight_payload(all_preserve.clone(), Some(metadata.clone()), true)
        .expect_err("a preview never copies audio");
    assert!(refusal.to_string().contains(PREVIEW_REFUSAL));
    let mut changed_mode = all_preserve.clone();
    changed_mode.audio_requests = vec![title_audio_request(Encode, native_encoder_settings()); 3];
    let encode_plan =
        preflight_payload(changed_mode, Some(metadata.clone()), false).expect("fixture value");
    assert_ne!(original_plan.plan_signature, encode_plan.plan_signature);
    assert_eq!(
        fs::read_dir(&output).expect("fixture value").count(),
        0,
        "preflight creates no library folders"
    );
    let mut misaligned = all_preserve;
    misaligned.audio_requests.truncate(1);
    assert!(preflight_payload(misaligned, Some(metadata), false)
        .expect_err("fixture must reject")
        .to_string()
        .contains("align"));
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
        faac_profile: abb_engine::audio::FaacProfile::Auto,
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

pub(crate) fn write_stereo_sine_wav(path: &Path, seconds: f64, left_hz: f64, right_hz: f64) {
    let total_samples = (seconds * f64::from(SAMPLE_RATE)) as u32;
    let data_len = total_samples * 4;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&2u16.to_le_bytes()); // stereo
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 4).to_le_bytes()); // byte rate
    bytes.extend_from_slice(&4u16.to_le_bytes()); // block align
    bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for n in 0..total_samples {
        let t = f64::from(n) / f64::from(SAMPLE_RATE);
        for freq in [left_hz, right_hz] {
            let sample =
                (0.3 * (2.0 * std::f64::consts::PI * freq * t).sin() * f64::from(i16::MAX)) as i16;
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
    }
    fs::write(path, bytes).expect("write stereo WAV fixture");
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

pub(crate) async fn assert_auto_merge_stereo(settings: EncoderSettings) {
    let fixtures = TempDir::new().expect("mixed channel fixtures");
    let mono = fixtures.path().join("mono.wav");
    let stereo = fixtures.path().join("stereo.wav");
    write_sine_wav(&mono, 0.3, 220.0);
    write_stereo_sine_wav(&stereo, 0.6, 440.0, 660.0);
    for inputs in [
        vec![mono.clone(), stereo.clone()],
        vec![stereo.clone(), mono.clone()],
    ] {
        let lane = MediaLane::for_inputs(inputs).with_encoder(EncoderSettings {
            channels: ChannelConfig::Auto,
            ..settings.clone()
        });
        let output = lane.process(None).await;
        let probe =
            get_file_list_info(&[&output], AacDecoder::Auto).expect("re-probe mixed-channel M4B");
        assert_eq!(probe.files[0].channels, Some(2), "Auto preserves stereo");
        let samples = decode_pcm_f32(&output);
        let frames = samples.as_chunks::<2>().0;
        let count = frames.len() as f64;
        let mut energy = [0.0_f64; 3];
        for frame in frames {
            let left = f64::from(frame[0]);
            let right = f64::from(frame[1]);
            energy[0] += left * left;
            energy[1] += right * right;
            energy[2] += (left - right).powi(2);
        }
        for (signal, energy) in ["left", "right", "left minus right"]
            .into_iter()
            .zip(energy)
        {
            let rms = (energy / count).sqrt();
            assert!(
                rms > 0.05,
                "{signal} RMS {rms}: missing audio or collapsed stereo"
            );
        }
    }
}

pub(crate) fn accepted_chapters(path: &Path) -> Vec<(Option<String>, i64, i64)> {
    get_file_list_info(&[path], AacDecoder::Auto)
        .expect("import fixture")
        .files[0]
        .chapter_plan
        .as_ref()
        .expect("accepted chapter plan")
        .chapters
        .iter()
        .map(|chapter| (chapter.title.clone(), chapter.start_ms, chapter.end_ms))
        .collect()
}

#[tokio::test]
async fn joined_title_drops_an_unpreparable_source_cover_but_not_a_chosen_one() {
    use abb_engine::audio::{AudioIntent, TitleAudioRequest};
    let tmp = TempDir::new().expect("cover workspace");
    let cover = tmp.path().join("cover.jpg");
    let cover_bytes = oversized_jpeg();
    fs::write(&cover, &cover_bytes).expect("write oversized cover");
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
    let paths = ["first.mp3", "second.mp3"].map(|name| tmp.path().join(name));
    for (index, path) in paths.iter().enumerate() {
        let mut command = Command::new(&binary);
        command
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!(
                "sine=frequency={}:sample_rate=44100:duration=0.31",
                440 + index * 440
            ));
        if index == 0 {
            command.arg("-i").arg(&cover).args([
                "-map",
                "0:a",
                "-map",
                "1",
                "-c:v",
                "copy",
                "-disposition:v",
                "attached_pic",
                "-id3v2_version",
                "3",
            ]);
        }
        assert!(command
            .args(["-c:a", "libmp3lame", "-b:a", "96k", "-write_xing", "0"])
            .arg(path)
            .status()
            .expect("MP3 fixture encoder")
            .success());
    }
    let payload = |output: &str| -> abb_engine::processing::ProcessPayload {
        let output_dir = tmp.path().join(output);
        fs::create_dir_all(&output_dir).expect("output folder");
        serde_json::from_value(serde_json::json!({
            "inputFiles": [&paths[0]],
            "titleSources": {paths[0].to_str().expect("utf8 path"): paths.iter().map(|path| serde_json::json!({"path": path})).collect::<Vec<_>>()},
            "audioRequests": [TitleAudioRequest {
                output: abb_engine::audio::OutputAudio::Mp3,
                intent: AudioIntent::Preserve,
                sample_rate: SampleRateConfig::Auto,
            }],
            "outputDir": output_dir,
            "aacDecoder": "auto"
        }))
        .expect("joined title request")
    };

    let (output, outcome) = run_planned_title(payload("source"), None, &tmp.path().join("work"))
        .await
        .expect("the title converts without its source cover");
    assert_eq!(read_metadata(&output).expect("read output").cover_art, None);
    assert!(
        outcome.warnings
            == ["The source file's cover could not be used, so this audiobook has no cover."],
        "{:?}",
        outcome.warnings
    );

    let chosen = std::collections::HashMap::from([(
        paths[0].to_string_lossy().into_owned(),
        MetadataIntentPatch {
            cover_art: Some(PatchOp::Set(cover_bytes)),
            ..Default::default()
        },
    )]);
    let error = run_planned_title(payload("chosen"), Some(chosen), &tmp.path().join("work"))
        .await
        .expect_err("a chosen cover is required");
    assert!(error.to_string().contains("Image"), "{error}");
}

#[tokio::test]
async fn import_marks_a_file_with_unrepairable_chapters_and_keeps_the_rest() {
    let desk = Desk::new();
    let tmp = desk.root.path();
    let audio = tmp.join("plain.mp3");
    write_sine_mp3(&audio, 0.9, 440.0);
    let end = duration_ms(&audio);
    let good = tmp.join("good.mp3");
    let bad = tmp.join("bad.mp3");
    mux_chapters(&audio, &[("One", 0, 400), ("Two", 400, end + 30)], &good);
    mux_chapters(
        &audio,
        &[("One", 0, 400), ("Epilogue", end + 500, end + 900)],
        &bad,
    );
    desk.send(SessionIntent::Import {
        paths: [&good, &bad]
            .map(|path| path.to_string_lossy().into_owned())
            .to_vec(),
    })
    .await;
    let files = desk.engine.session_snapshot().titles.expect("titles").files;
    assert_eq!(files.len(), 2, "{files:?}");
    let good = files
        .iter()
        .find(|file| file.path.ends_with("good.mp3"))
        .expect("good file listed");
    assert!(good.is_valid, "{:?}", good.error);
    let bad = files
        .iter()
        .find(|file| file.path.ends_with("bad.mp3"))
        .expect("bad file listed");
    assert!(!bad.is_valid);
    let reason = bad.error.as_deref().expect("refusal reason");
    assert!(
        reason.contains("Chapter 2") && reason.contains("Epilogue") && !reason.contains("CUE"),
        "{reason}"
    );
    assert_eq!(desk.engine.shutdown().await, ShutdownOutcome::Settled);
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
async fn preserved_title_rejects_interior_priming_without_publishing() {
    use abb_engine::processing::AudioHandling;
    let source_lane = MediaLane::with_fixtures(&[0.2]);
    let source = source_lane.process(None).await;
    let tmp = TempDir::new().expect("fixture value");
    let copy = tmp.path().join("second.m4b");
    fs::copy(&source, &copy).expect("fixture value");
    let destination = tmp.path().join("must-not-exist.m4b");
    let workspace = tmp.path().join("workspace");
    let info = get_file_list_info(&[source, copy], AacDecoder::Auto).expect("fixture value");
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        SampleRateConfig::Auto,
        OutputConfig::new(&destination),
        workspace.clone(),
    );
    let error = execute_audio_engine(AudioExecutionRequest::new(
        context,
        info,
        None,
        CoverArtPassthroughPolicy::Preserve,
        AudioHandling::Preserve,
    ))
    .await
    .expect_err("per-source priming cannot be lost at a join");
    assert!(
        error.to_string().contains("Pass-through is unavailable"),
        "{error}"
    );
    assert!(!destination.exists());
    assert!(
        !workspace.exists()
            || fs::read_dir(&workspace)
                .expect("fixture value")
                .next()
                .is_none()
    );
}

#[test]
fn mp3_stack_with_trimmed_boundaries_can_encode_but_cannot_pass_through() {
    use abb_engine::processing::ProcessPayload;
    let tmp = TempDir::new().expect("MP3 stack workspace");
    let output_dir = tmp.path().join("out");
    fs::create_dir(&output_dir).expect("output directory");
    let first = tmp.path().join("part-one.mp3");
    let second = tmp.path().join("part-two.mp3");
    write_sine_mp3(&first, 0.1, 440.0);
    fs::copy(&first, &second).expect("second MP3 source");
    let payload: ProcessPayload = serde_json::from_value(serde_json::json!({
        "inputFiles": [first],
        "titleSources": {first.to_str().expect("fixture value"): [{"path": second}, {"path": first}]},
        "outputDir": output_dir, "aacDecoder": "auto",
        "audioRequests": [title_audio_request(
 abb_engine::audio::AudioIntent::Encode, native_encoder_settings())]
    }))
    .expect("MP3 stack request");
    let plan = preflight_payload(payload.clone(), None, false).expect("encoding accepts MP3 stack");
    assert_eq!(plan.outputs.len(), 1);
    assert!(plan.outputs[0].resolved_path.ends_with(".m4b"));
    let mut preserved = payload;
    preserved.audio_requests[0].output = abb_engine::audio::OutputAudio::Mp3;
    preserved.audio_requests[0].intent = abb_engine::audio::AudioIntent::Auto;
    let error = preflight_payload(preserved, None, false)
        .expect_err("interior MP3 priming requires encoding");
    let message = error.to_string();
    assert!(
        message.contains("priming") || message.contains("shifted"),
        "{message}"
    );
    assert_eq!(
        fs::read_dir(&output_dir)
            .expect("read output directory")
            .count(),
        0,
        "preflight must not publish output"
    );
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "Sequential packet, chapter, cover and duration readbacks prove both Opus containers"
)]
async fn opus_title_plan_writes_chapters_cover_and_truthful_audio_in_both_containers() {
    use abb_engine::audio::{AudioIntent, AudiobookFormat, TitleAudioRequest};
    use abb_engine::processing::ProcessPayload;
    for (format, source_rate, input_rate) in [
        (AudiobookFormat::M4aOpus, 22_050, 24_000),
        (AudiobookFormat::MkaOpus, 44_100, 48_000),
    ] {
        let lane = MediaLane::with_fixtures(&[0.203, 0.307]);
        let channels = if format == AudiobookFormat::MkaOpus {
            write_stereo_sine_wav(&lane.inputs[1], 0.307, 440.0, 660.0);
            2
        } else {
            1
        };
        if source_rate != SAMPLE_RATE {
            for path in &lane.inputs {
                let bytes = fs::read(path).expect("fixture value");
                let mut bytes = bytes;
                bytes[24..28].copy_from_slice(&source_rate.to_le_bytes());
                bytes[28..32].copy_from_slice(&(source_rate * 2).to_le_bytes());
                fs::write(path, bytes).expect("fixture value");
            }
        }
        let paths = lane.inputs.iter().rev().cloned().collect::<Vec<_>>();
        let anchor = lane.inputs[0].to_string_lossy().into_owned();
        let out = lane.tmp.path().join("out");
        fs::create_dir_all(&out).expect("fixture value");
        let request = TitleAudioRequest {
            output: abb_engine::audio::OutputAudio::new(
                format,
                EncoderSettings {
                    encoder_type: EncoderType::Opus,
                    bitrate_mode: BitrateMode::VbrTarget,
                    channels: ChannelConfig::Auto,
                    ..native_encoder_settings()
                },
            ),
            intent: AudioIntent::Auto,
            sample_rate: SampleRateConfig::Auto,
        };
        let payload: ProcessPayload = serde_json::from_value(serde_json::json!({
            "inputFiles": [&anchor], "titleSources": {&anchor: paths.iter().map(|path| serde_json::json!({"path": path})).collect::<Vec<_>>()},
            "audioRequests": [request], "outputDir": out, "aacDecoder": "auto"
        })).expect("fixture value");
        let intent = std::collections::HashMap::from([(
            anchor.clone(),
            MetadataIntentPatch {
                title: Some(PatchOp::Set("Opus title".into())),
                artist: Some(PatchOp::Set("Test Author".into())),
                cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
                ..Default::default()
            },
        )]);
        let plan = preflight_payload(payload.clone(), None, false).expect("Opus preflight");
        let audio_plan = &plan.audio_plans[0];
        assert_eq!(audio_plan.sample_rate, input_rate);
        assert_eq!(audio_plan.channels, channels);
        assert_eq!(
            PathBuf::from(&plan.outputs[0].resolved_path)
                .extension()
                .expect("fixture value"),
            format.extension()
        );
        let duration = get_file_list_info(&paths, AacDecoder::Auto)
            .expect("fixture value")
            .total_duration;
        let (destination, _) =
            run_planned_title(payload, Some(intent), &lane.tmp.path().join("work"))
                .await
                .expect("Opus title output");
        let audio = get_file_list_info(std::slice::from_ref(&destination), AacDecoder::Auto)
            .expect("fixture value");
        assert!(audio.files[0]
            .codec_label
            .as_deref()
            .expect("fixture value")
            .to_lowercase()
            .contains("opus"));
        let decoded = decode_pcm_f32(&destination);
        let decoded_seconds = decoded.len() as f64 / (48_000.0 * f64::from(channels));
        assert!(
            (decoded_seconds - duration).abs() < 0.002,
            "{:?}: playable duration {} expected {}",
            format,
            decoded_seconds,
            duration
        );
        if channels == 2 {
            let difference_energy: f64 = decoded
                .as_chunks::<2>()
                .0
                .iter()
                .map(|frame| f64::from(frame[0] - frame[1]).powi(2))
                .sum();
            assert!(
                (difference_energy / (decoded.len() / 2) as f64).sqrt() > 0.05,
                "packed float encoding must preserve distinct stereo channels"
            );
        }
        let tags = read_metadata(&destination).expect("read output tags");
        assert_eq!(tags.title.as_deref(), Some("Opus title"));
        assert_eq!(tags.artist.as_deref(), Some("Test Author"));
        assert_eq!(tags.cover_art, Some(minimal_jpg_bytes()));
        let chapters = chapters_of(&destination);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].1, 0);
        assert!(
            (chapters[1].2 as f64 - duration * 1000.0).abs() <= 2.0,
            "{chapters:?}"
        );
        let other_format = if format == AudiobookFormat::M4aOpus {
            AudiobookFormat::MkaOpus
        } else {
            AudiobookFormat::M4aOpus
        };
        let info = get_file_list_info(std::slice::from_ref(&destination), AacDecoder::Auto)
            .expect("fixture value");
        let copy_plan = abb_engine::audio::resolve_title_audio(
            &TitleAudioRequest {
                output: abb_engine::audio::OutputAudio::new(
                    other_format,
                    abb_engine::audio::EncoderSettings::default(),
                ),
                intent: AudioIntent::Preserve,
                sample_rate: SampleRateConfig::Auto,
            },
            &info,
            false,
        )
        .expect("fixture value");
        assert_eq!(
            copy_plan.handling,
            abb_engine::processing::AudioHandling::Preserve
        );
        let copied = lane
            .tmp
            .path()
            .join("copy")
            .with_extension(other_format.extension());
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::new()),
            SampleRateConfig::Auto,
            OutputConfig::new(&copied),
            lane.tmp.path().join("work-copy"),
        );
        execute_audio_engine(
            AudioExecutionRequest::new(
                context,
                info,
                None,
                CoverArtPassthroughPolicy::Preserve,
                copy_plan.handling,
            )
            .with_metadata_intent(Some(MetadataIntentPatch {
                title: Some(PatchOp::Set("Retagged Opus".into())),
                ..Default::default()
            })),
        )
        .await
        .expect("fixture value");
        assert_eq!(
            audio_packet_bytes(&copied),
            audio_packet_bytes(&destination)
        );
        assert!(
            (decode_pcm_f32(&copied).len() as i64 - decoded.len() as i64).abs() <= 48,
            "{:?}: copy={} source={}",
            format,
            decode_pcm_f32(&copied).len(),
            decoded.len()
        );
        assert_eq!(chapters_of(&copied), accepted_chapters(&destination));
        let copied_tags = read_metadata(&copied).expect("fixture value");
        assert_eq!(copied_tags.title.as_deref(), Some("Retagged Opus"));
        assert_eq!(copied_tags.cover_art, Some(minimal_jpg_bytes()));

        let copied_plan = accepted_chapters(&copied);
        let aac_lane = MediaLane::for_inputs(vec![copied]).with_encoder(EncoderSettings {
            channels: ChannelConfig::Auto,
            ..native_encoder_settings()
        });
        let aac_output = aac_lane.process(None).await;
        let aac_seconds =
            decode_pcm_f32(&aac_output).len() as f64 / (48_000.0 * f64::from(channels));
        assert!(
            (aac_seconds - decoded_seconds).abs() < 0.003,
            "Opus import through ABB must preserve the playable interval"
        );
        assert_eq!(chapters_of(&aac_output), copied_plan);
        if format == AudiobookFormat::MkaOpus {
            assert!(read_audio_cover_thumbnail(&destination)
                .expect("fixture value")
                .is_some());
            let packets = audio_packet_bytes(&destination);
            let replacement = minimal_jpg_bytes();
            save_metadata_intent(
                &destination,
                &MetadataIntentPatch {
                    cover_art: Some(PatchOp::Set(replacement)),
                    ..Default::default()
                },
            )
            .expect("fixture value");
            {
                let input = ffmpeg_next::format::input(&destination).expect("fixture value");
                assert_eq!(
                    input
                        .streams()
                        .filter(
                            |s| s.parameters().medium() == ffmpeg_next::media::Type::Attachment
                                || s.disposition().contains(
                                    ffmpeg_next::format::stream::Disposition::ATTACHED_PIC
                                )
                        )
                        .count(),
                    1
                );
            }
            assert!(read_audio_cover_thumbnail(&destination)
                .expect("fixture value")
                .is_some());
            save_metadata_intent(
                &destination,
                &MetadataIntentPatch {
                    cover_art: Some(PatchOp::Clear),
                    ..Default::default()
                },
            )
            .expect("fixture value");
            assert_eq!(
                read_audio_cover_thumbnail(&destination).expect("fixture value"),
                None
            );
            assert_eq!(audio_packet_bytes(&destination), packets);
            assert_eq!(chapters_of(&destination), chapters);
        }
    }
}

#[tokio::test]
async fn opus_remux_keeps_variable_frame_timing_and_trim() {
    use abb_engine::processing::AudioHandling;
    let tmp = TempDir::new().expect("fixture value");
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
    for (frame_ms, bitrate) in [("2.5", "64k"), ("60", "12k"), ("120", "64k")] {
        let source = tmp.path().join(format!("source-{frame_ms}.mka"));
        let destination = tmp.path().join(format!("copy-{frame_ms}.m4a"));
        let created = Command::new(&binary)
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=0.413",
            ])
            .args([
                "-c:a",
                "libopus",
                "-frame_duration",
                frame_ms,
                "-b:a",
                bitrate,
            ])
            .arg(&source)
            .output()
            .expect("fixture value");
        assert!(
            created.status.success(),
            "{}",
            String::from_utf8_lossy(&created.stderr)
        );
        let source_samples = decode_pcm_f32(&source).len();
        let info = get_file_list_info(std::slice::from_ref(&source), AacDecoder::Auto)
            .expect("fixture value");
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::new()),
            SampleRateConfig::Auto,
            OutputConfig::new(&destination),
            tmp.path().join("work"),
        );
        execute_audio_engine(AudioExecutionRequest::new(
            context,
            info,
            None,
            CoverArtPassthroughPolicy::Preserve,
            AudioHandling::Preserve,
        ))
        .await
        .expect("Opus pass-through into M4A");
        assert_eq!(
            audio_packet_bytes(&destination),
            audio_packet_bytes(&source)
        );
        assert!(
            decode_pcm_f32(&destination).len().abs_diff(source_samples) <= 48,
            "{frame_ms} ms packets retain the playable audio interval"
        );
    }
}

pub(crate) fn title_audio_request(
    intent: abb_engine::audio::AudioIntent,
    settings: EncoderSettings,
) -> abb_engine::audio::TitleAudioRequest {
    abb_engine::audio::TitleAudioRequest {
        output: abb_engine::audio::OutputAudio::M4b { settings },
        intent,
        sample_rate: SampleRateConfig::Auto,
    }
}

#[tokio::test]
async fn default_audio_plan_reduces_large_aac_but_explicit_keep_never_encodes() {
    use abb_engine::audio::{resolve_title_audio, AudioIntent, OutputAudio};
    let lane = MediaLane::with_fixtures(&[1.5]);
    let source = lane.process(None).await;
    let probed = get_file_list_info(&[&source], AacDecoder::Auto).expect("fixture value");
    let mut request = title_audio_request(
        AudioIntent::Encode,
        EncoderSettings {
            encoder_type: EncoderType::Faac,
            bitrate_kbps: 80,
            bitrate_mode: BitrateMode::Abr,
            ..native_encoder_settings()
        },
    );
    request.intent = AudioIntent::Auto;
    for (bitrate, kept_unchanged) in [
        (Some(48_000), true),
        (Some(72_000), true),
        (Some(72_001), false),
        (None, false),
    ] {
        let mut info = probed.clone();
        info.files[0].bitrate = bitrate;
        let plan = resolve_title_audio(&request, &info, false).expect("fixture value");
        assert_eq!(plan.handling.preserves(), kept_unchanged, "{bitrate:?}");
        assert_eq!(
            plan.sample_rate,
            info.files[0].sample_rate.expect("fixture value")
        );
        assert_eq!(
            u32::from(plan.channels),
            info.files[0].channels.expect("fixture value")
        );
        if let AudioHandling::Encode { settings } = &plan.handling {
            assert_eq!(settings.encoder_type, EncoderType::NativeAac);
            assert_eq!(settings.bitrate_mode, BitrateMode::Cbr);
            assert_eq!(settings.bitrate_kbps, 65);
        }
    }

    let mut info = probed;
    info.files[0].bitrate = Some(128_000);
    request.intent = AudioIntent::Encode;
    let explicit = resolve_title_audio(&request, &info, false).expect("fixture value");
    let AudioHandling::Encode { settings } = &explicit.handling else {
        panic!("explicit Encode encodes: {explicit:?}");
    };
    assert_eq!(
        (settings.encoder_type, settings.bitrate_kbps),
        (EncoderType::Faac, 80)
    );
    request.intent = AudioIntent::Preserve;
    let kept = resolve_title_audio(&request, &info, false).expect("fixture value");
    assert_eq!(kept.handling, AudioHandling::Preserve);
    request.output = OutputAudio::Mp3;
    assert!(resolve_title_audio(&request, &info, false)
        .expect_err("fixture must reject")
        .to_string()
        .contains("MP3 source audio"));
    let refusal =
        resolve_title_audio(&request, &info, true).expect_err("a preview never copies audio");
    assert!(refusal.to_string().contains(PREVIEW_REFUSAL));
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
