# Output Panel

## Scope

- Solid Output workbench view under `src/ui/outputPanel/`.
- Output directory, naming, path preview, estimate, and collision review truth
  live in `src/app/outputPlan`. This owner renders that view and re-exports the
  submit/hydration strip processing still composes.

## Public API Strip

- Import from `src/ui/outputPanel`. The runtime export surface is `index.ts`,
  pinned by `__tests__/runtime-api-contract.test.ts`.
- Exports: `OutputView`, `applyOutputDefaultsFromSettings`,
  `readOutputDefaultsFromState`, `readOutputRequestConfig`,
  `runOutputPlanReviewWorkflow`.
- The non-view exports are #471 compatibility re-exports. Do not add callers;
  after migration this index exports the view and any genuinely view-owned
  types only. Processing and Settings use `runtime.output` directly.

## Private Cluster

- Files: `OutputView.tsx`, `outputView.css`.

## Cross-Strip Coupling

- `OutputView` reads Output Plan `view`.
- Estimated size is rendered in the encoder header from Output Plan. Do not
  add a second estimate readout here.

## Breaking-Change Triggers

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing a poke API (`updateOutputPath`, `updateEstimatedSize`) or a
  local output store.
