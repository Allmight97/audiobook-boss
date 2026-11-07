use super::complete_staged_output;
use crate::audio::CleanupGuard;
use crate::processing::{JobRegistry, OutputConfig, ProcessingContext, ProcessingSession};
use crate::test_cases::media_fixtures::SAMPLE_RATE;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

fn has_hidden_processing_artifact(root: &Path) -> bool {
    std::fs::read_dir(root).expect("read dir").any(|entry| {
        let name = entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        name.starts_with(".abb-processing-")
            || name.starts_with(".abb_meta_")
            || name.starts_with(".abb_replace_")
    })
}

#[tokio::test]
async fn complete_staged_output_commits_from_local_workspace_without_destination_residue() {
    let registry = JobRegistry::new(1);
    let (job_id, _permit) = registry.register_job().await.expect("register job");
    let checker = crate::processing::CancellationChecker::new(None);

    let local = TempDir::new().expect("local workspace root");
    let destination = TempDir::new().expect("destination root");
    let workspace_root = local.path().join("processing").join("sessions");
    let session_dir = workspace_root.join(".abb-processing-test");
    std::fs::create_dir_all(&session_dir).expect("session dir");
    let staged_output = session_dir.join("staged.m4b");
    let final_output = destination.path().join("final.m4b");
    fs::write(&staged_output, b"audio").expect("write staged output");

    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::with_cancellation(job_id.0, checker)),
        SAMPLE_RATE,
        OutputConfig::new(&final_output),
        workspace_root,
    );
    let mut cleanup_guard = CleanupGuard::new(context.session.id());
    cleanup_guard.add_path(&session_dir);
    cleanup_guard.add_path(&staged_output);

    complete_staged_output(&context, staged_output, &mut cleanup_guard)
        .expect("commit should succeed");

    assert_eq!(std::fs::read(&final_output).expect("read final"), b"audio");
    assert!(!session_dir.exists(), "local workspace should be cleaned");
    assert!(
        !has_hidden_processing_artifact(destination.path()),
        "destination must not contain ABB processing temp artifacts"
    );
}

#[tokio::test]
async fn complete_staged_output_cleans_local_workspace_after_commit_failure() {
    let registry = JobRegistry::new(1);
    let (job_id, _permit) = registry.register_job().await.expect("register job");
    let checker = crate::processing::CancellationChecker::new(None);

    let local = TempDir::new().expect("local workspace root");
    let destination = TempDir::new().expect("destination root");
    let workspace_root = local.path().join("processing").join("sessions");
    let session_dir = workspace_root.join(".abb-processing-test");
    std::fs::create_dir_all(&session_dir).expect("session dir");
    let staged_output = session_dir.join("staged.m4b");
    let final_output = destination.path().join("final.m4b");
    fs::write(&staged_output, b"audio").expect("write staged output");
    fs::write(&final_output, b"existing").expect("write existing output");

    let mut context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::with_cancellation(job_id.0, checker)),
        SAMPLE_RATE,
        OutputConfig::new(&final_output),
        workspace_root,
    );
    let stages = Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = Arc::clone(&stages);
    context.progress_listener = Some(Arc::new(move |event| {
        recorded.lock().expect("stages").push(event.stage);
    }));
    let mut cleanup_guard = CleanupGuard::new(context.session.id());
    cleanup_guard.add_path(&session_dir);
    cleanup_guard.add_path(&staged_output);

    let error = complete_staged_output(&context, staged_output, &mut cleanup_guard)
        .expect_err("existing destination should fail write action");
    drop(cleanup_guard);

    assert!(
        error
            .to_string()
            .contains("Review collisions and try again"),
        "unexpected error: {error}"
    );
    assert!(
        !stages
            .lock()
            .expect("stages")
            .contains(&crate::processing::EventStage::Completed),
        "a title whose publish failed must never report Completed"
    );
    assert_eq!(
        std::fs::read(&final_output).expect("read final"),
        b"existing"
    );
    assert!(!session_dir.exists(), "local workspace should be cleaned");
    assert!(
        !has_hidden_processing_artifact(destination.path()),
        "destination must not contain ABB processing temp artifacts"
    );
}

