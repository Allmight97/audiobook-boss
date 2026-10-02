# Job Registry Directives

## Scope

- Owns concurrency lifecycle and active-job state under `processing/job_registry/`.
- Source of truth for permit discipline, scheduling behavior, and reconfiguration safety.
- Source of truth for max-concurrent-job capability facts exposed to settings
  controls.
- Accepted-operation identity lives in `work_runtime`. Internal progress
  values and shared terminal summaries live in the parent `processing` public
  API; this directory owns active-job state.

## Preferred Path

- Register processing work through `register_job` before work starts.
- Use `BatchScheduler` to coordinate batch execution and preserve result ordering.
- Presence in the registry map is the job's only lifecycle state: a job is
  tracked from the moment it waits for a permit, and `complete_job`/`fail_job`
  remove it on every terminal path. A failed, cancelled, or dropped admission
  removes itself.
- Change concurrency via `update_max_concurrent` only when registry state is
  idle: no job registered and no `ActiveRun` held. WorkRuntime holds one from
  an export's acceptance to its end, and the session for a preview, so the
  gap between two titles never counts as idle.
- Cancellation is title-scoped: `CancellationChecker::new` takes the title's
  cancel flag, from WorkRuntime for an export and from the session for a
  preview. The registry holds no per-job or registry-wide cancel state.

## Hard Invariants

- `register_job` records the job and acquires its permit before execution.
- Scheduler preserves deterministic ordering while continuing to issue queued work after per-task errors.
- Terminal job paths always release/remove tracked job state.
- Every scheduled index produces a terminal result (success or failed), so no title is left without an outcome.
- Concurrency reconfiguration is idle-only to prevent dangling permits and inconsistent UI job counts.
  Admission and reconfiguration share one lock: admission records the job and
  clones the semaphore together, and reconfiguration checks for tracked jobs
  and swaps the semaphore together.

## Done Criteria

- Job lifecycle remains single-owner and terminal paths are complete.
- Batch error behavior is continue-on-error with deterministic ordered outcomes and terminalization for all queued indices.
- Concurrency updates preserve registry invariants and UX-visible job accuracy.
