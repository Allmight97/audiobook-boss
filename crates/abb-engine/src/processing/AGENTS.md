# Processing Lifecycle Boundary

## Public API Strip

- Processing Plan is engine-internal at `crate::processing::plan`.
  `InspectedProcessingPlan` carries the single source inspection and resolved
  audio/metadata plan through review, output records, and execution.
  `prepare_inspected_execution` refreshes collision facts at the already
  derived output path and enforces review before creating directories. Audio
  revalidates retained fingerprints after scheduler/permit waits.
  `title_file_info` projects the same inspection into each title's source
  order.
- Import shared lifecycle vocabulary and event helpers from `crate::processing`
  (its `pub use` list), not from `audio`. `ProgressEmitter` passes progress to
  the owning engine reducer. `operation_kind_log_label` is the stable dev-log
  label that `scripts/dev-log-analysis.ts` parses.
- `TitleOutput` is engine-internal; `OutputUpdate` and `OutputUpdateStatus` are
  host vocabulary (`title_output.rs`).
- Pure lifecycle and terminal summary classification lives in
  `abb-processing-core`.
- Processing consumes `crate::output_artifact` types in payloads and plans
  without re-exporting them; callers import output-owned types from
  `crate::output_artifact`.

## Title outputs

`title_output.rs` holds an export title's output after acceptance. An edit
accepted before publication is written to the staged file just before
publication, under the title's lock and the output file lock. One accepted later
is written to the published file once its size and modification time show ABB
wrote it. Text tags are diffed from the planned metadata; the cover follows the
edit's cover intent. A failed tag write never changes the title's export
outcome. An unreadable identity after publication still settles the title as
published and explicitly refuses unsafe tag updates. A title that ends
unpublished has its empty folders removed (`output_artifact/AGENTS.md`) before
it reports ended (`run/run_dispatch.rs`); a restart waits for that.

## Per-book Audio Handling

- `ProcessPayload.input_files` are output-title metadata anchors. Optional
  `title_sources` maps multi-file anchors to ordered sources and their
  identities; absent entries are single-source titles. Validate membership,
  uniqueness, every source path, and all source validity. One planned job and
  result index represent one output title. Source order participates in the
  review signature.
- `audio_requests` holds one request per output title. Audio resolves each
  request before output collision review; `audio_plans` exposes that result.
  The resolved format sets the extension. Source order and resolved audio plan
  participate in the review signature. Preview and CUE reconstruction require
  encoding. One title cannot compose CUE-bearing sources; the Audio planner
  rejects it in preflight and execution alike. Chapter plans apply once, in the
  validation preflight and execution share.
- Settings and sample-rate checks apply only to encoding plans. A copy plan
  carries no encoder settings and follows normal registration, cancellation,
  output review, and terminal reporting.
- Companion PDFs commit after the audiobook is published. A companion failure
  leaves the title Success with `ProcessResultEntry.supplemental_warning` set,
  so a published title stays Success.

## Progress and stages

- Processing emits internal `ProgressEvent` values to its supplied listener.
  WorkRuntime exports and session previews share the WorkRuntime state reducer;
  hosts receive operation/session snapshots rather than a second queue stream.
- `EventStage` in `progress/mod.rs` maps internal `ProcessingStage` into
  reducer input. The UI consumes `WorkProgressStage` from snapshots. Evolve the
  owning Rust types and reducers, regenerate bindings for public shape changes,
  and update snapshot rendering/proof together.

## Job registry (`job_registry/`)

- The registry owns concurrency permits, active-job state, and the
  max-concurrent capability facts settings expose
  (`MaxConcurrentJobsCapabilities`).
- Register each job through `register_job_with_external_cancel` before work
  starts (`register_job` is test-only). It records the job and acquires its
  permit before execution. `BatchScheduler` keeps result order deterministic,
  keeps issuing queued work after per-task errors, and gives every scheduled
  index a terminal result.
- Presence in the registry map is the job's only lifecycle state: tracked from
  the moment it waits for a permit; `complete_job` and `fail_job` remove it on
  every terminal path; a failed, cancelled, or dropped admission removes
  itself.
- `update_max_concurrent` works only while the registry is idle: no job
  registered and no `ActiveRun` held. WorkRuntime holds one `ActiveRun` from an
  export's acceptance to its end, and the session for a preview, so the gap
  between two titles never counts as idle. Admission and reconfiguration share
  one lock: admission records the job and clones the semaphore together;
  reconfiguration checks for tracked jobs and swaps the semaphore together.

## Edit Rules

- `run.rs`'s `metadata_workflow` tests connect production preflight/planning to
  real encode and preserve writers, then inspect output atoms. They complement
  the session's edit-retention tests and the IPC-contract proof; they do not
  exercise the Tauri window, scheduler, or UI. Keep this proof crate-local;
  commands live in `scripts/AGENTS.md`.
- Keep preflight side-effect-free; execution creates and tracks output dirs only
  after review enforcement.
- `ProcessingRunOptions.title_cancels` carries one cancel flag per output title
  from WorkRuntime or the session preview. Each job's admission and
  `CancellationChecker` (built from that title's flag) observe only that flag;
  the registry holds no other cancel state. Cancelled titles finish as
  Cancelled results while siblings continue.
- The runner handles encoder request validation, job registration, scheduler
  dispatch, audio execution requests through `crate::audio`, and handoff to
  terminal outcome helpers. Encoder selection stays audio-owned.
- Metadata-save operation truth belongs to `crate::work_runtime`; metadata save
  may reuse this strip's `OperationKind`, `ProgressEvent`, and
  `OperationResultSummary` vocabulary while metadata write policy stays inside
  metadata-owned APIs.
