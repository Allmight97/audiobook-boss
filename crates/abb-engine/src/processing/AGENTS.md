# Processing Lifecycle Boundary

## Public API Strip

- Processing Plan is engine-internal at `crate::processing::plan`.
  `InspectedProcessingPlan` carries the single source inspection and resolved
  audio/metadata plan through review, output records, and execution.
  `prepare_inspected_execution` refreshes collision facts at the already
  derived output path and enforces review before creating directories.
  `title_file_info` projects the same inspection into each title's source
  order.
- Shared lifecycle vocabulary comes from `crate::processing`'s `pub use` list.
  `ProgressEmitter` (`crate::execution`) passes progress to the owning engine
  reducer.
  `OperationKind::log_label` is the stable dev-log label that
  `scripts/dev-log-analysis.ts` parses.
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
edit's cover intent. The plan and this record name the requested path through
`plan::output_path_for`, so an edit's new name cannot differ from the planned
one. A failed tag write never changes the title's export
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
  participate in the review signature (`plan_signature.rs` hashes explicit
  fields; a new audio-plan or encoder field fails to compile until added).
  Preview and CUE reconstruction require
  encoding. Chapter plans apply once, in the validation preflight and
  execution share.
- Settings and sample-rate checks apply only to encoding plans. A copy plan
  carries no encoder settings and follows normal registration, cancellation,
  output review, and terminal reporting.
- Companion PDFs commit after the audiobook is published. A companion failure
  leaves the title Success with `ProcessResultEntry.supplemental_warning` set,
  so a published title stays Success. `ProcessResultEntry.warnings` carries
  Audio's display-only notes on a successful title (`AudioRunOutcome`), such
  as a source cover that was left off; they change no outcome. Planning
  refuses a title whose companion PDF name would not fit
  (`abb_output_artifact_core::check_supplemental_pdf_name`), before encoding;
  titles without a companion PDF are not limited by it.
- `run_job.rs` runs the output-path check and the companion commit (which
  waits on `lock_output_file`) on `spawn_blocking`, so a slow disk or a held
  lock never stalls an async worker. A stopped check fails the job through
  registration cleanup; a stopped companion commit is a companion warning.

## Progress and stages

- Processing emits internal `ProgressEvent` values to its supplied listener.
- `JobIdentity` (operation id, job id, input index) is built when a job
  registers; `ProcessingContext`, `ProgressEmitter`, and the `processing_job`
  log record hold that one value. A title skipped or cancelled before it starts
  has no job id.
- `EventStage` (`execution/progress/mod.rs`) is the stage every emitted event
  carries.
- Each title that runs ends as one `TitleOutcome` (Published, Cancelled,
  Failed); `TitleOutcome::report` builds its `ProcessResultEntry` (the wire
  projection) and emits the terminal event for Cancelled and Failed;
  Published's Completed event goes out at publication (`audio`). A failed or
  cancelled stage returns its error and emits nothing; the title runner
  (`run/run_dispatch.rs`) reports when that title ends, after its folders and
  `TitleOutput` end. Why: an earlier event shows a finished title whose
  staged files still exist; a later one leaves a failed row running.
  Exceptions: a skipped title never runs and reports at dispatch
  (`report_skipped`), and an all-skipped batch returns its entries with no
  event (`build_all_skipped_batch_result`); a title task that panics is
  reported Failed after the batch.

## Job registry (`job_registry/`)

- The registry owns concurrency permits, active-job state, and the
  max-concurrent capability facts settings expose
  (`MaxConcurrentJobsCapabilities`).
- Register each encode through `register_job_with_external_cancel` before work
  starts. It records the job and acquires its permit before execution. A
  metadata Save is a tag write, not an encode: it registers through
  `register_job_without_permit`, so it counts as active work and never waits
  behind export slots. `JobRegistry::run_batch` keeps result order deterministic,
  keeps issuing queued work after per-task errors, and gives every scheduled
  index a terminal result. A queued title whose cancel token is cancelled starts out
  of turn the moment it is cancelled, so it fails fast and never waits behind
  the titles ahead. The permit wait and the scheduler await the token; a
  wake-up never drops a running title.
- Presence in the registry map is the job's only lifecycle state: tracked from
  the moment it registers (a permit job, while it waits for the permit);
  an admitted job is held as a `RegisteredJob`, which leaves the registry
  through `complete` or `fail` and, if dropped unsettled, on its own; a failed,
  cancelled, or dropped admission removes itself.
- `update_max_concurrent` works only while the registry is idle: no job
  registered and no `ActiveRun` held. WorkRuntime holds one `ActiveRun` from an
  export's acceptance to its end, and the session for a preview, so the gap
  between two titles never counts as idle. Admission and reconfiguration share
  one lock: admission records the job and clones the semaphore together;
  reconfiguration checks for tracked jobs and swaps the semaphore together.

## Edit Rules

- `run_tests.rs`'s `metadata_workflow` tests connect production preflight and
  planning to real encode and preserve writers, then inspect output atoms.
- A skipped title never publishes, so an edit for it is refused at once.
  Collision facts refresh after review without re-inspecting sources or
  resolving audio and metadata again.
- A preview is `OutputKind::Preview`, set once when the plan resolves each
  title's output. Execution asks `ProcessingContext::is_preview()`; no second
  flag carries it; `RunMode` says only what the caller supplies per title.
- Keep preflight side-effect-free. Why: a plan that review refuses leaves no
  folders or files behind.
- A `ProcessingRun` always carries its operation id, its progress listener,
  and a `RunMode`: `Export` holds each title's cancel token and output record,
  `Preview` only each title's cancel token, in title order. WorkRuntime builds
  the export, the session the preview. Why: one mode makes "an export title
  without an output record" unbuildable. Each job's admission and
  `CancellationChecker` (built from that title's token) observe only that
  token; the registry holds no other cancel state. Cancelled titles finish as
  Cancelled results while siblings continue.
