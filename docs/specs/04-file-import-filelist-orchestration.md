# File Import and FileList Orchestration - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose

Outcome: local file import and FileList frontend workflows have clear ownership
for import append/dedupe, metadata draft staging, selection/order changes, and
output refresh. Duplicate handling has one UI workflow owner instead of
parallel message/status paths.

Acceptance signal: `fileList/actions.ts` is split or simplified along coherent
workflow boundaries, duplicate import status behavior is tested once through a
shared result path, and existing reorder, stage, select, and output-refresh
behavior remains stable.

## Current Evidence

- `src/ui/fileImport/AGENTS.md` defines the Local Audio Import Boundary sequence:
  order-lock check, backend-supported import metadata/discovery, backend
  analysis, pending metadata draft staging, append to file list, and visible
  status/error feedback.
- `src/ui/fileImport/importAnalysisWorkflow.ts` owns Effect-based import
  orchestration, backend discovery, metadata draft staging before import, and
  duplicate-only status handling.
- `src/ui/fileList/actions.ts` still owns append/dedupe, metadata staging and
  persistence, selection, sorting, clearing, ordering, totals, and output
  refresh.
- `src/ui/fileImport/importAnalysisWorkflow.ts` and `src/ui/fileList/actions.ts`
  both participate in the "No new files added" duplicate path.
- Existing tests cover import workflow, file-list keyboard handling, reorder,
  selection transition, pending metadata save, and file-import layout behavior.

## Decision Log

- Decision: keep this as a frontend workflow ownership cleanup, not TS/Rust
  contract drift.
  Rationale: Rust already owns local-audio importability and file analysis;
  remaining friction is UI-local orchestration and duplicated duplicate-status
  policy.
  Date: 2026-05-28.
- Decision: do not merge this with processing terminal truth.
  Rationale: processing owns backend lifecycle and terminal summaries; FileList
  owns pre-processing workbench state and user interaction.
  Date: 2026-05-28.

## Scope

In scope:

- Centralize import duplicate append/dedupe result calculation into one UI-local
  helper consumed by import analysis and FileList append.
- Split or simplify `fileList/actions.ts` by workflow responsibility if it
  reduces scan cost: append/import, metadata staging/persist, selection/order,
  totals/output refresh.
- Preserve the Local Audio Import Boundary sequence.
- Preserve local keyboard/focus ownership from the recent import ingress work.
- Add focused frontend tests for duplicate status, append behavior, metadata
  staging before import, selection/order behavior, and output refresh triggers.

Out of scope:

- Backend importability/path validation.
- Metadata intent validation contract changes.
- App Settings or durable preference behavior.
- Processing terminal truth.
- Output size estimate redesign unless a touched test exposes a direct
  dependency.
- Remote acquisition; future remote source work should use the cleaned local
  import boundary rather than duplicating import analysis.

Constraints:

- Do not reintroduce frontend supported-audio allowlists.
- Do not move backend validation into FileList helpers.
- Preserve current user-visible selection, ordering, metadata staging, and
  duplicate-status behavior unless explicitly changed and tested.
- Keep Tauri runtime calls routed through `tauriClient` or injected workflow
  services.

## Plan Of Work

- Extract a shared UI-local duplicate/append result helper with a result shape
  that both import workflow and FileList append can consume.
- Split `fileList/actions.ts` only where it creates real ownership clarity; do
  not add a compatibility barrel unless call sites require it.
- Move branch-heavy metadata staging/selection/output refresh helpers into
  named private modules when that reduces scan cost.
- Update tests to assert duplicate result/status behavior through the shared
  path.
- Keep public exports stable where possible; if exports change, update all
  callers rather than preserving obsolete aliases.

## Proof Path

- Focused Bun/Vitest tests for file import workflow and FileList behavior.
- Existing keyboard/focus tests for file-list/import region behavior.
- `bun scripts/proof/runner.ts focus frontend` for UI-only changes.
- `bun scripts/proof/runner.ts focus runtime` if runtime contracts or shared
  import surfaces change.
- `bun scripts/proof/runner.ts review` before handoff.

## Cleanup Trigger

When implemented, reviewed, validated, docs-aligned, and synced:

- Delete this spec.
- Distill enduring frontend workflow ownership guidance into the nearest
  `AGENTS.md` only if future agents need it.
