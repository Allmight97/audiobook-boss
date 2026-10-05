# WorkRuntime

## Public API Strip

- Engine-internal `WorkRuntime`, built with `new(tasks)` on the engine's
  `EngineTasks`. Hosts see operation and child snapshots, progress, summary,
  lane, and id vocabulary (`OperationId`); submit requests and accepted title
  handles stay crate-internal.
- `submit_processing_operation` spawns a background export and returns
  `WorkSubmissionAccepted`. Callers name the operation after its books; the
  request `title` is required and non-empty. `on_finished` receives the
  terminal snapshot; the session settles staged downloads from it.
- The inline metadata-save hooks (`begin_metadata_save_operation`,
  `record_metadata_save_progress`, `finish_metadata_save_operation`) are
  orchestrated by `crates/abb-engine/src/metadata_save.rs`, which the session's
  Save calls. `finish` takes the run's outcome: `InlineRunTerminal` results or
  the aborting error. `metadata_save` owns the metadata executor and each
  child's reason; WorkRuntime owns the lifecycle, snapshots, cancellation, and
  terminal classification, shared with processing runs.
- `sources_in_use` (every canonical source path of every queued or running
  title) and `subscribe_changes` (wakes on any operation change): the session's
  Save uses them to hold a write until no accepted export reads the file.
- `sources_held`: every source of every unfinished export, including titles
  already encoded that still copy companion files. The session's staged-download
  sweep uses it.
- `unfinished_exports`: what shutdown cancels and `running_work` counts;
  metadata Saves are left to finish.
- Each accepted export title gets a `TitleOutput` (built from the acceptance
  preflight; returned in `WorkSubmissionAccepted::titles`). The session hands it
  Save's edits; `stop_title` cancels one title and waits until it published or
  ended. Every title is ended when its operation ends, so one the run never
  reached cannot keep an edit waiting. The child's `output_update` shows the
  latest edit's state.

## Ownership

- Own operation identity, accepted submissions (after acceptance a title's
  tags change only through its `TitleOutput::update`, which the session
  calls), operation snapshots, operation and title cancellation, and Work
  Center event truth.
- Cancellation: a processing operation holds one cancel flag per output title,
  and whole-operation cancel sets them all. `cancel_operation` with a
  `child_job_id` cancels only that title. Only processing titles cancel one at
  a time; a metadata save's files share one flag. Repeating a cancel, or
  cancelling a finished title, returns the current snapshot. Child
  `cancellable` is the authority for offering title cancel.
- Each output title is one child. Operation and child `source_input_ids` retain
  all source identities, so a grouped title's downloads settle together at its
  terminal outcome, including in mixed-success batches.
- A child's `supplemental_warning` carries Processing's partial-publication
  fact: the audiobook was published but a requested companion PDF was not. The
  child stays Completed; the session keeps that title's download.
- Own child `startedAtMs` / `finishedAtMs` in retained snapshots: first active
  progress through output completion (100%, excluding the earlier cleanup
  event). Keep each child's finish when the batch settles; missing timestamps
  mean unknown, so derive them from neither logs nor batch duration.
- Stamp per-operation `revision` under the state lock before mutable access.
- Every change reaches hosts as one `EngineEvent::WorkOperations`
  (`WorkOperationsUpdate`): the changed operation plus the display order,
  numbered by a state-wide `revision` taken under the state lock when it is
  published. Emission happens after the lock, so updates can arrive out of
  order; a host keeps the newest snapshot per operation and the order with the
  highest revision, and needs no other merge rule. `list_operations` returns
  the same order and revision. The engine owns display order: active, then
  accepted, then finished; newest first within each. Session previews reuse the
  private `WorkRuntimeState` reducer and publish within `SessionUpdate.output`;
  they stay outside retained export history. Event names belong to the host
  (`src-tauri/src/events.rs`).
- Terminal-operation retention: `WorkRuntimeState` keeps at most
  `TERMINAL_OPERATIONS_CAP` (20) terminal operations, pruned oldest-first by
  terminalization order (a just-finished long-running operation must survive
  its own prune). Running and accepted operations are never pruned.
- Reach processing through `processing::run`; audio processor internals,
  output-artifact internals, and remote-source provider/materializer modules
  stay unimported.
- Derive operation terminal status from `crate::processing::classify_run_terminal`
  (`abb_processing_core`): map the canonical `RunTerminalClass` to
  `WorkOperationStatus`. `WorkProgressStage` remains a work_runtime-owned
  display vocabulary.

`mod.rs` is routing only. Add new behavior in a new module before expanding a
large one.
