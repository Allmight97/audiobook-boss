use super::{sanitize_chapter_title, ChapterMarker, PreviewState};
use crate::audio::processor::frame_pipeline::PreviewAction;

#[test]
fn sanitize_chapter_title_removes_extension() {
    assert_eq!(sanitize_chapter_title("Chapter 01.mp3"), "Chapter 01");
    assert_eq!(sanitize_chapter_title("file.m4a"), "file");
    assert_eq!(sanitize_chapter_title("audio.flac"), "audio");
}

#[test]
fn sanitize_chapter_title_handles_no_extension() {
    assert_eq!(sanitize_chapter_title("Chapter 01"), "Chapter 01");
}

#[test]
fn sanitize_chapter_title_replaces_special_chars() {
    assert_eq!(sanitize_chapter_title("chapter=1.mp3"), "chapter_1");
    assert_eq!(sanitize_chapter_title("chapter[1].mp3"), "chapter_1_");
    assert_eq!(sanitize_chapter_title("chapter#1.mp3"), "chapter_1");
    assert_eq!(sanitize_chapter_title("ch;1.mp3"), "ch_1");
    assert_eq!(sanitize_chapter_title("ch\\1.mp3"), "ch_1");
}

#[test]
fn sanitize_chapter_title_preserves_unicode() {
    assert_eq!(
        sanitize_chapter_title("\u{7B2C}\u{4E00}\u{7AE0}.mp3"),
        "\u{7B2C}\u{4E00}\u{7AE0}"
    );
    assert_eq!(
        sanitize_chapter_title("\u{041A}\u{0430}\u{043F}\u{0438}\u{0442}\u{0443}\u{043B} 1.mp3"),
        "\u{041A}\u{0430}\u{043F}\u{0438}\u{0442}\u{0443}\u{043B} 1"
    );
}

#[test]
fn sanitize_chapter_title_handles_multiple_dots() {
    assert_eq!(sanitize_chapter_title("01.Chapter.mp3"), "01.Chapter");
    assert_eq!(
        sanitize_chapter_title("Track.01.Audio.m4a"),
        "Track.01.Audio"
    );
}

#[test]
fn sanitize_chapter_title_handles_full_path() {
    assert_eq!(
        sanitize_chapter_title("/path/to/Chapter 01.mp3"),
        "Chapter 01"
    );
}

#[test]
fn preview_state_new_initializes_correctly() {
    let state = PreviewState::new(5, 10.0);
    assert_eq!(state.file_count, 5);
    assert!((state.per_file_seconds - 10.0).abs() < 0.001);
    assert_eq!(state.current_file_index, 0);
    assert_eq!(state.current_file_start_pts, 0);
    assert_eq!(state.current_file_elapsed_samples, 0);
    assert!(state.chapter_markers.is_empty());
}

#[test]
fn preview_state_start_new_file_resets_counters() {
    let mut state = PreviewState::new(5, 10.0);
    state.current_file_elapsed_samples = 48_000;
    state.start_new_file(2, "Track03.mp3", 144_000);

    assert_eq!(state.current_file_index, 2);
    assert_eq!(state.current_file_name, "Track03.mp3");
    assert_eq!(state.current_file_start_pts, 144_000);
    assert_eq!(state.current_file_elapsed_samples, 0);
}

#[test]
fn preview_state_all_files_complete() {
    let mut state = PreviewState::new(3, 10.0);
    assert!(!state.all_files_complete());

    state.current_file_index = 1;
    assert!(!state.all_files_complete());

    state.current_file_index = 2;
    assert!(state.all_files_complete());
}

#[test]
fn preview_state_record_chapter() {
    let mut state = PreviewState::new(3, 10.0);
    state.current_file_name = "Chapter 01.mp3".to_string();
    state.current_file_start_pts = 0;

    state.record_chapter(48_000, 48_000);

    assert_eq!(state.chapter_markers.len(), 1);
    assert_eq!(state.chapter_markers[0].start_ms, 0);
    assert_eq!(state.chapter_markers[0].end_ms, 1000);
    assert_eq!(state.chapter_markers[0].title, "Chapter 01");
}

#[test]
fn preview_action_enum_values() {
    assert_eq!(PreviewAction::Continue, PreviewAction::Continue);
    assert_ne!(PreviewAction::Continue, PreviewAction::NextFile);
    assert_ne!(PreviewAction::NextFile, PreviewAction::StopAll);
}

#[test]
fn chapter_marker_struct() {
    let marker = ChapterMarker {
        start_ms: 0,
        end_ms: 5000,
        title: "Intro".to_string(),
    };
    assert_eq!(marker.start_ms, 0);
    assert_eq!(marker.end_ms, 5000);
    assert_eq!(marker.title, "Intro");
}
