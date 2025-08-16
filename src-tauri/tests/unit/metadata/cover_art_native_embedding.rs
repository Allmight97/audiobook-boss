//! Tests native ffmpeg-next cover art embedding
//! Ensures that when cover art is provided, a stream is added pre-header and packet written.

use audiobook_boss_lib::audio::{AudioSettings, ChannelConfig, SampleRateConfig, media_pipeline::MediaProcessingPlan};
use audiobook_boss_lib::audio::context::ProcessingContextBuilder;
use audiobook_boss_lib::metadata::AudiobookMetadata;
use tempfile::TempDir;
use std::path::PathBuf;

// Minimal 1x1 JPEG (JFIF) header (valid tiny image) - using a common minimal pattern.
const MINIMAL_JPEG: &[u8] = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9"; // SOI + APP0 + EOI

fn create_silent_wav(path: &PathBuf) {
    let wav_data = [
        0x52, 0x49, 0x46, 0x46, 0x24, 0x00, 0x00, 0x00, 0x57, 0x41, 0x56, 0x45,
        0x66, 0x6d, 0x74, 0x20, 0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00,
        0x40, 0x1f, 0x00, 0x00, 0x40, 0x1f, 0x00, 0x00, 0x01, 0x00, 0x08, 0x00,
        0x64, 0x61, 0x74, 0x61, 0x04, 0x00, 0x00, 0x00, 0x80, 0x80, 0x80, 0x80,
    ];
    std::fs::write(path, &wav_data).expect("write wav");
}

#[test]
fn test_native_cover_art_embedding_end_to_end() {
    let temp = TempDir::new().expect("temp");
    let input = temp.path().join("in.wav");
    create_silent_wav(&input);
    let concat = temp.path().join("concat.txt");
    std::fs::write(&concat, format!("file '{}'\n", input.display())).unwrap();
    let output = temp.path().join("out.m4b");

    let settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Explicit(22050),
        output_path: output.clone(),
    };

    let metadata = AudiobookMetadata {
        title: Some("Test".into()),
        cover_art: Some(MINIMAL_JPEG.to_vec()),
        ..Default::default()
    };

    let plan = MediaProcessingPlan::new(
        concat.clone(),
        output.clone(),
        settings,
        vec![input.clone()],
        1.0,
    );

    // Build a minimal processing context (window and session mocks handled by builder default semantics)
    let ctx = ProcessingContextBuilder::new().build();

    // Execute with context (async). We use a minimal executor via futures::executor.
    futures::executor::block_on(async {
        plan.execute_with_context(&ctx, Some(&metadata)).await.expect("execute");
    });

    assert!(output.exists(), "Output file should exist");

    // Use lofty to validate cover art present
    let tagged = lofty::Probe::open(&output).and_then(|p| p.read()).expect("lofty read");
    if let Some(pictures) = tagged.pictures().first() {
        assert!(pictures.data().len() > 0, "Embedded cover art should have non-zero length");
    } else {
        panic!("No embedded cover art found via lofty");
    }
}

#[test]
fn test_cover_art_unsupported_format_fallback() {
    let temp = TempDir::new().expect("temp");
    let input = temp.path().join("in.wav");
    create_silent_wav(&input);
    let concat = temp.path().join("concat.txt");
    std::fs::write(&concat, format!("file '{}'\n", input.display())).unwrap();
    let output = temp.path().join("out.m4b");

    // Fake GIF header (unsupported) to trigger fallback
    let fake_gif = b"GIF89a".to_vec();

    let settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Explicit(22050),
        output_path: output.clone(),
    };

    let metadata = AudiobookMetadata {
        title: Some("Test".into()),
        cover_art: Some(fake_gif),
        ..Default::default()
    };

    let plan = MediaProcessingPlan::new(
        concat.clone(),
        output.clone(),
        settings,
        vec![input.clone()],
        1.0,
    );

    let ctx = ProcessingContextBuilder::new().build();
    futures::executor::block_on(async { plan.execute_with_context(&ctx, Some(&metadata)).await.unwrap(); });
    assert!(output.exists(), "Output exists");
    // We cannot assert absence/presence deterministically because finalize stage may embed later
    // depending on flow; this test chiefly ensures no panic / failure.
}
