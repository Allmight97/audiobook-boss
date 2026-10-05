# Output Panel

## Scope

- Solid Output workbench view under `src/ui/outputPanel/`.
- The engine owns the output directory, naming, path preview, and estimates;
  `src/app/outputPlan` words them and holds collision review. This owner
  renders that view.

## Public API Strip

- Import from `src/ui/outputPanel`. The runtime export surface is `index.ts`,
  pinned by `src/__tests__/public-api-strips.contract.test.ts`.
- Exports: `OutputView`.

## Private Cluster

- Files: `OutputView.tsx`, `outputView.css`.

## Cross-Strip Coupling

- `OutputView` reads Output Plan `view`.
- Output size estimates render beside individual titles in File List. Output
  presents naming and destination without a batch estimate.

## Boundary Changes

- Adding, removing, or renaming a Public API Strip export.
- Adding a local output store or a poke/refresh API.
