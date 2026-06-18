# Status Panel

## Lifecycle Classification

Status Panel is a **retained foreground/direct adapter**. It is **not** a
WorkRuntime consumer for background operations.

- **Preview execution**: direct `process_audiobook_files` with `previewSeconds`.
  Does not enter WorkRuntime.
- **Metadata batch save**: runs through `beginMetadataSaveInStatusPanel` /
  `completeMetadataSaveInStatusPanel` / `failMetadataSaveInStatusPanel`.
  Emits progress/queue events with `operation_kind: metadataSave` and
  `operation_id: None`.
- **Direct cancellation**: `cancel_processing` targets only foreground/direct
  job IDs. Background operations are cancelled through
  `cancel_work_operation` (Work Center / WorkRuntime).

Status Panel **ignores** operation-scoped events (`operation_id: Some(...)`)
in `applyQueueSnapshot` and `applyProgress` (see
`src/ui/statusPanel/domain/stateMachine.ts`). It only processes
foreground/direct-scoped events (`operation_id: None`).

## Public API Strip
- Import status runtime symbols from `src/ui/statusPanel` unless a local exception is documented.
- Exports: `beginMetadataSaveInStatusPanel`, `completeMetadataSaveInStatusPanel`, `failMetadataSaveInStatusPanel`, `initStatusPanel`, `isStatusPanelProcessing`, `pushStatusPanelTransientStatus`, `triggerCancelAllFromStatusPanel`, `triggerProcessFromStatusPanel`, `updateStatusPanelConcurrencyStatus`, `StatusPanelIsland`.

## Private Cluster
- Files: `controller.ts`, `runtimeApi.ts`, `events.ts`, `metadataSaveFeedback.ts`, `formatting.ts`, `preview.ts`, `processing.ts`, `processingConfig.ts`, `processingWorkflow.ts`, `processingWorkflow.deps.ts`, `processingWorkflowPreparation.ts`, `processingCancellationWorkflow.ts`, `render.ts`, `state.ts`, `viewState.svelte.ts`, `viewTypes.ts`, `domain/`, `services/`, `__tests__/`, `StatusPanelIsland.svelte`.
- The cluster consumes backend `OperationKind`, progress events, queue snapshots,
  cancellation facts, and terminal results as a read model. It owns visible
  status derivation, status feedback, controls, and processing request
  composition from panel Public API Strips; it does not own backend lifecycle
  vocabulary or terminal-summary truth.

## Allowed Agent Edits Without Escalation
- Change internals when focused status-panel tests stay green; run targeted
  Vitest files when proving UI behavior changes and generated-binding,
  Public API Strip, and runtime contract checks when generated event shapes are
  touched.
- Test visible status outcomes rather than private reducer shape when behavior is user-facing.
- Keep direct view-state/controller/runtimeApi imports inside this cluster or tests.
- Build `ProcessingRequestConfig` through `processingConfig.ts`; do not import encoder or output panel private state to assemble process payloads.
- Use backend `operation_kind` from queue/progress events to classify merge,
  batch, and metadata-save status behavior; do not infer operation identity only
  from caller choreography.

## Breaking-Change Triggers
- Adding, removing, or renaming any Public API Strip export.
- Changing progress, cancellation, queue terminalization, success/failure/cancel truth, or concurrency-status behavior.
- Letting external runtime code import `viewState.svelte`, `controller.ts`, `runtimeApi.ts`, or reducer/domain internals directly.
- Reintroducing mirrored encoder/sample-rate state outside the encoder panel.
- Converting Status Panel to a WorkRuntime consumer without explicit migration
  proof (this must be a deliberate architecture decision with documented
  evidence).
