use super::*;
use std::cell::Cell;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

struct DropProbe {
    dropped: Arc<AtomicBool>,
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

#[test]
fn commit_output_artifact_preserves_moved_output_on_post_move_cancel() {
    let root = TempDir::new().expect("temp root");
    let temp_dir = root.path().join("worker-temp");
    std::fs::create_dir_all(&temp_dir).expect("create worker temp dir");

    let temp_output = temp_dir.join("worker-output.m4b");
    std::fs::write(&temp_output, b"payload").expect("write temp output");

    let final_output = root.path().join("final-output.m4b");

    let cancelled = AtomicBool::new(false);
    let mut cleanup_guard = CleanupGuard::new("commit-test".to_string());
    cleanup_guard.add_path(&temp_dir);
    cleanup_guard.add_path(&temp_output);

    let request = OutputCommitRequest::new(&final_output, PlannedOutputAction::Write);
    let outcome = commit_output_artifact_after_move(
        request,
        temp_output,
        &mut cleanup_guard,
        || {
            cancelled.store(true, Ordering::Release);
        },
        || cancelled.load(Ordering::Acquire),
    )
    .expect("commit should succeed");

    assert!(outcome.cancelled, "expected cancellation after move");
    assert!(final_output.exists(), "moved output should be preserved");
    assert!(
        !temp_dir.exists(),
        "temp directory should still be cleaned after post-move cancellation"
    );
}

#[test]
fn finalized_output_success_keeps_success_messages_after_post_commit_cancel() {
    let output = Path::new("/tmp/final-output.m4b");

    let preview = finalized_output_success(OutputKind::Preview, output, true);
    assert_eq!(preview.ui_message, "Preview created successfully");
    assert_eq!(
        preview.result_message,
        "Successfully created preview: /tmp/final-output.m4b"
    );

    let full = finalized_output_success(OutputKind::Final, output, true);
    assert_eq!(full.ui_message, "Processing complete");
    assert_eq!(
        full.result_message,
        "Successfully created audiobook: /tmp/final-output.m4b"
    );
}

#[test]
fn commit_temp_output_rejects_existing_destination_for_write_action() {
    let root = TempDir::new().expect("temp root");
    let temp_output = root.path().join("temp-output.m4b");
    let final_output = root.path().join("final-output.m4b");
    std::fs::write(&temp_output, b"new").expect("write temp output");
    std::fs::write(&final_output, b"existing").expect("write existing output");

    let err = commit_temp_output_to_artifact(
        temp_output.clone(),
        &final_output,
        PlannedOutputAction::Write,
    )
    .expect_err("existing destination should fail");

    assert!(err.to_string().contains("Review collisions and try again"));
    assert_eq!(
        std::fs::read(&final_output).expect("read final output"),
        b"existing"
    );
    assert_eq!(
        std::fs::read(&temp_output).expect("read temp output"),
        b"new"
    );
}

#[test]
fn commit_temp_output_replaces_existing_destination_for_replace_existing_action() {
    let root = TempDir::new().expect("temp root");
    let temp_output = root.path().join("temp-output.m4b");
    let final_output = root.path().join("final-output.m4b");
    std::fs::write(&temp_output, b"new").expect("write temp output");
    std::fs::write(&final_output, b"existing").expect("write existing output");

    let committed = commit_temp_output_to_artifact(
        temp_output.clone(),
        &final_output,
        PlannedOutputAction::ReplaceExisting,
    )
    .expect("replace action should succeed");

    assert_eq!(committed, final_output);
    assert_eq!(
        std::fs::read(&final_output).expect("read final output"),
        b"new"
    );
    assert!(!temp_output.exists(), "temp output should be removed");
}

#[test]
fn replace_existing_copies_to_destination_temp_on_cross_device_rename() {
    let root = TempDir::new().expect("temp root");
    let worker = root.path().join("local-worker");
    let destination_dir = root.path().join("destination");
    std::fs::create_dir_all(&worker).expect("worker");
    std::fs::create_dir_all(&destination_dir).expect("destination");
    let temp_output = worker.join("temp-output.m4b");
    let final_output = destination_dir.join("final-output.m4b");
    std::fs::write(&temp_output, b"new").expect("write temp output");
    std::fs::write(&final_output, b"old").expect("write existing output");
    let calls = Cell::new(0);

    let committed =
        replace_existing_from_staged_output_with(&temp_output, &final_output, |source, dest| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                assert_eq!(source, temp_output.as_path());
                assert_eq!(dest, final_output.as_path());
                return Err(io::Error::new(
                    io::ErrorKind::CrossesDevices,
                    "cross-device",
                ));
            }
            crate::file_replace::replace_file(source, dest)
        })
        .expect("cross-device replace fallback should succeed");

    assert_eq!(committed, final_output);
    assert_eq!(
        std::fs::read(&final_output).expect("read final output"),
        b"new"
    );
    assert!(!temp_output.exists(), "staged source should be consumed");
    assert!(
        std::fs::read_dir(&destination_dir)
            .expect("read destination")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".abb_replace_install_")),
        "destination temp replacement should not remain"
    );
}

