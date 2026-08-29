# Processing

## Scope

- Owns preview submit, processing request composition, Status Panel runtime,
  and the status view atom under `src/app/processing/`.
- Solid views live in `src/ui/statusPanel` and `src/ui/previewAudio`. They
  render this owner; they do not keep a second status or preview store.

## Public API Strip

- Import processing runtime symbols from `src/app/processing`.
- Workbench callers that only need the composed UI strip import
  `src/ui/statusPanel` instead.
- `index.ts` is the export surface. Do not import `runtime.ts`, `workflow.ts`,
  `view.ts`, `config.ts`, `domain/`, or `services/` from outside this owner.

## Hard Invariants

- Compose submit config at `readProcessingRequestConfig()` from public
  `encodingRequestConfigAtom` and Output `readOutputRequestConfig()`. Those are
  getters, not poke APIs. Do not restore `updateOutputPath` or
  `updateEstimatedSize`.
- File-list and job-type truth come from Input (`inputViewAtom`, `jobTypeAtom`).
  Concurrency enable/disable uses `setConcurrencyControlsEnabledAtom`. Metadata
  staging uses the Metadata public strip. Do not read leftover file-list or
  job-control stores.
- Preview execution is direct `process_audiobook_files` with `previewSeconds`.
  It does not enter WorkRuntime. Background batch/merge submits through
  WorkRuntime; Status Panel is not a WorkRuntime consumer.
- Foreground cancel settles the local render only. Operation-scoped cancel
  lives in Work Operations.
- Consume the backend-owned terminal verdict (`RunTerminalClass` on
  `ProcessCommandResult`) for preview completion. Do not re-derive terminal
  precedence from per-job rows.
- Preview duration lives in `PreviewAudioControls` screen-local Solid state.
  Submit goes through `startProcessingAtom`.

## Testing

- Workflow tests inject `makeProcessingWorkflowServicesLayer`; they do not mock
  leftover file-list or job-control modules.
- `remote-source-boundary.test.ts` pins the Remote public strip this owner
  consumes. Do not import private Remote session or acquisition files here.
- Status UI strip is pinned by `src/ui/statusPanel/__tests__/runtime-api-contract.test.ts`.

## Breaking-Change Triggers

- Adding, removing, or renaming a public export.
- Reading leftover file-list, job-control, encoder, or output private state to
  build a process payload.
- Converting Status Panel into a WorkRuntime consumer without a documented
  architecture decision.
