//! Real-media execution proofs for Audio and Metadata handoffs.
//!
//! Smallest maintained real-media lane: every fixture is synthesized at test
//! time (no committed media, no licensing exposure) — WAVs in pure Rust, MP3s
//! via the environment's FFmpeg CLI, M4Bs by reusing the engine's own
//! committed output as a second-pass input. Execution runs the in-process
//! ffmpeg-next native path with a headless `ProcessingContext`.
//!
//! These tests prove workflow behavior structural tests cannot:
//! - import → configure → process → decodable M4B with truthful duration
//! - real input formats: WAV, M4B (AAC decode→encode), MP3, and Opus
//! - encoder routes: Native AAC, Apple AAC (AudioToolbox), bundled FAAC, and Opus. External FDK
//!   is deliberately absent from the normal suite — it needs a user-supplied
//!   libfdk_aac FFmpeg, which is environment-dependent by definition.
//! - metadata save → re-read tags from the real output artifact
//! - cover art: explicit save round-trips byte-identical; source-cover
//!   passthrough survives reprocessing
//! - chapters: synthesized per source on merge, preserved on reprocess
//! - cancellation → terminal error with no artifact and no staging residue
//!
//! Runtime budget: the module must stay under ~10s wall clock.
//! If it grows past that, shrink fixtures before widening the budget.

