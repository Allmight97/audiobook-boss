# Work Center

## Scope

- Solid Work Center view under `src/ui/workCenter/`.
- WorkRuntime snapshots, cancel, source-open, and purge tombstones live in
  `src/app/workOperations`. This owner renders that view and re-exports the
  leftover UI strip.

## Public API Strip

- Import from `src/ui/workCenter`. The runtime export surface is `index.ts`,
  pinned by `__tests__/runtime-api-contract.test.ts`.
- Exports: `WorkCenterView`, `initializeWorkCenter`, `workCenterState`,
  `workOperationsViewAtom`.

## Private Cluster

- Files: `WorkCenterView.tsx`, `workCenterView.css`.

## Cross-Strip Coupling

- `WorkCenterView` reads `workOperationsViewAtom` and calls
  `cancelWorkOperation` / `openChildSource`.
- Do not add a local operation store or subscribe to `processing-progress`.

## Breaking-Change Triggers

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing client-authored progress overlays for background operations.
