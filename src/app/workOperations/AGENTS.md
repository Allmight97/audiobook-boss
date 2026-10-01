# Work Operations

## Scope

- Owns the frontend read model for WorkRuntime operations, operation and
  title cancel (`cancel(operationId, childJobId?)`), revealing a completed
  title's exported file (`revealOutput`) under `src/app/workOperations/`. Only
  whole-operation cancels track pending state; title cancels are idempotent
  in the backend and appear in the returned snapshot.
- Solid view lives in `src/ui/workCenter`. It renders this owner; it does not
  keep a second operation store.

## Public API Strip

- Import `createWorkOperationsOwner` and owner/view types from
  `src/app/workOperations`. Merge helpers are private.
- Workbench callers that only need the composed UI strip import
  `src/ui/workCenter` instead.
- `index.ts` is the export surface. Do not import `runtime.ts` or `model.ts`
  from outside this owner.

## Hard Invariants

- Render only backend-authored WorkRuntime snapshot events
  (`work-operation-snapshot`, `work-operation-list-snapshot`). The
  `OperationSnapshot` is the sole progress source for accepted background
  operations.
- Do not subscribe to `processing-progress` or apply client-authored progress
  overlays for background work.
- Terminal operation status is backend-canonical through
  `abb_processing_core::classify_run_terminal`. Do not recalculate success,
  mixed, failed, skipped, or cancelled outcomes.
- Keep the highest per-operation revision across event/list/cancel responses.
  Only current list membership may prune history; retain operations created
  after a list's membership revision and reject late resurrection of removed
  members. Terminal effects use accepted model truth. Reset invalidates pending
  responses before a new session can publish.
- What happens to staged downloads when an export ends is the engine's
  (`crates/abb-engine/src/session/AGENTS.md`); this owner only renders.
- Do not own processing submission, metadata staging, output-plan review, or
  provider auth.

## Testing

- `state.test.ts` pins listener dispose, list/event merging, and reveal
  rejection.
- Work Center UI strip is pinned by
  `src/ui/workCenter/__tests__/runtime-api-contract.test.ts`.
- `runtime-api-contract.test.ts` independently pins the app owner export strip.

## Boundary Changes

- Adding, removing, or renaming a public export.
- Reintroducing `processing-progress` overlay consumption for background
  operations.
