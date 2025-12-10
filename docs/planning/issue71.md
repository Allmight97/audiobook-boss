# Plan: Enable Parallel Batch Processing (Issue #71)

**Date**: 2025-12-08
**Status**: Approved
**Issue**: https://github.com/Allmight97/audiobook-boss/issues/71

## Summary
Replace the global `is_processing` mutex with a job registry + semaphore pattern to enable N concurrent audiobook conversions, unlocking full hardware potential for batch processing. Frontend will allow starting multiple jobs, show aggregate progress, and expose a safe max-concurrency setting. Per-job cancel will be supported via job IDs.

## Design Decisions (from user input)
- **UI**: Aggregated progress view (single bar showing "2/4 jobs, 65%")
- **Cancel**: Single button cancels all active jobs
- **Default limit**: Auto (`num_cpus / 2`) with manual override setting

---

## Implementation Phases (Updated)

### Phase 1: Job Registry Core (Rust Backend)

**1.1 Create `src-tauri/src/audio/job_registry.rs`** (NEW)
```rust
pub struct JobId(pub Uuid);
pub enum JobState { Queued, Running, Completed, Cancelled, Failed(String) }
pub struct Job { id: JobId, state: JobState, cancel_flag: Arc<AtomicBool> }
pub struct JobRegistry {
    jobs: RwLock<HashMap<Uuid, Job>>,
    semaphore: Arc<Semaphore>,
    max_concurrent: usize,
    global_cancel: Arc<AtomicBool>,  // For "cancel all" behavior
}
```

Key methods:
- `new(max_concurrent: usize)` - Initialize with CPU-based default
- `register_job() -> Result<(JobId, OwnedSemaphorePermit)>` - Acquire permit, return job ID
- `cancel_all()` - Set global cancel flag
- `is_cancelled(job_id)` - Check per-job OR global cancel
- `complete_job(job_id)` - Cleanup
- `get_aggregate_progress() -> (active: usize, completed: usize, total: usize)`

**1.2 Modify `src-tauri/src/lib.rs`**
- Add `JobRegistry` to managed state alongside existing `ProcessingState`
- Initialize with `num_cpus::get() / 2` (add `num_cpus` crate)

**1.3 Modify `src-tauri/src/audio/mod.rs`**
- Add `pub mod job_registry;`

---

### Phase 2: Progress Event Enhancement

**2.1 Modify `src-tauri/src/audio/progress/reporter.rs`**

Add `job_id` to `ProgressEvent`:
```rust
pub struct ProgressEvent {
    pub stage: String,
    pub percentage: f32,
    pub message: String,
    pub current_file: Option<String>,
    pub eta_seconds: Option<f64>,
    pub job_id: Option<String>,  // NEW - optional for backward compat
}
```

Update `ProgressEmitter`:
```rust
pub struct ProgressEmitter {
    window: Window,
    job_id: Option<String>,  // NEW
}

impl ProgressEmitter {
    pub fn new(window: Window) -> Self { Self { window, job_id: None } }
    pub fn with_job_id(window: Window, job_id: String) -> Self {
        Self { window, job_id: Some(job_id) }
    }
    // Update emit_event() to include job_id in all events
}
```

**2.2 Modify `src/types/events.ts`**
```typescript
export interface ProcessingProgressEvent {
    stage: keyof typeof STAGES;
    percentage: number;
    message: string;
    current_file?: string;
    eta_seconds?: number;
    job_id?: string;  // NEW
}
```

---

### Phase 3: Command Integration

**3.1 Modify `src-tauri/src/commands/audio.rs`**

Update `process_audiobook_files_v2`:
```rust
pub async fn process_audiobook_files_v2(
    window: tauri::Window,
    state: tauri::State<'_, crate::ProcessingState>,
    registry: tauri::State<'_, Arc<JobRegistry>>,  // NEW
    payload: ProcessV2Payload,
    metadata: Option<AudiobookMetadata>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    // 1. Register job with registry (blocks if at capacity)
    let (job_id, _permit) = registry.register_job().await?;

    // 2. Create session with job-aware cancellation
    let session = ProcessingSession::from_job_registry(
        registry.inner().clone(),
        job_id.clone(),
    );

    // 3. Create context (existing pattern)
    let context = ProcessingContext::new(/* ... */);

    // 4. Create emitter with job_id
    let emitter = ProgressEmitter::with_job_id(window.clone(), job_id.to_string());

    // 5. Process (existing flow)
    let result = process_audiobook_with_context(&mut context, &file_info, metadata).await;

    // 6. Cleanup
    registry.complete_job(job_id);

    result
}
```

Update `cancel_processing`:
```rust
pub fn cancel_processing(
    registry: tauri::State<'_, Arc<JobRegistry>>,
) -> Result<String> {
    registry.cancel_all();
    Ok("All jobs cancelled".to_string())
}
```

