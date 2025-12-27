//! Tests native ffmpeg-next cover art embedding
//! Ensures that when cover art is provided, a stream is added pre-header and packet written.

use audiobook_boss_lib::audio::settings_encoder::{
    BitrateMode, ChannelConfig as EncoderChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};
use audiobook_boss_lib::audio::{
    context::ProcessingContext, session::ProcessingSession, MediaProcessingPlan, OutputConfig,
    SampleRateConfig,
};
use audiobook_boss_lib::commands::read_audio_metadata;
use audiobook_boss_lib::metadata::AudiobookMetadata;
use ffmpeg_next as ff;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{test::mock_app, WebviewUrl, WebviewWindowBuilder};
use tempfile::TempDir;

// Minimal 1x1 JPEG (JFIF) header (valid tiny image) - using a common minimal pattern.
const MINIMAL_JPEG: &[u8] =
    b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9"; // SOI + APP0 + EOI

fn create_silent_wav(path: &PathBuf) {
    let wav_data = [
        0x52, 0x49, 0x46, 0x46, 0x24, 0x00, 0x00, 0x00, 0x57, 0x41, 0x56, 0x45, 0x66, 0x6d, 0x74,
        0x20, 0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x40, 0x1f, 0x00, 0x00, 0x40, 0x1f,
        0x00, 0x00, 0x01, 0x00, 0x08, 0x00, 0x64, 0x61, 0x74, 0x61, 0x04, 0x00, 0x00, 0x00, 0x80,
        0x80, 0x80, 0x80,
    ];
    std::fs::write(path, &wav_data).expect("write wav");
}

#[test]
fn test_native_cover_art_embedding_end_to_end() {
    ff::init().expect("ffmpeg init");
    let temp = TempDir::new().expect("temp");
    let input = temp.path().join("in.wav");
    create_silent_wav(&input);
    let output = temp.path().join("out.m4b");

    let encoder_settings = EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Mono,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
    };
    let sample_rate = SampleRateConfig::Explicit(22050);

    let metadata = AudiobookMetadata {
        title: Some("Test".into()),
        cover_art: Some(MINIMAL_JPEG.to_vec()),
        ..Default::default()
    };

    let plan = MediaProcessingPlan::new(
        output.clone(),
        encoder_settings.clone(),
        sample_rate.clone(),
        encoder_settings.channels.forced_channels().unwrap_or(1),
        vec![input.clone()],
        1.0,
    );

    // Build a minimal processing context backed by a mock window/session
    let app = mock_app();
    let webview =
        WebviewWindowBuilder::new(&app, "cover-art-test", WebviewUrl::App("index.html".into()))
            .build()
            .expect("create mock webview window");
    let window = webview.as_ref().window();
    let session = Arc::new(ProcessingSession::new());
    let output_config = OutputConfig::new(output.clone());
    let ctx = ProcessingContext::new(
        window,
        session,
        encoder_settings.clone(),
        sample_rate.clone(),
        output_config,
    );

    // Execute with context (async). We use a minimal executor via futures::executor.
    futures::executor::block_on(async {
        let _ = plan
            .execute_with_context(&ctx, Some(&metadata), None, None)
            .await
            .expect("execute");
    });

    assert!(output.exists(), "Output file should exist");

    let ictx = ff::format::input(&output).expect("open output");
    let has_cover = ictx.streams().any(|s| {
        s.disposition()
            .contains(ff::format::stream::Disposition::ATTACHED_PIC)
    });
    assert!(
        has_cover,
        "Embedded cover art should result in attached_pic stream"
    );

    let read_back = read_audio_metadata(output.to_string_lossy().to_string())
        .expect("read metadata");
    let cover_bytes = read_back.cover_art.unwrap_or_default();
    assert!(
        !cover_bytes.is_empty(),
        "read metadata should return cover art bytes"
    );
}

#[test]
fn test_cover_art_unsupported_format_fallback() {
    ff::init().expect("ffmpeg init");
    let temp = TempDir::new().expect("temp");
    let input = temp.path().join("in.wav");
    create_silent_wav(&input);
    // No concat file in ffmpeg-next engine
    let output = temp.path().join("out.m4b");

    // Fake GIF header (unsupported) to trigger fallback
    let fake_gif = b"GIF89a".to_vec();

    let encoder_settings = EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Mono,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
    };
    let sample_rate = SampleRateConfig::Explicit(22050);

    let metadata = AudiobookMetadata {
        title: Some("Test".into()),
        cover_art: Some(fake_gif),
        ..Default::default()
    };

    let plan = MediaProcessingPlan::new(
        output.clone(),
        encoder_settings.clone(),
        sample_rate.clone(),
        encoder_settings.channels.forced_channels().unwrap_or(1),
        vec![input.clone()],
        1.0,
    );

    let app = mock_app();
    let webview = WebviewWindowBuilder::new(
        &app,
        "cover-art-fallback",
        WebviewUrl::App("index.html".into()),
    )
    .build()
    .expect("create mock webview window");
    let window = webview.as_ref().window();
    let session = Arc::new(ProcessingSession::new());
    let output_config = OutputConfig::new(output.clone());
    let ctx = ProcessingContext::new(
        window,
        session,
        encoder_settings.clone(),
        sample_rate.clone(),
        output_config,
    );
    futures::executor::block_on(async {
        let _ = plan
            .execute_with_context(&ctx, Some(&metadata), None, None)
            .await
            .expect("execute with unsupported cover art should not fail");
    });
    assert!(output.exists(), "Output exists");

    let ictx = ff::format::input(&output).expect("open output");
    let has_cover = ictx.streams().any(|s| {
        s.disposition()
            .contains(ff::format::stream::Disposition::ATTACHED_PIC)
    });
    assert!(
        !has_cover,
        "Unsupported cover art should be skipped without embedding"
    );
}
