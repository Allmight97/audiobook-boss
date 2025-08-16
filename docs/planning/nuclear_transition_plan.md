# P4.3 Nuclear Engine Transition & P4.4 Complete Legacy Purge

**Goal**: Eliminate all shell-based FFmpeg code and transition to 100% ffmpeg-next implementation

## Overview of Current State (POST-TRANSITION)

- **Single engine system**: ffmpeg-next only
- **Feature flags removed**: `legacy-adapters`, `safe-ffmpeg`
- **Shell dependencies removed**: `which`, direct `Command` spawning, bundled binaries
- **Conditional compilation purged**: All engine-selection `cfg` blocks removed
- **Goal achieved**: Clean ffmpeg-next implementation as the sole path

## Execution Plan - Atomized Steps

## Validation Strategy for Each Commit Point

For each commit point, validate with:

```bash
# Compilation check
cargo check

# Test compilation
cargo test --lib --no-run

# If tests compile, run them
cargo test --lib

# Build check
cargo build
```

### **Phase 1: Pre-Flight Validation & Git Safety Net** ✅
**Commit Point**: `P4.3-pre: Validate current state before nuclear transition`

1. **Baseline Test & Documentation**
   - Run comprehensive test suite in both configurations
   - Document current feature behavior for rollback reference
   - Commit current working state as recovery point

### **Phase 2: Cargo.toml Nuclear Changes** ✅
**Commit Point**: `P4.3.1: Nuclear Cargo.toml simplification - make ffmpeg-next mandatory`

2. **Remove Feature Flags & Dependencies**
   - **REMOVE** `legacy-adapters = []` feature entirely
   - **CHANGE** `default = ["legacy-adapters"]` to `default = []`
   - **REMOVE** `optional = true` from `ffmpeg-next` (make it mandatory)
   - **REMOVE** `safe-ffmpeg = ["dep:ffmpeg-next"]` feature
   - **REMOVE** `which = "6.0"` dependency entirely
   - **Test**: `cargo check` should compile successfully with simplified dependencies

### **Phase 3: Configuration File Cleanup** ✅
**Commit Point**: `P4.3.2: Remove FFmpeg binary bundling from Tauri config`

3. **Tauri Configuration Cleanup**
   - **DELETE** `"externalBin": ["binaries/ffmpeg-universal"]` from `tauri.conf.json`
   - **DELETE** entire `src-tauri/binaries/` directory
   - **UPDATE** `build.rs` to remove FFmpeg bundling warnings
   - **Test**: `cargo check` should compile without bundling warnings

### **Phase 4: Code Purge - Remove Shell FFmpeg Module** ✅
**Commit Point**: `P4.4.1: Delete shell FFmpeg module entirely`

4. **Delete Shell FFmpeg Code**
   - **DELETE** `src-tauri/src/ffmpeg/` directory entirely (command.rs, mod.rs)
   - **REMOVE** `pub mod ffmpeg;` from `src-tauri/src/lib.rs`
   - **Test**: Expect compilation errors - we'll fix these in next steps

### **Phase 5: Remove Conditional Compilation - Media Pipeline** ✅
**Commit Point**: `P4.4.2: Eliminate conditional compilation in media pipeline`

5. **Simplify Media Pipeline Module**
   - **REMOVE** all `#[cfg(not(feature = "safe-ffmpeg"))]` blocks in `media_pipeline.rs`
   - **REMOVE** shell-based imports: `process::Command`, `constants::*`, `progress_monitor`
   - **REMOVE** `ShellFFmpegProcessor` struct and implementation
   - **KEEP ONLY** `FfmpegNextProcessor` implementation (remove feature gates)
   - **REMOVE** synthetic command building for tests
   - **Test**: `cargo check` - expect more compilation errors

### **Phase 6: Processor Selection Simplification** ✅
**Commit Point**: `P4.4.3: Simplify processor selection to single ffmpeg-next engine`

6. **Update Processor Selection**
   - **REMOVE** conditional imports in `processor/selection.rs`
   - **REMOVE** `DefaultProcessor` type alias complexity
   - **HARDCODE** `FfmpegNextProcessor` as the only processor (Dev note: hardcode, is that a good senior software engineering practice inn this case?)
   - **SIMPLIFY** `get_engine_description()` to return static string
   - **SIMPLIFY** `create_default_processor()` to return `FfmpegNextProcessor`

### **Phase 7: Commands Module Cleanup** ✅
**Commit Point**: `P4.4.4: Remove shell dependencies from command handlers`

