use super::*;
use crate::processing::{JobRegistry, OutputConfig, ProcessingSession};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

fn inspect_source_fingerprint(path: &std::path::Path) -> String {
    crate::metadata::inspect_chapter_source(path, 1_000, false, &mut Vec::new())
        .expect("inspect source identity")
        .0
        .source_fingerprint
}

#[test]
fn copying_title_sources_advances_one_shared_progress_interval() {
    let tmp = tempfile::tempdir().expect("copy workspace");
    let source = tmp.path().join("source.mp3");
    std::fs::write(&source, vec![7_u8; 2 * 1024 * 1024]).expect("write source");
    let fingerprint = inspect_source_fingerprint(&source);
    let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
    let events = observed.clone();
    let mut context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::new()),
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(tmp.path().join("out.m4b")),
    );
    context.progress_listener = Some(Arc::new(move |event| {
        events
            .lock()
            .expect("capture progress")
            .push(event.percentage)
    }));
    copy_with_cancellation(
        &source,
        &fingerprint,
        &tmp.path().join("one.mp3"),
        &context,
        0.0..0.25,
    )
    .expect("copy source");
    copy_with_cancellation(
        &source,
        &fingerprint,
        &tmp.path().join("two.mp3"),
        &context,
        0.25..0.5,
    )
    .expect("copy source");
    let progress = observed.lock().expect("read progress");
    assert!(
        progress.len() >= 4 && progress.windows(2).all(|pair| pair[0] <= pair[1]),
        "{progress:?}"
    );
    assert!(
        progress.last().expect("copy reports progress")
            < &crate::processing::converting_percentage(1.0),
        "joining must retain its own progress interval"
    );
}

#[cfg(unix)]
#[test]
fn preserve_copy_rejects_a_source_replaced_with_a_symlink_after_inspection() {
    let tmp = tempfile::tempdir().expect("copy workspace");
    let source = tmp.path().join("source.mp3");
    let replacement = tmp.path().join("replacement.mp3");
    let staged = tmp.path().join("staged.mp3");
    std::fs::write(&source, b"inspected source").expect("write source");
    let inspected_path = crate::audio::validate_input_audio_path(&source)
        .expect("source accepted before scheduler wait");
    let source_fingerprint = inspect_source_fingerprint(&inspected_path);
    std::fs::write(&replacement, b"replacement audio").expect("write replacement");
    std::fs::remove_file(&source).expect("remove old source");
    std::os::unix::fs::symlink(&replacement, &source).expect("replace source with symlink");
    let context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::new()),
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(tmp.path().join("final.mp3")),
    );
    let error = copy_with_cancellation(
        &inspected_path,
        &source_fingerprint,
        &staged,
        &context,
        0.0..1.0,
    )
    .expect_err("queued replacement must be rejected before copying");
    assert!(matches!(error, AppError::InvalidInput(message) if message.contains("Symlinks")));
    assert!(!staged.exists());
    assert_eq!(
        std::fs::read(replacement).expect("unchanged replacement"),
        b"replacement audio"
    );
}

#[test]
fn preserve_copy_rejects_a_regular_source_replaced_after_inspection() {
    let tmp = tempfile::tempdir().expect("copy workspace");
    let source = tmp.path().join("source.mp3");
    let replacement = tmp.path().join("replacement.mp3");
    let staged = tmp.path().join("staged.mp3");
    std::fs::write(&source, b"inspected source").expect("write source");
    let inspected_path = crate::audio::validate_input_audio_path(&source)
        .expect("source accepted before scheduler wait");
    let source_fingerprint = inspect_source_fingerprint(&inspected_path);
    std::fs::write(&replacement, b"different replacement audio").expect("write replacement");
    std::fs::remove_file(&source).expect("remove old source");
    crate::test_cases::move_test_file(&replacement, &source);
    let context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::new()),
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(tmp.path().join("final.mp3")),
    );

    let error = copy_with_cancellation(
        &inspected_path,
        &source_fingerprint,
        &staged,
        &context,
        0.0..1.0,
    )
    .expect_err("queued regular-file replacement must be rejected before copying");

    assert!(matches!(error, AppError::InvalidInput(message) if message.contains("changed since")));
    assert!(!staged.exists());
    assert_eq!(
        std::fs::read(source).expect("unchanged replacement"),
        b"different replacement audio"
    );
}

