# Runtime Architecture PR Train

## 1. Purpose / Big Picture

Audiobook Boss has good boundary instincts, but several runtime concepts still have
ownership spread across command orchestration, helper modules, frontend runtime
classes, and review dialogs. This effort deepens those boundaries without adding
process noise or turning GitHub issues into a task ledger.

Good looks like five serial architecture PRs that each make one product concept
easier to reason about and test at its boundary:

1. Output Naming / Collision Planning
2. External FDK Processor Adapter
3. Processing Run Boundary
4. Metadata Draft / Intent Workflow
5. Status Panel Event State Machine

The primary tracking artifact is this spec. It should stay current until all five
PRs land, related docs align, and confirmed-done issues are closed.

## 2. Scope And Constraints

In scope:

- Land five short-lived serial PRs from synced `main`.
- Preserve public behavior unless a PR explicitly documents a product-facing
  change.
- Use existing GitHub issues only when they already map cleanly to a PR.
- Close related issues only after the PR demonstrably satisfies their success
  criteria.
- Keep validation tied to user outcomes: correct output paths, truthful progress,
  stable metadata intent, and reliable UI state.

Non-goals:

- Do not create new GitHub issues for PR1 or PR5.
- Do not create child issues by default when an issue proves broad.
- Do not run five parallel implementation worktrees for this cross-cutting work.
- Do not implement broad compatibility shims or silent fallbacks.
- Do not preserve bad seams purely to keep diffs small.

Hard constraints:

- Issue count must not grow from this planning effort.
- `Closes #...` belongs in PR descriptions only when the PR fully resolves the
  issue.
- `Refs #...` is acceptable for context when an issue is only related or partially
  addressed.
- Keep runtime IPC centralized in `src/lib/tauri/*`.
- Keep metadata intent compile/normalization at the frontend runtime boundary.
- Any fallback addition requires explicit trigger, observable signal, and removal
  condition.

## 3. Solution Posture

The chosen posture is a serial subsystem refactor train. A single giant branch
would make review and rollback harder; five unrelated issue branches would add
tracking churn and stale merge surfaces. Serial PRs preserve the payoff of scoped
commits and reviews while keeping the repo in one write lane.

The narrower option, "just add tests around current seams," is rejected because it
would preserve the same overloaded owners that caused recent output, preview,
cancellation, and metadata intent bugs. The broader option, "redesign runtime
architecture in one branch," is also rejected because it would blur behavioral
proof across output planning, processing lifecycle, metadata writes, and UI event
state.

Scope can broaden only when a PR discovers that the product invariant cannot be
owned coherently without one adjacent move. It should narrow when a deeper seam is
real but belongs to a later PR in this train.

## 4. Context And Orientation

Canonical docs and repo policy:

- `AGENTS.md`
- `PLANS.md`
- `docs/api-map.md`
- `docs/fallbacks.md`

Runtime surfaces:

- `src-tauri/src/commands/audio_processing.rs`
- `src-tauri/src/audio/output_path.rs`
- `src-tauri/src/audio/external_fdk.rs`
- `src-tauri/src/audio/processor/mod.rs`
- `src-tauri/src/audio/job_registry/`
- `src-tauri/src/audio/progress/`
- `src/ui/statusPanel/controller.ts`
- `src/ui/statusPanel/processing.ts`
- `src/ui/collisionDialog/`
- `src/ui/metadataState.ts`
- `src/ui/fileList/actions.ts`
- `src/ui/metadataLookup.ts`
- `src-tauri/src/metadata/`
- `src/lib/tauri/client.ts`
- `src/lib/generated/tauri.ts`

Existing issues:

- `#256` is open for xHE-AAC regression proof and external AAC path alignment. It
  may be referenced by PR2 only after inspection proves the adapter work satisfies
  its current done criteria.
- `#267` is open for metadata-intent doc cleanup. PR4 may close it only if Rust
  and generated contract docs are updated.
- `#268` is a broad pre-1.1 seam cleanup tracker. PRs may reference it, but should
  not close it unless the full stated concern is done.
- `#270` is separate metadata container-routing work. Keep it separate unless PR4
  directly changes container routing.
- `#272` is the Processing Run boundary RFC and is the intended issue for PR3.
- `#273` is the Metadata write intent / album sort RFC and is the intended issue
  for PR4.
