# Output Panel

## Public API Strip
- Import output panel runtime symbols from `src/ui/outputPanel`; do not reach into private files.
- Authoritative runtime export surface = `index.ts`, pinned by
  `__tests__/runtime-api-contract.test.ts`. Treat that test as the source of
  truth instead of a hand-listed export set here.
- External Svelte consumers read estimated size through `readEstimatedSizeText()`
  inside reactive markup (`$derived(...)`).
- App Shell reads the live naming preset label through `readOutputNamingSummaryLabel()`.

## Private Cluster
- Files: `OutputPanelIsland.svelte`, `actions.ts`, `preview.ts`, `state.svelte.ts`, `outputPlanWorkflow.ts`, `__tests__/`.
- The cluster owns output directory, output naming, output-path preview, estimated-size display, tag preview display, collision review, and output-plan preflight.

## Allowed Agent Edits Without Escalation
- Change internals when focused output panel tests stay green; run targeted
  Vitest files when proving UI behavior and generated-binding/Public API Strip/runtime
  contract checks when runtime surfaces change.
- Keep process-boundary output config reads behind `readOutputRequestConfig`.
- Keep App Settings hydration/persistence coordination behind
  `applyOutputDefaultsFromSettings` and `readOutputDefaultsFromState`; do not
  let other panels reach into `state.svelte.ts`.
- Read encoder panel config through its Public API Strip only for derived display, such as estimated size.

## Breaking-Change Triggers
- Adding, removing, or renaming a Public API Strip export.
- Reintroducing encoder/sample-rate process truth into this cluster.
- Letting another panel import `state.svelte.ts`, `preview.ts`, or workflow internals to build process payloads.
