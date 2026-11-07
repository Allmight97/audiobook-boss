use super::{complete_staged_output, settle_published};
use crate::audio::processor::workspace::Workspace;
use crate::output_artifact::{
    commit_output_artifact, finalized_output_success, OutputCommitRequest, OutputKind,
    PlannedOutputAction,
};
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

async fn context_for(
    local: &TempDir,
    final_output: &Path,
    cancelled: bool,
) -> (ProcessingContext, Workspace) {
    let registry = JobRegistry::new(1);
    let (job_id, _permit) = registry.register_job().await.expect("register job");
    let checker = crate::processing::CancellationChecker::new(
        cancelled.then(crate::processing::cancelled_token),
    );
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::with_cancellation(job_id.0, checker)),
        SAMPLE_RATE,
        OutputConfig::new(final_output),
        local.path().join("processing").join("sessions"),
    );
    let workspace = Workspace::open(&context).expect("open workspace");
    (context, workspace)
}

#[tokio::test]
async fn complete_staged_output_commits_from_local_workspace_without_destination_residue() {
    let local = TempDir::new().expect("local workspace root");
    let destination = TempDir::new().expect("destination root");
    let final_output = destination.path().join("final.m4b");
    let (context, mut workspace) = context_for(&local, &final_output, false).await;
    let staged_output = workspace.dir().join("staged.m4b");
    fs::write(&staged_output, b"audio").expect("write staged output");
    let session_dir = workspace.dir().to_path_buf();

    complete_staged_output(&context, staged_output, &mut workspace).expect("commit should succeed");

    assert_eq!(std::fs::read(&final_output).expect("read final"), b"audio");
    assert!(!session_dir.exists(), "local workspace should be cleaned");
    assert!(
        !has_hidden_processing_artifact(destination.path()),
        "destination must not contain ABB processing temp artifacts"
    );
}

#[tokio::test]
async fn complete_staged_output_cleans_local_workspace_after_commit_failure() {
    let local = TempDir::new().expect("local workspace root");
    let destination = TempDir::new().expect("destination root");
    let final_output = destination.path().join("final.m4b");
    fs::write(&final_output, b"existing").expect("write existing output");
    let (mut context, mut workspace) = context_for(&local, &final_output, false).await;
    let staged_output = workspace.dir().join("staged.m4b");
    fs::write(&staged_output, b"audio").expect("write staged output");
    let session_dir = workspace.dir().to_path_buf();
    let stages = Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = Arc::clone(&stages);
    context.progress_listener = Some(Arc::new(move |event| {
        recorded.lock().expect("stages").push(event.stage);
    }));

    let error = complete_staged_output(&context, staged_output, &mut workspace)
        .expect_err("existing destination should fail write action");
    drop(workspace);

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
    let local = TempDir::new().expect("local workspace root");
    let destination = TempDir::new().expect("destination root");
    let final_output = destination.path().join("final.m4b");
    let (context, mut workspace) = context_for(&local, &final_output, true).await;
    let staged_output = workspace.dir().join("staged.m4b");
    fs::write(&staged_output, b"audio").expect("write staged output");
    let session_dir = workspace.dir().to_path_buf();

    let error = complete_staged_output(&context, staged_output.clone(), &mut workspace)
        .expect_err("cancelled job should not commit staged output");

    assert!(
        error.to_string().contains("Processing was cancelled"),
        "unexpected error: {error}"
    );
    assert!(
        !final_output.exists(),
        "cancelled staged output should not be committed"
    );
    drop(workspace);
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
    let local = TempDir::new().expect("local workspace root");
    let destination = TempDir::new().expect("destination root");
    let final_output = destination.path().join("final.m4b");
    let (mut context, mut workspace) = context_for(&local, &final_output, false).await;
    let staged_output = local.path().join("elsewhere").join("staged.m4b");
    fs::create_dir_all(staged_output.parent().expect("staging folder")).expect("staging folder");
    fs::write(&staged_output, b"audio").expect("write staged output");
    let session_dir = workspace.dir().to_path_buf();
    let sessions = local.path().join("processing").join("sessions");
    let backup = local.path().join("sessions-backup");
    crate::test_cases::move_test_file(&sessions, &backup);
    fs::write(&sessions, b"a file where a folder is expected").expect("write blocker");
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = Arc::clone(&events);
    context.progress_listener = Some(Arc::new(move |event| {
        recorded
            .lock()
            .expect("events")
            .push((event.stage, event.message.clone()));
    }));

    let message = complete_staged_output(&context, staged_output, &mut workspace)
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
    fs::remove_file(&sessions).expect("clear the blocker");
    crate::test_cases::move_test_file(&backup, &sessions);
    assert!(session_dir.exists(), "the workspace stays owned for retry");
    drop(workspace);
    assert!(!session_dir.exists(), "the retry on drop removes it");
    assert_eq!(fs::read(&final_output).expect("read final"), b"audio");
}

#[tokio::test]
async fn a_cancel_after_publication_keeps_the_published_output_and_still_cleans_the_workspace() {
    let local = TempDir::new().expect("local workspace root");
    let destination = TempDir::new().expect("destination root");
    let final_output = destination.path().join("final-output.m4b");
    let (_context, mut workspace) = context_for(&local, &final_output, false).await;
    let temp_output = workspace.dir().join("worker-output.m4b");
    fs::write(&temp_output, b"payload").expect("write temp output");
    let temp_dir = workspace.dir().to_path_buf();

    let published = commit_output_artifact(
        OutputCommitRequest::new(&final_output, PlannedOutputAction::Write),
        temp_output,
    )
    .expect("commit should succeed");
    let outcome = settle_published(published, &mut workspace, || true);

    assert!(outcome.cancelled, "expected cancellation after move");
    assert!(final_output.exists(), "moved output should be preserved");
    assert!(
        !temp_dir.exists(),
        "temp directory should still be cleaned after post-move cancellation"
    );
}

#[tokio::test]
async fn committed_output_survives_cleanup_failure_with_visible_warning_and_retry() {
    let local = TempDir::new().expect("local workspace root");
    let destination = TempDir::new().expect("destination root");
    let output = destination.path().join("book.m4b");
    let (_context, mut workspace) = context_for(&local, &output, false).await;
    let sessions = local.path().join("processing").join("sessions");
    let backup = local.path().join("sessions-backup");
    let staged = workspace.dir().join("staged.m4b");
    fs::write(&staged, b"complete book").expect("write test fixture");
    let published = commit_output_artifact(
        OutputCommitRequest::new(&output, PlannedOutputAction::Write),
        staged.clone(),
    )
    .expect("publication remains successful when staged cleanup fails");
    let outcome = settle_published(published, &mut workspace, || {
        crate::test_cases::move_test_file(&sessions, &backup);
        fs::write(&sessions, b"blocks cleanup traversal").expect("write test fixture");
        false
    });
    assert_eq!(
        fs::read(&output).expect("read published artifact"),
        b"complete book"
    );
    let success = finalized_output_success(
        OutputKind::Final,
        &output,
        false,
        outcome.cleanup_warning.as_deref(),
    );
    assert!(success
        .result_message
        .contains("temporary files could not be removed"));
    fs::remove_file(&sessions).expect("remove test traversal blocker");
    crate::test_cases::move_test_file(&backup, &sessions);
    drop(workspace);
    assert!(!staged.exists());
    assert_eq!(
        fs::read(&output).expect("read published artifact"),
        b"complete book"
    );
}
