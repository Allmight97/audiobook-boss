# Work Operations

## Scope

- Owns the frontend read model for WorkRuntime operations, operation-level
  cancel, source-open, and purge-tombstone retention under
  `src/app/workOperations/`.
- Solid view lives in `src/ui/workCenter`. It renders this owner; it does not
  keep a second operation store.

## Public API Strip

- Import Work Operations runtime symbols from `src/app/workOperations`.
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
- `PURGED_OPERATION_TOMBSTONE_CAP` must stay strictly larger than backend
  `TERMINAL_OPERATIONS_CAP`. The contract test pins both sites.
- Keep terminal Input projection and the operation-id tombstone here, then call
  the injected Remote Source owner's `settleTerminalWork` once. Do not import
  private Remote session files or reproduce retain/release/purge sequencing.
- Do not own processing submission, metadata staging, output-plan review, or
  provider auth.

## Testing

- `retention-caps.contract.test.ts` pins the frontend tombstone against the
  backend cap.
- `state.test.ts` pins listener dispose, terminal purge races, and source-open
  rejection.
- Work Center UI strip is pinned by
  `src/ui/workCenter/__tests__/runtime-api-contract.test.ts`.

## Breaking-Change Triggers

- Adding, removing, or renaming a public export.
- Reintroducing `processing-progress` overlay consumption for background
  operations.
- Moving the purge tombstone or shrinking it to the backend cap.
