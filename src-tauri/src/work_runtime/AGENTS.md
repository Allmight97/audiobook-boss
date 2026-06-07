# WorkRuntime

## Public API Strip

- `WorkRuntime`
- `OperationId`
- operation snapshot, child snapshot, progress, summary, lane, and submit request types
- `WORK_OPERATION_SNAPSHOT_EVENT_NAME`
- `WORK_OPERATION_LIST_SNAPSHOT_EVENT_NAME`

## Ownership

- Own operation identity, immutable accepted submissions, operation snapshots,
  operation-scoped cancellation, and Work Center event truth.
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
