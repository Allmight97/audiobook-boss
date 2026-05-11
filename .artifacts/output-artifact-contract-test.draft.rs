//! DRAFT — Output Artifact Plan / Commit contract test seed.
//!
//! This file lives under .artifacts/ on purpose. It is NOT included in the
//! Cargo build. It is here so the repo owner can review the shape of the
//! Phase P0 contract test before it lands at
//! `src-tauri/src/audio/output_path/contract_tests.rs` and is wired into
//! `mod.rs` via `#[cfg(test)] mod contract_tests;`.
//!
//! Promotion plan: see docs/specs/grey-box-modules.md, section
//! "Promotion And Hardening Plan" → Phase P0, and the matching
//! "Seed: Output Artifact Plan / Commit Contract Test Sketch".
//!
//! Hard rule: this contract test must only depend on the Public API Strip
//! re-exported from `src-tauri/src/audio/output_path/mod.rs`. If a future
//! contract test needs a Private Cluster helper, the helper either becomes
//! public on purpose or the test moves closer to the cluster as an internal
//! test.

use std::path::PathBuf;

use crate::audio::output_path::{
    build_output_path_preview,
    CollisionPolicy,
    NamingPreset,
    OutputCollisionInfo,
    OutputCollisionKind,
    OutputKind,
    OutputNamingConfig,
    OutputReviewRequirement,
    PlannedOutput,
    PlannedOutputAction,
};
use crate::metadata::AudiobookMetadata;

#[test]
fn public_api_strip_is_stable() {
    let _kinds = [OutputKind::Final, OutputKind::Preview];
    let _policies = [
        CollisionPolicy::Fail,
        CollisionPolicy::ReplaceExisting,
        CollisionPolicy::RenameNew,
        CollisionPolicy::SkipExisting,
    ];
    let _actions = [
        PlannedOutputAction::Write,
        PlannedOutputAction::ReplaceExisting,
        PlannedOutputAction::RenameNew,
        PlannedOutputAction::SkipExisting,
        PlannedOutputAction::ReviewRequired,
    ];
    let _collision_kinds = [
        OutputCollisionKind::ExistingFile,
        OutputCollisionKind::BatchDuplicate,
        OutputCollisionKind::SourceDestinationOverlap,
        OutputCollisionKind::CanonicalPathOverlap,
        OutputCollisionKind::CaseInsensitiveMatch,
    ];
    let _presets = [NamingPreset::AbsDefault, NamingPreset::CustomTemplate];

    let _planned = PlannedOutput {
        input_index: Some(0),
        input_path: Some("/tmp/in.m4b".into()),
        kind: OutputKind::Final,
        requested_path: "/tmp/out.m4b".into(),
        resolved_path: "/tmp/out.m4b".into(),
        rename_candidate: None,
        collision: Some(OutputCollisionInfo {
            kind: OutputCollisionKind::ExistingFile,
            conflicting_path: Some("/tmp/out.m4b".into()),
            detail: None,
        }),
        review: Some(OutputReviewRequirement {
            can_proceed: true,
            message: "demo".into(),
        }),
        action: PlannedOutputAction::ReviewRequired,
    };
}

#[test]
fn preview_naming_invariant_holds_for_abs_default() {
    let naming = OutputNamingConfig {
        preset: NamingPreset::AbsDefault,
        include_year: false,
        custom_template: None,
    };

    let metadata = AudiobookMetadata {
        title: Some("Grey Smoke Test".into()),
        artist: Some("ABB Lab".into()),
        ..AudiobookMetadata::default()
    };

    let preview: PathBuf = build_output_path_preview(
        &PathBuf::from("/tmp/abb-output"),
        Some(&metadata),
        naming,
        None,
    )
    .expect("preview path");

    assert!(preview.starts_with("/tmp/abb-output/ABB Lab/Grey Smoke Test/"));
    assert_eq!(preview.extension().and_then(|ext| ext.to_str()), Some("m4b"));
}

// Pending invariants for Phase P0 (one assertion per behavior):
//
// 1. OutputPlanLedger::resolve flags source/destination overlap with
//    PlannedOutputAction::ReviewRequired + OutputCollisionKind::SourceDestinationOverlap.
// 2. enforce_output_plan_review rejects non-Fail policies without
//    expected_signature (AppError::InvalidInput).
// 3. enforce_output_plan_review rejects signature mismatch
//    (AppError::FileValidation, "collision state changed").
// 4. ensure_output_parent_dirs creates the parent directory for actions that
//    write and skips actions that do not.
// 5. commit_output_artifact preserves staged temp output if rename fails
//    (TempDir-backed test).
// 6. finalized_output_success returns preview vs final message variants and
//    keeps success messaging when cancellation arrives after commit.
//
// AudiobookMetadata must implement Default for the spread above; if it does
// not, replace `..AudiobookMetadata::default()` with explicit fields when
// lifting this file into src-tauri/.
