## Public API Strip
- Import status runtime symbols from `src/ui/statusPanel` unless a local exception is documented.
- Exports: `beginMetadataSaveInStatusPanel`, `completeMetadataSaveInStatusPanel`, `failMetadataSaveInStatusPanel`, `initStatusPanel`, `isStatusPanelProcessing`, `pushStatusPanelTransientStatus`, `triggerCancelAllFromStatusPanel`, `triggerProcessFromStatusPanel`, `updateStatusPanelConcurrencyStatus`, `StatusPanelIsland`.

## Private Cluster
- Files: `controller.ts`, `runtimeApi.ts`, `events.ts`, `feedback.ts`, `formatting.ts`, `outputPlanReview.ts`, `preview.ts`, `processing.ts`, `processingWorkflow.ts`, `processingWorkflowLive.ts`, `processingWorkflowServices.ts`, `processingCancellationWorkflow.ts`, `processingCancellationWorkflowLive.ts`, `processingCancellationWorkflowServices.ts`, `render.ts`, `state.ts`, `viewState.svelte.ts`, `viewTypes.ts`, `domain/`, `services/`, `__tests__/`, `StatusPanelIsland.svelte`.
- The cluster owns progress events, queue snapshots, cancellation, terminal status truth, view-state derivation, and status feedback.

## Allowed Agent Edits Without Escalation
- Change internals when `bun run test -- src/ui/statusPanel/__tests__/runtime-api-contract.test.ts src/ui/statusPanel/__tests__/statusPanel-lifecycle.test.ts src/ui/statusPanel/__tests__/statusPanel-island.test.ts` and `scripts/check-public-api-strips.sh` stay green.
- Test visible status outcomes rather than private reducer shape when behavior is user-facing.
- Keep direct view-state/controller/runtimeApi imports inside this cluster or tests.

## Breaking-Change Triggers
- Adding, removing, or renaming any Public API Strip export.
- Changing progress, cancellation, queue terminalization, success/failure/cancel truth, or concurrency-status behavior.
- Letting external runtime code import `viewState.svelte`, `controller.ts`, `runtimeApi.ts`, or reducer/domain internals directly.