#[test]
fn replace_existing_cross_device_fallback_preserves_source_and_destination_on_install_failure() {
    let root = TempDir::new().expect("temp root");
    let worker = root.path().join("local-worker");
    let destination_dir = root.path().join("destination");
    std::fs::create_dir_all(&worker).expect("worker");
    std::fs::create_dir_all(&destination_dir).expect("destination");
    let temp_output = worker.join("temp-output.m4b");
    let final_output = destination_dir.join("final-output.m4b");
    std::fs::write(&temp_output, b"new").expect("write temp output");
    std::fs::write(&final_output, b"old").expect("write existing output");
    let calls = Cell::new(0);

    let error =
        replace_existing_from_staged_output_with(&temp_output, &final_output, |_source, _dest| {
            calls.set(calls.get() + 1);
            match calls.get() {
                1 => Err(io::Error::new(
                    io::ErrorKind::CrossesDevices,
                    "cross-device",
                )),
                _ => Err(io::Error::new(io::ErrorKind::PermissionDenied, "blocked")),
            }
        })
        .expect_err("install failure should fail");

    assert!(
        error
            .to_string()
            .contains("destination refused the final file commit"),
        "unexpected error: {error}"
    );
    assert_eq!(
        std::fs::read(&final_output).expect("read final output"),
        b"old"
    );
    assert_eq!(
        std::fs::read(&temp_output).expect("read temp output"),
        b"new"
    );
    assert!(
        std::fs::read_dir(&destination_dir)
            .expect("read destination")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".abb_replace_install_")),
        "destination temp replacement should be cleaned on failure"
    );
}

#[test]
fn copy_fallback_closes_source_before_removing_temp_output() {
    let dropped = Arc::new(AtomicBool::new(false));
    let source = DropProbe {
        dropped: Arc::clone(&dropped),
    };
    let temp_output = Path::new("/tmp/staged-output.m4b");
    let final_output = Path::new("/tmp/final-output.m4b");

    remove_copied_temp_output_with(temp_output, final_output, source, |_path| {
        assert!(
            dropped.load(Ordering::Acquire),
            "staged source handle must be closed before removal"
        );
        Ok(())
    })
    .expect("remove should succeed");
}

#[test]
fn commit_temp_output_preserves_outputs_when_replace_rename_fails() {
    let root = TempDir::new().expect("temp root");
    let temp_output = root.path().join("temp-output.m4b");
    let final_output = root.path().join("final-output.m4b");
    std::fs::write(&temp_output, b"new").expect("write temp output");
    std::fs::create_dir(&final_output).expect("create occupied destination directory");

    let err = commit_temp_output_to_artifact(
        temp_output.clone(),
        &final_output,
        PlannedOutputAction::ReplaceExisting,
    )
    .expect_err("rename into occupied directory should fail");

    assert!(err
        .to_string()
        .contains("destination refused the final file commit"));
    assert!(final_output.is_dir(), "existing destination should remain");
    assert_eq!(
        std::fs::read(&temp_output).expect("read temp output"),
        b"new",
        "staged output should remain for cleanup after commit failure"
    );
}

