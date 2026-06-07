# Work Runtime Lifecycle Retirement — Active Spec

Status: temporary active spec.
Tracker: GitHub issue #361, WB-A.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: WorkRuntime is the single owner for accepted background processing
operation truth. Work Center renders backend-authored operation snapshots, and
legacy processing lifecycle channels are retired or explicitly scoped as
adapters.

Acceptance signal: a user can submit background batch/merge work, keep drafting
in FileList, watch Work Center progress and terminal summaries from backend
snapshots, cancel one operation without cancelling unrelated work, and see
Status Panel/preview/metadata-save behavior classified without duplicate
lifecycle truth.

## Progress

- [x] 2026-06-07: PR #362 lands the backend terminal-truth slice: canonical
  `abb_processing_core::classify_run_terminal`, WorkRuntime status mapping,
  honest fail/cancel summaries, dead `JobState` removal, and API-map/system
  doc updates.
- [ ] After PR #362 lands: finish the remaining frontend and lifecycle-channel
  retirement as one coherent workblock, not a string of small PRs.

## Surprises & Discoveries

- Observation: Work Center still overlays legacy `processing-progress` events
  for fine-grained child progress.
  Evidence: `src/ui/workCenter/model.ts`,
  `src/ui/workCenter/state.svelte.ts`.
- Observation: Status Panel still owns legacy processing and metadata-save
  lifecycle rendering, while WorkRuntime owns accepted background processing
  operation truth.
  Evidence: `src/ui/statusPanel/`, `src-tauri/src/commands/metadata/save_batch.rs`.
- Observation: `process_audiobook_files` remains a direct execution command
  for preview and any legacy survivor until the remaining consumers are
  classified or migrated.
  Evidence: `docs/api-map.md`, `src/ui/statusPanel/processingWorkflow.ts`.

## Decision Log

- Decision: PR #362 is WB-A backend terminal truth, not the full lifecycle
  retirement.
  Rationale: contract-neutral backend consolidation removes drift first and
  keeps the broader frontend migration coherent.
  Date: 2026-06-07.
- Decision: remaining WB-A work should land as one large ownership change when
  feasible.
  Rationale: splitting snapshot-only Work Center, Status Panel migration, and
  legacy event removal into tiny PRs invites correction loops.
  Date: 2026-06-07.
- Decision: use plain workblock language and owned-boundary terms; avoid
  version shorthand for this effort.
  Rationale: "first slice" labels do not add durable repo meaning.
  Date: 2026-06-07.

## Context And Orientation

- Owning backend boundary: `src-tauri/src/work_runtime/`.
- Owning frontend boundary: `src/ui/workCenter/`.
- Related legacy/adapter surfaces: `src-tauri/src/processing/`,
  `src/ui/statusPanel/`, `src-tauri/src/commands/audio.rs`,
  `src-tauri/src/commands/metadata/save_batch.rs`.
- Terms from `docs/ubiquitous-language.md`: Work Operation, Work Center,
  Backend Lifecycle, Operation Result Summary, Terminal Truth.
- Canon surfaces this spec must not redefine: `docs/system-map.md`,
  `docs/ubiquitous-language.md`, `docs/api-map.md`, nearest nested `AGENTS.md`.

## Scope And Constraints

In scope:

- Add backend operation progress detail needed for Work Center to stop consuming
  legacy progress events.
- Move Work Center to backend-authored operation snapshots only.
- Delete the Work Center legacy progress reducer once snapshot truth is rich
  enough.
- Classify Status Panel processing display as either foreground/direct
  execution UI, adapter, or migrated operation consumer.
- Classify preview execution and `process_audiobook_files` survival explicitly.
- Classify metadata-save lifecycle against #307: migrate to WorkRuntime when it
  improves operation truth, or document it as a deliberate adapter.
- Remove legacy processing commands/events/fixtures only when no runtime
  consumer remains.

Out of scope:

- Persistent operation history across app restart.
- Pause/resume, priority editing, multi-instance orchestration, or external
  queue platforms.
- Remote acquisition inbox work unless it becomes necessary for operation
  lifecycle correctness.
- Broad UI redesign of Status Panel or Work Center.

Constraints:

- Do not make `processing/run.rs`, `processingWorkflow.ts`, or Work Center
  modules larger without splitting by owner first.
- Do not remove legacy commands/events while preview, Status Panel, metadata
  save, or Work Center still consumes them.
- Do not let UI invent terminal status.
- Do not reintroduce parallel terminal-classification rules outside the
  canonical processing-core classifier.

## Plan Of Work

Edits:

- Trace current consumers of `processing-progress`, `processing-queue`,
  `process_audiobook_files`, and `cancel_processing`.
- Extend backend operation snapshots with the progress/child fields needed to
  replace the Work Center legacy overlay.
- Replace Work Center legacy-progress application with snapshot updates.
- Delete `src/ui/workCenter/model.ts` paths that exist only to interpret
  legacy progress events.
- Reclassify Status Panel, preview, and metadata-save lifecycle consumers; move
  or explicitly retain each.
- Remove or narrow command/event fixtures after consumers are gone.
- Update `docs/api-map.md`, `docs/system-map.md`, `docs/ubiquitous-language.md`,
  and local `AGENTS.md` files only for enduring ownership rules.

Verification steps:

- Focused Rust tests for WorkRuntime state/progress and processing lifecycle
  adapter behavior.
- Focused Work Center Vitest coverage for snapshot-only progress, terminal
  summaries, cancellation, and multi-operation ordering.
- Status Panel/preview/metadata-save tests for any migrated or retained paths.
- `bash scripts/check-generated-bindings.sh --mode local` if IPC shapes change.
- `bun scripts/check-tauri-runtime-boundary.ts`.
- `git diff --check`.

Expected repo-visible outcome:

- One PR that finishes WB-A's lifecycle ownership change or explicitly documents
  any survivor adapter with owner, trigger, and removal condition.

## Interfaces And Dependencies

- Commands/events: WorkRuntime operation commands/events, legacy processing
  commands/events, metadata-save progress events.
- Types: `OperationSnapshot`, `ChildJobSnapshot`, `ProgressSnapshot`,
  `OperationTerminalSummary`, `OperationKind`, processing progress/queue events.
- Dependency constraints: no new broad proof runner; no generated-binding drift
  without committed regenerated bindings and contract tests.

## Verification Path and Checks

Targeted checks:

- `cargo nextest run -p abb-processing-core`
- `cargo nextest run -p audiobook-boss --lib work_runtime`
- `cargo nextest run -p audiobook-boss --test all_tests <focused filters>`
- `bun run test -- src/ui/workCenter/__tests__/*.test.ts`
- Status Panel and metadata-save focused Vitest files when touched.
- `bash scripts/check-generated-bindings.sh --mode local` when IPC changes.
- `bun scripts/check-tauri-runtime-boundary.ts`
- `git diff --check`

Manual evidence, if static tests cannot prove it:

- Submit background batch, keep drafting, submit/prepare another operation, and
  cancel one operation without cancelling the other.
- Verify terminal summaries match disk-visible results for success, skipped,
  mixed, cancelled, and failed outcomes.

## Cleanup Trigger

When this effort is implemented, rejected, or superseded:

- Delete this spec.
- Distill only enduring behavior into:
  - `docs/system-map.md`
  - `docs/ubiquitous-language.md`
  - `docs/api-map.md`
  - nearest nested `AGENTS.md`
  - GitHub issue #307 if metadata-save lifecycle remains separately tracked
