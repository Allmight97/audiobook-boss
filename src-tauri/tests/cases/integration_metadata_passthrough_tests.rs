//! Integration tests for metadata passthrough helpers.

use audiobook_boss_lib::audio::AudioFile;
use audiobook_boss_lib::{add_chapters_to_output, extract_passthrough_metadata, ChapterSpec};
use ffmpeg_next as ff;
use tempfile::TempDir;

#[test]
fn add_chapters_to_output_accepts_ms_chapters() {
    ffmpeg_next::init().expect("ffmpeg init");

    let temp_dir = TempDir::new().expect("temp dir");
    let output = temp_dir.path().join("chapters.m4b");
    let mut octx = ff::format::output(&output).expect("create output context");

    let chapters = vec![
        ChapterSpec {
            title: Some("One".into()),
            start_ms: 0,
            end_ms: 1000,
        },
        ChapterSpec {
            title: Some("Two".into()),
            start_ms: 1000,
            end_ms: 2000,
        },
    ];

    let added = add_chapters_to_output(&mut octx, &chapters).expect("add chapters");
    assert_eq!(added, 2);
}

#[test]
fn extract_passthrough_metadata_synthesizes_chapters() {
    let mut first = AudioFile::new(std::path::PathBuf::from("01_intro.mp3"));
    first.duration = Some(1.0);
    first.is_valid = true;

    let mut second = AudioFile::new(std::path::PathBuf::from("02_chapter.mp3"));
    second.duration = Some(2.5);
    second.is_valid = true;

    let passthrough = extract_passthrough_metadata(&[first, second]);
    assert_eq!(passthrough.chapters.len(), 2);
    assert_eq!(passthrough.chapters[0].title.as_deref(), Some("01_intro"));
    assert_eq!(passthrough.chapters[0].start_ms, 0);
    assert_eq!(passthrough.chapters[0].end_ms, 1000);
    assert_eq!(passthrough.chapters[1].title.as_deref(), Some("02_chapter"));
    assert_eq!(passthrough.chapters[1].start_ms, 1000);
    assert_eq!(passthrough.chapters[1].end_ms, 3500);
}
