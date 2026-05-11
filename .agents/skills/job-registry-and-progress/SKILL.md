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

- Wire-stage authority: Rust `EventStage` enum in `src-tauri/src/processing/progress/mod.rs`
  (specta-generated into `src/lib/generated/tauri.ts` as a string-literal union).
- Event payload type (Rust): `ProgressEvent` in `src-tauri/src/processing/progress/mod.rs`.
- Event emission implementations: `src-tauri/src/processing/progress/emitter.rs`
  (`emit_event`, `emit_cancelled`) and `src-tauri/src/processing/run.rs`
  (`emit_terminal_failed_event`).
- Frontend re-export and runtime helpers: `src/types/events.ts` re-exports
  `EventStage` and exposes the readable `STAGES` value-level helper, typed
  `{ [K in EventStage]: K }` so missing variants fail the TS build.

When stages evolve, update the Rust `EventStage` enum + the matching
`From<&ProcessingStage>` impl first, regenerate bindings (`bun run bindings:generate`),
then adjust frontend consumers. The internal Rust `ProcessingStage` enum is
distinct from `EventStage` — `ProcessingStage` carries data (e.g.
`Failed(String)`) and drives processor orchestration, while `EventStage` is the
flat wire-shaped discriminator the UI consumes.

## Required Workflow

1. Acquire job permit from `JobRegistry` before work starts.
2. Run CPU-heavy sections in blocking wrapper.
3. Emit stage updates through `ProgressEmitter` on meaningful boundaries.
4. On exit paths, emit terminal state (`completed`, `failed`, `cancelled`) consistent with actual outcome.

## Pointers

- `src-tauri/src/processing/job_registry/`
- `src-tauri/src/audio/processor/`
- `src-tauri/src/processing/progress/emitter.rs`
- `src-tauri/src/commands/audio.rs`

## Done Criteria

- No parallel lifecycle tracker outside `JobRegistry`.
- Cancellation is responsive in long-running loops.
- Backend `EventStage` and frontend consumers stay in sync via specta-generated
  bindings (the `check-generated-bindings` gate enforces drift detection).

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.
