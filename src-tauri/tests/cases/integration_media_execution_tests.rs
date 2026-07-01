//! Media-execution lane (issue #341, closeout route: add now).
//!
//! Smallest maintained real-media lane: fixtures are WAV files synthesized at
//! test time (no committed media, no licensing exposure), executed through the
//! in-process ffmpeg-next native path with a headless `ProcessingContext`.
//!
//! These tests prove workflow behavior structural tests cannot:
//! - import → configure → process → decodable M4B with truthful duration
//! - metadata save → re-read tags from the real output artifact
//! - cancellation → terminal error with no artifact and no staging residue
//!
//! Runtime budget: a few seconds of mono sine audio; the whole module must
//! stay under ~10s. If it grows past that, shrink fixtures before widening
//! the budget.

use audiobook_boss_lib::audio::{
    execute_audio_engine, get_file_list_info, AudioExecutionRequest, BitrateMode, ChannelConfig,
    EncoderSettings, EncoderType, SampleRateConfig, ThreadSetting,
};
use audiobook_boss_lib::processing::job_registry::{JobId, JobRegistry};
use audiobook_boss_lib::processing::{OutputConfig, ProcessingContext, ProcessingSession};
use audiobook_boss_lib::{
    read_metadata, save_metadata_intent, AlbumSortPatchOp, AppError, AudiobookMetadata,
    CoverArtPassthroughPolicy, MetadataIntentPatch, PatchOp,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

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
        threads: ThreadSetting::Auto,
        twoloop: true,
    }
}

/// Isolated on-disk lane: fixture inputs, output destination, and a private
/// processing workspace root, all inside one TempDir.
struct MediaLane {
    tmp: TempDir,
    inputs: Vec<PathBuf>,
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
        Self { tmp, inputs }
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
            native_encoder_settings(),
            SampleRateConfig::Auto,
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
            native_encoder_settings(),
        )
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
