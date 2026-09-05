# Output Panel

## Scope

- Solid Output workbench view under `src/ui/outputPanel/`.
- Output directory, naming, path preview, estimate, and collision review truth
  live in `src/app/outputPlan`. This owner renders that view and re-exports the
  submit/hydration strip processing still composes.

## Public API Strip

- Import from `src/ui/outputPanel`. The runtime export surface is `index.ts`,
  pinned by `__tests__/runtime-api-contract.test.ts`.
- The non-view exports are compatibility re-exports. New Processing and
  Settings callers use `runtime.output` directly; do not extend that UI strip
  for application coordination.

## Private Cluster

- Files: `OutputView.tsx`, `outputView.css`.

## Cross-Strip Coupling

- `OutputView` reads Output Plan `view`.
- Estimated size is rendered in the encoder header from Output Plan. Do not
  add a second estimate readout here.

## Boundary Changes

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing a poke API (`updateOutputPath`, `updateEstimatedSize`) or a
  local output store.
