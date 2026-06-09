use crate::audio::CleanupGuard;
use crate::output_artifact::{
    commit_output_artifact, commit_supplemental_output_assets_for_output,
    enforce_output_plan_review, ensure_output_parent_dirs, finalized_output_success,
    CollisionPolicy, OutputCollisionKind, OutputCommitRequest, OutputKind, OutputPlanLedger,
    OutputPlanReview, PlannedOutputAction, SupplementalOutputAssetsCommitRequest,
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

    let _cleanup = ensure_output_parent_dirs(temp_dir.path(), [&write_plan, &skip_plan])
        .expect("ensure parents");

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

#[test]
fn supplemental_output_asset_contract_gates_preview_and_commits_multiple_final_assets() {
    let temp_dir = TempDir::new().expect("temp dir");
    let first_bytes = b"%PDF-1.7\nfirst";
    let second_bytes = b"%PDF-1.7\nsecond";
    let first = temp_dir.path().join("first.pdf");
    let second = temp_dir.path().join("second.pdf");
    std::fs::write(&first, first_bytes).expect("write first pdf");
    std::fs::write(&second, second_bytes).expect("write second pdf");
    let preview_audio = temp_dir.path().join("Preview.m4b");
    let final_audio = temp_dir.path().join("Book.m4b");

    commit_supplemental_output_assets_for_output(
        SupplementalOutputAssetsCommitRequest::new(OutputKind::Preview, &preview_audio).with_asset(
            &first,
            first_bytes.len() as u64,
            &abb_media_core::sha256_hex(first_bytes),
        ),
    )
    .expect("preview should not commit supplemental assets");

    assert!(
        !temp_dir
            .path()
            .join("Preview - Supplemental PDF.pdf")
            .exists(),
        "preview outputs must not commit final-sidecar supplemental assets"
    );

    commit_supplemental_output_assets_for_output(
        SupplementalOutputAssetsCommitRequest::new(OutputKind::Final, &final_audio)
            .with_asset(
                &first,
                first_bytes.len() as u64,
                &abb_media_core::sha256_hex(first_bytes),
            )
            .with_asset(
                &second,
                second_bytes.len() as u64,
                &abb_media_core::sha256_hex(second_bytes),
            ),
    )
    .expect("final should commit supplemental assets");

    assert_eq!(
        std::fs::read(temp_dir.path().join("Book - Supplemental PDF.pdf"))
            .expect("read first commit"),
        first_bytes
    );
    assert_eq!(
        std::fs::read(temp_dir.path().join("Book - Supplemental PDF (2).pdf"))
            .expect("read second commit"),
        second_bytes
    );
}

#[test]
fn supplemental_output_asset_contract_reports_partial_success_failure() {
    let temp_dir = TempDir::new().expect("temp dir");
    let good_bytes = b"%PDF-1.7\ngood";
    let stale_original = b"%PDF-1.7\noriginal";
    let stale_changed = b"%PDF-1.7\nchanged";
    let good = temp_dir.path().join("good.pdf");
    let stale = temp_dir.path().join("stale.pdf");
    std::fs::write(&good, good_bytes).expect("write good pdf");
    std::fs::write(&stale, stale_original).expect("write stale original");
    let stale_hash = abb_media_core::sha256_hex(stale_original);
    std::fs::write(&stale, stale_changed).expect("write stale changed");
    let final_audio = temp_dir.path().join("Book.m4b");

    let error = commit_supplemental_output_assets_for_output(
        SupplementalOutputAssetsCommitRequest::new(OutputKind::Final, &final_audio)
            .with_asset(
                &good,
                good_bytes.len() as u64,
                &abb_media_core::sha256_hex(good_bytes),
            )
            .with_asset(&stale, stale_changed.len() as u64, &stale_hash),
    )
    .expect_err("stale second asset should fail after first commit");

    let message = error.to_string();
    assert!(
        message.contains("Audiobook output 'Book.m4b' was created"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("one or more requested Supplemental PDFs could not be committed"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("hash changed"),
        "unexpected error: {message}"
    );
    assert!(
        !message.contains(temp_dir.path().to_string_lossy().as_ref()),
        "failure message should sanitize output path: {message}"
    );
    assert!(
        temp_dir.path().join("Book - Supplemental PDF.pdf").exists(),
        "already-committed supplemental asset should remain with audio output"
    );
}
