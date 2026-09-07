# Output Artifact Boundary

## Public API Strip
- Import from `crate::output_artifact`, not private child modules.
- Functions: `build_output_path_preview`, `derive_output_artifact_path`, `enforce_output_plan_review`, `ensure_output_parent_dirs`, `commit_output_artifact`, `finalized_output_success`, `commit_supplemental_output_assets_for_output`.
- Types: `OutputCommitRequest`, `OutputParentDirCleanup`, `SupplementalOutputAssetsCommitRequest`, `OutputPlanLedger`, `OutputPlanReview`, `OutputKind`, `CollisionPolicy`, `NamingPreset`, `OutputNamingConfig`, `PlannedOutput`, `PlannedOutputAction`, `OutputReviewRequirement`, `OutputCollisionInfo`, `OutputCollisionKind`, `ResolvedOutputPlan`.
- Pure naming/collision/review data facts are packaged in
  `abb-output-artifact-core`; `src-tauri/src/output_artifact` owns runtime file
  I/O and final commit behavior.

## Private Cluster
- Files: `artifact.rs`, `collision.rs`, `commit.rs`, `commit_tests.rs`, `naming.rs`, `parent_dirs.rs`, `parent_dirs_tests.rs`, `plan.rs`, `review.rs`, `supplemental.rs`, `types.rs`, `contract_tests.rs`.
- The cluster owns artifact path derivation, collision detection, review signatures, parent-dir creation and cleanup of ABB-created empty parent dirs, final artifact commit behavior, destination-adjacent replacement temps, and final-sidecar Supplemental PDF commit behavior.

## Edit Rules
- Change pure output planning rules when `cargo nextest run -p abb-output-artifact-core` stays green.
- Change private implementation files when targeted `audiobook-boss` Nextest
  and Public API Strip checks stay green.
- Add behavior coverage inside this cluster when requested path, resolved path, collision, review, or commit behavior changes.
- Keep final artifact writes and replacement policy here; processor code should ask this boundary for artifact truth.
- Use explicit cross-platform replacement semantics for final artifacts; do not rely on Unix-only rename-over-existing behavior.

## Boundary Changes
- Adding, removing, or renaming any Public API Strip symbol.
- Changing collision/review semantics, source-destination overlap handling, final commit behavior, or success message truth.
- Moving final artifact commit truth outside this boundary.
