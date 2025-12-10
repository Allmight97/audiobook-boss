# PR #77 Audit Report: Parallel Batch Processing

**Status**: ⛔️ Recommended Changes Before Merge
**Date**: 2025-12-08
**Auditor**: Senior Engineer (Antigravity)
**Reference Plan**: `docs/planning/issue71.md`

## 1. Executive Summary

The architectural direction of PR #77 is sound and aligns well with the "single engine" simplicity goal. The introduction of `JobRegistry` and `ProcessingSession` bridges successfully modernized the state management without breaking legacy contracts.

However, a **critical blocking I/O issue** was identified in the `FfmpegNextProcessor` that presents a high risk of application unresponsiveness ("freezing") under load. This must be addressed before merging to ensure a "premium" user experience.

## 2. Critical Findings & Directives

### 🚨 Major: Blocking Operations in Async Runtime

**Location**: `src-tauri/src/audio/processor/engine.rs` (Lines 100-200)

**Observation**:
The `execute` method performs CPU-intensive audio encoding and I/O directly within a `Box::pin(async move { ... })` block. In Rust's `tokio` runtime (used by Tauri), async tasks are cooperative. If a task runs a long calculation or synchronous I/O without yielding, it **blocks the worker thread**.

**Validation**:
Using `exa` and `tokio` documentation, we confirmed that `tokio::task::spawn_blocking` is the required pattern for CPU-bound tasks.
> "Runs the provided closure on a thread where blocking is acceptable." - *Tokio Docs*

**Impact Analysis**:
*   **1st Order (Immediate)**: If you run 8 concurrent jobs on an 8-core machine, all 8 Tokio worker threads may become occupied by FFmpeg logic.
*   **2nd Order (Systemic)**: The application event loop will stall. UI events, progress updates, and even the "Cancel" button listener might fail to process until a defined "await" point is reached (which could be seconds or minutes in tight encoding loops).
*   **3rd Order (User Experience)**: The app will feel "frozen" or "janky," violating the "Premium Application" design principle.

**✅ Directive 1: Offload to Blocking Thread**
Wrap the encoding logic in `tokio::task::spawn_blocking`.
```rust
// Proposed Change in src-tauri/src/audio/processor/engine.rs
fn execute(...) -> ... {
    Box::pin(async move {
        // ... setup code ...
        
        // MOVED: Heavy lifting inside spawn_blocking
        let result = tokio::task::spawn_blocking(move || {
             // ... encoding loop ...
        }).await.map_err(|e| AppError::General(format!("Task join error: {}", e)))??;
        
        Ok(result)
    })
}
```

## 3. Structural & Documentation Audit

### ✅ Job Registry & Session
The `JobRegistry` correctly uses `tokio::sync::Semaphore` to manage concurrency. The interaction with `ProcessingSession` via the `CancellationSource` enum is a clever way to support both the legacy global cancellation and the new per-job cancellation.

### ⚠️ Documentation Gaps
The `AGENTS.md` file was updated generally but lacks specific details on the new `JobRegistry` architecture.
**✅ Directive 2**: Update `AGENTS.md` -> `Architecture Fundamentals` to explicitly mention `JobRegistry` as the source of truth for active jobs, replacing the "Single Engine" description's implication of serial processing.

## 4. Testing & Validation Strategy

The current test suite covers unit logic but misses the "chaos" of real concurrency.

**✅ Directive 3: Implementation of "Thundering Herd" Test**
Create a new integration test file `src-tauri/tests/stress_concurrency.rs`:
1.  **Objective**: Verify the semaphore works under high pressure.
2.  **Method**: Spawn 50 async tasks that call `registry.register_job()`.
3.  **Success Criteria**:
    *   Only `max_concurrent` jobs enter the "Running" state at once.
    *   The app acts effectively as a FIFO queue.
    *   Memory usage stays flat (no leaks from queued futures).

## 5. Alignment with Original Plan (`issue71.md`)

*   **UI Aggregation**: Implemented as planned in `StatusPanel`.
*   **Default Limit**: Auto-detection implemented.
*   **Per-Job Cancel**: Implemented in backend and frontend.

**Deviation Note**: The plan mentioned exposing `max_concurrent` settings in the UI (Phase 5). This audit did not see a user-facing setting for this yet, only the backend logic. This is acceptable for an MVP but should be tracked for the next sprint.

## 6. Value Proposition
*   **DX**: The code is cleaner; `JobRegistry` isolates complex locking logic from the business logic.
*   **UX**: (Once fixed) True parallel processing significantly reduces total batch time for users with powerful machines (Mac M1/M2/M3).

## 7. Approval Status
**Withhold Merge** until Directive 1 (blocking I/O fix) is implemented and verified.

## 8. Code Health Analysis (Automated)

Ran `scripts/analyze_code_lines.py` to check for module size violations (>400 LOC).

**Findings**:
*   **New Violation**: `src-tauri/src/audio/job_registry.rs` (481 lines).
    *   *Analysis*: This is a new file introducing substantial logic. While acceptable for a first pass, it is already over the limit.
    *   *Recommendation*: Monitor for growth. If it grows further, split `Job` struct definition and internal helper methods into `job.rs` or `job_model.rs`.
*   **Existing Major Violation**: `src/ui/statusPanel/logic.ts` (779 lines).
    *   *Analysis*: This file handles UI updates, progress aggregation, and now job list management. It is becoming a "God Class" for the status panel.
    *   *Recommendation*: **Post-Merge Refactor**. Extract the new job tracking logic into a dedicated `JobTracker` class or `useJobProgress` hook equivalent to separate state management from DOM rendering.

Other existing large files (e.g., `ffmpeg_bridge.rs`) remain unchanged but persistent tech debt.
