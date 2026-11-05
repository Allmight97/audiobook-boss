use crate::audio::CleanupGuard;
use crate::output_artifact::{
    commit_output_artifact, commit_supplemental_output_assets_for_output, CollisionPolicy,
    OutputCollisionKind, OutputCommitRequest, OutputKind, OutputPlanLedger, PlannedOutputAction,
    SupplementalOutputAssetsCommitRequest,
};
use tempfile::TempDir;

#[test]
fn output_artifact_plan_contract_blocks_source_destination_overlap() {
    let temp_dir = TempDir::new().expect("temp dir");
    let source = temp_dir.path().join("book.mp3");
    std::fs::write(&source, b"source").expect("write source");

    let requested = temp_dir.path().join("book.mp3");
    assert_eq!(requested, source);
    let mut ledger = OutputPlanLedger::new();
    let plan = ledger
        .resolve(
            &requested,
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
    assert_eq!(plan.rename_candidate, None);

    let public = plan.to_public(0, &source);
    assert_eq!(public.input_index, 0);
    assert_eq!(public.kind, OutputKind::Final);
    assert!(public.review.is_some_and(|review| !review.can_proceed));
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

#[test]
fn a_240_byte_final_name_without_a_companion_pdf_plans_and_commits() {
    let temp_dir = TempDir::new().expect("temp dir");
    let staged = temp_dir.path().join("staged.m4b");
    std::fs::write(&staged, b"payload").expect("write staged output");
    let requested = temp_dir.path().join(format!("{}.m4b", "a".repeat(236)));

    let plan = OutputPlanLedger::new()
        .resolve(&requested, OutputKind::Final, CollisionPolicy::Fail, &[])
        .expect("a name that fits plans");
    assert_eq!(plan.action, PlannedOutputAction::Write);

    let mut cleanup_guard = CleanupGuard::new("long-name".to_string());
    commit_output_artifact(
        OutputCommitRequest::new(&plan.resolved_path, plan.action),
        staged,
        &mut cleanup_guard,
        || false,
    )
    .expect("a name that fits commits");
    assert!(requested.exists());
}
