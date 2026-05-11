## Public API Strip
- Import from `crate::output_artifact`, not private child modules.
- Functions: `build_output_path_preview`, `derive_output_artifact_path`, `build_output_path`, `action_requires_output_write`, `plan_is_hard_block`, `enforce_output_plan_review`, `ensure_output_parent_dirs`, `commit_output_artifact`, `finalized_output_success`.
- Types: `OutputPlanLedger`, `OutputPlanReview`, `OutputKind`, `CollisionPolicy`, `NamingPreset`, `OutputNamingConfig`, `PlannedOutput`, `PlannedOutputAction`, `OutputReviewRequirement`, `OutputCollisionInfo`, `OutputCollisionKind`, `OutputCollision`, `ResolvedOutputPlan`.

## Private Cluster
- Files: `artifact.rs`, `collision.rs`, `commit.rs`, `naming.rs`, `plan.rs`, `review.rs`, `types.rs`, `contract_tests.rs`.
- The cluster owns artifact path derivation, collision detection, review signatures, parent-dir creation, and final artifact commit behavior.

## Allowed Agent Edits Without Escalation
- Change private implementation files when `cargo test contract_tests` and `scripts/check-public-api-strips.sh` stay green.
- Add behavior coverage inside this cluster when requested path, resolved path, collision, review, or commit behavior changes.
- Keep final artifact writes and replacement policy here; processor code should ask this boundary for artifact truth.

## Breaking-Change Triggers
- Adding, removing, or renaming any Public API Strip symbol.
- Changing collision/review semantics, source-destination overlap handling, final commit behavior, or success message truth.
- Relaxing `scripts/check-public-api-strips.sh` or output-artifact assertions in `scripts/check-no-bridge-imports.sh`.
