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
        cover: None,
        error: None,
    }
}

#[tokio::test]
async fn cancellation_after_workspace_creation_keeps_cleanup_ownership() {
    use crate::audio::{CleanupGuard, SampleRateConfig};
    use crate::processing::{JobRegistry, OutputConfig, ProcessingSession};
    let registry = JobRegistry::new(1);
    let (job, _permit) = registry.register_job().await.expect("register test job");
    let checker = crate::processing::CancellationChecker::new(Some(std::sync::Arc::new(
        std::sync::atomic::AtomicBool::new(true),
    )));
    let root = tempfile::TempDir::new().expect("create isolated test directory");
    let workspace = root.path().join("sessions");
    let context = ProcessingContext::new_headless_with_workspace_root(
        std::sync::Arc::new(ProcessingSession::with_cancellation(job.0, checker)),
        SampleRateConfig::Auto,
        OutputConfig::new(root.path().join("output.m4b")),
        workspace.clone(),
    );
    let mut cleanup = CleanupGuard::new(context.session.id());
    assert!(prepare_workspace(&context, &[], &mut cleanup).is_err());
    drop(cleanup);
    assert_eq!(
        std::fs::read_dir(workspace)
            .expect("inspect workspace residue")
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

    assert_eq!(progress_total_duration(&files, Some(&PreviewConfig)), 53.0);
}

#[test]
fn export_progress_sums_the_file_durations() {
    let files = vec![
        audio_file("first.mp3", Some(120.0), true),
        audio_file("second.mp3", Some(240.0), true),
    ];

    assert_eq!(progress_total_duration(&files, None), 360.0);
}
