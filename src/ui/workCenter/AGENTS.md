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
  `workOperations.cancel`. A title row offers Cancel only in a multi-title
  operation whose child snapshot is `cancellable` and not yet cancelling.
  A completed title with an `outputPath` offers a reveal button labelled for
  the host file manager (Finder, File Explorer, or a generic folder).
- Do not add a local operation store or subscribe to `processing-progress`.

## Boundary Changes

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing client-authored progress overlays for background operations.
