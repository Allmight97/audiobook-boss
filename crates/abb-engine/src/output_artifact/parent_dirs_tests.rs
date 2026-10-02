use super::*;
use crate::output_artifact::{OutputKind, PlannedOutputAction, ResolvedOutputPlan};
use std::path::PathBuf;
use tempfile::TempDir;

fn output_plan(action: PlannedOutputAction, path: impl Into<PathBuf>) -> ResolvedOutputPlan {
    let path = path.into();
    ResolvedOutputPlan {
        kind: OutputKind::Final,
        requested_path: path.clone(),
        resolved_path: path,
        rename_candidate: None,
        collision: None,
        action,
    }
}

#[test]
fn creates_parent_dirs_for_writable_outputs_only() {
    let temp_dir = TempDir::new().expect("temp dir");
    let writable = output_plan(
        PlannedOutputAction::Write,
        temp_dir.path().join("created").join("book.m4b"),
    );
    let skipped = output_plan(
        PlannedOutputAction::SkipExisting,
        temp_dir.path().join("skipped").join("book.m4b"),
    );

    let _cleanup =
        ensure_output_parent_dirs(temp_dir.path(), [&writable, &skipped]).expect("parent dirs");

    assert!(temp_dir.path().join("created").exists());
    assert!(!temp_dir.path().join("skipped").exists());
}

#[test]
fn cleanup_removes_newly_created_empty_output_parents() {
    let temp_dir = TempDir::new().expect("temp dir");
    let output = output_plan(
        PlannedOutputAction::Write,
        temp_dir
            .path()
            .join("author")
            .join("title")
            .join("book.m4b"),
    );

    let mut cleanup = ensure_output_parent_dirs(temp_dir.path(), [&output]).expect("parent dirs");

    assert!(temp_dir.path().join("author").join("title").exists());

    cleanup.cleanup_now().expect("cleanup output parents");

    assert!(!temp_dir.path().join("author").join("title").exists());
    assert!(!temp_dir.path().join("author").exists());
    assert!(temp_dir.path().exists());
}

#[test]
fn cleanup_preserves_preexisting_empty_output_parents() {
    let temp_dir = TempDir::new().expect("temp dir");
    let parent = temp_dir.path().join("author").join("title");
    std::fs::create_dir_all(&parent).expect("precreate parent");
    let output = output_plan(PlannedOutputAction::Write, parent.join("book.m4b"));

    let mut cleanup = ensure_output_parent_dirs(temp_dir.path(), [&output]).expect("parent dirs");

    cleanup.cleanup_now().expect("cleanup output parents");

    assert!(parent.exists(), "preexisting parent should remain");
}

#[test]
fn cleanup_preserves_non_empty_output_parents() {
    let temp_dir = TempDir::new().expect("temp dir");
    let parent = temp_dir.path().join("author").join("title");
    let output = output_plan(PlannedOutputAction::Write, parent.join("book.m4b"));
    let mut cleanup = ensure_output_parent_dirs(temp_dir.path(), [&output]).expect("parent dirs");

    std::fs::write(parent.join("marker.txt"), b"keep").expect("write marker");

    cleanup.cleanup_now().expect("cleanup output parents");

    assert!(parent.exists(), "non-empty parent should remain");
    assert!(
        temp_dir.path().join("author").exists(),
        "non-empty child should keep ancestor"
    );
}

#[test]
fn cleanup_preserves_successful_sibling_and_removes_cancelled_empty_sibling() {
    let temp_dir = TempDir::new().expect("temp dir");
    let successful_parent = temp_dir.path().join("author").join("success");
    let cancelled_parent = temp_dir.path().join("author").join("cancelled");
    let successful = output_plan(
        PlannedOutputAction::Write,
        successful_parent.join("book.m4b"),
    );
    let cancelled = output_plan(
        PlannedOutputAction::Write,
        cancelled_parent.join("book.m4b"),
    );
    let mut cleanup =
        ensure_output_parent_dirs(temp_dir.path(), [&successful, &cancelled]).expect("parents");

    std::fs::write(successful_parent.join("book.m4b"), b"committed")
        .expect("write committed output");

    cleanup.cleanup_now().expect("cleanup output parents");

    assert!(
        successful_parent.exists(),
        "successful parent should remain"
    );
    assert!(
        !cancelled_parent.exists(),
        "empty cancelled parent should be removed"
    );
    assert!(temp_dir.path().join("author").exists());
}

