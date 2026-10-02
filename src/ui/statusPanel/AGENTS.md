# Status Panel

## Scope

- Solid Status Panel view under `src/ui/statusPanel/`.
- Submit (export or preview) and the status runtime live in `src/app/processing`. This owner
  renders that view.

## Public API Strip

- Import from `src/ui/statusPanel`. The runtime export surface is `index.ts`,
  pinned by `__tests__/runtime-api-contract.test.ts`.
- Export only `StatusPanelView`. Callers use the runtime Processing owner for
  status and start/cancel.

## Private Cluster

- Files: `StatusPanelView.tsx`, `statusPanelView.css`.

## Cross-Strip Coupling

- `StatusPanelView` reads Processing `status` and submits through
  `processing.start`.
- Cancel follows the engine preview snapshot, including preparation and queued
  work. A native job ID is not required before offering whole-preview cancel.
- Do not add a local status store.

## Boundary Changes

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing client-authored preview lifecycle or a poke API for
  concurrency text.
