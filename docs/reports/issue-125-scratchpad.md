# Issue #125 Scratchpad

## Scope (as written)
- Split `src-tauri/src/audio/job_registry.rs` into cohesive modules.
- Move inline tests into `src-tauri/tests/unit/audio/`.
- Preserve behavior; no feature changes.

## Invariants to preserve
- JobRegistry is the single source of truth for active jobs.
- Concurrency limit logic and semaphore gating unchanged.
- Cancellation semantics (per-job + global) unchanged.

## Baseline Notes
- Public re-exports currently via `src-tauri/src/audio/mod.rs`.
- Call sites: commands/audio.rs, audio/session.rs, lib.rs.
- Inline tests are ~205 LOC and will move.
