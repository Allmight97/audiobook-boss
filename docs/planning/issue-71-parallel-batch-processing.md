# Plan: Enable Parallel Batch Processing (Issue #71)

**Date**: 2025-12-08
**Status**: Approved
**Issue**: https://github.com/Allmight97/audiobook-boss/issues/71

## Summary
Replace the global `is_processing` mutex with a job registry + semaphore pattern to enable N concurrent audiobook conversions, unlocking full hardware potential for batch processing.

## Design Decisions (from user input)
- **UI**: Aggregated progress view (single bar showing "2/4 jobs, 65%")
- **Cancel**: Single button cancels all active jobs
- **Default limit**: Auto (`num_cpus / 2`) with manual override setting

---

## Implementation Phases

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

### Phase 4: UI Integration

**4.1 Modify `src/ui/statusPanel/logic.ts`**

Track aggregated state:
```typescript
interface AggregateProgress {
    activeJobs: number;
    completedJobs: number;
    totalJobs: number;
    overallPercentage: number;
}

// Update progress handler to aggregate by job_id
public updateProgress(event: ProcessingProgressEvent): void {
    const jobId = event.job_id ?? 'default';
    this.jobProgress.set(jobId, event);

    // Calculate aggregate
    const aggregate = this.calculateAggregate();
    this.updateAggregateUI(aggregate);
}
```

**4.2 Modify `src/ui/statusPanel/dom.ts`**

Update status text format:
```typescript
export const updateStatusText = (status: AggregateProgress): void => {
    const text = status.activeJobs > 1
        ? `Converting ${status.activeJobs} files (${status.overallPercentage.toFixed(0)}%)`
        : `Converting (${status.overallPercentage.toFixed(0)}%)`;
    // ...
};
```

---

### Phase 5: Settings (max_concurrent Override)

**5.1 Create `src-tauri/src/audio/settings_app.rs`** (NEW)
```rust
pub struct AppSettings {
    pub max_concurrent_jobs: Option<usize>,  // None = auto
}

pub fn get_effective_max_concurrent() -> usize {
    // Check env var / stored setting, fallback to num_cpus / 2
}
```

**5.2 Add new command**
```rust
#[tauri::command]
pub fn set_max_concurrent_jobs(max: Option<usize>) -> Result<String>;

#[tauri::command]
pub fn get_max_concurrent_jobs() -> usize;
```

**5.3 Modify `src/ui/encoderPanel/` or create `src/ui/settingsPanel/`**
- Add slider/input for "Max Concurrent Conversions"
- Options: "Auto (X cores)" or manual 1-8
- Persist to localStorage

---

## Critical Files Summary

| File | Change Type | Purpose |
|------|-------------|---------|
| `src-tauri/src/audio/job_registry.rs` | NEW | Job registry + semaphore |
| `src-tauri/src/audio/progress/reporter.rs` | MODIFY | Add job_id to events |
| `src-tauri/src/commands/audio.rs` | MODIFY | Integrate registry |
| `src-tauri/src/audio/session.rs` | MODIFY | Job-aware cancellation |
| `src-tauri/src/lib.rs` | MODIFY | Register JobRegistry |
| `src/types/events.ts` | MODIFY | Add job_id field |
| `src/ui/statusPanel/logic.ts` | MODIFY | Aggregate progress |
| `src-tauri/Cargo.toml` | MODIFY | Add num_cpus crate |

---

## Testing Requirements

**Rust Unit Tests** (`src-tauri/tests/unit/job_registry_tests.rs`):
- `test_semaphore_blocks_at_limit` - 3rd job waits when max=2
- `test_cancel_all_affects_all_jobs` - Global cancel flag works
- `test_job_id_uniqueness` - UUIDs are unique
- `test_completed_job_releases_permit` - Semaphore permit returns

**Integration Tests** (`src-tauri/tests/integration/parallel_processing.rs`):
- `test_two_jobs_complete_faster_than_serial` - Timing verification
- `test_progress_events_include_job_id` - Event contract

**Frontend Tests** (`src/ui/statusPanel/logic.test.ts`):
- `test_aggregate_progress_calculation` - Math correctness

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Race conditions in job registry | Use `RwLock` for registry, `AtomicBool` for cancel flags |
| Memory exhaustion with many jobs | Cap max_concurrent at 8, warn in UI above 4 |
| Backward compat breakage | Keep `job_id` optional in events, preserve existing cancel behavior |

---

## Out of Scope (Future)
- Per-job cancel buttons (user chose "cancel all")
- Individual job progress bars (user chose "aggregated view")
- Persistent job queue across app restarts