- `#277` is open for frontend boundary glue cleanup. It is not a task ledger for
  this train. Use it as a reference for high-ROI wrapper removals only when the
  owning PR already touches that boundary, and close it only after the accepted
  cleanup is complete or explicitly narrowed.

Current setup status:

- `main` is expected to start synced with `origin/main`.
- Existing untracked hook/cache artifacts are outside this effort and should not
  be staged.

## 5. Plan Of Work

### Setup On Main

Branch: `main`

- Create this spec.
- Commit as `doc: plan runtime architecture PR train`.
- Push `main`.
- Verify `HEAD == origin/main`.

### PR1: Output Naming / Collision Planning

Branch: `arch/output-planning-boundary`

Goal: deepen the output planning boundary without changing public IPC behavior.
No new issue.

Backend:

- Split `src-tauri/src/audio/output_path.rs` into an `output_path/` module with a
  facade plus `types`, `naming`, `artifact`, `collision`, and `plan` ownership.
- Add an `OutputPlanLedger` or equivalent owner for claimed outputs, duplicate
  detection, rename candidates, preview suffix planning, and action resolution.
- Preserve `preflight_processing_plan`, `process_audiobook_files`, and
  `preview_output_path` behavior.

Command layer:

- Extract processing plan construction/review enforcement from
  `audio_processing.rs` into an internal planning module.
- Keep dispatch, execution, and result normalization in command orchestration for
  PR1.
- Preserve preflight as side-effect-free; parent directory creation remains
  execution-only.

Frontend:

- Extract output-plan review from `statusPanel/processing.ts` into a dedicated
  review service.
- Keep collision dialog as presentation/state only.
- Preserve the two-preflight flow: initial plan, optional dialog policy, reviewed
  plan with signature.

Docs:

- Update `docs/api-map.md` only if command ownership moves enough that the current
  map would become misleading.

Sub-agent lanes:

- Rust output planning split risk.
- TS review flow extraction.
- Harness and test impact.

### PR2: External FDK Processor Adapter

Branch: `arch/external-fdk-processor-adapter`

Goal: make external FDK a real processor adapter/port.

- Inspect `#256` before implementation.
- Use `Closes #256` only if the PR also satisfies the issue's current fixture
  proof and architecture-doc alignment criteria.
- Otherwise track the adapter work in this spec and use `Refs #256` only if
  contextually useful.

Expected direction:

- Isolate external FFmpeg/FDK execution behind a processor adapter boundary.
- Share progress, cancellation, temp-output, cover/metadata merge, final commit,
  and terminal-result semantics with the normal processor path where appropriate.
- Test adapter command-building, cancel behavior, and finalization without
  requiring real FFmpeg for every case.

Boundary glue cleanup, if the PR touches encoder-panel/toolchain UI (`Refs #277`):

- Remove `initEncoderPanel` from `src/ui/encoderPanel/index.ts` and update tests
  to import `initializeEncoderPanelLogic` directly. This wrapper is dead in
  production and its options parameter is ignored.
- Skip this cleanup if PR2 remains backend-only. Do not pull encoder-panel test
  churn into the adapter PR without a real touched boundary.

### PR3: Processing Run Boundary

Branch: `arch/processing-run-boundary`

Goal: one domain owner for preflight, job lifecycle, execution routing,
cancellation interpretation, terminal results, progress truth, and artifact truth.

- Reuse `#272`.
- Use `Closes #272` only if the new run boundary genuinely owns all acceptance
  points in that issue.
- Otherwise record partial progress here and keep `#272` open.

Expected direction:

- Introduce a boundary shaped roughly like `ProcessingRun::preflight(...)` and
  `ProcessingRun::execute(...)`.
- Move responsibilities out of command orchestration only when the new owner can
  keep tests at the boundary rather than scattering helper tests.

Boundary glue posture (`Refs #277`):

- Do not collapse `tauriClient` and `commandSpecs` as part of PR3. The current
  `tauriClient` object is the frontend runtime boundary; `commandSpecs` is a
  private implementation table. Collapsing them is not a Processing Run concern.
- Keep `statusPanel/errorHelpers.ts` unless PR3's status/error handling work makes
  the names misleading. The helpers are thin, but they preserve local
  status-panel intent.
- Treat `toGeneratedMetadataSource` / `toGeneratedMetadataSources` as optional
  low-ROI cleanup only if PR3 already changes the relevant Tauri client adapter
  path. Do not make it a required PR3 acceptance item.

