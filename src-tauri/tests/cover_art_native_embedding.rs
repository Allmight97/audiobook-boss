//! Tests native ffmpeg-next cover art embedding (placeholder).
//! Currently skipped: native cover art embedding path is disabled post nuclear cleanup.
//! This file is retained as a scaffold for future implementation.

use audiobook_boss_lib::AudiobookMetadata;
use ffmpeg_next as ff;
use tempfile::TempDir;
use std::path::PathBuf;
use lofty::probe::Probe;
use lofty::file::TaggedFileExt;

// Minimal 1x1 JPEG (JFIF) header (valid tiny image) - using a common minimal pattern.
const MINIMAL_JPEG: &[u8] = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9"; // SOI + APP0 + EOI

#[allow(dead_code)] // Helper retained for future activation when native cover art embedding is implemented
fn create_silent_wav(path: &PathBuf) {
    let wav_data = [
        0x52, 0x49, 0x46, 0x46, 0x24, 0x00, 0x00, 0x00, 0x57, 0x41, 0x56, 0x45,
        0x66, 0x6d, 0x74, 0x20, 0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00,
        0x40, 0x1f, 0x00, 0x00, 0x40, 0x1f, 0x00, 0x00, 0x01, 0x00, 0x08, 0x00,
        0x64, 0x61, 0x74, 0x61, 0x04, 0x00, 0x00, 0x00, 0x80, 0x80, 0x80, 0x80,
    ];
    std::fs::write(path, &wav_data).expect("write wav");
}

#[ignore]
#[test]
fn test_native_cover_art_embedding_end_to_end_placeholder() {
    let temp = TempDir::new().expect("temp");
    let output = temp.path().join("out.m4b");

    ff::init().expect("ffmpeg init");

    // Create output context directly
    let mut octx = ff::format::output(&output).expect("create output");

    let metadata = AudiobookMetadata {
        title: Some("Test".into()),
        cover_art: Some(MINIMAL_JPEG.to_vec()),
        ..Default::default()
    };

    // Set container metadata
    audiobook_boss_lib::ffmpeg_set_container_metadata(&mut octx, &metadata).expect("set metadata");

    // Pre-header: add cover art stream
    let cover_info = audiobook_boss_lib::ffmpeg_add_cover_art_stream_pre_header(&mut octx, metadata.cover_art.as_ref().unwrap());
    assert!(cover_info.is_some(), "Cover art stream should be added");
    let (cover_stream_index, cover_format) = cover_info.unwrap();

    // Add a dummy audio stream so container is valid (parameters may remain default)
    let codec = ff::encoder::find(ff::codec::Id::AAC).expect("aac codec");
    let _stream = octx.add_stream(codec).expect("add audio stream");

    octx.write_header().expect("write header");

    // Post-header: write cover art packet
    audiobook_boss_lib::ffmpeg_write_cover_art_packet_post_header(&mut octx, cover_stream_index, metadata.cover_art.as_ref().unwrap(), cover_format);

    octx.write_trailer().expect("write trailer");

    assert!(output.exists(), "Output file should exist");
    let tagged = Probe::open(&output).and_then(|p| p.read()).expect("lofty read");
    let primary = tagged.primary_tag().or_else(|| tagged.first_tag());
    if let Some(tag) = primary {
        let pics = tag.pictures();
        assert!(!pics.is_empty(), "Should have at least one picture");
        assert!(pics.first().unwrap().data().len() > 0, "Cover art bytes non-zero");
    } else {
        panic!("No tag found in output");
    }
}

#[ignore]
#[test]
fn test_cover_art_unsupported_format_fallback_placeholder() {
    let temp = TempDir::new().expect("temp");
    let output = temp.path().join("out.m4b");
    ff::init().expect("ffmpeg init");
    let mut octx = ff::format::output(&output).expect("create output");
    let fake_gif = b"GIF89a".to_vec();
    let metadata = AudiobookMetadata { title: Some("Test".into()), cover_art: Some(fake_gif), ..Default::default() };
    audiobook_boss_lib::ffmpeg_set_container_metadata(&mut octx, &metadata).expect("set metadata");
    // Attempt to add unsupported cover art -> should return None
    let cover_info = audiobook_boss_lib::ffmpeg_add_cover_art_stream_pre_header(&mut octx, metadata.cover_art.as_ref().unwrap());
    assert!(cover_info.is_none(), "Unsupported format should not add stream");
    // Add audio stream anyway to make a valid container
    let codec = ff::encoder::find(ff::codec::Id::AAC).expect("aac codec");
    let _stream = octx.add_stream(codec).expect("add audio stream");
    octx.write_header().expect("header");
    octx.write_trailer().expect("trailer");
    assert!(output.exists(), "Output exists");
}
