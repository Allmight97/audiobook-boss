use super::*;
use std::cell::Cell;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use tempfile::TempDir;

fn replacement_temp_named(final_path: &Path, suffix: &str) -> std::path::PathBuf {
    let tag = replacement_temp_tag(final_path).expect("final path has a name");
    final_path
        .parent()
        .expect("final path has a folder")
        .join(format!(".abb_{tag}_{suffix}"))
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
fn committed_output_survives_cleanup_failure_with_visible_warning_and_retry() {
    let root = TempDir::new().expect("create isolated test directory");
    let workspace = root.path().join("workspace");
    let backup = root.path().join("workspace-backup");
    std::fs::create_dir(&workspace).expect("create test workspace");
    let staged = workspace.join("staged.m4b");
    let output = root.path().join("book.m4b");
    std::fs::write(&staged, b"complete book").expect("write test fixture");
    let mut guard = CleanupGuard::new("cleanup-warning".into());
    let outcome = commit_output_artifact_after_move(
        OutputCommitRequest::new(&output, PlannedOutputAction::Write),
        staged.clone(),
        &mut guard,
        || {
            std::fs::rename(&workspace, &backup).expect("move test workspace");
            std::fs::write(&workspace, b"blocks cleanup traversal").expect("write test fixture");
        },
        || false,
    )
    .expect("publication remains successful when staged cleanup fails");
    assert_eq!(
        std::fs::read(&output).expect("read published artifact"),
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
    std::fs::remove_file(&workspace).expect("remove test traversal blocker");
    std::fs::rename(&backup, &workspace).expect("move test workspace");
    drop(guard);
    assert!(!staged.exists());
    assert_eq!(
        std::fs::read(&output).expect("read published artifact"),
        b"complete book"
    );
}

#[test]
fn finalized_output_success_keeps_success_messages_after_post_commit_cancel() {
    let output = Path::new("/tmp/final-output.m4b");

    let preview = finalized_output_success(OutputKind::Preview, output, true, None);
    assert_eq!(preview.ui_message, "Preview created successfully");
    assert_eq!(
        preview.result_message,
        "Successfully created preview: /tmp/final-output.m4b"
    );

    let full = finalized_output_success(OutputKind::Final, output, true, None);
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
    assert!(
        temp_output.exists(),
        "the commit owner cleans staged source after publication"
    );
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
    let stale_temp = replacement_temp_named(&final_output, "0000-crashed");
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

    let other_artifact_temp =
        replacement_temp_named(&root.path().join("other.m4b"), "0000-crashed");
    std::fs::write(&other_artifact_temp, b"other").expect("write other temp");
    let user_dotfile = root.path().join(".book.m4b");
    std::fs::write(&user_dotfile, b"user").expect("write user dotfile");
    let imitation_dir = replacement_temp_named(&final_output, "dir");
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
    let link = replacement_temp_named(&final_output, "link");
    std::os::unix::fs::symlink(&target, &link).expect("create symlink");

    commit_temp_output_to_artifact(temp_output, &final_output, PlannedOutputAction::Write)
        .expect("commit succeeds");

    assert!(link.exists(), "symlink imitating temp naming is preserved");
    assert!(target.exists(), "symlink target is preserved");
}

#[test]
fn commit_keeps_the_active_replacement_temp_of_a_sibling_title() {
    let root = TempDir::new().expect("temp root");
    let temp_output = root.path().join("temp-output.m4b");
    let final_output = root.path().join("book.m4b");
    std::fs::write(&temp_output, b"new").expect("write temp output");
    let sibling_temp = replacement_temp_named(&root.path().join("a_book.m4b"), "0000-active");
    std::fs::write(&sibling_temp, b"sibling copy in progress").expect("write sibling temp");

    commit_temp_output_to_artifact(temp_output, &final_output, PlannedOutputAction::Write)
        .expect("commit succeeds");

    assert!(
        sibling_temp.exists(),
        "another title's active replacement temp is not swept"
    );
}

#[test]
fn replace_existing_commits_a_204_byte_final_name_across_disks() {
    let root = TempDir::new().expect("temp root");
    let worker = root.path().join("local-worker");
    let destination_dir = root.path().join("destination");
    std::fs::create_dir_all(&worker).expect("worker");
    std::fs::create_dir_all(&destination_dir).expect("destination");
    let temp_output = worker.join("temp-output.m4b");
    let final_output = destination_dir.join(format!("{}.m4b", "a".repeat(200)));
    std::fs::write(&temp_output, b"new").expect("write temp output");
    std::fs::write(&final_output, b"old").expect("write existing output");
    let calls = Cell::new(0);

    replace_existing_from_staged_output_with(&temp_output, &final_output, |source, dest| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            return Err(io::Error::new(
                io::ErrorKind::CrossesDevices,
                "cross-device",
            ));
        }
        crate::file_replace::replace_file(source, dest)
    })
    .expect("a long final name still commits");

    assert_eq!(std::fs::read(&final_output).expect("read final"), b"new");
}

