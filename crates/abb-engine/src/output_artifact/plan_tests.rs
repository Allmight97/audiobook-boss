use super::{resolve_output_plan, OutputPlanLedger};
use crate::output_artifact::{
    CollisionPolicy, OutputCollisionKind, OutputKind, PlannedOutputAction,
};
use std::collections::HashSet;
use std::fs::write;
use tempfile::TempDir;

#[test]
fn resolve_output_plan_marks_existing_file_for_review_by_default() {
    let temp_dir = TempDir::new().expect("temp dir");
    let existing_path = temp_dir.path().join("book.m4b");
    write(&existing_path, b"existing").expect("write existing file");

    let plan = resolve_output_plan(
        &existing_path,
        OutputKind::Final,
        CollisionPolicy::Fail,
        &HashSet::new(),
        &[],
    )
    .expect("plan");

    assert_eq!(plan.action, PlannedOutputAction::ReviewRequired);
    assert_eq!(
        plan.collision.as_ref().map(|value| value.kind),
        Some(OutputCollisionKind::ExistingFile)
    );
    assert_eq!(
        plan.rename_candidate,
        Some(temp_dir.path().join("book-1.m4b"))
    );
}

#[test]
fn resolve_output_plan_renames_batch_duplicates() {
    let temp_dir = TempDir::new().expect("temp dir");
    let requested_path = temp_dir.path().join("book.m4b");
    let mut claimed = HashSet::new();
    claimed.insert(requested_path.clone());

    let plan = resolve_output_plan(
        &requested_path,
        OutputKind::Final,
        CollisionPolicy::RenameNew,
        &claimed,
        &[],
    )
    .expect("resolved path");

    assert_eq!(plan.action, PlannedOutputAction::RenameNew);
    assert_eq!(plan.resolved_path, temp_dir.path().join("book-1.m4b"));
    assert_eq!(
        plan.collision.as_ref().map(|value| value.kind),
        Some(OutputCollisionKind::BatchDuplicate)
    );
}

#[test]
fn resolve_output_plan_blocks_source_destination_overlap() {
    let temp_dir = TempDir::new().expect("temp dir");
    let source_path = temp_dir.path().join("input.m4b");
    write(&source_path, b"input").expect("write input");

    let plan = resolve_output_plan(
        &source_path,
        OutputKind::Final,
        CollisionPolicy::ReplaceExisting,
        &HashSet::new(),
        std::slice::from_ref(&source_path),
    )
    .expect("plan");

    assert_eq!(plan.action, PlannedOutputAction::ReviewRequired);
    assert_eq!(
        plan.collision.as_ref().map(|value| value.kind),
        Some(OutputCollisionKind::SourceDestinationOverlap)
    );
    assert_eq!(plan.rename_candidate, None);
}

#[test]
fn resolve_output_plan_skips_existing_when_requested() {
    let temp_dir = TempDir::new().expect("temp dir");
    let existing_path = temp_dir.path().join("book.m4b");
    write(&existing_path, b"existing").expect("write existing file");

    let plan = resolve_output_plan(
        &existing_path,
        OutputKind::Final,
        CollisionPolicy::SkipExisting,
        &HashSet::new(),
        &[],
    )
    .expect("plan");

    assert_eq!(plan.action, PlannedOutputAction::SkipExisting);
    assert_eq!(plan.resolved_path, existing_path);
}

#[test]
fn resolve_output_plan_refuses_a_256_byte_file_name_before_any_work() {
    let temp_dir = TempDir::new().expect("temp dir");
    let requested = temp_dir.path().join(format!("{}.m4b", "a".repeat(252)));

    let error = resolve_output_plan(
        &requested,
        OutputKind::Final,
        CollisionPolicy::Fail,
        &HashSet::new(),
        &[],
    )
    .expect_err("an overlong name is refused");

    assert!(error.to_string().contains("at most 255"), "{error}");
}