use audiobook_boss_lib::audio::{
    execute_audio_engine, get_file_list_info, AudioExecutionRequest, BitrateMode, ChannelConfig,
    EncoderSettings, EncoderType, SampleRateConfig,
};
use audiobook_boss_lib::processing::job_registry::{JobId, JobRegistry};
use audiobook_boss_lib::processing::{OutputConfig, ProcessingContext, ProcessingSession};
use audiobook_boss_lib::{
    extract_passthrough_metadata, finalize_artifact_metadata, read_audio_cover_thumbnail,
    read_metadata, save_metadata_intent, AlbumSortPatchOp, AppError, AudiobookMetadata,
    CoverArtPassthroughPolicy, MetadataIntentPatch, PassthroughSource, PatchOp,
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
fn ffprobe_format_tags(path: &Path) -> serde_json::Map<String, serde_json::Value> {
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

/// Case-insensitive tag lookup: MP4 freeform atom families differing only in
/// name case collapse into one ffprobe dict entry whose case follows atom
/// order, so exact-case assertions would be brittle.
fn ffprobe_tag_ci(tags: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    tags.iter()
        .find(|(tag_key, _)| tag_key.eq_ignore_ascii_case(key))
        .and_then(|(_, value)| value.as_str().map(str::to_string))
}

fn assert_ffprobe_tag(
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

const SAMPLE_RATE: u32 = 44_100;

/// An encoder-sized zero pad must not become playable source audio.
#[tokio::test]
async fn native_aac_reprocessing_keeps_the_original_playable_sample_count() {
    for settings in [
        native_encoder_settings(),
        EncoderSettings {
            native_aac_speed: 4,
            faac_profile: audiobook_boss_lib::audio::FaacProfile::Auto,
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
            .arg(&output)
            .output()
            .expect("ffprobe sample duration");
        assert!(probe.status.success());
        let facts: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
        let stream = &facts["streams"][0];
        assert_eq!(stream["time_base"], "1/44100");
        assert_eq!(
            stream["duration_ts"].as_u64(),
            Some(expected),
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

fn decode_pcm_f32(path: &Path) -> Vec<f32> {
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
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
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
        .unwrap();
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
        .unwrap();
    assert!(probe.status.success());
    let facts: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    let packet = &facts["packets"][30];
    let end = packet["pos"].as_str().unwrap().parse::<u64>().unwrap()
        + packet["size"].as_str().unwrap().parse::<u64>().unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&broken)
        .unwrap()
        .set_len(end)
        .unwrap();
    let broken_lane = MediaLane::for_inputs(vec![broken]);
    let info = get_file_list_info(&broken_lane.inputs).unwrap();
    assert_eq!(
        info.invalid_count, 1,
        "declared packets extend past the end of this MP4"
    );
    assert!(info.files[0]
        .error
        .as_deref()
        .unwrap()
        .contains("incomplete or truncated"));
    let request = AudioExecutionRequest::new(
        broken_lane.context(ProcessingSession::new()),
        info,
        None,
        CoverArtPassthroughPolicy::Preserve,
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
    let mut tag = mp4ameta::Tag::read_from_path(&output).unwrap();
    tag.set_artists(["First Author".to_string(), "Second Author".to_string()]);
    tag.set_album_artists([
        "First Album Author".to_string(),
        "Second Album Author".to_string(),
    ]);
    tag.set_composers(["First Narrator".to_string(), "Second Narrator".to_string()]);
    tag.write_to_path(&output).unwrap();

    let metadata = read_metadata(&output).unwrap();
    assert_eq!(
        metadata.artist.as_deref(),
        Some("First Author;Second Author")
    );
    assert_eq!(
        metadata.composer.as_deref(),
        Some("First Narrator;Second Narrator")
    );
    let imported = get_file_list_info(&[&output]).unwrap();
    assert_eq!(imported.files[0].tag_artist, metadata.artist);

    save_metadata_intent(
        &output,
        &MetadataIntentPatch {
            title: PatchOp::Set("Retitled".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let mut tag = mp4ameta::Tag::read_from_path(&output).unwrap();
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
    tag.write_to_path(&output).unwrap();
    assert_eq!(
        read_metadata(&output).unwrap().artist.as_deref(),
        Some("First Album Author;Second Album Author")
    );

    let reprocess = MediaLane::for_inputs(vec![output]);
    let finalized = reprocess.process(Some(metadata)).await;
    let reread = read_metadata(&finalized).unwrap();
    assert_eq!(reread.artist.as_deref(), Some("First Author;Second Author"));
    assert_eq!(
        reread.composer.as_deref(),
        Some("First Narrator;Second Narrator")
    );
}

/// Writes a mono 16-bit PCM WAV of `seconds` of sine at `freq_hz`.
fn write_sine_wav(path: &Path, seconds: f64, freq_hz: f64) {
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

fn native_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: ChannelConfig::Mono,
        afterburner: false,
        native_aac_speed: 0,
        faac_profile: audiobook_boss_lib::audio::FaacProfile::Auto,
    }
}

/// Isolated on-disk lane: fixture inputs, output destination, and a private
/// processing workspace root, all inside one TempDir.
struct MediaLane {
    tmp: TempDir,
    inputs: Vec<PathBuf>,
    encoder_settings: EncoderSettings,
    sample_rate: SampleRateConfig,
}

impl MediaLane {
    fn with_fixtures(durations_secs: &[f64]) -> Self {
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
        }
    }

    /// A lane whose inputs are pre-built media files (e.g. a committed M4B
    /// from an earlier engine pass, or a synthesized MP3) instead of WAVs.
    fn for_inputs(inputs: Vec<PathBuf>) -> Self {
        let tmp = TempDir::new().expect("create media lane tempdir");
        Self {
            tmp,
            inputs,
            encoder_settings: native_encoder_settings(),
            sample_rate: SampleRateConfig::Auto,
        }
    }

    /// Same lane, different encoder route (e.g. Apple AAC via AudioToolbox).
    fn with_encoder(mut self, encoder_settings: EncoderSettings) -> Self {
        self.encoder_settings = encoder_settings;
        self
    }

    /// Same lane, explicit output sample rate (exercises the resample path).
    fn with_sample_rate(mut self, sample_rate: SampleRateConfig) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    fn output_path(&self) -> PathBuf {
        self.tmp.path().join("out").join("lane-output.m4b")
    }

    fn workspace_root(&self) -> PathBuf {
        self.tmp.path().join("workspace")
    }

    fn context(&self, session: ProcessingSession) -> ProcessingContext {
        let output_dir = self.output_path();
        fs::create_dir_all(output_dir.parent().expect("output parent")).expect("create output dir");
        ProcessingContext::new_headless_with_workspace_root(
            Arc::new(session),
            self.encoder_settings.clone(),
            self.sample_rate.clone(),
            OutputConfig::new(self.output_path()),
            self.workspace_root(),
        )
    }

    fn execution_request(
        &self,
        session: ProcessingSession,
        metadata: Option<AudiobookMetadata>,
    ) -> AudioExecutionRequest {
        let file_info = get_file_list_info(&self.inputs).expect("probe WAV fixtures");
        assert_eq!(
            file_info.invalid_count, 0,
            "generated fixtures must probe as valid audio"
        );
        AudioExecutionRequest::new(
            self.context(session),
            file_info,
            metadata,
            CoverArtPassthroughPolicy::Preserve,
        )
    }

    /// Runs the engine on this lane's inputs and returns the committed path.
    async fn process(&self, metadata: Option<AudiobookMetadata>) -> PathBuf {
        execute_audio_engine(self.execution_request(ProcessingSession::new(), metadata))
            .await
            .expect("native processing succeeds");
        self.output_path()
    }

    /// Directories left under the private workspace root after a run.
    fn residual_workspace_dirs(&self) -> Vec<PathBuf> {
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

#[tokio::test]
async fn import_configure_process_produces_decodable_m4b_with_truthful_duration() {
    let lane = MediaLane::with_fixtures(&[1.5, 1.0]);
    let expected_duration = 2.5;

    let result = execute_audio_engine(lane.execution_request(ProcessingSession::new(), None))
        .await
        .expect("native processing of generated WAV fixtures succeeds");

    let output = lane.output_path();
    assert!(output.exists(), "final artifact exists at planned path");
    assert!(
        result.ends_with(output.to_str().expect("utf8 temp path")),
        "engine success message names the committed final path, got: {result}"
    );

    // Re-import the artifact through the same public probe the app uses:
    // proves container validity and decodability, not just file presence.
    let probe = get_file_list_info(&[&output]).expect("re-probe committed M4B");
    assert_eq!(probe.valid_count, 1, "output M4B probes as valid audio");
    let drift = (probe.total_duration - expected_duration).abs();
    assert!(
        drift < 0.5,
        "merged duration {} differs from source total {expected_duration} by {drift}",
        probe.total_duration
    );

    assert!(
        lane.residual_workspace_dirs().is_empty(),
        "successful run leaves no staging residue in the workspace root"
    );
}

#[tokio::test]
async fn analysis_exposes_embedded_chapters_from_real_m4b() {
    let lane = MediaLane::with_fixtures(&[1.0, 1.5]);
    let output = lane.process(None).await;

    let probe = get_file_list_info(&[&output]).expect("analyze chaptered M4B");
    let chapters = &probe.files[0].chapters;
    assert_eq!(chapters.len(), 2, "analysis returns both embedded chapters");
    assert_eq!(chapters[0].title.as_deref(), Some("fixture-0"));
    assert_eq!(chapters[1].title.as_deref(), Some("fixture-1"));
}

#[tokio::test]
async fn metadata_saved_during_processing_rereads_from_output_artifact() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let mut metadata = AudiobookMetadata::new();
    metadata.title = Some("Media Lane Title".to_string());
    metadata.artist = Some("Lane Narrator".to_string());
    metadata.album = Some("Lane Album".to_string());
    metadata.genre = Some("Audiobook".to_string());

    execute_audio_engine(lane.execution_request(ProcessingSession::new(), Some(metadata)))
        .await
        .expect("processing with metadata succeeds");

    let reread = read_metadata(lane.output_path()).expect("re-read tags from committed artifact");
    assert_eq!(reread.title.as_deref(), Some("Media Lane Title"));
    assert_eq!(reread.artist.as_deref(), Some("Lane Narrator"));
    assert_eq!(reread.album.as_deref(), Some("Lane Album"));
    assert_eq!(reread.genre.as_deref(), Some("Audiobook"));
}

#[tokio::test]
async fn analysis_populates_display_tags_from_an_existing_tagged_fixture() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let output = lane.process(None).await;
    let patch = MetadataIntentPatch {
        title: PatchOp::Set("Analyzed Fixture Title".to_string()),
        artist: PatchOp::Set("Analyzed Fixture Artist".to_string()),
        ..Default::default()
    };
    save_metadata_intent(&output, &patch).expect("tag fixture through metadata boundary");

    let analyzed = get_file_list_info(&[&output]).expect("analyze tagged fixture");
    let file = analyzed.files.first().expect("one analyzed fixture");
    assert_eq!(file.tag_title.as_deref(), Some("Analyzed Fixture Title"));
    assert_eq!(file.tag_artist.as_deref(), Some("Analyzed Fixture Artist"));
}

#[tokio::test]
async fn cancellation_yields_terminal_error_without_artifact_or_staging_residue() {
    let lane = MediaLane::with_fixtures(&[1.0]);

    let registry = JobRegistry::new(1);
    let checker = registry.cancellation_checker(JobId::new()).await;
    let session = ProcessingSession::from_job_registry(uuid::Uuid::new_v4(), checker);
    registry.cancel_all();

    let err = execute_audio_engine(lane.execution_request(session, None))
        .await
        .expect_err("cancelled session must not report success");
    assert!(
        matches!(err, AppError::Cancellation(_)),
        "cancellation surfaces as the typed Cancellation error, got: {err:?}"
    );

    assert!(
        !lane.output_path().exists(),
        "no artifact is committed for a cancelled run"
    );
    assert!(
        lane.residual_workspace_dirs().is_empty(),
        "cancelled run leaves no staging residue in the workspace root"
    );
}

/// #281 artifact round-trip on a real committed artifact: normal saves
/// preserve artifact fields; explicit clear intent removes exactly the
/// cleared fields.
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

    // A normal save that only touches a primary field preserves artifacts.
    let title_only = MetadataIntentPatch {
        title: PatchOp::Set("Renamed Artifact Book".to_string()),
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

    // Explicit clear intent removes exactly the cleared artifact fields.
    let clear_artifacts = MetadataIntentPatch {
        album_sort: AlbumSortPatchOp::Clear,
        comment: PatchOp::Clear,
        track: PatchOp::Clear,
        disk: PatchOp::Clear,
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

/// The external FDK adapter finalizes a freshly encoded M4B by re-applying
/// effective metadata and chapter/cover passthrough onto the artifact. This
/// pins the container-aware finalize owner: series-family tags and album_sort
/// must land as real MP4 atoms (the FFmpeg mov muxer silently drops dict keys
/// outside its known-atom table), and the chapters written by the remux must
/// survive the MP4 tag rewrite. Proven against ABB readback AND ffprobe.
#[tokio::test]
async fn artifact_finalize_preserves_series_tags_and_chapters_on_mp4_route() {
    let durations = [1.0, 1.5];
    let lane = MediaLane::with_fixtures(&durations);
    let output = lane.process(None).await;

    let sources: Vec<PassthroughSource> = lane
        .inputs
        .iter()
        .zip(durations)
        .map(|(path, duration)| PassthroughSource {
            chapters: None,
            path: path.clone(),
            duration: Some(duration),
            is_valid: true,
        })
        .collect();
    let passthrough = extract_passthrough_metadata(&sources);
    assert_eq!(
        passthrough.chapters.len(),
        2,
        "chapterless multi-file sources synthesize one chapter per file"
    );

    let metadata = AudiobookMetadata {
        title: Some("Finalized Title".to_string()),
        artist: Some("Finalized Author".to_string()),
        series: Some("Finalize Series".to_string()),
        series_part: Some("3".to_string()),
        album_sort: Some("Finalize Series 03 - Finalized Title".to_string()),
        ..Default::default()
    };

    finalize_artifact_metadata(&output, Some(&metadata), Some(&passthrough))
        .expect("artifact metadata finalize");

    // ABB readback: the same reader the app uses after import.
    let read_back = read_metadata(&output).expect("read finalized artifact");
    assert_eq!(read_back.title.as_deref(), Some("Finalized Title"));
    assert_eq!(
        read_back.series.as_deref(),
        Some("Finalize Series"),
        "series must survive artifact finalize on the MP4 route"
    );
    assert_eq!(read_back.series_part.as_deref(), Some("3"));
    assert_eq!(
        read_back.album_sort.as_deref(),
        Some("Finalize Series 03 - Finalized Title"),
        "album_sort must survive artifact finalize on the MP4 route"
    );

    // External-reader truth, not just ABB readback.
    let tags = ffprobe_format_tags(&output);
    assert_eq!(
        ffprobe_tag_ci(&tags, "title").as_deref(),
        Some("Finalized Title")
    );
    assert_eq!(
        ffprobe_tag_ci(&tags, "series").as_deref(),
        Some("Finalize Series"),
        "series must be externally visible; full tags: {tags:?}"
    );
    assert_eq!(
        ffprobe_tag_ci(&tags, "series-part").as_deref(),
        Some("3"),
        "series-part must be externally visible; full tags: {tags:?}"
    );
    assert_eq!(
        ffprobe_tag_ci(&tags, "sort_album").as_deref(),
        Some("Finalize Series 03 - Finalized Title"),
        "album-sort must be externally visible; full tags: {tags:?}"
    );

    // Chapters written during finalize survive the MP4 tag rewrite.
    let artifact_chapters = extract_passthrough_metadata(&[PassthroughSource {
        chapters: None,
        path: output.clone(),
        duration: None,
        is_valid: true,
    }])
    .chapters;
    assert_eq!(
        artifact_chapters.len(),
        2,
        "finalized artifact keeps both passthrough chapters"
    );
}

#[tokio::test]
async fn metadata_save_writes_external_ffprobe_visible_mp4_tags() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let output = lane.process(None).await;

    let patch = MetadataIntentPatch {
        title: PatchOp::Set("External Probe Title".to_string()),
        artist: PatchOp::Set("External Probe Author".to_string()),
        album: PatchOp::Set("External Probe Album".to_string()),
        composer: PatchOp::Set("External Probe Composer".to_string()),
        genre: PatchOp::Set("Audiobook".to_string()),
        date: PatchOp::Set("2024-05-06".to_string()),
        description: PatchOp::Set("External reader proof".to_string()),
        series: PatchOp::Set("Probe Series".to_string()),
        series_part: PatchOp::Set("2".to_string()),
        subseries: PatchOp::Set("Probe Subseries".to_string()),
        subseries_part: PatchOp::Set("7".to_string()),
        comment: PatchOp::Set("Probe Comment".to_string()),
        track: PatchOp::Set((3, Some(9))),
        disk: PatchOp::Set((1, Some(2))),
        ..Default::default()
    };
    save_metadata_intent(&output, &patch).expect("metadata save through mp4ameta path");

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

fn minimal_jpg_bytes() -> Vec<u8> {
    fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/minimal.jpg"
    ))
    .expect("read minimal.jpg support fixture")
}

fn chapters_of(path: &Path) -> Vec<(Option<String>, i64, i64)> {
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
        let timescale = u32::from_be_bytes(bytes[start + 20..start + 24].try_into().unwrap());
        let duration = u64::from_be_bytes(bytes[start + 24..start + 32].try_into().unwrap());
        (timescale, duration)
    } else {
        assert!(start + 20 <= end, "mdhd v0 too small");
        let timescale = u32::from_be_bytes(bytes[start + 12..start + 16].try_into().unwrap());
        let duration = u32::from_be_bytes(bytes[start + 16..start + 20].try_into().unwrap()) as u64;
        (timescale, duration)
    }
}

/// The user's dominant real input is M4B, not WAV. Two-pass: the engine's own
/// committed output becomes the single input for a second run — exercising
/// AAC decode → encode and the MP4 tag read path with no committed media.
#[tokio::test]
async fn m4b_input_processes_with_metadata_intact_and_cover_passthrough() {
    let wav_lane = MediaLane::with_fixtures(&[1.5]);
    let mut first_pass = AudiobookMetadata::new();
    first_pass.title = Some("First Pass".to_string());
    first_pass.artist = Some("Lane Narrator".to_string());
    first_pass.cover_art = Some(minimal_jpg_bytes());
    let m4b_input = wav_lane.process(Some(first_pass)).await;

    let m4b_lane = MediaLane::for_inputs(vec![m4b_input]);
    let mut second_pass = AudiobookMetadata::new();
    second_pass.title = Some("Second Pass".to_string());
    // No cover art in pass B: with the Preserve policy the source M4B's
    // embedded cover must pass through to the new artifact.
    let output = m4b_lane.process(Some(second_pass)).await;

    let probe = get_file_list_info(&[&output]).expect("re-probe reprocessed M4B");
    assert_eq!(
        probe.valid_count, 1,
        "reprocessed M4B probes as valid audio"
    );
    let drift = (probe.total_duration - 1.5).abs();
    assert!(
        drift < 0.5,
        "reprocessed duration {} drifted from source 1.5 by {drift}",
        probe.total_duration
    );

    let reread = read_metadata(&output).expect("re-read tags from reprocessed artifact");
    assert_eq!(reread.title.as_deref(), Some("Second Pass"));
    let cover = reread
        .cover_art
        .expect("source M4B cover art passes through to the reprocessed artifact");
    assert!(!cover.is_empty(), "passed-through cover art has bytes");
}

/// Cover art supplied with the save must land in the committed artifact and
/// read back through the same public metadata reader the app uses.
#[tokio::test]
async fn cover_art_saved_during_processing_rereads_from_output_artifact() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let jpg = minimal_jpg_bytes();
    let mut metadata = AudiobookMetadata::new();
    metadata.title = Some("Covered Book".to_string());
    metadata.cover_art = Some(jpg.clone());

    let output = lane.process(Some(metadata)).await;

    let reread = read_metadata(&output).expect("re-read committed artifact");
    let cover = reread.cover_art.expect("cover art embedded in output");
    assert_eq!(
        cover, jpg,
        "cover art bytes round-trip unchanged through processing"
    );
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
        width <= 64 && height <= 64,
        "thumbnail dimensions {width}x{height} exceed the 64px bound"
    );

    let coverless_lane = MediaLane::with_fixtures(&[1.0]);
    let coverless_output = coverless_lane.process(None).await;
    assert_eq!(
        read_audio_cover_thumbnail(&coverless_output).expect("read coverless artifact thumbnail"),
        None
    );
}

/// Chapter truth across the two behaviors the pipeline owns: multi-file
/// merges synthesize one chapter per source, and reprocessing a chaptered
/// M4B as a single input preserves the embedded chapters (#341 residual).
#[tokio::test]
async fn chapters_synthesize_on_merge_and_survive_reprocessing() {
    let merge_lane = MediaLane::with_fixtures(&[1.5, 1.0]);
    let merged = merge_lane.process(None).await;

    let synthesized = chapters_of(&merged);
    assert_eq!(
        synthesized.len(),
        2,
        "merge without source chapters synthesizes one chapter per input"
    );
    assert_eq!(synthesized[0].1, 0, "first chapter starts at zero");
    let boundary = synthesized[1].1;
    assert!(
        (boundary - 1_500).abs() < 500,
        "second chapter boundary {boundary}ms should sit near the first fixture's 1500ms"
    );

    let reprocess_lane = MediaLane::for_inputs(vec![merged]);
    let reprocessed = reprocess_lane.process(None).await;

    let preserved = chapters_of(&reprocessed);
    assert_eq!(
        preserved.len(),
        2,
        "embedded chapters survive single-input reprocessing"
    );
    assert_eq!(
        preserved.iter().map(|c| c.0.clone()).collect::<Vec<_>>(),
        synthesized.iter().map(|c| c.0.clone()).collect::<Vec<_>>(),
        "chapter titles are preserved"
    );
    let preserved_boundary = preserved[1].1;
    assert!(
        (preserved_boundary - boundary).abs() < 200,
        "preserved chapter boundary {preserved_boundary}ms should match the source's {boundary}ms"
    );
}

/// Cover art is a second output stream. If its time_base stays 0/0, FFmpeg's
/// ipod muxer defaults it to 1/90000, LCMs with audio 1/44100 into movie
/// timescale 4410000, and the QuickTime chapter packets overflow. Apple Books
/// reads that broken timed-text track; Nero `chpl` stays correct. Cover art is
/// the reproducing trigger, not a decoration.
#[tokio::test]
async fn qt_chapter_track_times_span_the_book_when_cover_art_is_muxed() {
    let lane = MediaLane::with_fixtures(&[1.5, 1.0]);
    let mut metadata = AudiobookMetadata::new();
    metadata.cover_art = Some(minimal_jpg_bytes());
    let output = lane.process(Some(metadata)).await;

    let chpl = chapters_of(&output);
    assert_eq!(
        chpl.len(),
        2,
        "Nero chpl still synthesizes one chapter per input"
    );

    let (qt_duration, text_timescale, audio_timescale) = timed_text_chapter_track(&output);
    assert_eq!(
        text_timescale, audio_timescale,
        "timed-text timescale {text_timescale} must match audio {audio_timescale}; 4410000 is the 44100×90000 LCM from an unset cover-art time_base"
    );
    assert!(
        (qt_duration - 2.5).abs() < 0.75,
        "timed-text track duration {qt_duration}s must span the whole book (~2.5s), not only the last chapter"
    );
}

/// Synthesizes a mono sine MP3 with the environment's FFmpeg CLI. Fixture
/// generation stays independent of ABB's linked decoder/encoder feature set;
/// the test below still exercises the real linked MP3 import path.
fn write_sine_mp3(path: &Path, seconds: f64, freq_hz: f64) {
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
            "-b:a",
            "64k",
            "-y",
        ])
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
    let chapters = path.with_file_name("chapters.ffmetadata");
    write_sine_mp3(&audio, seconds, freq_hz);
    fs::write(
        &chapters,
        ";FFMETADATA1\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=450\ntitle=Opening\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=450\nEND=900\ntitle=Closing\n",
    )
    .expect("write MP3 chapter metadata");

    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let output = Command::new(&binary)
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(&audio)
        .args(["-f", "ffmetadata", "-i"])
        .arg(&chapters)
        .args([
            "-map",
            "0:a:0",
            "-map_metadata",
            "1",
            "-map_chapters",
            "1",
            "-codec:a",
            "copy",
            "-id3v2_version",
            "3",
            "-y",
        ])
        .arg(path)
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "ffmpeg must be on PATH or set via ABB_FFMPEG for chaptered MP3 fixture synthesis; spawning `{binary}` failed: {error}"
            )
        });
    assert!(
        output.status.success(),
        "ffmpeg failed to synthesize chaptered {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn id3_language_comments_round_trip_through_unrelated_set_and_clear_intent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("comments.mp3");
    write_sine_mp3(&path, 0.1, 440.0);
    let audio = fs::read(&path).unwrap();
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
    fs::write(&path, tagged).unwrap();
    assert_eq!(
        read_metadata(&path).unwrap().comment.as_deref(),
        Some("English comment")
    );
    save_metadata_intent(
        &path,
        &MetadataIntentPatch {
            genre: PatchOp::Set("Audiobook".into()),
            ..Default::default()
        },
    )
    .unwrap();
    {
        let input = ffmpeg_next::format::input(&path).unwrap();
        let tags = input.metadata();
        assert_eq!(tags.get("comment-eng"), Some("English comment"));
        assert_eq!(tags.get("comment-comment-fra"), Some("French comment"));
    }
    save_metadata_intent(
        &path,
        &MetadataIntentPatch {
            comment: PatchOp::Set("New comment".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        read_metadata(&path).unwrap().comment.as_deref(),
        Some("New comment")
    );
    save_metadata_intent(
        &path,
        &MetadataIntentPatch {
            comment: PatchOp::Clear,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(read_metadata(&path).unwrap().comment, None);
    let input = ffmpeg_next::format::input(&path).unwrap();
    assert_eq!(
        input.metadata().get("comment-iTunNORM-eng"),
        Some("technical normalization")
    );
}

/// MP3 is the other common real input. The fixture is a genuine lame-encoded
/// MP3 synthesized at test time; the assertions mirror the M4B-input test:
/// decodable output, truthful duration, and tags re-read from the artifact.
#[tokio::test]
async fn mp3_input_processes_with_metadata_intact() {
    let tmp = TempDir::new().expect("mp3 fixture tempdir");
    let mp3_path = tmp.path().join("fixture.mp3");
    write_sine_mp3(&mp3_path, 1.5, 440.0);

    let lane = MediaLane::for_inputs(vec![mp3_path]);
    let mut metadata = AudiobookMetadata::new();
    metadata.title = Some("MP3 Origin".to_string());
    metadata.artist = Some("Lane Narrator".to_string());

    let output = lane.process(Some(metadata)).await;

    let probe = get_file_list_info(&[&output]).expect("re-probe M4B produced from MP3");
    assert_eq!(probe.valid_count, 1, "output probes as valid audio");
    let drift = (probe.total_duration - 1.5).abs();
    assert!(
        drift < 0.5,
        "duration {} drifted from MP3 source 1.5 by {drift}",
        probe.total_duration
    );

    let reread = read_metadata(&output).expect("re-read tags from artifact");
    assert_eq!(reread.title.as_deref(), Some("MP3 Origin"));
    assert_eq!(reread.artist.as_deref(), Some("Lane Narrator"));
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
        let info = get_file_list_info(std::slice::from_ref(&source)).expect("probe source");
        assert!(
            info.files[0]
                .preservation
                .expect("preservation facts")
                .can_preserve
        );
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::new()),
            None,
            SampleRateConfig::Auto,
            OutputConfig::new(&destination),
            lane.workspace_root(),
        );
        execute_audio_engine(
            AudioExecutionRequest::new(context, info, None, CoverArtPassthroughPolicy::Preserve)
                .with_handling(audiobook_boss_lib::processing::AudioHandling::Preserve),
        )
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
        title: PatchOp::Set("Preserved title".into()),
        cover_art: PatchOp::Set(minimal_jpg_bytes()),
        ..Default::default()
    };
    let info = get_file_list_info(std::slice::from_ref(&source)).expect("probe source");
    assert!(info.files[0].bitrate.unwrap() > 72_000);
    let plan = audiobook_boss_lib::audio::resolve_title_audio(
        &title_audio_request(
            audiobook_boss_lib::processing::AudioHandling::Preserve,
            native_encoder_settings(),
        ),
        &info,
        false,
    )
    .expect("explicit Keep overrides the size-reduction recommendation");
    assert!(plan.settings.is_none());
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        None,
        SampleRateConfig::Auto,
        OutputConfig::new(&destination),
        lane.workspace_root(),
    );
    execute_audio_engine(
        AudioExecutionRequest::new(context, info, None, CoverArtPassthroughPolicy::Preserve)
            .with_handling(plan.handling)
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
    let mp3_info = get_file_list_info(std::slice::from_ref(&mp3_source)).expect("probe MP3");
    let mp3_context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        None,
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
        )
        .with_handling(audiobook_boss_lib::processing::AudioHandling::Preserve)
        .with_metadata_intent(Some(MetadataIntentPatch {
            title: PatchOp::Set("Preserved MP3 title".into()),
            cover_art: PatchOp::Set(minimal_jpg_bytes()),
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
        read_metadata(&mp3_destination).unwrap().cover_art,
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
async fn cancelled_preserve_does_not_publish_or_leave_staging_residue() {
    let tmp = TempDir::new().expect("cancel fixture tempdir");
    let source = tmp.path().join("source.mp3");
    write_sine_mp3(&source, 1.0, 440.0);
    let source_bytes = fs::read(&source).expect("read cancellation source");
    let destination = tmp.path().join("cancelled.mp3");
    let workspace = tmp.path().join("workspace");
    let registry = JobRegistry::new(1);
    let (job_id, _permit) = registry.register_job().await.expect("register job");
    let checker = registry.cancellation_checker(job_id).await;
    registry.cancel_job(job_id).await.expect("cancel job");
    let session = ProcessingSession::from_job_registry(job_id.0, checker);
    let info = get_file_list_info(std::slice::from_ref(&source)).expect("probe source");
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(session),
        None,
        SampleRateConfig::Auto,
        OutputConfig::new(&destination),
        workspace.clone(),
    );
    let error = execute_audio_engine(
        AudioExecutionRequest::new(context, info, None, CoverArtPassthroughPolicy::Preserve)
            .with_handling(audiobook_boss_lib::processing::AudioHandling::Preserve),
    )
    .await
    .expect_err("cancelled preserve should terminate before publication");
    assert!(matches!(error, AppError::Cancellation(_)));
    assert!(!destination.exists());
    assert_eq!(
        fs::read(&source).expect("source remains unchanged"),
        source_bytes
    );
    assert!(!workspace.exists() || fs::read_dir(&workspace).unwrap().next().is_none());
}

#[tokio::test]
async fn mixed_preservation_preflight_applies_encoder_constraints_only_to_encoded_books() {
    use audiobook_boss_lib::commands::audio::preflight_processing_plan;
    use audiobook_boss_lib::processing::{
        AudioHandling::{Encode, Preserve},
        JobType, ProcessPayload,
    };
    let source_lane =
        MediaLane::with_fixtures(&[0.5]).with_sample_rate(SampleRateConfig::Explicit(22_050));
    let source = source_lane.process(None).await;
    let larger = MediaLane::with_fixtures(&[0.5, 0.6]);
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("out");
    fs::create_dir(&output).unwrap();
    let mut paths = Vec::new();
    for name in ["prey1.m4b", "prey2.m4a", "prey3.m4b"] {
        let path = tmp.path().join(name);
        fs::copy(&source, &path).unwrap();
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
            .map(|handling| title_audio_request(handling, faac_encoder_settings()))
            .to_vec(),
        job_type: Some(JobType::Batch),
        output_naming: None,
        collision_policy: None,
        preflight_signature: None,
        supplemental_assets_by_input_id: None,
    };
    let metadata: std::collections::HashMap<_, _> = paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            (
                path.clone(),
                MetadataIntentPatch {
                    title: PatchOp::Set(format!("Library book {i}")),
                    ..Default::default()
                },
            )
        })
        .collect();
    let plan = preflight_processing_plan(payload.clone(), Some(metadata.clone()), None).unwrap();
    assert_eq!(plan.outputs.len(), 5);
    for (index, extension) in ["m4b", "m4b", "m4b", "m4b", "m4b"].iter().enumerate() {
        assert_eq!(
            std::path::Path::new(&plan.outputs[index].resolved_path)
                .extension()
                .unwrap(),
            *extension
        );
        assert!(plan.outputs[index]
            .resolved_path
            .contains(&format!("Library book {index}")));
    }
    let mut missing_settings = payload.clone();
    missing_settings
        .audio_requests
        .iter_mut()
        .for_each(|request| request.settings = None);
    assert!(
        preflight_processing_plan(missing_settings, Some(metadata.clone()), None)
            .unwrap_err()
            .message
            .contains("encoding settings")
    );
    let mut encode_low_rate = payload.clone();
    encode_low_rate.audio_requests[0].intent = audiobook_boss_lib::audio::AudioIntent::Encode;
    assert!(
        preflight_processing_plan(encode_low_rate.clone(), Some(metadata.clone()), None).is_ok()
    );
    encode_low_rate.audio_requests[0].sample_rate = SampleRateConfig::Explicit(22_050);
    assert!(preflight_processing_plan(encode_low_rate, Some(metadata.clone()), None).is_err());
    let mut all_preserve = payload.clone();
    all_preserve.input_files.truncate(3);
    all_preserve.audio_requests.truncate(3);
    for request in &mut all_preserve.audio_requests {
        request.settings = None;
        request.sample_rate = SampleRateConfig::Explicit(1); // irrelevant while passing through
    }
    let original_plan =
        preflight_processing_plan(all_preserve.clone(), Some(metadata.clone()), None).unwrap();
    assert!(
        preflight_processing_plan(all_preserve.clone(), Some(metadata.clone()), Some(1.0)).is_err()
    );
    let mut changed_mode = all_preserve.clone();
    changed_mode.audio_requests = vec![title_audio_request(Encode, native_encoder_settings()); 3];
    let encode_plan =
        preflight_processing_plan(changed_mode, Some(metadata.clone()), None).unwrap();
    assert_ne!(original_plan.plan_signature, encode_plan.plan_signature);
    assert_eq!(
        fs::read_dir(&output).unwrap().count(),
        0,
        "preflight creates no library folders"
    );
    let mut misaligned = all_preserve;
    misaligned.audio_requests.truncate(1);
    assert!(preflight_processing_plan(misaligned, Some(metadata), None)
        .unwrap_err()
        .message
        .contains("align"));
}

/// Apple AAC (AudioToolbox) is the second in-process encoder route and is
/// present on every macOS machine, so it earns a deterministic lane test.
/// macOS-only: `aac_at` does not exist elsewhere, so the lane skips it on
/// Linux/Windows agents rather than failing.
/// External FDK stays out of the normal suite: it needs a user-supplied
/// libfdk_aac FFmpeg, which is environment-dependent by definition.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn apple_aac_encoder_route_produces_valid_m4b_with_metadata() {
    let lane = MediaLane::with_fixtures(&[1.5]).with_encoder(EncoderSettings {
        encoder_type: EncoderType::AacAt,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cvbr,
        channels: ChannelConfig::Mono,
        afterburner: false,
        native_aac_speed: 0,
        faac_profile: audiobook_boss_lib::audio::FaacProfile::Auto,
    });
    let mut metadata = AudiobookMetadata::new();
    metadata.title = Some("Apple AAC Route".to_string());

    let output = lane.process(Some(metadata)).await;

    let probe = get_file_list_info(&[&output]).expect("re-probe aac_at output");
    assert_eq!(probe.valid_count, 1, "aac_at output probes as valid audio");
    let drift = (probe.total_duration - 1.5).abs();
    assert!(
        drift < 0.5,
        "aac_at duration {} drifted from source 1.5 by {drift}",
        probe.total_duration
    );

    let reread = read_metadata(&output).expect("re-read tags from aac_at artifact");
    assert_eq!(reread.title.as_deref(), Some("Apple AAC Route"));
}

/// Writes a stereo 16-bit PCM WAV with a distinct sine per channel.
fn write_stereo_sine_wav(path: &Path, seconds: f64, left_hz: f64, right_hz: f64) {
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

/// Rate-converted merge: the resample path (44.1kHz WAV → 22.05kHz output)
/// must keep truthful duration and the requested output rate. Guards the
/// resampler + tail-flush boundary the same-rate lane never exercises.
#[tokio::test]
async fn rate_converted_merge_keeps_truthful_duration_and_rate() {
    let lane =
        MediaLane::with_fixtures(&[1.5, 1.0]).with_sample_rate(SampleRateConfig::Explicit(22_050));
    let expected_duration = 2.5;

    execute_audio_engine(lane.execution_request(ProcessingSession::new(), None))
        .await
        .expect("rate-converted native processing succeeds");

    let output = lane.output_path();
    let probe = get_file_list_info(&[&output]).expect("re-probe rate-converted M4B");
    assert_eq!(probe.valid_count, 1, "output M4B probes as valid audio");
    assert_eq!(
        probe.files[0].sample_rate,
        Some(22_050),
        "output carries the requested explicit sample rate"
    );
    let drift = (probe.total_duration - expected_duration).abs();
    assert!(
        drift < 0.2,
        "rate-converted duration {} differs from source total {expected_duration} by {drift}",
        probe.total_duration
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

async fn assert_auto_merge_stereo(settings: EncoderSettings) {
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
        let probe = get_file_list_info(&[&output]).expect("re-probe mixed-channel M4B");
        assert_eq!(probe.files[0].channels, Some(2), "Auto preserves stereo");
        let samples = decode_pcm_f32(&output);
        let frames = samples.chunks_exact(2);
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

/// One CUE case: supplied names/start positions survive encoding and final tags.
#[tokio::test]
async fn cue_chapters_survive_mp3_encoding_and_finalization() {
    let tmp = TempDir::new().expect("CUE fixture directory");
    let mp3 = tmp.path().join("book.mp3");
    write_sine_mp3(&mp3, 1.5, 440.0);
    fs::write(tmp.path().join("book.cue"), "FILE \"stale.mp3\" MP3\nTRACK 01 AUDIO\nTITLE \"Opening\"\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nTITLE \"Near end\"\nINDEX 01 00:01:36\n").expect("write CUE");
    // The popover must use the accepted chapter choice, just like preflight.
    let info = get_file_list_info(std::slice::from_ref(&mp3)).unwrap();
    let request = audiobook_boss_lib::audio::TitleAudioRequest {
        format: audiobook_boss_lib::audio::AudiobookFormat::Mp3,
        intent: audiobook_boss_lib::audio::AudioIntent::Auto,
        settings: None,
        sample_rate: SampleRateConfig::Auto,
    };
    let paths = vec![info.files[0].path.to_str().unwrap().to_owned()];
    let preview = audiobook_boss_lib::commands::audio::preview_title_audio;
    assert!(preview(paths.clone(), request.clone(), None).await.is_err());
    let mut ignored = info.files[0].chapter_plan.clone().unwrap();
    ignored.from_cue = false;
    ignored.chapters.clear();
    let plan = preview(
        paths.clone(),
        request,
        Some(std::collections::HashMap::from([(
            paths[0].clone(),
            ignored,
        )])),
    )
    .await
    .expect("ignored CUE permits single-file MP3 pass-through");
    assert_eq!(
        plan.handling,
        audiobook_boss_lib::processing::AudioHandling::Preserve
    );
    let lane = MediaLane::for_inputs(vec![mp3]);
    let output = lane
        .process(Some(AudiobookMetadata {
            title: Some("CUE book".into()),
            ..Default::default()
        }))
        .await;
    let chapters = chapters_of(&output);
    assert_eq!(chapters.len(), 2);
    assert_eq!(chapters[0].0.as_deref(), Some("Opening"));
    assert_eq!(chapters[0].1, 0);
    assert_eq!(chapters[1].0.as_deref(), Some("Near end"));
    assert_eq!(chapters[1].1, 1_480);
    assert!(chapters[1].2 > 1_480, "short last chapter remains");
    let (duration, _, _) = timed_text_chapter_track(&output);
    assert!(
        duration > 1.48 && duration < 1.6,
        "chapter track spans the audio: {duration}"
    );
    assert!(lane.residual_workspace_dirs().is_empty());
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
        let read_u32 = |at| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
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

fn faac_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::Faac,
        faac_profile: audiobook_boss_lib::audio::FaacProfile::HeAacV1,
        bitrate_mode: BitrateMode::Abr,
        ..native_encoder_settings()
    }
}

#[tokio::test]
async fn faac_he_merge_preserves_metadata_chapters_and_resampled_channels() {
    for (sample_rate, source_rate, expected_rate) in [
        (SampleRateConfig::Explicit(32000), 44100_u32, 32000),
        (SampleRateConfig::Explicit(44100), 44100, 44100),
        (SampleRateConfig::Explicit(48000), 44100, 48000),
        (SampleRateConfig::Auto, 22050, 32000),
    ] {
        let lane = MediaLane::with_fixtures(&[0.13, 0.17])
            .with_encoder(faac_encoder_settings())
            .with_sample_rate(sample_rate);
        if source_rate != SAMPLE_RATE {
            for path in &lane.inputs {
                let mut bytes = fs::read(path).unwrap();
                bytes[24..28].copy_from_slice(&source_rate.to_le_bytes());
                bytes[28..32].copy_from_slice(&(source_rate * 2).to_le_bytes());
                fs::write(path, bytes).unwrap();
            }
            let plan = audiobook_boss_lib::audio::resolve_title_audio(
                &title_audio_request(
                    audiobook_boss_lib::processing::AudioHandling::Encode,
                    faac_encoder_settings(),
                ),
                &get_file_list_info(&lane.inputs).unwrap(),
                false,
            )
            .expect("FAAC HE Auto plans its required source-rate conversion");
            assert_eq!(plan.sample_rate, expected_rate);
        }
        let mut metadata = AudiobookMetadata::new();
        metadata.title = Some("FAAC Merge".into());
        let output = lane.process(Some(metadata)).await;
        let probe = get_file_list_info(&[&output]).unwrap();
        assert_eq!(probe.valid_count, 1);
        assert_eq!(probe.files[0].sample_rate, Some(expected_rate));
        assert_eq!(
            probe.files[0].channels,
            Some(1),
            "mono HE must not be inferred as PS stereo"
        );
        assert!(
            (probe.total_duration - 0.3 * f64::from(SAMPLE_RATE) / f64::from(source_rate)).abs()
                < 0.002
        );
        let tags = read_metadata(&output).unwrap();
        assert_eq!(tags.title.as_deref(), Some("FAAC Merge"));
        assert_eq!(probe.files[0].chapters.len(), 2);
        assert!(lane.residual_workspace_dirs().is_empty());
    }
}

#[tokio::test]
async fn faac_auto_merge_preserves_distinct_stereo_in_either_input_order() {
    assert_auto_merge_stereo(faac_encoder_settings()).await;
}

#[tokio::test]
async fn faac_preview_omits_chapters_and_rejects_unsupported_rate_without_residue() {
    let lane = MediaLane::with_fixtures(&[6.0, 6.0]).with_encoder(faac_encoder_settings());
    let mut context = lane.context(ProcessingSession::new());
    context.preview = Some(audiobook_boss_lib::processing::PreviewConfig::new(10.0));
    let info = get_file_list_info(&lane.inputs).unwrap();
    execute_audio_engine(AudioExecutionRequest::new(
        context,
        info,
        None,
        CoverArtPassthroughPolicy::Preserve,
    ))
    .await
    .unwrap();
    let probe = get_file_list_info(&[lane.output_path()]).unwrap();
    assert!(
        (probe.total_duration - 10.0).abs() < 0.1,
        "preview duration {}",
        probe.total_duration
    );
    assert!(probe.files[0].chapters.is_empty());
    assert!(lane.residual_workspace_dirs().is_empty());

    let invalid = MediaLane::with_fixtures(&[0.1])
        .with_encoder(faac_encoder_settings())
        .with_sample_rate(SampleRateConfig::Explicit(22050));
    assert!(
        execute_audio_engine(invalid.execution_request(ProcessingSession::new(), None))
            .await
            .is_err()
    );
    assert!(!invalid.output_path().exists());
    assert!(invalid.residual_workspace_dirs().is_empty());
}

/// Apple's HE reader must retain short clips and the tail after final tag writes.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn faac_apple_readback_preserves_short_clip_and_final_tail() {
    for profile in [
        audiobook_boss_lib::audio::FaacProfile::HeAacV1,
        audiobook_boss_lib::audio::FaacProfile::AacLc,
    ] {
        for samples in [257, 4096, 12117] {
            let lane = MediaLane::with_fixtures(&[f64::from(samples) / f64::from(SAMPLE_RATE)])
                .with_encoder(EncoderSettings {
                    faac_profile: profile,
                    ..faac_encoder_settings()
                });
            let source = decode_pcm_f32(&lane.inputs[0]);
            let output = lane.process(None).await;
            let decoded_path = lane.tmp.path().join("apple.wav");
            let converted = Command::new("afconvert")
                .args(["-f", "WAVE", "-d", "LEF32"])
                .arg(&output)
                .arg(&decoded_path)
                .output()
                .unwrap();
            assert!(
                converted.status.success(),
                "{}",
                String::from_utf8_lossy(&converted.stderr)
            );
            let decoded = decode_pcm_f32(&decoded_path);
            // HE's half-rate timeline cannot express an odd full-rate duration.
            // Even HE durations and LC must retain an exact sample count.
            let tolerance = usize::from(
                profile == audiobook_boss_lib::audio::FaacProfile::HeAacV1 && samples % 2 != 0,
            );
            assert!(
                decoded.len().abs_diff(source.len()) <= tolerance,
                "Apple playable sample count: source={} decoded={}",
                source.len(),
                decoded.len()
            );
            let tail = &decoded[decoded.len() - 32..];
            let rms = (tail
                .iter()
                .map(|value| f64::from(*value).powi(2))
                .sum::<f64>()
                / 32.0)
                .sqrt();
            assert!(rms > 0.05, "missing final audio tail: {rms}");
        }
    }
}

/// Re-import must preserve position and the final audio,
/// including clips whose last playable sample needs an extra HE access unit.
#[tokio::test]
async fn faac_reimport_preserves_audio_alignment_and_tail() {
    assert_faac_reimport(
        faac_encoder_settings(),
        "AudioBook Boss FAAC HE-AAC timing-2",
        native_encoder_settings(),
        &[
            (32000, 4096),
            (44100, 12117),
            (48000, 22050),
            (44100, 22050),
        ],
    )
    .await;
}

#[tokio::test]
async fn faac_lc_and_auto_vbr_reimport_preserve_audio_interval() {
    use audiobook_boss_lib::audio::FaacProfile;
    for (profile, mode, tool) in [
        (
            FaacProfile::AacLc,
            BitrateMode::Abr,
            "AudioBook Boss FAAC AAC-LC",
        ),
        (
            FaacProfile::Auto,
            BitrateMode::Vbr(100),
            "AudioBook Boss FAAC AAC-LC",
        ),
        (
            FaacProfile::Auto,
            BitrateMode::Vbr(50),
            "AudioBook Boss FAAC HE-AAC timing-2",
        ),
        (
            FaacProfile::HeAacV1,
            BitrateMode::Vbr(100),
            "AudioBook Boss FAAC HE-AAC timing-2",
        ),
    ] {
        let input = EncoderSettings {
            faac_profile: profile,
            bitrate_mode: mode,
            ..faac_encoder_settings()
        };
        let cases: &[(u32, u32)] = if profile == FaacProfile::AacLc {
            &[(22050, 8192), (44100, 12117)]
        } else if profile == FaacProfile::HeAacV1 {
            &[(44100, 12117), (48000, 22050)]
        } else {
            &[(44100, 12117), (48000, 22050), (96000, 32768)]
        };
        assert_faac_reimport(input, tool, native_encoder_settings(), cases).await;
    }
}

#[tokio::test]
#[ignore = "requires external FFmpeg with libfdk_aac; run explicitly on an FDK host"]
async fn faac_reimport_through_external_fdk_preserves_audio_interval() {
    for channels in [ChannelConfig::Mono, ChannelConfig::Stereo] {
        assert_faac_reimport(
            faac_encoder_settings(),
            "AudioBook Boss FAAC HE-AAC timing-2",
            EncoderSettings {
                encoder_type: EncoderType::FdkHeAac,
                bitrate_mode: BitrateMode::Vbr(3),
                channels,
                ..native_encoder_settings()
            },
            &[(32000, 16000), (44100, 22050), (48000, 24000)],
        )
        .await;
    }
}

async fn assert_faac_reimport(
    input_settings: EncoderSettings,
    expected_tool: &str,
    output_settings: EncoderSettings,
    cases: &[(u32, u32)],
) {
    for &(rate, samples) in cases {
        let lane = MediaLane::with_fixtures(&[f64::from(samples) / f64::from(SAMPLE_RATE)])
            .with_encoder(input_settings.clone());
        let mut wav = fs::read(&lane.inputs[0]).unwrap();
        wav[24..28].copy_from_slice(&rate.to_le_bytes());
        wav[28..32].copy_from_slice(&(rate * 2).to_le_bytes());
        let mut state = 42u32;
        let mut filtered = 0.0_f64;
        for sample in wav[44..].chunks_exact_mut(2) {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = f64::from(state) / f64::from(u32::MAX) - 0.5;
            filtered = filtered * 0.85 + noise * 0.15;
            sample.copy_from_slice(&((filtered * 50000.0) as i16).to_le_bytes());
        }
        fs::write(&lane.inputs[0], wav).unwrap();
        let source = decode_pcm_f32(&lane.inputs[0]);
        let faac_output = lane.process(None).await;
        if rate == 44100 && samples == 22050 {
            save_metadata_intent(
                &faac_output,
                &MetadataIntentPatch {
                    title: PatchOp::Set("Edited FAAC book".into()),
                    ..Default::default()
                },
            )
            .expect("unrelated metadata edit preserves timing provenance");
        }
        assert_ffprobe_tag(&ffprobe_format_tags(&faac_output), "encoder", expected_tool);
        let reimport =
            MediaLane::for_inputs(vec![faac_output]).with_encoder(output_settings.clone());
        let second_output = reimport.process(None).await;
        let expected_channels = if output_settings.channels == ChannelConfig::Stereo {
            2
        } else {
            1
        };
        let inspected = get_file_list_info(std::slice::from_ref(&second_output)).unwrap();
        assert_eq!(
            inspected.files[0].channels,
            Some(expected_channels),
            "declared output channels"
        );
        #[cfg(target_os = "macos")]
        if output_settings.encoder_type == EncoderType::FdkHeAac {
            let apple_path = reimport.tmp.path().join("apple.wav");
            let converted = Command::new("afconvert")
                .args(["-f", "WAVE", "-d", "LEF32"])
                .arg(&second_output)
                .arg(&apple_path)
                .output()
                .unwrap();
            assert!(
                converted.status.success(),
                "{}",
                String::from_utf8_lossy(&converted.stderr)
            );
            let apple = get_file_list_info(std::slice::from_ref(&apple_path)).unwrap();
            assert_eq!(
                apple.files[0].channels,
                Some(expected_channels),
                "Apple output channels"
            );
            assert!(
                decode_pcm_f32(&apple_path)
                    .len()
                    .abs_diff(source.len() * expected_channels as usize)
                    <= expected_channels as usize
            );
        }
        let decoded = decode_pcm_f32(&second_output);
        // A forced-stereo output carries the mono reference in each channel.
        let decoded = if output_settings.channels == ChannelConfig::Stereo {
            decoded
                .chunks_exact(2)
                .map(|frame| frame[0])
                .collect::<Vec<_>>()
        } else {
            decoded
        };
        assert_eq!(
            decoded.len(),
            source.len(),
            "playable count at {rate} Hz / {samples} samples"
        );
        let lag = best_signal_lag(&source, &decoded, 1200);
        assert!(
            lag.abs() <= 2,
            "audio shifted by {lag} samples at {rate} Hz"
        );
        let tail = &decoded[decoded.len() - 512..];
        let energy = tail.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / tail.len() as f64;
        assert!(
            energy.sqrt() > 0.03,
            "missing tail at {rate} Hz / {samples} samples: rms={}",
            energy.sqrt()
        );
    }
}

fn best_signal_lag(reference: &[f32], decoded: &[f32], radius: i32) -> i32 {
    // Compare a fixed interior window, so silence at either edge cannot improve a score.
    let start = radius as usize;
    let end = reference.len().min(decoded.len()) - radius as usize;
    (-radius..=radius)
        .map(|lag| {
            let score = (start..end)
                .map(|i| f64::from(reference[i]) * f64::from(decoded[(i as i32 + lag) as usize]))
                .sum::<f64>();
            (lag, score)
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .unwrap()
        .0
}

fn write_complete_frame_aac(path: &Path, frequency: u32) {
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
    let raw = path.with_extension("aac");
    let status = Command::new(&binary)
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!(
            "sine=frequency={frequency}:sample_rate=44100:duration=0.2"
        ))
        .args(["-c:a", "aac", "-f", "adts"])
        .arg(&raw)
        .status()
        .expect("AAC fixture encoder");
    assert!(status.success());
    let status = Command::new(&binary)
        .args(["-v", "error", "-y", "-i"])
        .arg(&raw)
        .args(["-c:a", "copy"])
        .arg(path)
        .status()
        .expect("AAC fixture muxer");
    assert!(status.success());
}

fn audio_packet_bytes(path: &Path) -> Vec<Vec<u8>> {
    let mut input = ffmpeg_next::format::input(path).unwrap();
    let index = input
        .streams()
        .best(ffmpeg_next::media::Type::Audio)
        .unwrap()
        .index();
    input
        .packets()
        .filter(|(stream, _)| stream.index() == index)
        .map(|(_, packet)| packet.data().unwrap().to_vec())
        .collect()
}

#[tokio::test]
async fn preserved_title_stack_keeps_packet_order_and_writes_one_tagged_chaptered_m4b() {
    use audiobook_boss_lib::commands::audio::preflight_processing_plan;
    use audiobook_boss_lib::processing::{AudioHandling, ProcessPayload};
    let tmp = TempDir::new().unwrap();
    let one = tmp.path().join("first.m4a");
    let two = tmp.path().join("second.m4a");
    write_complete_frame_aac(&one, 440);
    write_complete_frame_aac(&two, 880);
    let originals = [fs::read(&one).unwrap(), fs::read(&two).unwrap()];
    let paths = vec![two.clone(), one.clone()];
    let expected_packets: Vec<_> = paths
        .iter()
        .flat_map(|path| audio_packet_bytes(path))
        .collect();
    let samples: usize = paths.iter().map(|path| decode_pcm_f32(path).len()).sum();
    let output_dir = tmp.path().join("out");
    fs::create_dir(&output_dir).unwrap();
    let payload: ProcessPayload = serde_json::from_value(serde_json::json!({
        "inputFiles": [one], "titleSources": {one.to_str().unwrap(): [{"path": two}, {"path": one}]},
        "outputDir": output_dir, "audioRequests": [title_audio_request(AudioHandling::Preserve, native_encoder_settings())], "jobType": "batch"
    })).unwrap();
    let metadata = std::collections::HashMap::from([(
        one.to_string_lossy().into_owned(),
        MetadataIntentPatch {
            title: PatchOp::Set("One grouped title".into()),
            ..Default::default()
        },
    )]);
    let plan = preflight_processing_plan(payload.clone(), Some(metadata.clone()), None)
        .expect("compatible grouped preflight");
    assert_eq!(plan.outputs.len(), 1);
    assert!(plan.outputs[0].resolved_path.ends_with(".m4b"));
    let mut reversed = payload.clone();
    reversed
        .title_sources
        .as_mut()
        .unwrap()
        .get_mut(one.to_str().unwrap())
        .unwrap()
        .reverse();
    let reordered = preflight_processing_plan(reversed, Some(metadata), None).unwrap();
    assert_ne!(
        plan.plan_signature, reordered.plan_signature,
        "review pins source order"
    );
    let destination = tmp.path().join("joined.m4b");
    let workspace = tmp.path().join("workspace");
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        None,
        SampleRateConfig::Auto,
        OutputConfig::new(&destination),
        workspace.clone(),
    );
    let info = get_file_list_info(&paths).unwrap();
    let title_metadata = AudiobookMetadata {
        title: Some("One grouped title".into()),
        cover_art: Some(minimal_jpg_bytes()),
        ..Default::default()
    };
    execute_audio_engine(
        AudioExecutionRequest::new(
            context,
            info,
            Some(title_metadata),
            CoverArtPassthroughPolicy::Preserve,
        )
        .with_handling(AudioHandling::Preserve),
    )
    .await
    .expect("packet-copy title merge");
    assert_eq!(audio_packet_bytes(&destination), expected_packets);
    assert_eq!(decode_pcm_f32(&destination).len(), samples);
    let tags = read_metadata(&destination).unwrap();
    assert_eq!(tags.title.as_deref(), Some("One grouped title"));
    assert_eq!(tags.cover_art, Some(minimal_jpg_bytes()));
    assert_eq!(
        chapters_of(&destination)
            .iter()
            .map(|chapter| chapter.0.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("second"), Some("first")]
    );
    assert_eq!(fs::read(&one).unwrap(), originals[0]);
    assert_eq!(fs::read(&two).unwrap(), originals[1]);
    assert!(!workspace.exists() || fs::read_dir(&workspace).unwrap().next().is_none());
}

#[tokio::test]
async fn preserved_title_rejects_interior_priming_without_publishing() {
    use audiobook_boss_lib::processing::AudioHandling;
    let source_lane = MediaLane::with_fixtures(&[0.2]);
    let source = source_lane.process(None).await;
    let tmp = TempDir::new().unwrap();
    let copy = tmp.path().join("second.m4b");
    fs::copy(&source, &copy).unwrap();
    let destination = tmp.path().join("must-not-exist.m4b");
    let workspace = tmp.path().join("workspace");
    let info = get_file_list_info(&[source, copy]).unwrap();
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        None,
        SampleRateConfig::Auto,
        OutputConfig::new(&destination),
        workspace.clone(),
    );
    let error = execute_audio_engine(
        AudioExecutionRequest::new(context, info, None, CoverArtPassthroughPolicy::Preserve)
            .with_handling(AudioHandling::Preserve),
    )
    .await
    .expect_err("per-source priming cannot be lost at a join");
    assert!(
        error.to_string().contains("Pass-through is unavailable"),
        "{error}"
    );
    assert!(!destination.exists());
    assert!(!workspace.exists() || fs::read_dir(&workspace).unwrap().next().is_none());
}

#[test]
fn mp3_stack_with_trimmed_boundaries_can_encode_but_cannot_pass_through() {
    use audiobook_boss_lib::commands::audio::preflight_processing_plan;
    use audiobook_boss_lib::processing::{AudioHandling, ProcessPayload};
    let tmp = TempDir::new().expect("MP3 stack workspace");
    let output_dir = tmp.path().join("out");
    fs::create_dir(&output_dir).expect("output directory");
    let first = tmp.path().join("part-one.mp3");
    let second = tmp.path().join("part-two.mp3");
    write_sine_mp3(&first, 0.1, 440.0);
    fs::copy(&first, &second).expect("second MP3 source");
    let payload: ProcessPayload = serde_json::from_value(serde_json::json!({
        "inputFiles": [first],
        "titleSources": {first.to_str().unwrap(): [{"path": second}, {"path": first}]},
        "outputDir": output_dir, "jobType": "batch",
        "audioRequests": [title_audio_request(AudioHandling::Encode, native_encoder_settings())]
    }))
    .expect("MP3 stack request");
    let plan =
        preflight_processing_plan(payload.clone(), None, None).expect("encoding accepts MP3 stack");
    assert_eq!(plan.outputs.len(), 1);
    assert!(plan.outputs[0].resolved_path.ends_with(".m4b"));
    let mut preserved = payload;
    preserved.audio_requests[0].format = audiobook_boss_lib::audio::AudiobookFormat::Mp3;
    preserved.audio_requests[0].intent = audiobook_boss_lib::audio::AudioIntent::Auto;
    preserved.audio_requests[0].settings = None;
    let error = preflight_processing_plan(preserved, None, None)
        .expect_err("interior MP3 priming requires encoding");
    assert!(
        error.message.contains("priming") || error.message.contains("shifted"),
        "{}",
        error.message
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
async fn opus_title_plan_writes_chapters_cover_and_truthful_audio_in_both_containers() {
    use audiobook_boss_lib::audio::{AudioIntent, AudiobookFormat, TitleAudioRequest};
    use audiobook_boss_lib::commands::audio::preflight_processing_plan;
    use audiobook_boss_lib::processing::ProcessPayload;
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
                let bytes = fs::read(path).unwrap();
                let mut bytes = bytes;
                bytes[24..28].copy_from_slice(&source_rate.to_le_bytes());
                bytes[28..32].copy_from_slice(&(source_rate * 2).to_le_bytes());
                fs::write(path, bytes).unwrap();
            }
        }
        let paths = lane.inputs.iter().rev().cloned().collect::<Vec<_>>();
        let anchor = lane.inputs[0].to_string_lossy().into_owned();
        let out = lane.tmp.path().join("out");
        fs::create_dir_all(&out).unwrap();
        let request = TitleAudioRequest {
            format,
            intent: AudioIntent::Auto,
            settings: Some(EncoderSettings {
                encoder_type: EncoderType::Opus,
                bitrate_mode: BitrateMode::VbrTarget,
                channels: ChannelConfig::Auto,
                ..native_encoder_settings()
            }),
            sample_rate: SampleRateConfig::Auto,
        };
        let payload: ProcessPayload = serde_json::from_value(serde_json::json!({
            "inputFiles": [&anchor], "titleSources": {&anchor: paths.iter().map(|path| serde_json::json!({"path": path})).collect::<Vec<_>>()},
            "audioRequests": [request], "outputDir": out, "jobType": "batch"
        })).unwrap();
        let metadata = AudiobookMetadata {
            title: Some("Opus title".into()),
            artist: Some("Test Author".into()),
            cover_art: Some(minimal_jpg_bytes()),
            ..Default::default()
        };
        let plan = preflight_processing_plan(payload, None, None).expect("Opus preflight");
        let audio_plan = &plan.audio_plans[0];
        assert_eq!(audio_plan.sample_rate, input_rate);
        assert_eq!(audio_plan.channels, channels);
        let destination = PathBuf::from(&plan.outputs[0].resolved_path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        assert_eq!(destination.extension().unwrap(), format.extension());
        let info = get_file_list_info(&paths).unwrap();
        let duration = info.total_duration;
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::new()),
            audio_plan.settings.clone(),
            SampleRateConfig::Explicit(audio_plan.sample_rate),
            OutputConfig::new(&destination),
            lane.tmp.path().join("work"),
        );
        execute_audio_engine(
            AudioExecutionRequest::new(
                context,
                info,
                Some(metadata),
                CoverArtPassthroughPolicy::Preserve,
            )
            .with_handling(audio_plan.handling),
        )
        .await
        .expect("Opus title output");
        let audio = get_file_list_info(std::slice::from_ref(&destination)).unwrap();
        assert!(audio.files[0]
            .codec_label
            .as_deref()
            .unwrap()
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
                .chunks_exact(2)
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
        let info = get_file_list_info(std::slice::from_ref(&destination)).unwrap();
        let copy_plan = audiobook_boss_lib::audio::resolve_title_audio(
            &TitleAudioRequest {
                format: other_format,
                intent: AudioIntent::Preserve,
                settings: None,
                sample_rate: SampleRateConfig::Auto,
            },
            &info,
            false,
        )
        .unwrap();
        assert_eq!(
            copy_plan.handling,
            audiobook_boss_lib::processing::AudioHandling::Preserve
        );
        let copied = lane
            .tmp
            .path()
            .join("copy")
            .with_extension(other_format.extension());
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::new()),
            None,
            SampleRateConfig::Auto,
            OutputConfig::new(&copied),
            lane.tmp.path().join("work-copy"),
        );
        execute_audio_engine(
            AudioExecutionRequest::new(context, info, None, CoverArtPassthroughPolicy::Preserve)
                .with_handling(copy_plan.handling)
                .with_metadata_intent(Some(MetadataIntentPatch {
                    title: PatchOp::Set("Retagged Opus".into()),
                    ..Default::default()
                })),
        )
        .await
        .unwrap();
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
        assert_eq!(chapters_of(&copied), chapters);
        let copied_tags = read_metadata(&copied).unwrap();
        assert_eq!(copied_tags.title.as_deref(), Some("Retagged Opus"));
        assert_eq!(copied_tags.cover_art, Some(minimal_jpg_bytes()));

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
        assert_eq!(chapters_of(&aac_output), chapters);
    }
}

