# Work Center

## Scope

- Solid Work Center view under `src/ui/workCenter/`.
- WorkRuntime snapshots, cancel, and output reveal live in
  `src/app/workOperations`. This owner renders that view.

## Public API Strip

- Import from `src/ui/workCenter`. The runtime export surface is `index.ts`,
  pinned by `src/__tests__/public-api-strips.contract.test.ts`.

## Private Cluster

- Files: `WorkCenterView.tsx`, `workCenterView.css`.

## Cross-Strip Coupling

- `WorkCenterView` reads Work Operations `view` and calls
  `workOperations.cancel`. A title row offers Cancel only in a multi-title
  operation whose child snapshot is `cancellable` and not yet cancelling.
  A completed title with an `outputPath` offers a reveal button labelled for
  the host file manager (Finder, File Explorer, or a generic folder).
- A title with a matching engine restart offer shows Restart and Keep Location
  through Processing. Match both operation and input identity so a later export
  never lends its actions to older retained rows; refused restart stays retryable.
- Do not add a local operation store or client-authored progress.

## Boundary Changes

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing client-authored progress overlays for background operations.