#[cfg(unix)]
#[test]
fn commit_temp_output_rejects_dangling_symlink_destination_for_write_action() {
    let root = TempDir::new().expect("temp root");
    let temp_output = root.path().join("temp-output.m4b");
    let final_output = root.path().join("final-output.m4b");
    let missing_target = root.path().join("missing-output.m4b");
    std::fs::write(&temp_output, b"new").expect("write temp output");
    std::os::unix::fs::symlink(&missing_target, &final_output).expect("create dangling symlink");

    let err = commit_temp_output_to_artifact(
        temp_output.clone(),
        &final_output,
        PlannedOutputAction::Write,
    )
    .expect_err("dangling symlink should fail");

    assert!(err.to_string().contains("Review collisions and try again"));
    assert_eq!(
        std::fs::read_link(&final_output).expect("read symlink"),
        missing_target
    );
    assert_eq!(
        std::fs::read(&temp_output).expect("read temp output"),
        b"new"
    );
}

#[test]
fn commit_removes_stale_replacement_temp_for_same_final_artifact() {
    let root = TempDir::new().expect("temp root");
    let temp_output = root.path().join("temp-output.m4b");
    let final_output = root.path().join("book.m4b");
    std::fs::write(&temp_output, b"new").expect("write temp output");
    // Simulate a hard crash mid replace-commit: full-size copy stranded
    // beside the destination.
    let stale_temp = root
        .path()
        .join(".abb_replace_install_0000-crashed_book.m4b");
    std::fs::write(&stale_temp, b"stranded copy").expect("write stale temp");

    let committed =
        commit_temp_output_to_artifact(temp_output, &final_output, PlannedOutputAction::Write)
            .expect("commit succeeds");

    assert_eq!(committed, final_output);
    assert!(!stale_temp.exists(), "stale ABB replacement temp is swept");
    assert!(final_output.exists(), "new artifact committed");
}

#[test]
fn commit_preserves_unrelated_and_imitation_entries_in_destination() {
    let root = TempDir::new().expect("temp root");
    let temp_output = root.path().join("temp-output.m4b");
    let final_output = root.path().join("book.m4b");
    std::fs::write(&temp_output, b"new").expect("write temp output");

    // Same ABB prefix, different final artifact: not ours to sweep here.
    let other_artifact_temp = root
        .path()
        .join(".abb_replace_install_0000-crashed_other.m4b");
    std::fs::write(&other_artifact_temp, b"other").expect("write other temp");
    // User dotfile without ABB naming.
    let user_dotfile = root.path().join(".book.m4b");
    std::fs::write(&user_dotfile, b"user").expect("write user dotfile");
    // Directory imitating the temp naming: regular files only.
    let imitation_dir = root.path().join(".abb_replace_install_dir_book.m4b");
    std::fs::create_dir(&imitation_dir).expect("create imitation dir");

    commit_temp_output_to_artifact(temp_output, &final_output, PlannedOutputAction::Write)
        .expect("commit succeeds");

    assert!(
        other_artifact_temp.exists(),
        "temps for other artifacts are preserved"
    );
    assert!(user_dotfile.exists(), "non-ABB dotfiles are preserved");
    assert!(
        imitation_dir.exists(),
        "directories imitating temp naming are preserved"
    );
}

#[cfg(unix)]
#[test]
fn commit_preserves_symlink_imitating_replacement_temp() {
    let root = TempDir::new().expect("temp root");
    let temp_output = root.path().join("temp-output.m4b");
    let final_output = root.path().join("book.m4b");
    std::fs::write(&temp_output, b"new").expect("write temp output");
    let target = root.path().join("target.m4b");
    std::fs::write(&target, b"target").expect("write target");
    let link = root.path().join(".abb_replace_install_link_book.m4b");
    std::os::unix::fs::symlink(&target, &link).expect("create symlink");

    commit_temp_output_to_artifact(temp_output, &final_output, PlannedOutputAction::Write)
        .expect("commit succeeds");

    assert!(link.exists(), "symlink imitating temp naming is preserved");
    assert!(target.exists(), "symlink target is preserved");
}
