#![cfg(feature = "safe-ffmpeg")]

use std::path::PathBuf;
use tempfile::TempDir;

use audiobook_boss_lib::audio::{AudioSettings, ChannelConfig, SampleRateConfig};
use audiobook_boss_lib::audio::media_pipeline::MediaProcessingPlan;
use audiobook_boss_lib::audio::context::ProgressContextBuilder;

const TEST_MEDIA_FILE: &str = "../media/01 - Introduction.mp3";

fn ensure_media() -> Option<PathBuf> {
    let p = PathBuf::from(TEST_MEDIA_FILE);
    if p.exists() && p.is_file() { Some(p) } else { None }
}

#[tokio::test]
async fn test_ffmpegnext_happy_path_single_input() {
    let Some(media) = ensure_media() else {
        eprintln!("Skipping ffmpegnext test - media missing: {TEST_MEDIA_FILE}");
        return;
    };

    let tmp = TempDir::new().expect("create temp dir");
    let out = tmp.path().join("out.m4b");
    let settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: out.clone(),
    };

    let files = vec![media.clone()];
    let plan = MediaProcessingPlan::new(
        PathBuf::from("/dev/null"), // unused by ffmpegnext
        out.clone(),
        settings.clone(),
        files.clone(),
        60.0,
    );

    let (_app, window) = tauri::test::mock_builder().build();
    let session = std::sync::Arc::new(audiobook_boss_lib::audio::session::ProcessingSession::new());
    let context = audiobook_boss_lib::audio::context::ProcessingContext::new(window, session, settings);

    let processor = audiobook_boss_lib::audio::media_pipeline::FfmpegNextProcessor;
    let result = processor.execute(&plan, &context).await;
    assert!(result.is_ok(), "ffmpegnext execution should succeed: {:?}", result.err());
    assert!(out.exists() && out.is_file(), "output file should exist");
}

#[tokio::test]
async fn test_ffmpegnext_error_for_missing_input() {
    let tmp = TempDir::new().expect("create temp dir");
    let out = tmp.path().join("out.m4b");
    let settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Explicit(22050),
        output_path: out.clone(),
    };

    let files = vec![PathBuf::from("/definitely/not/found.mp3")];
    let plan = MediaProcessingPlan::new(
        PathBuf::from("/dev/null"),
        out.clone(),
        settings.clone(),
        files,
        10.0,
    );

    let (_app, window) = tauri::test::mock_builder().build();
    let session = std::sync::Arc::new(audiobook_boss_lib::audio::session::ProcessingSession::new());
    let context = audiobook_boss_lib::audio::context::ProcessingContext::new(window, session, settings);

    let processor = audiobook_boss_lib::audio::media_pipeline::FfmpegNextProcessor;
    let result = processor.execute(&plan, &context).await;
    assert!(result.is_err(), "missing input should error");
}

#[test]
fn test_progress_context_builder_usage() {
    // Minimal usage of ProgressContextBuilder to ensure it remains exercised in feature-on builds
    let ctx = ProgressContextBuilder::new(audiobook_boss_lib::audio::ProcessingStage::Analyzing)
        .progress(5.0)
        .message("testing")
        .file_progress(0, 1)
        .eta(10.0)
        .build();
    assert_eq!(ctx.progress, 5.0);
}
