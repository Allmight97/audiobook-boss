# Job Registry Directives

## Scope

- Owns concurrency lifecycle and active-job state under `processing/job_registry/`.
- Source of truth for permit discipline, scheduling behavior, and reconfiguration safety.
- Source of truth for max-concurrent-job capability facts exposed to settings
  controls.
- Operation identity, progress/queue event vocabulary, and shared terminal
  summaries live in the parent `processing` lifecycle/progress public API;
  this directory owns active-job state, not the whole lifecycle contract.

## Preferred Path

- Register processing work through `register_job` before work starts.
- Use `BatchScheduler` to coordinate batch execution and preserve result ordering.
- Transition job state through `complete_job` or `fail_job` on every terminal path.
- Change concurrency via `update_max_concurrent` only when registry state is idle.

## Hard Invariants

- `register_job` acquires/records permit and cancellation state before execution.
- Scheduler preserves deterministic ordering while continuing to issue queued work after per-task errors.
- Terminal job paths always release/remove tracked job state.
- Queue snapshot items must always become terminal outcomes (success or failed) so UI state never hangs on missing indices.
- Concurrency reconfiguration is idle-only to prevent dangling permits and inconsistent UI job counts.

## Lifecycle Ownership Traps

- If ownership between scheduler, permit handling, and cancellation appears split or implicit, name the seam and working assumption.
- Add or propose the smallest invariant, test, or doc guard that would prevent recurrence.
- Block when ambiguity risks leaked permits, incorrect counts, or stuck cancellation.

## Done Criteria

- Job lifecycle remains single-owner and terminal paths are complete.
- Batch error behavior is continue-on-error with deterministic ordered outcomes and terminalization for all queued indices.
- Concurrency updates preserve registry invariants and UX-visible job accuracy.
