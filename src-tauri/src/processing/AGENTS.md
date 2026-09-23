# Processing Lifecycle Boundary

## Public API Strip
- Processing Plan: import from `crate::processing::plan`, not private helpers.
  Functions: `resolve_preflight_plan`, `prepare_execution_plan`. Types:
  `ExecutionProcessingPlan`, `ResolvedProcessingPlan`, `PlannedProcessingJob`.
  Private `title_file_info` projects an inspected source set in title order.
  Callers provide the fresh phase `FileListInfo`; planning never re-inspects
  inputs. Merge execution retains that inspection, while queued batch jobs
  inspect again when scheduled to run.
- Backend Lifecycle: import shared lifecycle vocabulary and event helpers from
  `crate::processing`, not `audio`, `commands`, or Status Panel internals.
  Types: `OperationKind`, `OperationResultSummary`, `EventStage`,
  `ProgressEvent`, `QueueEvent`, `QueueItem`, `JobId`, `CancellationChecker`.
  Functions/helpers: `emit_progress_event`, `emit_queue_event`,
  `ProgressEmitter`, `operation_kind_log_label` (stable dev-log label parsed
  by `scripts/dev-log-analysis.ts`).
- Pure lifecycle/terminal summary classification that has no runtime/media
  dependency is packaged in `abb-processing-core`.
- Processing may consume `crate::output_artifact` types in payloads and plans;
  it does not re-export output-artifact ownership. Command layers import
  output-owned types from `crate::output_artifact` directly.

## Private Cluster
- Files: `../processing.rs`, `plan.rs`, `run.rs`, `terminal_outcomes/`,
  `lifecycle.rs`, `context/`, `job_registry/`, `output_parent_cleanup.rs`,
  `progress/`, `preview_config.rs`, `session.rs`, `types.rs`.
- The cluster owns preflight planning, execution-plan preparation, runner
  orchestration, processing context/session state, backend lifecycle
  vocabulary, job lifecycle, queue/progress event types, terminal result
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
  encoding. Global-merge requests cannot also carry per-title groups.
- Settings and sample-rate checks apply only to encoding plans. A copy plan
  carries no encoder settings and follows normal registration, cancellation,
  output review, and terminal reporting.

## Progress / Stage Evolution

- `processing-progress` and `processing-queue` are emitted by
  `process_audiobook_files`. They have no operation-id discriminator. Accepted
  background work publishes WorkRuntime snapshot events instead; do not mix the
  two event families. Direct-preview callers must pass `preview_seconds`; ingress
  rejects `None`. Final processing enters through WorkRuntime for operation
  identity, snapshots, and operation-scoped cancellation.
- Wire-stage authority is the Rust `EventStage` enum in `progress/mod.rs` (specta-generated into
  `src/lib/generated/tauri.ts`); event payload is `ProgressEvent` (same file). Emitters:
  `progress/emitter.rs` (`emit_event`, `emit_cancelled`) and
  `run/run_dispatch.rs` (`emit_terminal_failed_event`).
  Frontend re-exports `EventStage` and the `STAGES` helper from `src/types/events.ts`.
- To evolve stages: update the Rust `EventStage` enum + its `From<&ProcessingStage>` impl, run
  `bun run bindings:generate`, then adjust frontend consumers. `EventStage` is the flat wire-shaped
  discriminator the UI consumes; the internal `ProcessingStage` carries data (e.g. `Failed(String)`)
  and drives orchestration — keep them distinct.

## Edit Rules
- Change pure processing classification/summarization when
  `cargo nextest run -p abb-processing-core` stays green.
- Change planner or runner internals when targeted `audiobook-boss` Nextest and
  Public API Strip checks stay green.
- Keep preflight side-effect-free; execution may create and track output dirs only after review enforcement.
- Keep runner responsibilities to encoder request validation, job registration,
  scheduler dispatch, audio execution requests through `crate::audio`, and
  handoff to terminal outcome helpers. Toolchain selection stays audio-owned.
- Metadata-save operation truth belongs to `crate::work_runtime`; metadata save
  may reuse this strip's `OperationKind`, `ProgressEvent`, and
  `OperationResultSummary` vocabulary while metadata write policy stays inside
  metadata-owned APIs.

## Boundary Changes
- Adding, removing, or renaming any Public API Strip symbol.
- Changing preflight signature behavior, collision-review enforcement, metadata projection, path validation, or parent-dir side effects.
- Moving artifact truth, metadata intent semantics, backend lifecycle ownership,
  or status terminal truth out of their owning boundaries.