7. **Update Command Modules**
   - **REMOVE** `#[cfg(feature = "legacy-adapters")]` blocks in `commands/audio.rs`
   - **REMOVE** shell-based command implementations
   - **REMOVE** FFmpeg version detection in `commands/system.rs`
   - **REMOVE** imports of `crate::ffmpeg::` modules
   - **UPDATE** error handling to remove `FFmpegError` variants

### **Phase 8: Error Handling Simplification** ✅
8. **Update Error Types**
   - **REMOVE** `FFmpegError` import and variants from `errors.rs`
   - **SIMPLIFY** error conversion functions
   - **REMOVE** shell-specific error handling

   **Commit Point**: Write concise commit message summarizing what was done push commit.

### **Phase 9: Progress Monitor & Process Management Cleanup** ✅

9. **Remove Shell Process Code**
   - **DELETE** `progress_monitor.rs` entirely (shell process monitoring)
   - **REMOVE** process management imports from other modules
   - **REMOVE** `Child` process handling in cleanup modules
   - **REMOVE** `#[cfg(feature = "legacy-adapters")]` blocks

   **Commit Point**: Write concise commit message summarizing what was done and push commit.

### **Phase 10: Build System & Integration Cleanup** (PARTIAL / FOLLOW-UP) ✅

10. **Final System Cleanup**
    - **REMOVE** FFmpeg setup scripts from `package.json`
    - **SIMPLIFY** `build.rs` to remove all FFmpeg bundling logic
    - **REMOVE** FFmpeg binary references from `.gitignore` and docs
    - **UPDATE** any remaining documentation references

    **Commit Point**: Write concise commit message summarizing what was done and push commit.

### **Phase 11: Post-Nuclear Debug & Validation** ✅ (core engine aspects)

11. **Compilation Error Resolution**
    - **FIX** remaining compilation errors from removed dependencies
    - **UPDATE** imports to use only ffmpeg-next paths
    - **RESOLVE** any remaining shell command references
    - **ENSURE** `cargo build` succeeds

    **Commit Point**: Write concise commit message summarizing what was done and push commit.

12. **Test Suite Restoration**
    - **UPDATE** tests to work without shell command mocking
    - **FIX** integration tests that relied on shell FFmpeg
    - **ENSURE** `cargo test` passes with ffmpeg-next only

    **Commit Point**: Write concise commit message summarizing what was done and push commit.

13. **Functional Validation**
    - **TEST** audiobook processing works end-to-end
    - **VERIFY** metadata and cover art functionality preserved
    - **CONFIRM** progress tracking and cancellation work
    - **VALIDATE** all UI workflows function correctly

    **Commit Point**: Write concise commit message summarizing what was done and push commit.

### **Pre-Phase 12: Stabilization & Validation Interlude** ✅ COMPLETED

Purpose: Capture and track non-feature technical stabilization tasks that are NOT core Phase 12 feature work but should ideally be addressed (or consciously deferred) before deeper technical debt / feature reactivation begins.

Scope: Manual verification, optional test harness improvements, observability hardening, and risk gating items that ensure the single-engine baseline is solid.

#### A. Manual End-to-End Validation Checklist (Operator Runbook)
Run locally before cutting a Phase 12 branch:
1. Build & basic tests
   - `cargo check` (should be clean)
   - `cargo test --lib` (all pass: currently 58/58)
2. UI smoke
   - Launch app: `npm run tauri dev`
   - Drag a valid MP3 → confirm analysis data populates (duration, bitrate, sample rate)
3. Full processing
   - Process single MP3 → verify output `.m4b` exists, playable, approximate duration match
   - Confirm bitrate & channels via external tool (e.g. `ffprobe` optional)
4. Metadata
   - Read input metadata (title/artist) → process with modified metadata → re-read output via Lofty; fields persist
5. Cancellation (multi-file scenario)
   - Duplicate test file 3×, start processing, cancel mid-way
   - Expected: partial output removed (CleanupGuard), UI state resets, no panic
6. Environment flag behavior
   - Set `ABB_DISABLE_TWOOLOOP=1` and re-run processing → log should indicate twoloop disabled vs placeholder message when unset
7. Logging sanity
   - Ensure no warnings about removed legacy modules or feature flags

#### B. Optional Env-Gated End-to-End Processing Test (Deferred unless timeboxed ≤1h)
Goal: Add a deterministic integration test that executes the full pipeline when `ABB_E2E=1`.
Sketch:
```
#[tokio::test]
async fn e2e_single_file_smoke() { if std::env::var("ABB_E2E").ok().as_deref() != Some("1") { return; }
  // 1. Locate sample MP3 (skip if absent)
  // 2. Build context + session + settings (Auto sample rate)
  // 3. Call process_audiobook_with_context
  // 4. Assert output file exists & > 0 size
  // 5. (Optional) Re-read metadata
}
```
Deferral Reason: Avoid coupling CI to availability of ffmpeg shared libs until container image / CI strategy defined.

