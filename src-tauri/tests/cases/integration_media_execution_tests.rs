//! Media-execution lane (issue #341, closeout route: add now).
//!
//! Smallest maintained real-media lane: every fixture is synthesized at test
//! time (no committed media, no licensing exposure) — WAVs in pure Rust, MP3s
//! via the linked FFmpeg's libmp3lame, M4Bs by reusing the engine's own
//! committed output as a second-pass input. Execution runs the in-process
//! ffmpeg-next native path with a headless `ProcessingContext`.
//!
//! These tests prove workflow behavior structural tests cannot:
//! - import → configure → process → decodable M4B with truthful duration
//! - real input formats: WAV, M4B (AAC decode→encode), and MP3
//! - encoder routes: Native AAC and Apple AAC (AudioToolbox). External FDK
//!   is deliberately absent from the normal suite — it needs a user-supplied
//!   libfdk_aac FFmpeg, which is environment-dependent by definition.
//! - metadata save → re-read tags from the real output artifact
//! - cover art: explicit save round-trips byte-identical; source-cover
//!   passthrough survives reprocessing
//! - chapters: synthesized per source on merge, preserved on reprocess
//! - cancellation → terminal error with no artifact and no staging residue
//!
//! Runtime budget: the module must stay under ~10s wall clock (currently ~1s).
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
            self.encoder_settings.clone(),
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

