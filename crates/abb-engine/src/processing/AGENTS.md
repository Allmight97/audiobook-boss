# Processing Lifecycle Boundary

## Public API Strip
- Processing Plan is engine-internal at `crate::processing::plan`.
  `InspectedProcessingPlan` carries the single source inspection and resolved
  audio/metadata plan through review, output records, and execution.
  `prepare_inspected_execution` refreshes collision facts at the already
  derived output path and enforces review before creating directories.
  Audio revalidates retained fingerprints after scheduler/permit waits.
  `title_file_info` projects the same inspection into each title's source order.
- Backend Lifecycle: import shared lifecycle vocabulary and event helpers from
  `crate::processing`, not `audio` or Status Panel internals.
  Types: `OperationKind`, `OperationResultSummary`, `EventStage`,
  `ProgressEvent`, `JobId`, `CancellationChecker`.
  Helpers: `ProgressEmitter` (passes progress to the owning engine reducer), `operation_kind_log_label` (stable dev-log label parsed
  by `scripts/dev-log-analysis.ts`).
- Engine-internal export title outputs: `TitleOutput`; host vocabulary:
  `OutputUpdate`, `OutputUpdateStatus`
  (`title_output.rs`).
- Pure lifecycle/terminal summary classification that has no runtime/media
  dependency is packaged in `abb-processing-core`.
- Processing may consume `crate::output_artifact` types in payloads and plans;
  it does not re-export output-artifact ownership. Hosts and other callers
  import output-owned types from `crate::output_artifact` directly.

## Private Cluster
- Files: `../processing.rs`, `plan.rs`, `run.rs`, `terminal_outcomes/`,
  `lifecycle.rs`, `context/`, `job_registry/`, `output_parent_cleanup.rs`,
  `progress/`, `preview_config.rs`, `session.rs`, `title_output.rs`,
  `types.rs`.
- `title_output.rs`: an export title's output after acceptance. An edit
  accepted before publication is written to the staged file just before
  publication, under the title's lock and the output file lock; one accepted
  later is written to the published file once its size and modification time
  show ABB wrote it. Text tags are diffed from the planned metadata; the cover
  follows the edit's cover intent. A failed tag write never changes the
  title's export outcome. An unreadable identity after publication still
  settles the title as published and explicitly refuses unsafe tag updates. A title that ends unpublished has its empty
  folders removed (`output_artifact/AGENTS.md`) before it reports ended
  (`run_dispatch.rs`); a restart waits for that.
- The cluster owns preflight planning, execution-plan preparation, runner
  orchestration, processing context/session state, backend lifecycle
  vocabulary, job lifecycle, internal progress types, terminal result
  normalization, and their behavior tests.

## Per-book Audio Handling

- `ProcessPayload.input_files` are output-title metadata anchors. Optional
  `title_sources` maps multi-file anchors to ordered sources and their identities;
  absent entries are single-source titles. Validate membership, uniqueness,
  every source path, and all source validity. One planned job and result index
  represent one output title. Source order participates in the review signature.
- `audio_requests` contains one request per output title. Audio resolves each
  request before output collision review; `audio_plans` exposes that result.
  The resolved format sets the extension. Source order and resolved audio plan
  participate in the review signature. Preview and CUE reconstruction require
  encoding. One title cannot compose CUE-bearing sources; the Audio planner
  rejects it in preflight and execution alike. Chapter plans are applied once,
  in the validation preflight and execution share.
- Settings and sample-rate checks apply only to encoding plans. A copy plan
  carries no encoder settings and follows normal registration, cancellation,
  output review, and terminal reporting.

- Companion PDFs commit after the audiobook is published. A companion
  failure leaves the title Success with `ProcessResultEntry.supplemental_warning`
  set; it never reclassifies a published title as Failed.

## Progress / Stage Evolution

- Processing emits internal `ProgressEvent` values to its supplied listener.
  WorkRuntime exports and session previews share the WorkRuntime state reducer;
  hosts receive operation/session snapshots rather than a second queue stream.
- `EventStage` in `progress/mod.rs` maps internal `ProcessingStage` into reducer
  input. The UI consumes `WorkProgressStage` from snapshots. Evolve the owning
  Rust types and reducers, regenerate bindings for public shape changes, and
  update snapshot rendering/proof together.

## Edit Rules
- Change pure processing classification/summarization when
  `cargo test --locked -p abb-processing-core` stays green.
- Change planner or runner internals when targeted
  `cargo test --locked -p abb-engine --features bundled-ffmpeg` runs and Public
  API Strip checks stay green.
- `run.rs`'s `metadata_workflow` tests connect production preflight/planning to
  real encode and preserve writers, then inspect output atoms. They complement
  the session's edit-retention tests and the IPC-contract proof; they do not
  exercise the Tauri window, scheduler, or UI. Keep this proof crate-local rather than
  exposing planner internals for tests; commands live in `scripts/AGENTS.md`.
- Keep preflight side-effect-free; execution may create and track output dirs only after review enforcement.
- `ProcessingRunOptions.title_cancels` carries one cancel flag per output title
  from WorkRuntime or the session preview; each job's admission and `CancellationChecker` observe only
  its own title's flag. Cancelled titles finish as Cancelled results while
  siblings continue.
- Keep runner responsibilities to encoder request validation, job registration,
  scheduler dispatch, audio execution requests through `crate::audio`, and
  handoff to terminal outcome helpers. Encoder selection stays audio-owned.
- Metadata-save operation truth belongs to `crate::work_runtime`; metadata save
  may reuse this strip's `OperationKind`, `ProgressEvent`, and
  `OperationResultSummary` vocabulary while metadata write policy stays inside
  metadata-owned APIs.

## Boundary Changes
- Adding, removing, or renaming any Public API Strip symbol.
- Changing preflight signature behavior, collision-review enforcement, metadata projection, path validation, or parent-dir side effects.
- Moving artifact truth, metadata intent semantics, backend lifecycle ownership,
  or status terminal truth out of their owning boundaries.
