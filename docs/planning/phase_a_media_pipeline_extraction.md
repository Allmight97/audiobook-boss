# Phase A: Media Pipeline Extraction Plan

**Purpose**: Extract FFMPEG operations from `processor.rs` into a unified `media_pipeline.rs` module, implementing mentor's recommendations while achieving Phase A stabilization goals.

---

## Current State Analysis

### Target Metrics
- **`processor.rs`**: 1,455 lines → **<500 lines** (reduction of ~1,000 lines)
- **All functions**: **<60 lines** (currently many >60 lines)
- **All modules**: **<400 lines** (except processor.rs during transition)
- **Tests**: Move from implementation files to separate test modules

### Critical Issues Identified
1. **Dual FFMPEG Approaches**: Direct shell calls + `FFmpegCommand` abstraction
2. **Godzilla Functions**: Many functions >60 lines in `processor.rs`
3. **Tests Mixed with Implementation**: 400+ lines of tests in `processor.rs`
4. **No Unified Logging Strategy**: Scattered `log::` calls without abstraction

---

## Implementation Strategy

### **Step 1: Test Extraction (Foundation)**
**Goal**: Separate tests from implementation code
**Impact**: Reduces `processor.rs` by ~400 lines immediately

#### Actions:
1. Create `tests/audio/processor_tests.rs`
2. Move all `#[cfg(test)]` modules from implementation files
3. Update test imports to use `use crate::audio::processor::*;`
4. Verify all 130+ tests still pass

#### Files Affected:
- `src-tauri/src/audio/processor.rs` (remove tests)
- `tests/audio/processor_tests.rs` (new file)
- `tests/audio/mod.rs` (new file if needed)

---

### **Step 2: Create Media Pipeline Foundation**
**Goal**: Implement mentor's `media_pipeline.rs` with `MediaProcessingPlan`
**Impact**: Establishes unified FFMPEG interface

#### Core Structures:
```rust
// src-tauri/src/audio/media_pipeline.rs

pub struct MediaProcessingPlan {
    pub inputs: Vec<PathBuf>,
    pub output_path: PathBuf,
    pub settings: AudioSettings,
    pub metadata: Option<AudiobookMetadata>,
    pub session_id: String,
}

pub struct MediaPipeline {
    plan: MediaProcessingPlan,
    logger: Box<dyn PipelineLogger>,
}

pub trait PipelineLogger {
    fn log_start(&self, plan: &MediaProcessingPlan);
    fn log_progress(&self, stage: ProcessingStage, progress: f64);
    fn log_error(&self, error: &AppError);
    fn log_completion(&self, metrics: &ProcessingMetrics);
}
```

#### Actions:
1. Create `src-tauri/src/audio/media_pipeline.rs`
2. Implement `MediaProcessingPlan` struct
3. Create `PipelineLogger` trait with default implementation
4. Add module to `src-tauri/src/audio/mod.rs`

---

### **Step 3: Extract FFMPEG Command Building**
**Goal**: Unify dual FFMPEG approaches
**Impact**: Reduces `processor.rs` by ~200 lines

#### Functions to Extract from `processor.rs`:
- `build_merge_command()` (38 lines)
- `execute_with_progress_context()` (11 lines)
- `execute_with_progress_events()` (32 lines)
- `setup_process_execution()` (18 lines)
- `handle_progress_line()` (36 lines)
- `monitor_process_with_progress()` (18 lines)
- `finalize_process_execution()` (30 lines)

#### New `MediaPipeline` Methods:
```rust
impl MediaPipeline {
    pub fn new(plan: MediaProcessingPlan) -> Self { }
    pub fn build_command(&self) -> Result<Command> { }
    pub fn execute_with_progress(&self, reporter: &mut ProgressReporter) -> Result<PathBuf> { }
}
```

#### Actions:
1. Move FFMPEG command building logic to `media_pipeline.rs`
2. Replace direct `Command::new()` calls with `MediaPipeline` methods
3. Deprecate old `ffmpeg/command.rs` approach (keep for compatibility)
4. Update `processor.rs` to use new pipeline interface

---

### **Step 4: Extract Progress Monitoring**
**Goal**: Centralize progress tracking logic
**Impact**: Reduces `processor.rs` by ~300 lines

#### Functions to Extract:
- `process_progress_update_context()` (41 lines)
- `process_progress_update()` (26 lines)
- `calculate_and_display_progress()` (30 lines)
- `display_progress_with_duration()` (19 lines)
- `display_analysis_progress()` (6 lines)
- `parse_speed_multiplier()` (14 lines)
- `update_time_estimation()` (14 lines)
- `handle_progress_completion()` (10 lines)

