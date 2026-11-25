use super::OutputPlanLedger;
use crate::output_artifact::{
    CollisionPolicy, OutputCollisionKind, OutputKind, PlannedOutputAction,
};
use abb_output_artifact_core::RenameOption;
use std::fs::write;
use tempfile::TempDir;

#[test]
fn resolve_output_plan_marks_existing_file_for_review_by_default() {
    let temp_dir = TempDir::new().expect("temp dir");
    let existing_path = temp_dir.path().join("book.m4b");
    write(&existing_path, b"existing").expect("write existing file");

    let plan = OutputPlanLedger::new()
        .resolve(
            &existing_path,
            OutputKind::Final,
            CollisionPolicy::Fail,
            &[],
        )
        .expect("plan");

    assert_eq!(plan.action, PlannedOutputAction::ReviewRequired);
    assert_eq!(
        plan.collision.as_ref().map(|value| value.kind),
        Some(OutputCollisionKind::ExistingFile)
    );
    assert_eq!(
        plan.rename,
        RenameOption::Free(temp_dir.path().join("book-1.m4b"))
    );
}

#[test]
fn resolve_output_plan_renames_batch_duplicates() {
    let temp_dir = TempDir::new().expect("temp dir");
    let requested_path = temp_dir.path().join("book.m4b");
    let mut ledger = OutputPlanLedger::new();
    let first = ledger
        .resolve(
            &requested_path,
            OutputKind::Final,
            CollisionPolicy::RenameNew,
            &[],
        )
        .expect("first plan");
    assert_eq!(first.action, PlannedOutputAction::Write);

    let plan = ledger
        .resolve(
            &requested_path,
            OutputKind::Final,
            CollisionPolicy::RenameNew,
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
fn resolve_output_plan_skips_existing_when_requested() {
    let temp_dir = TempDir::new().expect("temp dir");
    let existing_path = temp_dir.path().join("book.m4b");
    write(&existing_path, b"existing").expect("write existing file");

    let plan = OutputPlanLedger::new()
        .resolve(
            &existing_path,
            OutputKind::Final,
            CollisionPolicy::SkipExisting,
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

    let error = OutputPlanLedger::new()
        .resolve(&requested, OutputKind::Final, CollisionPolicy::Fail, &[])
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

    let plan = OutputPlanLedger::new()
        .resolve(
            &requested,
            OutputKind::Preview,
            CollisionPolicy::ReplaceExisting,
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

    let second = ledger
        .resolve(
            &existing_path,
            OutputKind::Final,
            CollisionPolicy::ReplaceExisting,
            &source_paths,
        )
        .expect("second plan");

    assert_eq!(second.action, PlannedOutputAction::ReplaceExisting);
    assert_eq!(second.resolved_path, existing_path);
    assert_eq!(
        second.collision.as_ref().map(|value| value.kind),
        Some(OutputCollisionKind::ExistingFile)
    );
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
        plan.rename,
        RenameOption::Free(temp_dir.path().join("book.preview-1.m4b"))
    );
}

#[test]
fn two_full_length_titles_in_one_run_refuse_replace_and_rename_but_skip() {
    let temp_dir = TempDir::new().expect("temp dir");
    let requested = temp_dir.path().join(format!("{}.m4b", "t".repeat(251)));
    let outcomes: Vec<_> = [
        CollisionPolicy::Fail,
        CollisionPolicy::ReplaceExisting,
        CollisionPolicy::RenameNew,
        CollisionPolicy::SkipExisting,
    ]
    .into_iter()
    .map(|policy| {
        let mut ledger = OutputPlanLedger::new();
        let first = ledger
            .resolve(&requested, OutputKind::Final, policy, &[])
            .expect("the first title plans");
        assert_eq!(first.action, PlannedOutputAction::Write);
        ledger
            .resolve(&requested, OutputKind::Final, policy, &[])
            .map(|plan| (plan.action, plan.rename))
            .map_err(|error| error.to_string())
    })
    .collect();

    let too_long = "A renamed output name would be too long";
    assert_eq!(
        outcomes[0],
        Ok((PlannedOutputAction::ReviewRequired, RenameOption::TooLong))
    );
    assert!(
        matches!(&outcomes[1], Err(error) if error.contains(too_long)),
        "{:?}",
        outcomes[1]
    );
    assert!(
        matches!(&outcomes[2], Err(error) if error.contains(too_long)),
        "{:?}",
        outcomes[2]
    );
    assert_eq!(
        outcomes[3],
        Ok((PlannedOutputAction::SkipExisting, RenameOption::TooLong))
    );
}
