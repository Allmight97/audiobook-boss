# State Shape Follow-Ups

## 1. Purpose / Big Picture

Close the remaining gaps left by the `1.0.4` state-shape tightening pass and fix the release-preview `SIGPIPE` bug in `scripts/release.sh`.

The status-panel follow-up should make the two reviewed impossible states unrepresentable:

- a new active Rust `EventStage` must force acknowledgement in the frontend helper that classifies active stages
- `ProcessingStatus` idle must always mean `percentage: 0`

This follow-up also scopes in the adjacent `JobProgress.stage` cleanup so job rows cannot carry the UI-only `idle` stage, plus the missing regression coverage around failed/cancelled terminal aggregates.

The release-script follow-up remains separate because it is a tooling boundary with its own regression surface.

“Good” looks like:

- both branches land cleanly on local `main`
- targeted tests, harness proof, and `scripts/checks.sh standard` pass on each branch
- dedicated audit passes find no remaining correctness or plan-alignment issues
- local docs/comments/changelog no longer overclaim guarantees the code does not actually provide

## 2. Scope And Constraints

In scope:

- `status-shape-followups` branch
  - `ProcessingStatus` idle pinned to `0`
  - exhaustive active-stage classification based on `ActiveEventStage`
  - `JobProgress.stage` tightened to `EventStage`
  - status-panel tests updated/added
  - local status comments/changelog sanitized
- `release-preview-pipefail` branch
  - replace `git log ... | head -15` with `git log --max-count=15 ...`
  - add automated regression coverage
  - include the new script test in the standard gate

Out of scope:

- README / canon docs cleanup unless implementation reveals a concrete contradiction
- controller refactors beyond what is required by the type/test changes
- behavioral changes to status rendering, queue semantics, or processing
- release workflow copy polish not required for correctness

Constraints:

- keep the two branches separate
- do not mutate `docs/diagrams/`
- use dedicated review passes with `gpt-5.4` at `xhigh`
- non-doc code/config changes must leave `scripts/checks.sh standard` green

## 3. Context And Orientation

Relevant code surfaces:

- `src/ui/statusPanel/state.ts`
- `src/ui/statusPanel/controller.ts`
- `src/ui/statusPanel/processing.ts`
- `src/ui/statusPanel/__tests__/progressAggregator.test.ts`
- `src/ui/statusPanel/__tests__/statusPanel-lifecycle.test.ts`
- `src/ui/statusPanel/__tests__/renderIncrements.test.ts`
- `src/ui/statusPanel/__tests__/renderOrder.test.ts`
- `CHANGELOG.md`
- `scripts/release.sh`
- `scripts/checks.sh`

Current implementation shape:

- `ActiveEventStage` is derived from generated `EventStage`, but `isActiveEventStage()` still hardcodes `analyzing | converting | writing`
- idle in `ProcessingStatus` is still `percentage: number`
- `JobProgress.stage` is still `ProcessingStatus['stage']`
- stale-field regression coverage exists only for `completed`
- `scripts/release.sh` still uses `git log ... | head -15` under `set -euo pipefail`

Boundary ownership:

- state/type invariants live in `src/ui/statusPanel/state.ts`
- status runtime behavior stays owned by `controller.ts` / `processing.ts`
- release tooling stays isolated to `scripts/*`

## 4. Plan Of Work

### Branch 1: `status-shape-followups`

1. Update `state.ts`
   - idle arm becomes `{ stage: 'idle'; percentage: 0; message: string }`
   - `JobProgress.stage` becomes `EventStage`
   - add exhaustive `ACTIVE_EVENT_STAGES` map typed as `{ readonly [K in ActiveEventStage]: true }`
   - update `isActiveEventStage()` to use the map
   - make `buildStatus()` force idle percentage to `0`
   - revise the local docblocks so they match the real guarantees
2. Update tests
   - add `src/ui/statusPanel/__tests__/state.test.ts`
   - expand terminal stale-field regression to `completed`, `failed`, `cancelled`
   - tighten lifecycle tests for toast exclusivity
   - adjust any `JobProgress` fixtures that need the stricter stage type
