//! Real-media proofs for the copy-or-encode decision: which sources a format
//! can keep unchanged, the reason shown when it cannot, and a source that was
//! replaced after import.

use std::fs;
use std::path::Path;

use abb_engine::audio::{
    execute_audio_engine, get_file_list_info, resolve_title_audio, AacDecoder,
    AudioExecutionRequest, AudioIntent, AudiobookFormat, BitrateMode, ChannelConfig,
    EncoderSettings, EncoderType, OutputAudio, SampleRateConfig, TitleAudioPlan, TitleAudioRequest,
};
use abb_engine::processing::{AudioHandling, OutputConfig, ProcessingContext, ProcessingSession};
use abb_engine::session::{AudioEdit, SessionIntent, TitlePlan};
use abb_engine::CoverArtPassthroughPolicy;
use std::sync::Arc;
use tempfile::TempDir;

use super::integration_session_desk::Desk;
use super::media_fixtures::{ffmpeg, ffprobe, native_encoder_settings};

const NOT_PRESERVABLE: &str = "Source audio is not a supported AAC or Opus container for preservation. Choose encoding settings or keep the sources as separate titles.";
const CHANGED_SINCE_INSPECTION: &str =
    "Invalid input: Audio changed since chapter inspection. Remove and import it again.";

fn cannot_be_kept_in(extension: &str) -> String {
    format!("This audio cannot be kept unchanged in {extension}. Change the output format or choose encoding settings.")
}

fn aac_source(path: &Path, muxer: &str) {
    ffmpeg([
        "-v",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=44100:duration=1",
        "-c:a",
        "aac",
        "-b:a",
        "48k",
        "-f",
        muxer,
        path.to_str().expect("utf8 path"),
    ]);
}

fn opus_source(path: &Path, muxer: &str) {
    ffmpeg([
        "-v",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:duration=1",
        "-c:a",
        "libopus",
        "-b:a",
        "32k",
        "-f",
        muxer,
        path.to_str().expect("utf8 path"),
    ]);
}

fn opus_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::Opus,
        bitrate_mode: BitrateMode::VbrTarget,
        channels: ChannelConfig::Auto,
        ..native_encoder_settings()
    }
}

fn plan_for(
    source: &Path,
    format: AudiobookFormat,
    intent: AudioIntent,
) -> Result<TitleAudioPlan, String> {
    let settings = if format == AudiobookFormat::M4b {
        native_encoder_settings()
    } else {
        opus_settings()
    };
    let request = TitleAudioRequest {
        output: OutputAudio::new(format, settings),
        intent,
        sample_rate: SampleRateConfig::Auto,
    };
    let info = get_file_list_info(&[source], AacDecoder::Auto).expect("inspect fixture");
    resolve_title_audio(&request, &info, false).map_err(|error| error.message())
}

#[test]
fn raw_adts_aac_is_never_copied_into_any_format() {
    let dir = TempDir::new().expect("dir");
    let source = dir.path().join("book.aac");
    aac_source(&source, "adts");

    assert_eq!(
        plan_for(&source, AudiobookFormat::M4b, AudioIntent::Preserve)
            .expect_err("not a container"),
        NOT_PRESERVABLE
    );
    assert_eq!(
        plan_for(&source, AudiobookFormat::M4aOpus, AudioIntent::Preserve)
            .expect_err("AAC is not Opus"),
        cannot_be_kept_in("M4A")
    );
    let info = get_file_list_info(&[&source], AacDecoder::Auto).expect("inspect fixture");
    assert_eq!(
        info.files[0].preservation.map(|p| p.can_preserve),
        Some(false)
    );
}

#[test]
fn aac_in_matroska_is_refused_in_every_format() {
    let dir = TempDir::new().expect("dir");
    let source = dir.path().join("book.mka");
    aac_source(&source, "matroska");

    assert_eq!(
        plan_for(&source, AudiobookFormat::M4b, AudioIntent::Preserve)
            .expect_err("not a container"),
        NOT_PRESERVABLE
    );
    assert_eq!(
        plan_for(&source, AudiobookFormat::MkaOpus, AudioIntent::Preserve)
            .expect_err("AAC is not Opus"),
        cannot_be_kept_in("MKA")
    );
    let auto = plan_for(&source, AudiobookFormat::MkaOpus, AudioIntent::Auto)
        .expect("Auto encodes what it cannot keep");
    assert_eq!(
        auto.reason.as_deref(),
        Some("Encoding is required for the selected MKA output.")
    );
}

#[tokio::test]
async fn opus_in_an_m4b_is_kept_in_both_opus_formats_but_not_in_m4b() {
    let dir = TempDir::new().expect("dir");
    let source = dir.path().join("book.m4b");
    opus_source(&source, "mp4");

    assert_eq!(
        plan_for(&source, AudiobookFormat::M4b, AudioIntent::Preserve)
            .expect_err("Opus is not AAC"),
        cannot_be_kept_in("M4B")
    );
    for format in [AudiobookFormat::M4aOpus, AudiobookFormat::MkaOpus] {
        let plan = plan_for(&source, format, AudioIntent::Preserve).expect("Opus keeps");
        assert_eq!(plan.handling, AudioHandling::Preserve, "{format:?}");
        assert_eq!(plan.reason, None);
    }

    let copy = dir.path().join("book.mka");
    let info = get_file_list_info(&[&source], AacDecoder::Auto).expect("inspect fixture");
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        SampleRateConfig::Auto,
        OutputConfig::new(&copy),
        dir.path().join("work"),
    );
    execute_audio_engine(AudioExecutionRequest::new(
        context,
        info,
        None,
        CoverArtPassthroughPolicy::Preserve,
        AudioHandling::Preserve,
    ))
    .await
    .expect("Opus copies into Matroska");
    let facts = ffprobe([
        "-v",
        "error",
        "-select_streams",
        "a:0",
        "-show_entries",
        "format=format_name:stream=codec_name",
        "-of",
        "default=nw=1",
        copy.to_str().expect("utf8 path"),
    ]);
    let facts = String::from_utf8(facts).expect("utf8");
    assert!(facts.contains("codec_name=opus"), "{facts}");
    assert!(facts.contains("format_name=matroska,webm"), "{facts}");
}

#[tokio::test]
async fn a_source_replaced_after_import_fails_its_plan_before_any_codec_is_read() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/book.m4b"), 1.0)
        .await;
    desk.import(&book).await;
    desk.wait_until("first plan", |d| d.plan() != TitlePlan::Pending)
        .await;

    fs::remove_file(&book).expect("remove source");
    opus_source(&book, "mp4");
    let title_ids: Vec<_> = desk
        .engine
        .session_snapshot()
        .titles
        .expect("titles")
        .files
        .into_iter()
        .map(|file| file.input_id)
        .collect();
    desk.send(SessionIntent::SetTitleAudio {
        title_ids,
        edit: AudioEdit::Intent(AudioIntent::Preserve),
    })
    .await;
    desk.wait_until("replanned", |d| {
        matches!(d.plan(), TitlePlan::Failed { .. })
    })
    .await;

    let TitlePlan::Failed { message, .. } = desk.plan() else {
        panic!("{:?}", desk.plan());
    };
    assert_eq!(message, CHANGED_SINCE_INSPECTION);
}