#[test]
fn resolve_output_plan_replaces_a_full_length_name_whose_rename_would_be_overlong() {
    let temp_dir = TempDir::new().expect("temp dir");
    let requested = temp_dir.path().join(format!("{}.m4b", "a".repeat(243)));
    let preview = temp_dir
        .path()
        .join(format!("{}.preview.m4b", "a".repeat(243)));
    write(&preview, b"existing preview").expect("write existing preview");

    let plan = resolve_output_plan(
        &requested,
        OutputKind::Preview,
        CollisionPolicy::ReplaceExisting,
        &HashSet::new(),
        &[],
    )
    .expect("a valid replacement is planned");

    assert_eq!(plan.action, PlannedOutputAction::ReplaceExisting);
    assert_eq!(plan.resolved_path, preview);
}

#[test]
fn output_plan_ledger_preserves_duplicate_ordering() {
    let temp_dir = TempDir::new().expect("temp dir");
    let requested_path = temp_dir.path().join("book.m4b");
    let source_paths = Vec::new();
    let mut ledger = OutputPlanLedger::new();

    let first = ledger
        .resolve(
            &requested_path,
            OutputKind::Final,
            CollisionPolicy::RenameNew,
            &source_paths,
        )
        .expect("first plan");
    let second = ledger
        .resolve(
            &requested_path,
            OutputKind::Final,
            CollisionPolicy::RenameNew,
            &source_paths,
        )
        .expect("second plan");
    let third = ledger
        .resolve(
            &requested_path,
            OutputKind::Final,
            CollisionPolicy::RenameNew,
            &source_paths,
        )
        .expect("third plan");

    assert_eq!(first.action, PlannedOutputAction::Write);
    assert_eq!(first.resolved_path, requested_path);
    assert_eq!(second.action, PlannedOutputAction::RenameNew);
    assert_eq!(second.resolved_path, temp_dir.path().join("book-1.m4b"));
    assert_eq!(third.action, PlannedOutputAction::RenameNew);
    assert_eq!(third.resolved_path, temp_dir.path().join("book-2.m4b"));
    assert_eq!(ledger.claimed_paths().len(), 3);
}

#[test]
fn output_plan_ledger_does_not_claim_skipped_outputs() {
    let temp_dir = TempDir::new().expect("temp dir");
    let existing_path = temp_dir.path().join("book.m4b");
    write(&existing_path, b"existing").expect("write existing file");
    let source_paths = Vec::new();
    let mut ledger = OutputPlanLedger::new();

    let first = ledger
        .resolve(
            &existing_path,
            OutputKind::Final,
            CollisionPolicy::SkipExisting,
            &source_paths,
        )
        .expect("first plan");
    assert_eq!(first.action, PlannedOutputAction::SkipExisting);
    assert!(ledger.claimed_paths().is_empty());

    let second = ledger
        .resolve(
            &existing_path,
            OutputKind::Final,
            CollisionPolicy::RenameNew,
            &source_paths,
        )
        .expect("second plan");

    assert_eq!(second.action, PlannedOutputAction::RenameNew);
    assert_eq!(second.resolved_path, temp_dir.path().join("book-1.m4b"));
    assert_eq!(ledger.claimed_paths().len(), 1);
}

#[test]
fn output_plan_ledger_detects_preview_suffix_collisions() {
    let temp_dir = TempDir::new().expect("temp dir");
    let requested_path = temp_dir.path().join("book.m4b");
    let preview_path = temp_dir.path().join("book.preview.m4b");
    write(&preview_path, b"existing preview").expect("write existing preview");
    let source_paths = Vec::new();
    let mut ledger = OutputPlanLedger::new();

    let plan = ledger
        .resolve(
            &requested_path,
            OutputKind::Preview,
            CollisionPolicy::Fail,
            &source_paths,
        )
        .expect("preview plan");

    assert_eq!(plan.requested_path, preview_path);
    assert_eq!(plan.action, PlannedOutputAction::ReviewRequired);
    assert_eq!(
        plan.collision.as_ref().map(|value| value.kind),
        Some(OutputCollisionKind::ExistingFile)
    );
    assert_eq!(
        plan.rename_candidate,
        Some(temp_dir.path().join("book.preview-1.m4b"))
    );
}
