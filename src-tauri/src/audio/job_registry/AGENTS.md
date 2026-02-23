# Job Registry Directives

## Scope

- Owns concurrency lifecycle and active-job state under `audio/job_registry/`.
- Source of truth for permit discipline, scheduling behavior, and reconfiguration safety.

## Preferred Path

- Register processing work through `register_job` before work starts.
- Use `BatchScheduler` to coordinate batch execution and preserve result ordering.
- Transition job state through `complete_job` or `fail_job` on every terminal path.
- Change concurrency via `update_max_concurrent` only when registry state is idle.

## Hard Invariants

- `register_job` acquires/records permit and cancellation state before execution.
- Scheduler stops issuing new work after first batch error and drains in-flight tasks deterministically.
- Terminal job paths always release/remove tracked job state.
- Concurrency reconfiguration is idle-only to prevent dangling permits and inconsistent UI job counts.

## Canary Trigger

- Trigger Canary when lifecycle ownership between scheduler, permit handling, and cancellation appears split or implicit.
- Report the ambiguous ownership, working assumption, and minimal invariant update proposal.
- Continue unless the ambiguity risks leaked permits, incorrect counts, or stuck cancellation.

## Done Criteria

- Job lifecycle remains single-owner and terminal paths are complete.
- Batch error behavior is fail-fast for new scheduling and deterministic for in-flight drain.
- Concurrency updates preserve registry invariants and UX-visible job accuracy.
