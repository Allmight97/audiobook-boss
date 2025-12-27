use audiobook_boss_lib::commands::metadata::{read_audio_metadata, save_metadata_to_file};
use audiobook_boss_lib::AudiobookMetadata;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn sample_mp3_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest parent")
        .join("media")
        .join("media_20sec.mp3")
}

#[test]
fn save_metadata_non_mp4_uses_ffmpeg_path() {
    let temp = TempDir::new().expect("temp dir");
    let source = sample_mp3_path();
    assert!(source.exists(), "sample mp3 should exist");
    let target = temp.path().join("metadata-test.mp3");
    std::fs::copy(&source, &target).expect("copy mp3 fixture");

    let metadata = AudiobookMetadata {
        title: Some("Non-MP4 Title".into()),
        artist: Some("Non-MP4 Author".into()),
        ..Default::default()
    };

    save_metadata_to_file(target.to_string_lossy().to_string(), metadata).expect("save metadata");

    let read_back =
        read_audio_metadata(target.to_string_lossy().to_string()).expect("read metadata");
    assert_eq!(read_back.title.as_deref(), Some("Non-MP4 Title"));
    assert_eq!(read_back.artist.as_deref(), Some("Non-MP4 Author"));
}

#[test]
fn mp4ameta_error_falls_back_to_ffmpeg() {
    let temp = TempDir::new().expect("temp dir");
    let source = sample_mp3_path();
    assert!(source.exists(), "sample mp3 should exist");
    let mp3_path = temp.path().join("fallback.mp3");
    std::fs::copy(&source, &mp3_path).expect("copy mp3 fixture");

    let metadata = AudiobookMetadata {
        title: Some("Fallback Title".into()),
        artist: Some("Fallback Author".into()),
        ..Default::default()
    };

    save_metadata_to_file(mp3_path.to_string_lossy().to_string(), metadata).expect("save metadata");

    // Rename to .m4b to force mp4ameta path, then ensure ffmpeg fallback returns data.
    let spoofed = temp.path().join("fallback.m4b");
    std::fs::rename(&mp3_path, &spoofed).expect("rename mp3 to m4b");

    let read_back =
        read_audio_metadata(spoofed.to_string_lossy().to_string()).expect("read metadata");
    assert_eq!(read_back.title.as_deref(), Some("Fallback Title"));
    assert_eq!(read_back.artist.as_deref(), Some("Fallback Author"));
}
