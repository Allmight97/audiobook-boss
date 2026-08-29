# WorkRuntime

## Public API Strip

- `WorkRuntime`, including:
  - `submit_processing_operation` (spawned background batch/merge, returns
    `WorkSubmissionAccepted`).
  - inline metadata-save lifecycle hooks — `begin_metadata_save_operation`,
    `record_metadata_save_progress`, `finish_metadata_save_operation`,
    `fail_metadata_save_operation` — orchestrated by `commands/metadata/save_batch.rs`
    (the command owns the metadata executor; WorkRuntime owns the operation
    lifecycle/snapshots/cancellation).
- `OperationId`
- operation snapshot, child snapshot, progress, summary, lane, and submit request types
- `WORK_OPERATION_SNAPSHOT_EVENT_NAME`
- `WORK_OPERATION_LIST_SNAPSHOT_EVENT_NAME`

## Ownership

- Own operation identity, immutable accepted submissions, operation snapshots,
  operation-scoped cancellation, and Work Center event truth.
- Terminal-operation retention: `WorkRuntimeState` keeps at most
  `TERMINAL_OPERATIONS_CAP` (20) terminal operations, pruned oldest-first by
  TERMINALIZATION order (never submission sequence — a just-finished
  long-running operation must survive its own prune). Running/accepted
  operations are never pruned. The frontend purge tombstone
  (`PURGED_OPERATION_TOMBSTONE_CAP` in `src/app/workOperations/runtime.ts`)
  must stay strictly larger than this cap; a contract test pins the
  relationship — keep both sites and that test in sync.
- Use `processing::run` as the processing executor boundary. Do not import audio
  processor internals, output-artifact internals, or remote-source private
  provider/materializer modules.
- Derive operation terminal status from the canonical
  `crate::processing::classify_run_terminal` (`abb_processing_core`) classifier.
  Do not reintroduce a parallel terminal-classification rule from snapshot
  counts; map the canonical `RunTerminalClass` to `WorkOperationStatus` instead.
  `WorkProgressStage` remains a work_runtime-owned display vocabulary.
- Keep Tauri command handlers in `src-tauri/src/commands/work_runtime.rs`.
- Keep provider secrets, raw provider payloads, protected intermediates, and
  remote staging mechanics inside `remote_source`.

## Size Guardrails

- Add new behavior inside this module before expanding existing large modules.
- `mod.rs` is routing only. Split state, types, and runtime behavior when logic
  grows past scan-friendly boundaries.