/// Synthesizes a mono sine MP3 at test time via the linked FFmpeg's
/// `libmp3lame` encoder. MP3 cannot be fabricated by remuxing PCM, so this is
/// the smallest honest fixture path. Panics loudly if `libmp3lame` is absent:
/// both supported environments (local brew FFmpeg and the CI runner's brew
/// FFmpeg) ship it, and a silent skip would be false green.
fn write_sine_mp3(path: &Path, seconds: f64, freq_hz: f64) {
    use ffmpeg_next as ff;

    ff::init().expect("ffmpeg init");
    let codec = ff::encoder::find_by_name("libmp3lame")
        .expect("libmp3lame encoder must be available in the linked FFmpeg (brew ffmpeg ships it)")
        .audio()
        .expect("libmp3lame is an audio codec");

    let mut octx = ff::format::output(&path).expect("create mp3 output context");
    let mut stream = octx.add_stream(codec).expect("add mp3 stream");
    let context = ff::codec::context::Context::from_parameters(stream.parameters())
        .expect("encoder context from stream parameters");
    let mut encoder = context.encoder().audio().expect("audio encoder");

    let channel_layout = codec
        .channel_layouts()
        .map(|layouts| layouts.best(1))
        .unwrap_or(ff::ChannelLayout::MONO);
    let sample_format = ff::format::Sample::I16(ff::format::sample::Type::Planar);
    encoder.set_rate(SAMPLE_RATE as i32);
    encoder.set_channel_layout(channel_layout);
    encoder.set_format(sample_format);
    encoder.set_bit_rate(64_000);
    encoder.set_time_base((1, SAMPLE_RATE as i32));
    stream.set_time_base((1, SAMPLE_RATE as i32));

    let mut encoder = encoder.open_as(codec).expect("open libmp3lame encoder");
    stream.set_parameters(&encoder);
    octx.write_header().expect("write mp3 header");
    let out_time_base = octx.stream(0).expect("mp3 stream").time_base();

    let frame_size = encoder.frame_size() as usize;
    assert!(frame_size > 0, "libmp3lame reports a fixed frame size");
    let total_samples = (seconds * f64::from(SAMPLE_RATE)) as usize;
    let mut written = 0usize;
    let receive_packets = |encoder: &mut ff::codec::encoder::Audio,
                           octx: &mut ff::format::context::Output| {
        let mut packet = ff::Packet::empty();
        while encoder.receive_packet(&mut packet).is_ok() {
            packet.set_stream(0);
            packet.rescale_ts(ff::Rational(1, SAMPLE_RATE as i32), out_time_base);
            packet.write_interleaved(octx).expect("write mp3 packet");
        }
    };

    while written < total_samples {
        let count = frame_size.min(total_samples - written);
        let mut frame = ff::frame::Audio::new(sample_format, count, channel_layout);
        frame.set_rate(SAMPLE_RATE);
        frame.set_pts(Some(written as i64));
        {
            let plane = frame.plane_mut::<i16>(0);
            for (offset, sample) in plane.iter_mut().enumerate().take(count) {
                let t = (written + offset) as f64 / f64::from(SAMPLE_RATE);
                *sample = (0.3
                    * (2.0 * std::f64::consts::PI * freq_hz * t).sin()
                    * f64::from(i16::MAX)) as i16;
            }
        }
        encoder.send_frame(&frame).expect("send mp3 frame");
        receive_packets(&mut encoder, &mut octx);
        written += count;
    }

    encoder.send_eof().expect("flush mp3 encoder");
    receive_packets(&mut encoder, &mut octx);
    octx.write_trailer().expect("write mp3 trailer");
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

/// Decodes an artifact and returns per-channel RMS over the whole stream.
/// Used to prove real, non-silent audio reaches every output channel.
fn per_channel_rms(path: &Path) -> Vec<f64> {
    use ffmpeg_next as ff;

    ff::init().expect("ffmpeg init");
    let mut ictx = ff::format::input(path).expect("open artifact for RMS probe");
    let stream = ictx
        .streams()
        .best(ff::media::Type::Audio)
        .expect("artifact has an audio stream");
    let stream_index = stream.index();
    let mut decoder = ff::codec::context::Context::from_parameters(stream.parameters())
        .expect("decoder context from artifact params")
        .decoder()
        .audio()
        .expect("open artifact audio decoder");

    let channels = decoder.channels() as usize;
    let mut sum_squares = vec![0f64; channels];
    let mut sample_counts = vec![0u64; channels];
    let drain = |decoder: &mut ff::codec::decoder::Audio,
                 sum_squares: &mut Vec<f64>,
                 sample_counts: &mut Vec<u64>| {
        let mut frame = ff::frame::Audio::empty();
        while decoder.receive_frame(&mut frame).is_ok() {
            assert_eq!(
                frame.format(),
                ff::format::Sample::F32(ff::format::sample::Type::Planar),
                "RMS probe expects planar f32 decoder output"
            );
            for ch in 0..channels.min(frame.planes()) {
                let plane = frame.plane::<f32>(ch);
                for &v in &plane[..frame.samples()] {
                    sum_squares[ch] += f64::from(v) * f64::from(v);
                }
                sample_counts[ch] += frame.samples() as u64;
            }
        }
    };

    for (si, packet) in ictx.packets() {
        if si.index() != stream_index {
            continue;
        }
        decoder.send_packet(&packet).expect("send artifact packet");
        drain(&mut decoder, &mut sum_squares, &mut sample_counts);
    }
    let _ = decoder.send_eof();
    drain(&mut decoder, &mut sum_squares, &mut sample_counts);

    sum_squares
        .iter()
        .zip(sample_counts.iter())
        .map(|(sq, count)| {
            if *count == 0 {
                0.0
            } else {
                (sq / *count as f64).sqrt()
            }
        })
        .collect()
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

/// Stereo channel preservation: both output channels must carry real audio
/// (RMS well above silence) after decode → resample → encode. Pins the
/// "missing or silent output channels" trap from the audio directives.
#[tokio::test]
async fn stereo_merge_preserves_audio_in_both_channels() {
    let lane = MediaLane::for_inputs(Vec::new());
    let input = lane.tmp.path().join("stereo-fixture.wav");
    write_stereo_sine_wav(&input, 1.5, 440.0, 660.0);
    let lane = MediaLane {
        inputs: vec![input],
        ..lane
    }
    .with_encoder(EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: ChannelConfig::Stereo,
        afterburner: false,
    });

    let output = lane.process(None).await;

    let probe = get_file_list_info(&[&output]).expect("re-probe stereo M4B");
    assert_eq!(probe.files[0].channels, Some(2), "output stays stereo");

    let rms = per_channel_rms(&output);
    assert_eq!(rms.len(), 2, "RMS probe sees two channels");
    for (ch, value) in rms.iter().enumerate() {
        assert!(
            *value > 0.05,
            "channel {ch} RMS {value} indicates missing or silent audio (expected ~0.21 for a 0.3-amplitude sine)"
        );
    }
}