### PR4: Metadata Draft / Intent Workflow

Branch: `arch/metadata-draft-intent`

Goal: clarify one metadata draft / write-intent workflow across staged UI state,
lookup application, process overlays, and Rust write semantics.

- Reuse `#273`.
- Fold `#267` only if the PR updates Rust and generated metadata-intent docs.
- Keep `#270` separate unless container routing is directly changed.

Expected direction:

- Make preserve/set/clear/recompute intent explicit where a field can have more
  than simple user-edited text semantics.
- Keep ABS/Plex/Apple compatibility rules visible without leaking backend tag
  details into UI state.
- Keep generated bindings and contract tests aligned with any shape changes.

Boundary glue cleanup (`Refs #277`):

- Remove `setMetadataSaveInProgress` and `isMetadataSaveInProgress` wrappers from
  `src/ui/metadataSaveState.ts`. Export/use `metadataSaveInProgressStore`
  directly from `src/ui/fileList/events.ts` and `src/ui/core/actions.ts`.
- If PR4 already touches `src/ui/metadataValidation.ts`, deduplicate
  `getSeriesPartValidationError` and `getSubseriesPartValidationError` behind a
  shared sequence-validation helper and add direct validator coverage. Otherwise
  leave this as optional cleanup; it is safe but not core to the metadata
  draft/intent boundary.

### PR5: Status Panel Event State Machine

Branch: `arch/status-panel-state-machine`

Goal: move status-panel event behavior into a testable event-sequence state
machine while keeping rendering and DOM concerns local to the UI layer.
No new issue.

- Reference `#268` only if useful.
- Close `#268` only if the full broad seam-cleanup concern is actually done.

Expected direction:

- Separate queue/progress event reduction from listener lifecycle, throttling,
  completion timers, feedback, cover-art syncing, and DOM rendering.
- Use reducer-style event-sequence tests for cancellation, skipped jobs, completed
  jobs, progress updates, and terminal delay behavior.

Boundary glue posture (`Refs #277`):

- Do not make `initTagPreview` cleanup part of PR5. It is thin, but it is used by
  `TagPreviewIsland.svelte` in production and belongs to metadata/tag-preview
  ownership, not status-panel state.
- Do not remove status-panel error helpers solely because they are thin. Revisit
  only if the PR5 reducer creates a clearer local error-normalization boundary.

## 6. Progress

- 2026-04-23: Created spec on `main` as the single planning/tracking artifact.
  Confirmed no new GitHub issues should be created for this PR train by default.
- 2026-04-23: Started PR1 on `arch/output-planning-boundary`. Split
  `audio/output_path` into facade/types/naming/artifact/collision/plan modules,
  added `OutputPlanLedger`, extracted command planning/review ownership into
  `commands/audio_processing/plan.rs`, and extracted frontend output-review flow
  into `statusPanel/outputPlanReview.ts`.
- 2026-04-23: PR1 validation passed with `scripts/checks.sh standard`,
  `bun run harness:verify --scenario collision-dialog`, and
  `bun run harness:verify --scenario output-preview`.
- 2026-04-23: Addressed PR1 review feedback by caching output collision
  directory listings and canonical source paths in the output planning ledger.
  Pushed follow-up commit `5049902`.
- 2026-04-23: Reviewed `#277` with local inspection plus targeted scout lanes.
  Integrated only high-ROI boundary glue cleanup into this train: PR4
  metadata-save wrappers, PR2 encoder-panel init only if that UI boundary is
  touched, and optional validator/tag-preview cleanup only at the owning
  boundary.

## 7. Surprises And Discoveries

- `docs/specs/` did not exist at setup time, so creating this spec also creates
  the active-spec directory.
- Current issue state reinforces the no-new-issues rule: `#272` and `#273` are
  clean reuse points, while `#256` and `#268` are broader than the first likely
  PR slices.
- Direct `bun test` does not load the Vite/Svelte rune transform used by this
  repo; frontend targeted tests should run through `bun run test ...`.
- `cargo clippy --workspace --all-targets -- -D warnings` flagged
  `external_fdk::run_external_ffmpeg` as exceeding the line threshold. PR1 added
  a local exception comment only; PR2 is still the owning adapter refactor.