#[cfg(unix)]
#[test]
fn write_publish_of_a_dangling_staged_output_leaves_no_final_name() {
    let root = TempDir::new().expect("temp root");
    let temp_output = root.path().join("temp-output.m4b");
    let final_output = root.path().join("book.m4b");
    std::os::unix::fs::symlink(root.path().join("lost.m4b"), &temp_output)
        .expect("staged output points at nothing");

    let result =
        commit_temp_output_to_artifact(temp_output, &final_output, PlannedOutputAction::Write);

    assert!(
        result.is_err(),
        "a staged output with no content is refused"
    );
    assert!(
        std::fs::symlink_metadata(&final_output).is_err(),
        "no entry appears under the final name"
    );
}

#[test]
fn write_publish_of_a_directory_staged_output_leaves_no_final_name() {
    let root = TempDir::new().expect("temp root");
    let temp_output = root.path().join("temp-output.m4b");
    let final_output = root.path().join("book.m4b");
    std::fs::create_dir(&temp_output).expect("staged path is a directory");

    commit_temp_output_to_artifact(temp_output, &final_output, PlannedOutputAction::Write)
        .expect_err("a directory cannot be published");

    assert!(
        !final_output.exists(),
        "no file appears under the final name"
    );
}

#[test]
fn placeholder_publish_installs_the_copied_temp_under_a_free_name() {
    let root = TempDir::new().expect("temp root");
    let final_output = root.path().join("book.m4b");
    let temp = replacement_temp(&final_output).expect("temp");
    std::fs::write(temp.path(), b"complete book").expect("write temp");
    let temp_path = temp.path().to_path_buf();

    publish_by_placeholder(temp, &final_output).expect("publish");

    assert_eq!(
        std::fs::read(&final_output).expect("read"),
        b"complete book"
    );
    assert!(!temp_path.exists(), "the temp became the final file");
}

#[test]
fn placeholder_publish_refuses_a_taken_name_and_keeps_that_file() {
    let root = TempDir::new().expect("temp root");
    let final_output = root.path().join("book.m4b");
    std::fs::write(&final_output, b"someone else's book").expect("write existing");
    let temp = replacement_temp(&final_output).expect("temp");
    std::fs::write(temp.path(), b"complete book").expect("write temp");
    let temp_path = temp.path().to_path_buf();

    let error = publish_by_placeholder(temp, &final_output).expect_err("name is taken");

    assert!(error.to_string().contains("Review collisions"), "{error}");
    assert_eq!(
        std::fs::read(&final_output).expect("read"),
        b"someone else's book"
    );
    assert!(!temp_path.exists(), "the refused temp is removed");
}

#[test]
fn a_taken_name_is_a_collision_and_any_other_link_failure_copies() {
    let root = TempDir::new().expect("temp root");
    let taken = root.path().join("taken.m4b");
    std::fs::write(&taken, b"someone else's book").expect("write existing");
    let free = root.path().join("free.m4b");

    let error = classify_link_failure(&io::Error::from(io::ErrorKind::AlreadyExists), &taken)
        .expect_err("a taken name");
    assert!(error.to_string().contains("Review collisions"), "{error}");
    for kind in [
        io::ErrorKind::CrossesDevices,
        io::ErrorKind::PermissionDenied,
        io::ErrorKind::Unsupported,
        io::ErrorKind::InvalidInput,
        io::ErrorKind::Other,
    ] {
        assert!(
            !classify_link_failure(&io::Error::from(kind), &free).expect("copy path"),
            "{kind:?}"
        );
    }
}