#[tokio::test]
async fn opus_remux_keeps_variable_frame_timing_and_trim() {
    use audiobook_boss_lib::processing::AudioHandling;
    let tmp = TempDir::new().unwrap();
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
            .unwrap();
        assert!(
            created.status.success(),
            "{}",
            String::from_utf8_lossy(&created.stderr)
        );
        let source_samples = decode_pcm_f32(&source).len();
        let info = get_file_list_info(std::slice::from_ref(&source)).unwrap();
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::new()),
            None,
            SampleRateConfig::Auto,
            OutputConfig::new(&destination),
            tmp.path().join("work"),
        );
        execute_audio_engine(
            AudioExecutionRequest::new(context, info, None, CoverArtPassthroughPolicy::Preserve)
                .with_handling(AudioHandling::Preserve),
        )
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

fn title_audio_request(
    handling: audiobook_boss_lib::processing::AudioHandling,
    settings: EncoderSettings,
) -> audiobook_boss_lib::audio::TitleAudioRequest {
    use audiobook_boss_lib::audio::{AudioIntent, AudiobookFormat, TitleAudioRequest};
    TitleAudioRequest {
        format: AudiobookFormat::M4b,
        intent: if handling == audiobook_boss_lib::processing::AudioHandling::Preserve {
            AudioIntent::Preserve
        } else {
            AudioIntent::Encode
        },
        settings: Some(settings),
        sample_rate: SampleRateConfig::Auto,
    }
}