**3.2 Modify `src-tauri/src/audio/session.rs`**

Add job-registry-aware constructor:
```rust
impl ProcessingSession {
    pub fn from_job_registry(
        registry: Arc<JobRegistry>,
        job_id: JobId,
    ) -> Self {
        Self {
            id: job_id.0,
            cancel_flag: registry.get_cancel_flag(job_id),
            // ...
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }
}
```

---

### Phase 4: UI Integration (Aggregate + Parallel Start + Per-Job Cancel)

- Allow starting additional jobs while others run (remove single-job gate on the "Process" button). Keep one "Cancel All" control plus per-job cancel affordances once job IDs are known.
- Aggregate progress in `StatusPanel`: track per-job events (job_id, stage, pct, message, ETA), compute aggregate percentage (document weighting choice), and keep legacy path when job_id is absent.
- Emit and consume cancelled events per job to clean up aggregates. Ensure DOM helpers still honor unique ids/data-testid.

### Phase 5: Settings (max_concurrent Override)

- Add commands to read/update `max_concurrent_jobs` and apply to JobRegistry (safe semaphore resize or gated update when idle). Default = auto (`num_cpus/2`, clamped 1–8).
- UI control in advanced settings: "Auto (X cores)" plus manual 1–8; persist locally; push setting to backend before submitting jobs.

### Phase 6: Per-Job Cancel UX

- Return job_id in `ProcessCommandResult` to enable targeted cancel buttons.
- Add cancel command variant that accepts a job_id; keep `cancel_all` behavior for the existing button.
- UI: add per-job cancel buttons in the aggregated list; fall back to Cancel All for legacy.

### Phase 7: Testing & Docs

- Rust unit: registry semaphore limits, cancel-all, cancel-single, permit release on error, safe resize.
- Rust integration: concurrent preview jobs finish faster than serial; progress events include job_id; cancel-all stops all; cancel-single stops only that job.
- TS tests: aggregator math, cleanup on terminal events, job_id-less legacy path, per-job cancel dispatch.
- Contract: `scripts/ensure-contract.sh` after command additions; update mocks to emit job_id.
- Docs: update README/AGENTS or relevant user-facing docs once implementation is done; hold PR push until manual local test is completed.

---

## Current Status (before remaining work)
- ✅ Job registry with semaphore (auto `num_cpus/2`), cancellation flags, and aggregate status.
- ✅ JobId added to progress events; ProgressEmitter supports job_id.
- ✅ process_audiobook_files_v2 integrates registry; cancel_all wired to registry.
- ✅ TS types carry optional job_id; StatusPanel aggregates per-job progress (initial pass).
- 🚧 UI still blocks multiple starts; per-job cancel not exposed.
- 🚧 Max concurrency override not exposed (fixed auto only).
- 🚧 Tests for concurrency/aggregation not added.
- 🚧 Docs unchanged post-implementation.

---

## UX Considerations
- **Parallel start:** Keep a single primary “Process” action that can launch more jobs even while others run. Retain a “Cancel All” control and add per-job cancel buttons in the aggregated list for targeted stops.
- **Aggregate progress weighting:** Default to simple average for clarity. Option: weight by active/(active+done) to reduce skew when some jobs finish early. Choose and document in code comments/UI help text.
- **Messaging:** When multiple jobs run, show counts (e.g., “Processing 2 jobs (1 completed)”) and a single aggregate bar. When only one job runs, show its message for familiarity.
- **Failure/cancel:** Per-job terminal events should remove that job from the map; aggregate message should surface failure/cancel state with minimal noise.

---

## Open Decisions (now resolved from owner’s answers)
1) Parallel starts allowed; ensure button logic and cancel UX match.
2) Aggregate weighting: present the simple-average default plus a brief note on the weighted option; align in implementation with the chosen default.
3) Per-job cancel is desired: add job_id to ProcessCommandResult, add cancel-by-job-id command, and surface per-job cancel buttons.

---

## What Tok io Brings Here (DX/UX impact)
- Tokio powers async orchestration: the semaphore (`OwnedSemaphorePermit`) gates concurrent jobs; dropping the permit frees a slot. Fair queuing avoids starvation.
- DX: async registry and cancellation remain responsive; ensure permits are not leaked on early errors to avoid deadlocks.
- UX: users see true parallelism when multiple jobs start; cancellation is responsive via atomic flags checked in the pipeline.

---

## Next Actions
- Unlock UI for parallel submissions; add per-job cancel UI using job_id from command result.
- Add max_concurrent getter/setter commands and UI control; wire to JobRegistry safely.
- Solidify aggregate weighting choice and document it; update StatusPanel accordingly.
- Add tests (Rust unit/integration + TS) and update mocks for job_id.
- Update README/AGENTS or relevant docs after implementation; wait for manual local verification before PR push.
