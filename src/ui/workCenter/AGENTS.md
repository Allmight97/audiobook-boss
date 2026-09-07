# Work Center

## Scope

- Solid Work Center view under `src/ui/workCenter/`.
- WorkRuntime snapshots, cancel, source-open, and purge tombstones live in
  `src/app/workOperations`. This owner renders that view.

## Public API Strip

- Import from `src/ui/workCenter`. The runtime export surface is `index.ts`,
  pinned by `__tests__/runtime-api-contract.test.ts`.

## Private Cluster

- Files: `WorkCenterView.tsx`, `workCenterView.css`.

## Cross-Strip Coupling

- `WorkCenterView` reads Work Operations `view` and calls
  `workOperations.cancel`.
- Do not add a local operation store or subscribe to `processing-progress`.

## Boundary Changes

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing client-authored progress overlays for background operations.
