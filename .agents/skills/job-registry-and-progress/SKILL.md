---
name: job-registry-and-progress
description: Concurrency, cancellation, and progress patterns. Use when modifying long-running jobs, queueing, cancellation, or status emission.
---

# Job Registry and Progress

Use this skill for backend job lifecycle and UX-truthful processing state.

## Hard Invariants

- Active job lifecycle flows through `JobRegistry`.
- CPU-bound encoding executes via `tokio::task::spawn_blocking`.
- Long loops check cancellation checkpoints.
- Progress/failure/cancel states must emit events that match user-visible status.

## Canonical Progress Source

- Stage/type authority: `src/types/events.ts` (`ProgressEvent.stage`)
- Event emission implementation: `src-tauri/src/audio/progress/emitter.rs`

When stages evolve, update `src/types/events.ts` first, then emitter and command usage.

## Required Workflow

1. Acquire job permit from `JobRegistry` before work starts.
2. Run CPU-heavy sections in blocking wrapper.
3. Emit stage updates through `ProgressEmitter` on meaningful boundaries.
4. On exit paths, emit terminal state (`completed`, `failed`, `cancelled`) consistent with actual outcome.

## Pointers

- `src-tauri/src/audio/job_registry/`
- `src-tauri/src/audio/processor/`
- `src-tauri/src/audio/progress/emitter.rs`
- `src-tauri/src/commands/audio.rs`

## Done Criteria

- No parallel lifecycle tracker outside `JobRegistry`.
- Cancellation is responsive in long-running loops.
- Frontend event types and backend stage strings remain in sync.

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.
