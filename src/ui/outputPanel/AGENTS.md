## Public API Strip
- Import output panel runtime symbols from `src/ui/outputPanel`.
- Exports: `initOutputPanel`, `applyOutputDefaultsFromSettings`, `getState`,
  `readOutputDefaultsFromState`, `readOutputRequestConfig`, `updateOutputPath`,
  `updateEstimatedSize`, `getCurrentMetadata`, `OutputPanelIsland`.

## Private Cluster
- Files: `OutputPanelIsland.svelte`, `actions.ts`, `preview.ts`, `state.svelte.ts`, `outputPlanWorkflow.ts`, `outputPlanWorkflowLive.ts`, `outputPlanWorkflowServices.ts`, `__tests__/`.
- The cluster owns output directory, output naming, output-path preview, estimated-size display, tag preview display, collision review, and output-plan preflight.

## Allowed Agent Edits Without Escalation
- Change internals when focused output panel tests stay green; run
  `bun scripts/proof/runner.ts focus frontend` before handoff and `bun scripts/proof/runner.ts focus runtime`
  when the Public API Strip or runtime contract surfaces change.
- Keep process-boundary output config reads behind `readOutputRequestConfig`.
- Keep App Settings hydration/persistence coordination behind
  `applyOutputDefaultsFromSettings` and `readOutputDefaultsFromState`; do not
  let other panels reach into `state.svelte.ts`.
- Read encoder panel config through its Public API Strip only for derived display, such as estimated size.

## Breaking-Change Triggers
- Adding, removing, or renaming a Public API Strip export.
- Reintroducing encoder/sample-rate process truth into this cluster.
- Letting another panel import `state.svelte.ts`, `preview.ts`, or workflow internals to build process payloads.
