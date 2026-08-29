# Status Panel

## Scope

- Solid Status Panel view under `src/ui/statusPanel/`.
- Preview submit, status runtime, and the status view atom live in
  `src/app/processing`. This owner renders that view and re-exports the leftover
  UI strip processing callers still import.

## Public API Strip

- Import from `src/ui/statusPanel`. The runtime export surface is `index.ts`,
  pinned by `__tests__/runtime-api-contract.test.ts`.
- Exports: `StatusPanelView`, `initStatusPanel`, `isStatusPanelProcessing`,
  `pushStatusPanelTransientStatus`, `readProcessingRequestConfig`,
  `triggerCancelAllFromStatusPanel`, `triggerProcessFromStatusPanel`.

## Private Cluster

- Files: `StatusPanelView.tsx`, `statusPanelView.css`.

## Cross-Strip Coupling

- `StatusPanelView` reads `statusViewAtom` and submits through
  `startProcessingAtom`.
- Do not add a local status store or restore `updateStatusPanelConcurrencyStatus`.

## Breaking-Change Triggers

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing Status Panel as a WorkRuntime consumer or a poke API for
  concurrency text.
