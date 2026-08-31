# Status Panel

## Scope

- Solid Status Panel view under `src/ui/statusPanel/`.
- Preview submit and status runtime live in `src/app/processing`. This owner
  renders that view.

## Public API Strip

- Import from `src/ui/statusPanel`. The runtime export surface is `index.ts`,
  pinned by `__tests__/runtime-api-contract.test.ts`.
- Exports: `StatusPanelView`, `initStatusPanel`, `isStatusPanelProcessing`,
  `pushStatusPanelTransientStatus`, `readProcessingRequestConfig`,
  `triggerCancelAllFromStatusPanel`, `triggerProcessFromStatusPanel`.
- The non-view names are #471 compatibility re-exports over process-wide
  Processing state. Do not add callers; the target index exports the view while
  callers use the runtime Processing owner.

## Private Cluster

- Files: `StatusPanelView.tsx`, `statusPanelView.css`.

## Cross-Strip Coupling

- `StatusPanelView` reads Processing `status` and submits through
  `processing.start`.
- Do not add a local status store or restore `updateStatusPanelConcurrencyStatus`.

## Breaking-Change Triggers

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing Status Panel as a WorkRuntime consumer or a poke API for
  concurrency text.
