# Output Panel

## Scope

- Solid Output workbench view under `src/ui/outputPanel/`.
- Output directory, naming, path preview, estimate, and collision review truth
  live in `src/app/outputPlan`. This owner renders that view.

## Public API Strip

- Import from `src/ui/outputPanel`. The runtime export surface is `index.ts`,
  pinned by `__tests__/runtime-api-contract.test.ts`.
- Exports: `OutputView`.

## Private Cluster

- Files: `OutputView.tsx`, `outputView.css`.

## Cross-Strip Coupling

- `OutputView` reads Output Plan `view`.
- Output size estimates render beside individual titles in File List. Output
  presents naming and destination without a batch estimate.

## Boundary Changes

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing a poke API (`updateOutputPath`, `updateEstimatedSize`) or a
  local output store.
