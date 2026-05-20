use crate::audio::CleanupGuard;
use crate::output_artifact::{
    commit_output_artifact, enforce_output_plan_review, ensure_output_parent_dirs,
    finalized_output_success, CollisionPolicy, OutputCollisionKind, OutputCommitRequest,
    OutputKind, OutputPlanLedger, OutputPlanReview, PlannedOutputAction,
};
use tempfile::TempDir;

#[test]
fn output_artifact_plan_contract_blocks_source_destination_overlap() {
    let temp_dir = TempDir::new().expect("temp dir");
    let source = temp_dir.path().join("book.m4b");
    std::fs::write(&source, b"source").expect("write source");

    let mut ledger = OutputPlanLedger::new();
    let plan = ledger
        .resolve(
            &source,
            OutputKind::Final,
            CollisionPolicy::ReplaceExisting,
            std::slice::from_ref(&source),
        )
        .expect("resolve output plan");

    assert_eq!(plan.action, PlannedOutputAction::ReviewRequired);
    assert_eq!(
        plan.collision.as_ref().map(|collision| collision.kind),
        Some(OutputCollisionKind::SourceDestinationOverlap)
    );

    let public = plan.to_public(Some(0), Some(&source));
    assert_eq!(public.input_index, Some(0));
    assert_eq!(public.kind, OutputKind::Final);
    assert!(public.review.is_some_and(|review| !review.can_proceed));
}

#[test]
fn output_artifact_review_contract_requires_fresh_review_signature() {
    let temp_dir = TempDir::new().expect("temp dir");
    let existing = temp_dir.path().join("existing.m4b");
    std::fs::write(&existing, b"existing").expect("write existing");

    let mut ledger = OutputPlanLedger::new();
    let plan = ledger
        .resolve(&existing, OutputKind::Final, CollisionPolicy::Fail, &[])
        .expect("resolve output plan");

    let err = enforce_output_plan_review(
        OutputPlanReview {
            expected_signature: None,
            current_signature: "sig",
            collision_policy: CollisionPolicy::RenameNew,
        },
        [&plan],
    )
    .expect_err("policy selections require reviewed signature");
    assert!(err
        .to_string()
        .contains("Collision policy selections require"));

    let err = enforce_output_plan_review(
        OutputPlanReview {
            expected_signature: Some("old"),
            current_signature: "new",
            collision_policy: CollisionPolicy::RenameNew,
        },
        [&plan],
    )
    .expect_err("stale signature should fail");
    assert!(err.to_string().contains("collision state changed"));
}

#[test]
fn output_artifact_parent_dir_contract_only_creates_writable_destinations() {
    let temp_dir = TempDir::new().expect("temp dir");
    let writable = temp_dir.path().join("write").join("book.m4b");
    let skipped = temp_dir.path().join("skip").join("book.m4b");

    let mut ledger = OutputPlanLedger::new();
    let write_plan = ledger
        .resolve(&writable, OutputKind::Final, CollisionPolicy::Fail, &[])
        .expect("write plan");
    let mut skip_plan = ledger
        .resolve(&skipped, OutputKind::Final, CollisionPolicy::Fail, &[])
        .expect("skip plan");
    skip_plan.action = PlannedOutputAction::SkipExisting;

    ensure_output_parent_dirs([&write_plan, &skip_plan]).expect("ensure parents");

    assert!(temp_dir.path().join("write").exists());
    assert!(!temp_dir.path().join("skip").exists());
}

#[test]
fn output_artifact_commit_contract_promotes_temp_output_and_reports_success() {
    let temp_dir = TempDir::new().expect("temp dir");
    let worker_dir = temp_dir.path().join("worker");
    std::fs::create_dir_all(&worker_dir).expect("create worker dir");
    let temp_output = worker_dir.join("staged.m4b");
    std::fs::write(&temp_output, b"payload").expect("write temp output");
    let final_output = temp_dir.path().join("final").join("book.m4b");

    let mut cleanup_guard = CleanupGuard::new("output-artifact-contract".to_string());
    cleanup_guard.add_path(&worker_dir);
    cleanup_guard.add_path(&temp_output);

    let request = OutputCommitRequest::new(&final_output, PlannedOutputAction::Write);
    let outcome = commit_output_artifact(request, temp_output, &mut cleanup_guard, || false)
        .expect("commit output");

    assert_eq!(outcome.final_output, final_output);
    assert!(final_output.exists());
    assert!(!outcome.cancelled);

    let preview_success = finalized_output_success(OutputKind::Preview, &final_output, false);
    assert_eq!(preview_success.ui_message, "Preview created successfully");
    assert!(preview_success.result_message.contains("preview"));

    let final_success = finalized_output_success(OutputKind::Final, &final_output, false);
    assert_eq!(final_success.ui_message, "Processing complete");
    assert!(final_success.result_message.contains("audiobook"));
}