#### C. Targeted Test Coverage Enhancements (Pre-Phase 12 Suggestions)
| Area | Gap | Proposed Minimal Test | Status |
|------|-----|-----------------------|--------|
| Cancellation | Only indirect via guards | Multi-file cancel triggers error + no output | Deferred |
| Metadata roundtrip | Finalize writes + read-back not asserted | After process, re-read & compare selected fields | Deferred |
| CleanupGuard success path | Only indirectly covered | Unit test: add/remove paths, ensure preserve on manual remove | Deferred |
| Progress emission timing | Logic relies on 200ms throttle | Unit test using manual Instant injection (requires minor refactor) | Deferred |

Decision: All deferred to keep Phase 12 focused; create a separate tracking issue if not implemented in first Phase 12 sprint.

#### D. Observability & Logging Hardening
Current State: Logs report twoloop status and metadata placeholders.
Improvements (Optional Pre-Phase 12):
1. Add structured log prefix `engine=ffmpegnext` to start-up summary.
2. Add one-line summary after finalize: duration(s), output bitrate, sample rate, channels.
3. Promote cancellation cause to WARN (currently INFO).
Status: Not implemented (intentional defer; low risk).

#### E. Risk Gating Before Starting Phase 12
| Risk | Mitigation Now | Residual Risk if Ignored |
|------|----------------|---------------------------|
| Hidden regression in full encode path | Manual runbook E2E run | Minor (tests still unit-level) |
| Metadata drift after future cover art re-enable | Document current disabled state | Medium (will need snapshot tests) |
| Cancellation leaves orphan file | Manual multi-file cancel check | Low if runbook executed |
| Twoloop API changes upstream | Logged placeholder clarifies intent | Low |

Exit Criteria to begin Phase 12: ✅ COMPLETED
1. ✅ Runbook executed and recorded (timestamp + output size noted)
2. ✅ No unexpected WARN/ERROR log lines  
3. ✅ Output `.m4b` passes basic playback check (any media player)
4. ✅ All legacy modules and stale TODO references removed
5. ✅ Documentation updated to reflect single-engine architecture

#### F. Explicit Deferrals (Not in Phase 12 Scope Unless Re-Prioritized)
- Progress emission throttling refactor for testability
- Structured logging (JSON) mode
- CI container image with bundled FFmpeg libs for deterministic E2E

#### Summary
Baseline is stable (single engine, clean tests). We proceed to Phase 12 once the manual runbook passes. Optional env-gated E2E test can be added opportunistically but is not a blocker.

### **Phase 12: Technical Debt Resolution** ✅ PREPARATION COMPLETED - READY FOR IMPLEMENTATION

**Phase 12 Preparation Completed** ✅  
All nuclear cleanup tasks finished. Legacy modules removed, stale references updated.

**Phase 12 Implementation Plans Available**:
- **Preparation Plan**: [phase_12_preparation_plan.md](phase_12_preparation_plan.md) ✅ COMPLETED
- **Implementation Plan**: [phase_12_implementation_plan.md](phase_12_implementation_plan.md) ⏳ READY FOR EXECUTION

14. **Complete Deferred Features**
    - **IMPLEMENT** native cover art embedding via ffmpeg-next
    - **REPLACE** `#[cfg(feature = "never")]` placeholders with working code
    - **RESEARCH** and implement twoloop AAC enhancement properly
    - **ELIMINATE** Lofty fallback in finalize stage

    **Commit Point**: Write concise commit message summarizing what was done and push commit.

## Risk Mitigation

1. **Git Safety**: Each phase creates a commit point for potential rollback
2. **Incremental Validation**: Test compilation after each major change
3. **Focused Debugging**: Single implementation easier to debug than dual-engine
4. **No Production Impact**: Solo development environment

## Success Criteria (Updated)

- ✅ `cargo build` compiles successfully (single engine)
- ✅ `cargo test` passes for current implemented features
- ✅ No engine-related feature flags or conditional compilation
- ✅ No shell dependencies or bundled FFmpeg binaries
- ⏳ Native cover art embedding via ffmpeg-next (stream insertion) – temporarily disabled pending follow-up
- ⏳ Twoloop AAC enhancement – logging placeholder only
- ✅ UI processing produces valid M4B files (core path)
- ⏳ Remaining technical debt items tracked in roadmap
---

**Nuclear Approach Rationale**: The dual-engine complexity is causing more problems than it solves. Better to debug a clean, single-implementation codebase than continue with feature flag spaghetti code.