3. Update local docs
   - add `[Unreleased]` notes in `CHANGELOG.md`
   - revise any overclaimed wording in the `1.0.4` section that would otherwise remain false after the fix
4. Verify and audit
   - targeted Bun tests
   - `bun run harness:verify --scenario status-processing`
   - `scripts/checks.sh standard`
   - dedicated `gpt-5.4` `xhigh` audit
5. Merge branch into local `main`

### Branch 2: `release-preview-pipefail`

1. Update `scripts/release.sh`
   - replace preview pipes with `--max-count=15`
2. Add `scripts/release.test.ts`
   - temp repo fixture with tag + >15 commits after tag
   - safe stubs for `bun` / `scripts/bump-version.sh`
   - execute the real script with `--no-commit-tag`
   - assert exit `0`
3. Update `scripts/checks.sh`
   - include `scripts/release.test.ts` in the direct Bun script-test step
4. Verify and audit
   - targeted Bun test
   - `bash -n scripts/release.sh`
   - `scripts/checks.sh standard`
   - dedicated `gpt-5.4` `xhigh` audit
5. Merge branch into local `main`

## 5. Progress

- 2026-04-16: Spec created. Current `main` equals `origin/main` at `5f4f310`, only `docs/diagrams/` is untracked. Work starts from branch `status-shape-followups`.

## 6. Surprises And Discoveries

- The repo has no active `docs/specs/<task>.md`; this file is newly created for this follow-up.
- The status-panel cleanup is smaller than a controller refactor; most work is type-level plus tests.
- `JobProgress.stage` tightening is low churn because existing render fixtures already use real wire stages.
- The release-script regression test needs `scripts/checks.sh` updated or it would not run in the standard gate.

## 7. Decision Log

- Include `JobProgress.stage` now rather than keeping it deferred.
- Keep the release-script fix on a separate branch from the status-panel follow-up.
- Do not broaden into README or canon-doc cleanup unless implementation finds an actual contradiction.
- Keep `processing.ts` behavior unchanged unless integration forces a trivial consistency edit.

## 8. Validation And Acceptance

Branch 1:

- `bun run test -- src/ui/statusPanel/__tests__/state.test.ts src/ui/statusPanel/__tests__/progressAggregator.test.ts src/ui/statusPanel/__tests__/statusPanel-lifecycle.test.ts src/ui/statusPanel/__tests__/renderIncrements.test.ts src/ui/statusPanel/__tests__/renderOrder.test.ts`
- `bun run harness:verify --scenario status-processing`
- `scripts/checks.sh standard`
- final read-only audit using `gpt-5.4` with `xhigh`

Branch 2:

- `bun test scripts/release.test.ts`
- `bash -n scripts/release.sh`
- `scripts/checks.sh standard`
- final read-only audit using `gpt-5.4` with `xhigh`

Acceptance:

- no open audit findings remain on either branch
- local `main` contains both merged branches
- `docs/specs/state-shape-followups.md` can be deleted

## 9. Interfaces And Dependencies

Public/type changes:

- `ProcessingStatus` idle variant narrows to `percentage: 0`
- `JobProgress.stage` narrows to `EventStage`

Dependent surfaces:

- status-panel tests and any render fixtures that construct `JobProgress`
- `CHANGELOG.md` must stay aligned with the landed guarantees
- `scripts/checks.sh` must stay aligned with any new direct Bun script tests

## 10. Idempotence And Recovery

- Branches can be recreated from current `main` if implementation stalls.
- The targeted tests and harness scenario are safe to rerun.
- If either branch audit finds drift, fix the branch before merging, then rerun only the touched-target tests plus the full required gate.

## 11. Completion And Cleanup

Before deleting this spec:

- both branches are merged into local `main`
- audits are clean
- required docs/comments/changelog are aligned
- verification commands passed on the final merged states

Delete this file after both branches are complete; do not archive it in-repo.