#[tokio::test]
async fn compatible_mp3_stack_passes_through_as_one_tagged_chaptered_title() {
    use audiobook_boss_lib::audio::{AudioIntent, AudiobookFormat};
    use audiobook_boss_lib::commands::audio::preflight_processing_plan;
    use audiobook_boss_lib::processing::{AudioHandling, ProcessPayload};
    let tmp = TempDir::new().unwrap();
    let paths: Vec<_> = ["last.mp3", "first.mp3"]
        .map(|name| tmp.path().join(name))
        .to_vec();
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
    for (index, path) in paths.iter().enumerate() {
        assert!(Command::new(&binary)
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!(
                "sine=frequency={}:sample_rate=44100:duration=0.31",
                440 + index * 440
            ))
            .args(["-c:a", "libmp3lame", "-b:a", "96k", "-write_xing", "0"])
            .arg(path)
            .status()
            .unwrap()
            .success());
    }
    let expected_packets: Vec<_> = paths
        .iter()
        .flat_map(|path| audio_packet_bytes(path))
        .collect();
    let original_bytes: Vec<_> = paths.iter().map(|path| fs::read(path).unwrap()).collect();
    let expected_samples: usize = paths.iter().map(|path| decode_pcm_f32(path).len()).sum();
    let anchor = &paths[1];
    let request = audiobook_boss_lib::audio::TitleAudioRequest {
        format: AudiobookFormat::Mp3,
        intent: AudioIntent::Auto,
        settings: None,
        sample_rate: SampleRateConfig::Auto,
    };
    let payload: ProcessPayload = serde_json::from_value(serde_json::json!({
        "inputFiles": [anchor], "titleSources": {anchor.to_str().unwrap(): paths.iter().map(|path| serde_json::json!({"path": path})).collect::<Vec<_>>()},
        "audioRequests": [request], "outputDir": tmp.path(), "jobType": "batch"
    })).unwrap();
    let plan = preflight_processing_plan(payload, None, None).unwrap();
    assert_eq!(plan.audio_plans[0].handling, AudioHandling::Preserve);
    assert!(plan.audio_plans[0].settings.is_none());
    let destination = PathBuf::from(&plan.outputs[0].resolved_path);
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        None,
        SampleRateConfig::Auto,
        OutputConfig::new(&destination),
        tmp.path().join("work"),
    );
    let metadata = AudiobookMetadata {
        title: Some("Joined MP3".into()),
        artist: Some("Author".into()),
        cover_art: Some(minimal_jpg_bytes()),
        ..Default::default()
    };
    execute_audio_engine(
        AudioExecutionRequest::new(
            context,
            get_file_list_info(&paths).unwrap(),
            Some(metadata),
            CoverArtPassthroughPolicy::Preserve,
        )
        .with_handling(plan.audio_plans[0].handling),
    )
    .await
    .unwrap();
    assert_eq!(audio_packet_bytes(&destination), expected_packets);
    assert_eq!(decode_pcm_f32(&destination).len(), expected_samples);
    let metadata = read_metadata(&destination).unwrap();
    assert_eq!(metadata.title.as_deref(), Some("Joined MP3"));
    assert_eq!(metadata.artist.as_deref(), Some("Author"));
    assert_eq!(metadata.cover_art, Some(minimal_jpg_bytes()));
    let chapters = chapters_of(&destination);
    assert_eq!(
        chapters
            .iter()
            .map(|chapter| chapter.0.as_deref())
            .collect::<Vec<_>>(),
        [Some("last"), Some("first")]
    );
    assert!((chapters[1].2 as f64 - expected_samples as f64 / 44.1).abs() <= 2.0);
    for (path, original) in paths.iter().zip(original_bytes) {
        assert_eq!(fs::read(path).unwrap(), original);
    }
}

