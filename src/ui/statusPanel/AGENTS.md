# Status Panel

## Lifecycle Classification

Status Panel is a **retained foreground/direct adapter**. It is **not** a
WorkRuntime consumer for background operations.

- **Preview execution**: direct `process_audiobook_files` with `previewSeconds`.
  Does not enter WorkRuntime.
- **Metadata batch save**: **not** handled by the Status Panel. `save_metadata_batch`
  runs as a WorkRuntime `MetadataSave` operation (rendered by the Work Center); the
  command still returns the per-file `MetadataSaveBatchResult` so the metadata-save
  workflow can clear pending drafts only for files that succeeded.
- **Cancellation**: operation-scoped only, through `cancel_work_operation`
  (Work Center / WorkRuntime). The retained foreground/direct lane has **no**
  backend cancel command; the Status Panel cancel button settles the local
  render only and never reaches the backend.

There is no `operation_id` discriminator. Background WorkRuntime operations emit
no `processing-progress`/`processing-queue` window events, so every such event the
Status Panel receives is foreground preview. The Status Panel renders the in-flight
preview bar and the **backend-owned terminal verdict** (`RunTerminalClass` on
`ProcessCommandResult`, mapped in `domain/stateMachineHelpers.ts::feedbackFromResult`);
it does **not** re-derive terminal precedence from per-job rows. Failure-detail text
(`summarizeBatchOutcome`) and the preview reducer state (`jobProgress`/`queueOrder`/
`latestProgressEvent`) remain — they render backend-sourced state, they do not
re-classify the terminal outcome.

## Public API Strip
- Import status runtime symbols from `src/ui/statusPanel` unless a local exception is documented.
- Authoritative runtime export surface = `index.ts`, pinned by
  `__tests__/runtime-api-contract.test.ts`. Treat that test as the source of
  truth instead of a hand-listed export set here.

## Private Cluster
- Files: `controller.ts`, `runtimeApi.ts`, `events.ts`, `formatting.ts`, `preview.ts`, `processing.ts`, `processingConfig.ts`, `processingWorkflow.ts`, `processingWorkflow.deps.ts`, `processingWorkflowPreparation.ts`, `render.ts`, `state.ts`, `viewState.svelte.ts`, `viewTypes.ts`, `domain/`, `services/`, `__tests__/`, `StatusTransportIsland.svelte`.
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
- Consume the backend-owned terminal verdict (`RunTerminalClass` on
  `ProcessCommandResult`) for the preview completion outcome; do not re-derive
  terminal precedence from per-job rows. Backend `operation_kind` on
  progress/queue events classifies the in-flight preview kind, not operation
  identity.

## Breaking-Change Triggers
- Adding, removing, or renaming any Public API Strip export.
- Changing progress, cancellation, queue terminalization, success/failure/cancel truth, or concurrency-status behavior.
- Letting external runtime code import `viewState.svelte`, `controller.ts`, `runtimeApi.ts`, or reducer/domain internals directly.
- Reintroducing mirrored encoder/sample-rate state outside the encoder panel.
- Converting Status Panel to a WorkRuntime consumer without explicit migration
  proof (this must be a deliberate architecture decision with documented
  evidence).
