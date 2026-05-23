## Public API Strip
- Processing Plan: import from `crate::processing::plan`, not private helpers.
  Functions: `resolve_preflight_plan`, `prepare_execution_plan`. Types:
  `ResolvedProcessingPlan`, `PlannedProcessingJob`.
- Backend Lifecycle: import shared lifecycle vocabulary and event helpers from
  `crate::processing`, not `audio`, `commands`, or Status Panel internals.
  Types: `OperationKind`, `OperationResultSummary`, `EventStage`,
  `ProgressEvent`, `QueueEvent`, `QueueItem`, `JobId`, `CancellationChecker`.
  Functions/helpers: `emit_progress_event`, `emit_queue_event`,
  `ProgressEmitter`.

## Private Cluster
- Files: `../processing.rs`, `plan.rs`, `run.rs`, `terminal_outcomes.rs`,
  `lifecycle.rs`, `context/`, `job_registry/`, `progress/`,
  `preview_config.rs`, `session.rs`, `contract_tests.rs`.
- The cluster owns preflight planning, execution-plan preparation, runner
  orchestration, processing context/session state, backend lifecycle
  vocabulary, job lifecycle, queue/progress event types, terminal result
  normalization, and their behavior tests.

## Allowed Agent Edits Without Escalation
- Change planner or runner internals when `scripts/proof.sh rust-contract` stays green.
- Keep preflight side-effect-free; execution may create output dirs only after review enforcement.
- Keep runner responsibilities to encoder/toolchain validation, events, job registration, scheduler dispatch, audio execution requests through `crate::audio`, and terminal normalization.
- Keep metadata save reporting lifecycle truth through this strip while leaving
  metadata write policy inside metadata-owned APIs.

## Breaking-Change Triggers
- Adding, removing, or renaming any Public API Strip symbol.
- Changing preflight signature behavior, collision-review enforcement, metadata projection, path validation, or parent-dir side effects.
- Moving artifact truth, metadata intent semantics, backend lifecycle ownership,
  or status terminal truth out of their owning boundaries.