#[tokio::test]
async fn recommended_audio_plan_reduces_large_aac_but_explicit_keep_never_encodes() {
    use audiobook_boss_lib::audio::{resolve_title_audio, AudioIntent, AudiobookFormat};
    use audiobook_boss_lib::processing::AudioHandling;
    for (bitrate_kbps, expected) in [(48, AudioHandling::Preserve), (128, AudioHandling::Encode)] {
        let lane = MediaLane::with_fixtures(&[1.5]).with_encoder(EncoderSettings {
            bitrate_kbps,
            ..native_encoder_settings()
        });
        let source = lane.process(None).await;
        let info = get_file_list_info(&[&source]).unwrap();
        let bitrate = info.files[0].bitrate.unwrap();
        assert_eq!(bitrate <= 72_000, expected == AudioHandling::Preserve);
        let mut request = title_audio_request(AudioHandling::Encode, native_encoder_settings());
        request.intent = AudioIntent::Auto;
        let plan = resolve_title_audio(&request, &info, false).unwrap();
        assert_eq!(plan.handling, expected);
        assert_eq!(plan.sample_rate, info.files[0].sample_rate.unwrap());
        assert_eq!(u32::from(plan.channels), info.files[0].channels.unwrap());
        request.intent = AudioIntent::Preserve;
        request.settings = None;
        let kept = resolve_title_audio(&request, &info, false).unwrap();
        assert_eq!(kept.handling, AudioHandling::Preserve);
        assert!(kept.settings.is_none());
        request.format = AudiobookFormat::Mp3;
        assert!(resolve_title_audio(&request, &info, false)
            .unwrap_err()
            .to_string()
            .contains("MP3 source audio"));
        assert!(resolve_title_audio(&request, &info, true).is_err());
    }
}