#[tokio::test]
async fn complete_staged_output_cleans_without_committing_when_cancelled_before_commit() {
    let registry = JobRegistry::new(1);
    let (job_id, _permit) = registry.register_job().await.expect("register job");
    let checker =
        crate::processing::CancellationChecker::new(Some(crate::processing::cancelled_token()));

    let local = TempDir::new().expect("local workspace root");
    let destination = TempDir::new().expect("destination root");
    let workspace_root = local.path().join("processing").join("sessions");
    let session_dir = workspace_root.join(".abb-processing-test");
    std::fs::create_dir_all(&session_dir).expect("session dir");
    let staged_output = session_dir.join("staged.m4b");
    let final_output = destination.path().join("final.m4b");
    fs::write(&staged_output, b"audio").expect("write staged output");

    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::with_cancellation(job_id.0, checker)),
        SAMPLE_RATE,
        OutputConfig::new(&final_output),
        workspace_root,
    );
    let mut cleanup_guard = CleanupGuard::new(context.session.id());
    cleanup_guard.add_path(&session_dir);
    cleanup_guard.add_path(&staged_output);

    let error = complete_staged_output(&context, staged_output.clone(), &mut cleanup_guard)
        .expect_err("cancelled job should not commit staged output");

    assert!(
        error.to_string().contains("Processing was cancelled"),
        "unexpected error: {error}"
    );
    assert!(
        !final_output.exists(),
        "cancelled staged output should not be committed"
    );
    drop(cleanup_guard);
    assert!(
        !session_dir.exists(),
        "cancelled local workspace should be cleaned"
    );
    assert!(
        !has_hidden_processing_artifact(destination.path()),
        "destination must not contain ABB processing temp artifacts"
    );
}

#[tokio::test]
async fn a_cleanup_failure_after_publication_is_a_success_with_a_warning_and_a_retry() {
    let registry = JobRegistry::new(1);
    let (job_id, _permit) = registry.register_job().await.expect("register job");
    let checker = crate::processing::CancellationChecker::new(None);

    let local = TempDir::new().expect("local workspace root");
    let destination = TempDir::new().expect("destination root");
    let workspace_root = local.path().join("processing").join("sessions");
    let session_dir = workspace_root.join(".abb-processing-test");
    fs::create_dir_all(&session_dir).expect("session dir");
    let staged_output = session_dir.join("staged.m4b");
    let final_output = destination.path().join("final.m4b");
    fs::write(&staged_output, b"audio").expect("write staged output");
    let blocker = local.path().join("blocker");
    fs::write(&blocker, b"a file where a folder is expected").expect("write blocker");

    let mut context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::with_cancellation(job_id.0, checker)),
        SAMPLE_RATE,
        OutputConfig::new(&final_output),
        workspace_root,
    );
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = Arc::clone(&events);
    context.progress_listener = Some(Arc::new(move |event| {
        recorded
            .lock()
            .expect("events")
            .push((event.stage, event.message.clone()));
    }));
    let mut cleanup_guard = CleanupGuard::new(context.session.id());
    cleanup_guard.add_path(&session_dir);
    cleanup_guard.add_path(&staged_output);
    cleanup_guard.add_path(blocker.join("child"));

    let message = complete_staged_output(&context, staged_output, &mut cleanup_guard)
        .expect("a published title stays a success");

    assert_eq!(fs::read(&final_output).expect("read final"), b"audio");
    assert!(
        message.starts_with("Successfully created audiobook: ")
            && message.ends_with("some temporary files could not be removed."),
        "{message}"
    );
    let completed: Vec<_> = events
        .lock()
        .expect("events")
        .iter()
        .filter(|(stage, _)| *stage == crate::processing::EventStage::Completed)
        .map(|(_, message)| message.clone())
        .collect();
    assert_eq!(
        completed,
        ["Output created; temporary cleanup needs attention"]
    );
    assert!(!session_dir.exists(), "what could be removed was removed");
    cleanup_guard
        .cleanup_now()
        .expect_err("the path that failed stays tracked and fails again");
    fs::remove_file(&blocker).expect("clear the blocker");
    cleanup_guard
        .cleanup_now()
        .expect("the retry succeeds once the blocker is gone");
    assert_eq!(fs::read(&final_output).expect("read final"), b"audio");
}