#### New Module: `src-tauri/src/audio/progress_monitor.rs`
```rust
pub struct ProgressMonitor {
    logger: Box<dyn PipelineLogger>,
    emitter: ProgressEmitter,
}

impl ProgressMonitor {
    pub fn process_ffmpeg_line(&mut self, line: &str, total_duration: f64) -> Result<()> { }
    pub fn calculate_progress(&self, progress_time: f32, total_duration: f64) -> f64 { }
}
```

---

### **Step 5: Extract Workflow Orchestration**
**Goal**: Separate high-level workflow from implementation details
**Impact**: Reduces `processor.rs` by ~200 lines

#### Functions to Extract:
- `validate_and_prepare()` (8 lines)
- `prepare_workspace()` (26 lines)
- `execute_processing()` (31 lines)
- `write_metadata_stage()` (19 lines)
- `complete_processing()` (24 lines)
- `finalize_processing()` (11 lines)

#### New Module: `src-tauri/src/audio/workflow.rs`
```rust
pub struct ProcessingWorkflow {
    pub pipeline: MediaPipeline,
    pub monitor: ProgressMonitor,
    pub context: ProcessingContext,
}

impl ProcessingWorkflow {
    pub fn execute(&mut self, files: Vec<AudioFile>) -> Result<String> { }
}
```

---

### **Step 6: Function Size Optimization**
**Goal**: Ensure all functions are <60 lines
**Impact**: Improves readability and maintainability

#### Large Functions to Split (>60 lines):
- `process_audiobook_with_context()` (41 lines) ✅ Already compliant
- `merge_audio_files_with_events()` (33 lines) ✅ Already compliant
- `check_cancellation_and_kill()` (32 lines) ✅ Already compliant

#### Actions:
1. Audit all remaining functions in `processor.rs`
2. Split any functions >60 lines using extract method refactoring
3. Ensure single responsibility principle

---

## Expected Outcomes

### File Size Reductions:
```
processor.rs: 1,455 lines → ~450 lines (-1,005 lines)
├── Tests moved: -400 lines
├── FFMPEG commands: -200 lines  
├── Progress monitoring: -300 lines
├── Workflow orchestration: -200 lines
└── Remaining core logic: ~450 lines
```

### New Module Structure:
```
src-tauri/src/audio/
├── processor.rs (~450 lines - core logic only)
├── media_pipeline.rs (~200 lines - FFMPEG operations)
├── progress_monitor.rs (~150 lines - progress tracking)
├── workflow.rs (~200 lines - orchestration)
└── mod.rs (updated exports)

tests/audio/
├── processor_tests.rs (~400 lines)
├── media_pipeline_tests.rs (~100 lines)
├── progress_monitor_tests.rs (~50 lines)
└── workflow_tests.rs (~100 lines)
```

---

## Risk Mitigation

### Incremental Approach:
1. **One step at a time** - complete each step before moving to next
2. **Test after each step** - ensure all 130+ tests pass
3. **Commit after each step** - easy rollback if issues arise
4. **Adapter functions** - maintain backward compatibility during transition

### Validation Gates:
- [ ] All existing tests pass
- [ ] No breaking changes to public API
- [ ] Clippy warnings addressed
- [ ] Documentation updated

### Rollback Strategy:
- Git commits after each successful step
- Adapter functions maintain old interfaces
- Feature flags for new vs old code paths (if needed)

---

## Success Criteria

### Phase A Completion Metrics:
- ✅ `processor.rs` <500 lines (target: ~450 lines)
- ✅ All functions <60 lines
- ✅ Tests separated from implementation
- ✅ Unified FFMPEG interface via `MediaPipeline`
- ✅ `PipelineLogger` trait implemented
- ✅ All 130+ tests continue to pass
- ✅ No breaking changes to existing API

### Foundation for Phase B:
- Clear module boundaries established
- Extraction patterns proven and documented
- Systematic approach validated
- Ready for remaining module splits

---

## Implementation Order

This plan should be executed **BEFORE** the existing refactoring roadmap phases, as it:
1. **Establishes patterns** for systematic extraction
2. **Reduces complexity** before tackling other godzilla modules
3. **Validates approach** with the most critical module first
4. **Creates foundation** for mentor's recommended architecture

The existing Phase A, B, C roadmap can then proceed with this new foundation in place.
