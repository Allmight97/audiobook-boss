# File Import and FileList Orchestration - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: FileList and file-import frontend workflows expose clear private
helpers for import append/dedupe, metadata staging, selection/order changes,
and output refresh. Duplicate handling has one UI workflow owner instead of
parallel message/status paths.

Acceptance signal: `fileList/actions.ts` is split or simplified along coherent
workflow boundaries, duplicate import status behavior is tested once through a
shared result path, and existing reorder/stage/select behavior remains stable.

## Progress

- [x] 2026-05-26: Audit validated item `2n` and original item `6` as a
  coherent UI workflow refactor workblock.
- [ ] Split FileList private responsibilities without changing UI behavior.
- [ ] Centralize duplicate import result/status behavior.

## Surprises & Discoveries

- Observation: `fileList/actions.ts` owns append/dedupe, metadata
  validation/staging/persist, selection, sorting, clearing, ordering, and output
  refresh.
  Evidence: `src/ui/fileList/actions.ts`.
- Observation: import duplicate detection and "No new files added" messaging
  exist in both import analysis workflow and fileList append logic.
  Evidence: `src/ui/fileImport/importAnalysisWorkflow.ts` and
  `src/ui/fileList/actions.ts`.
- Observation: tests already depend on current split behavior and intentionally
  suppress duplicate status in one path.
  Evidence: `src/ui/fileImport/__tests__/importAnalysisWorkflow.test.ts` and
  `src/ui/__tests__/fileList-reorder-behavior.test.ts`.

## Three-Order Trace / Blast Radius

- Order 1, duplicated UI workflow facts:
  duplicate-file detection/status handling exists in file import analysis and
  FileList append logic, while FileList actions also owns multiple unrelated
  workflows.
- Order 2, immediate blast radius:
  picker/drop import analysis, append/dedupe status, file selection, metadata
  staging/persist, ordering/reordering, clear/sort behavior, and output preview
  refresh.
- Order 3, downstream effects:
  future file workflow changes can produce duplicate or missing user status,
  accidentally couple metadata staging to import append logic, or make ordering
  regressions hard to localize.

## Decision Log

- Decision: Treat this as frontend workflow ownership cleanup, not TS/Rust
  contract drift.
  Rationale: the duplicated logic is UI-local, while backend importability
  remains Rust-owned.
  Date: 2026-05-26.

## Context And Orientation

- Current repo state checked: `main` is synced with `origin/main`; audit input
  exists at `docs/audit-high-roi-backlog.md`.
- Owning boundaries:
  - Local Audio Import Boundary:
    `src/ui/fileImport/*` and nested AGENTS guidance.
  - FileList UI:
    `src/ui/fileList/*`.
  - Metadata workflow consumers:
    `src/ui/fileList/actions.ts` and metadata helpers.
  - Output preview consumer:
    `src/ui/outputPanel/*`.
- Canon surfaces this spec must not redefine:
  - Rust owns local-audio importability.
  - Frontend import workflows coordinate UI state and status, not backend file
    validity.
  - Metadata validation/normalization belongs to the Rust Metadata Outcome
    boundary and its `validate_metadata_intent_patch` public strip.

## Scope And Constraints

In scope:

- `2n`: import duplicate detection/message duplication.
- Original item `6`: `fileList/actions.ts` orchestration hub.
- Extracting UI-local private helpers for append/dedupe, metadata staging,
  selection, ordering, and output refresh if it improves cohesion.
- Focused frontend tests for unchanged behavior.

Out of scope:

- Backend importability/path validation.
- Metadata intent validation contract changes.
- Encoder/settings capability changes.
- Output size estimate heuristic (`2o`) unless a touched test exposes a direct
  dependency.

Constraints:

- Do not reintroduce frontend supported-audio allowlists.
- Do not move backend validation into fileList helpers.
- Preserve current user-visible selection, ordering, and metadata staging
  behavior unless explicitly changed and tested.

## Plan Of Work

- Edits:
  - Extract duplicate append/dedupe result calculation into one UI-local helper
    consumed by import analysis and fileList append.
  - Split `fileList/actions.ts` by workflow responsibility if doing so reduces
    scan cost: append/import, metadata staging/persist, selection/order, output
    refresh.
  - Update tests to assert result objects and status behavior through the
    shared path.
  - Keep public exports stable or provide a thin compatibility barrel if
    callers are numerous.
- Proof steps:
  - Focused Bun/Vitest tests for file import analysis and fileList behavior.
  - `bun scripts/proof/runner.ts focus frontend` if UI-only.
  - `bun scripts/proof/runner.ts review` only if runtime contract behavior changes.
- Expected repo-visible outcome:
  - The file import/FileList workflow is easier to modify without duplicating
    duplicate-file status rules.

## Interfaces And Dependencies

- Modules/commands/types:
  - `src/ui/fileList/actions.ts`
  - `src/ui/fileImport/importAnalysisWorkflow.ts`
  - `src/ui/fileImport/__tests__/*`
  - `src/ui/__tests__/fileList-*`
- Libraries/external behavior:
  - None.
- Dependency constraints:
  - No new external dependency should be needed.

## Proof Path and Checks

- Targeted checks:
  - File import workflow tests.
  - FileList reorder/staging/selection tests.
- Full gate:
  - `bun scripts/proof/runner.ts focus frontend` for UI-only changes.
  - `bun scripts/proof/runner.ts review` if runtime behavior is touched.
- Manual or visual evidence:
  - Not required unless visible status copy/control flow changes materially.

## Cleanup Trigger

When implemented, reviewed, validated, and synced:

- Delete this spec.
- Distill only enduring frontend workflow ownership guidance into the nearest
  `AGENTS.md` if future agents need it.