#[test]
fn rejects_output_parent_escape_from_output_root() {
    let temp_dir = TempDir::new().expect("temp dir");
    let escaped = temp_dir
        .path()
        .join("..")
        .join("outside-output-root")
        .join("book.m4b");
    let output = output_plan(PlannedOutputAction::Write, escaped);

    let err = ensure_output_parent_dirs(temp_dir.path(), [&output])
        .expect_err("escaped parent should fail");

    assert!(err
        .to_string()
        .contains("escapes the configured output root"));
}

#[cfg(unix)]
#[test]
fn cleanup_refuses_symlinked_output_parent() {
    let temp_dir = TempDir::new().expect("temp dir");
    let outside = TempDir::new().expect("outside dir");
    let parent = temp_dir.path().join("author").join("title");
    let output = output_plan(PlannedOutputAction::Write, parent.join("book.m4b"));
    let mut cleanup = ensure_output_parent_dirs(temp_dir.path(), [&output]).expect("parent dirs");

    std::fs::remove_dir(&parent).expect("remove created parent");
    std::os::unix::fs::symlink(outside.path(), &parent).expect("swap symlink parent");

    let err = cleanup
        .cleanup_now()
        .expect_err("symlinked parent cleanup should fail");

    assert!(err.to_string().contains("symlinked output directory"));
    assert!(outside.path().exists(), "outside target must remain");
}

#[test]
fn creates_a_missing_output_root_and_cleanup_removes_it() {
    let temp_dir = TempDir::new().expect("temp dir");
    let root = temp_dir.path().join("library");
    let output = output_plan(
        PlannedOutputAction::Write,
        root.join("author").join("book.m4b"),
    );

    let mut cleanup = ensure_output_parent_dirs(&root, [&output]).expect("output dirs");
    assert!(root.join("author").is_dir());

    cleanup.cleanup_now().expect("cleanup output dirs");
    assert!(!root.exists(), "an ABB-created output root is rolled back");
    assert!(temp_dir.path().exists());
}

#[test]
fn a_failed_later_output_rolls_back_the_created_output_root() {
    let temp_dir = TempDir::new().expect("temp dir");
    let root = temp_dir.path().join("library");
    let inside = output_plan(
        PlannedOutputAction::Write,
        root.join("author").join("book.m4b"),
    );
    let escaping = output_plan(
        PlannedOutputAction::Write,
        temp_dir.path().join("elsewhere").join("book.m4b"),
    );

    ensure_output_parent_dirs(&root, [&inside, &escaping])
        .expect_err("an escaping output parent must fail");

    assert!(
        !root.exists(),
        "no directory ABB created survives the failure"
    );
    assert!(!temp_dir.path().join("elsewhere").exists());
}

#[test]
fn a_title_ending_unpublished_removes_only_the_empty_folders_made_for_it_alone() {
    let temp_dir = TempDir::new().expect("temp dir");
    let root = temp_dir.path();
    std::fs::create_dir(root.join("existing")).expect("folder from before");
    let book = |parts: &[&str]| {
        let path = parts.iter().fold(root.to_path_buf(), |path, part| path.join(part));
        output_plan(PlannedOutputAction::Write, path.join("book.m4b"))
    };
    let alpha = book(&["author", "alpha"]);
    let beta = book(&["author", "beta"]);
    let gamma = book(&["existing", "gamma"]);
    let delta = book(&["existing"]);

    let cleanup =
        ensure_output_parent_dirs(root, [&alpha, &beta, &gamma, &delta]).expect("parent dirs");

    cleanup.cleanup_title(0).expect("alpha's folders");
    assert!(!root.join("author").join("alpha").exists());
    assert!(root.join("author").join("beta").exists(), "beta's folder stays");
    assert!(root.join("author").exists(), "shared with beta");

    cleanup.cleanup_title(2).expect("gamma's folders");
    assert!(!root.join("existing").join("gamma").exists());
    assert!(root.join("existing").exists(), "existed before the run");
}
