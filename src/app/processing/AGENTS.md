# Processing

## Scope

- Owns preview submit, processing request composition, and Status Panel
  runtime under `src/app/processing/`.
- Solid views live in `src/ui/statusPanel` and `src/ui/previewAudio`. They
  render this owner; they do not keep a second status or preview store.

## Public API Strip

- Import `createProcessingOwner` and owner types from `src/app/processing`.
- Workbench callers that only need the composed UI strip import
  `src/ui/statusPanel` instead.
- `index.ts` is the export surface. Do not import `runtime.ts`, `workflow.ts`,
  `view.ts`, `workflow.deps.ts`, `domain/`, or `services/` from outside this
  owner.
- Keep the Effect fake layer private: workflow tests import
  `makeProcessingWorkflowServicesLayer` from `workflow.ts`. Do not re-export
  bind slots, `initStatusPanel`, `getStatusView`, or
  `readProcessingRequestConfig`.

## Hard Invariants

- Compose submit config inside the runtime Processing owner from injected
  Encoding Configuration and Output owners (`encoding.request()` +
  `output.readRequestConfig()`). Collision review calls
  `runOutputPlanReviewWorkflow(request, output)` with that same owner.
  Do not restore `updateOutputPath`, `updateEstimatedSize`, or a
  process-wide encoding/output getter.
- File-list and job-type truth come from Input. Concurrency enable/disable uses
  Settings. Metadata staging uses the Metadata public strip. Do not read
  leftover file-list or job-control stores.
- Preview execution is direct `process_audiobook_files` with `previewSeconds`.
  It does not enter WorkRuntime, and command ingress rejects an omitted preview
  duration. Background batch/merge submits through WorkRuntime; Status Panel
  is not a WorkRuntime consumer.
- `processing-progress` and `processing-queue` are direct-preview events with
  no operation-id discriminator. Do not consume them as background-operation
  state; Work Operations consumes WorkRuntime snapshots instead.
- Foreground cancel settles the local render only. Operation-scoped cancel
  lives in Work Operations.
- Consume the backend-owned terminal verdict (`RunTerminalClass` on
  `ProcessCommandResult`) for preview completion. Do not re-derive terminal
  precedence from per-job rows.
- Preview duration lives in `PreviewAudioControls` screen-local Solid state.
  Submit goes through Processing `start`.
- Supplemental payload assets come from the injected Remote Source owner.
  Background submission runs inside its `withSubmissionRetention` operation;
  Processing must not reproduce retain/release/purge ordering.
- Each Processing owner instance owns its status view store and
  `StatusPanelRuntime`. Two live App Runtimes isolate preview status.
  Disposing A cannot publish into B.

## Testing

- Workflow tests inject `makeProcessingWorkflowServicesLayer`; they do not mock
  leftover file-list or job-control modules.
- `remote-source-boundary.test.ts` pins the visual Remote UI strip and proves
  production Processing does not import UI or private Remote implementation
  files.
- `runtime-api-contract.test.ts` pins this owner's public export strip.
- Status UI strip is pinned by `src/ui/statusPanel/__tests__/runtime-api-contract.test.ts`.
- Two-runtime preview-status isolation lives in `src/app/runtime/runtime.test.ts`.

## Breaking-Change Triggers

- Adding, removing, or renaming a public export.
- Reading leftover file-list, job-control, encoder, or output private state to
  build a process payload.
- Converting Status Panel into a WorkRuntime consumer without a documented
  architecture decision.
- Restoring `bindProcessing*` or a module-global status publisher.