#[tokio::test]
async fn preserve_copy_stops_after_cancellation_during_progress() {
    let tmp = tempfile::tempdir().expect("copy workspace");
    let source = tmp.path().join("source.mp3");
    let staged = tmp.path().join("staged.mp3");
    std::fs::write(&source, vec![7_u8; 4 * 1024 * 1024]).expect("write copy source");
    let source_fingerprint = inspect_source_fingerprint(&source);
    let registry = JobRegistry::new(1);
    let (job_id, _permit) = registry.register_job().await.expect("register copy job");
    let cancelled = Arc::new(AtomicBool::new(false));
    let checker = crate::processing::CancellationChecker::new(Some(cancelled.clone()));
    let mut context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::with_cancellation(job_id.0, checker)),
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(tmp.path().join("final.mp3")),
    );
    context.progress_listener = Some(Arc::new(move |_| {
        cancelled.store(true, Ordering::Release);
    }));
    let error = copy_with_cancellation(&source, &source_fingerprint, &staged, &context, 0.0..1.0)
        .expect_err("copy must observe cancellation");
    assert!(matches!(error, AppError::Cancellation(_)));
    let copied_bytes = std::fs::metadata(&staged).expect("partial copy").len();
    assert!(copied_bytes > 0 && copied_bytes < 4 * 1024 * 1024);
    assert_eq!(
        std::fs::metadata(&source).expect("original source").len(),
        4 * 1024 * 1024
    );
}

#[test]
fn preserve_copy_rejects_source_truncated_during_copy() {
    let tmp = tempfile::tempdir().expect("copy workspace");
    let source = tmp.path().join("source.mp3");
    let staged = tmp.path().join("staged.mp3");
    std::fs::write(&source, vec![7_u8; 4 * 1024 * 1024]).expect("write copy source");
    let source_fingerprint = inspect_source_fingerprint(&source);
    let source_for_listener = source.clone();
    let mut context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::new()),
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(tmp.path().join("final.mp3")),
    );
    context.progress_listener = Some(Arc::new(move |_| {
        std::fs::File::create(&source_for_listener).expect("truncate source during copy");
    }));

    let error = copy_with_cancellation(&source, &source_fingerprint, &staged, &context, 0.0..1.0)
        .expect_err("source truncation must be rejected after copy EOF");

    assert!(matches!(error, AppError::InvalidInput(message) if message.contains("changed since")));
    assert_eq!(
        std::fs::metadata(&source).expect("truncated source").len(),
        0
    );
}

#[test]
fn preserve_copy_rejects_same_size_source_overwrite_during_copy() {
    let tmp = tempfile::tempdir().expect("copy workspace");
    let source = tmp.path().join("source.mp3");
    let staged = tmp.path().join("staged.mp3");
    std::fs::write(&source, vec![7_u8; 4 * 1024 * 1024]).expect("write copy source");
    let source_fingerprint = inspect_source_fingerprint(&source);
    let source_for_listener = source.clone();
    let mut context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::new()),
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(tmp.path().join("final.mp3")),
    );
    context.progress_listener = Some(Arc::new(move |_| {
        let mut source = std::fs::OpenOptions::new()
            .write(true)
            .open(&source_for_listener)
            .expect("open source during copy");
        source
            .write_all(&[9_u8; 4 * 1024 * 1024])
            .expect("overwrite source during copy");
        source
            .set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1))
            .expect("set source mtime");
    }));

    let error = copy_with_cancellation(&source, &source_fingerprint, &staged, &context, 0.0..1.0)
        .expect_err("same-size source overwrite must be rejected after copy EOF");

    assert!(matches!(error, AppError::InvalidInput(message) if message.contains("changed since")));
    assert!(std::fs::read(&source)
        .expect("overwritten source")
        .iter()
        .all(|byte| *byte == 9));
}
