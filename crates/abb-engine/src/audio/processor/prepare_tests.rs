use super::*;
use std::path::PathBuf;

fn audio_file(name: &str, duration: Option<f64>, is_valid: bool) -> AudioFile {
    AudioFile {
        chapter_plan: None,
        cue_source: None,
        input_id: format!("{name}-input"),
        path: PathBuf::from(name),
        size: Some(1.0),
        duration,
        format: Some("MP3".to_string()),
        bitrate: Some(64),
        sample_rate: Some(44_100),
        channels: Some(2),
        codec_label: Some("AAC".to_string()),
        preservation: None,
        selected_decoder: Some("Native AAC (FFmpeg)".to_string()),
        label: name.to_string(),
        artist: None,
        tag_title: None,
        chapters: Vec::new(),
        is_valid,
        source_removed: false,
        status: crate::audio::TitleStatus::Ready,
        cover: None,
        error: None,
    }
}

#[tokio::test]
async fn cancellation_after_workspace_creation_keeps_cleanup_ownership() {
    use crate::audio::{get_file_list_info, AacDecoder};
    use crate::execution::ProcessingSession;
    use crate::test_cases::media_fixtures::MediaLane;
    let lane = MediaLane::with_fixtures(&[1.0]);
    let files = get_file_list_info(&lane.inputs, AacDecoder::Auto)
        .expect("probe fixture")
        .files;
    let checker =
        crate::execution::CancellationChecker::new(Some(crate::execution::cancelled_token()));
    let context = lane.context(ProcessingSession::with_cancellation(
        uuid::Uuid::new_v4(),
        checker,
    ));
    assert!(prepare(&context, &files).is_err());
    assert_eq!(
        std::fs::read_dir(lane.workspace_root())
            .expect("the workspace root was created")
            .count(),
        0
    );
}

#[test]
fn preview_progress_caps_each_file_at_its_excerpt() {
    let files = vec![
        audio_file("short.mp3", Some(8.0), true),
        audio_file("long.mp3", Some(240.0), true),
    ];

    assert_eq!(progress_total_duration(&files, true), 53.0);
}

#[test]
fn export_progress_sums_the_file_durations() {
    let files = vec![
        audio_file("first.mp3", Some(120.0), true),
        audio_file("second.mp3", Some(240.0), true),
    ];

    assert_eq!(progress_total_duration(&files, false), 360.0);
}