- `#277` overstates two items. `initTagPreview` is not test-only; it is used by
  `TagPreviewIsland.svelte` at mount. `statusPanel/errorHelpers.ts` is thin, but
  its names carry status-panel intent and are not automatic deletion candidates.
- The `tauriClient` / `commandSpecs` double layer is not a PR3 target. The former
  is the public frontend runtime boundary and the latter is a private generated
  command adapter table.

## 8. Decision Log

- 2026-04-23: Use one living spec instead of GitHub issues as the task ledger.
  Reason: this effort needs planning continuity, but the project owner explicitly
  does not want issue count growth.
- 2026-04-23: Use serial PRs from synced `main` instead of five parallel
  implementation worktrees. Reason: these seams share contracts, generated
  bindings, runtime progress semantics, and documentation surfaces; parallel
  implementation would raise stale-branch and rebase cost.
- 2026-04-23: Start with Output Naming / Collision Planning. Reason: it has
  concrete input/output invariants and reduces the blast radius before the heavier
  Processing Run boundary.
- 2026-04-23: Fold `#277` into the PR train only where cleanup follows the
  owning boundary. Reason: frontend wrapper cleanup is useful when it reduces
  tracing inside a boundary already being edited, but creating standalone churn or
  broadening PR3/PR5 around unrelated UI helpers would weaken the roadmap.

## 9. Validation And Acceptance

Setup validation:

- `bash scripts/check-context-surface.sh`
- `git rev-parse HEAD origin/main` after push, with both SHAs equal.

PR1 targeted Rust coverage:

- duplicate output ordering
- rename candidate insertion
- case-insensitive conflicts
- preview suffix collisions
- clean preflight signature
- stale signature rejection
- collision policy without signature rejection
- source-overlap hard block
- preflight creates no parent directories

PR1 targeted TS coverage:

- clean preflight attaches signature without dialog
- hard block shows error and stops
- selected collision policy triggers second preflight
- dialog cancel returns null

Per code PR validation:

- `scripts/checks.sh standard`
- `bun run harness:verify --scenario collision-dialog` when output planning or
  review flow changes.
- `bun run harness:verify --scenario output-preview` when output preview or
  preview behavior changes.
- For `#277` boundary-glue cleanup, run targeted frontend tests for the touched
  module plus the standard gate. Before deleting a wrapper, verify usage with
  `rg` and distinguish production usage from test-only imports.

Acceptance for the entire train:

- All five PRs land.
- Public behavior changes, if any, are documented in the relevant PR and canon
  surfaces.
- Related issues are closed only when confirmed complete.
- The issue count does not grow from this planning effort.
- This spec is deleted after the full effort completes.

## 10. Interfaces And Dependencies

Public interfaces expected to remain stable in PR1:

- `preflight_processing_plan`
- `process_audiobook_files`
- `preview_output_path`
- frontend `tauriClient` call sites and generated binding shapes unless a later
  PR explicitly changes the contract

Dependencies to keep aligned:

- Rust command implementations and `src-tauri/src/ipc_contract.rs`
- generated TypeScript bindings in `src/lib/generated/tauri.ts`
- frontend runtime boundary in `src/lib/tauri/client.ts`
- harness scenarios for collision and preview output behavior
- `docs/api-map.md` when command/helper ownership changes
- `docs/fallbacks.md` if any fallback work is touched

## 11. Idempotence And Recovery

Safe restart points:

- After setup commit is pushed and `HEAD == origin/main`, create PR1 from fresh
  `main`.
- If a PR branch gets stale, rebase it onto current `origin/main` before running
  full validation.
- If generated bindings drift, regenerate them using the repo's standard contract
  sync path and commit them with the PR that changed the contract.
- If a sub-agent lane discovers adjacent debt, record it here or in the PR notes;
  do not create an issue unless it meets the high-ROI issue hygiene rules.

Interruption recovery:

- Read this spec first.
- Check `git status --short --branch`.
- Check open PRs and related issues before assuming which phase is active.
- Resume the current PR's validation before moving to the next branch.

## 12. Completion And Cleanup

This spec can be deleted only after:

- PR1 through PR5 are merged.
- `main` is synced with `origin/main`.
- related canon docs are aligned with landed ownership.
- related existing issues are either closed as confirmed done or left open with
  accurate remaining criteria.
- no new issue clutter was introduced as a task ledger.

Do not archive this file in the repo. Delete it as ephemeral planning state once
the full train is complete.
