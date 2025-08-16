# P4.3 Nuclear Engine Transition & P4.4 Complete Legacy Purge

**Goal**: Eliminate all shell-based FFmpeg code and transition to 100% ffmpeg-next implementation

## Overview of Current State

- **Dual engine system**: Shell-based FFmpeg + ffmpeg-next with feature flags
- **Feature flags**: `legacy-adapters` (default), `safe-ffmpeg` (optional)
- **Shell dependencies**: `which` crate, `Command`/`process`, FFmpeg binaries
- **Complex conditional compilation**: `#[cfg(feature = "...")]` blocks throughout codebase
- **Working ffmpeg-next implementation**: Already functional with `--features safe-ffmpeg`

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

### **Phase 2: Cargo.toml Nuclear Changes**
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

### **Phase 7: Commands Module Cleanup**
**Commit Point**: `P4.4.4: Remove shell dependencies from command handlers`

7. **Update Command Modules**
   - **REMOVE** `#[cfg(feature = "legacy-adapters")]` blocks in `commands/audio.rs`
   - **REMOVE** shell-based command implementations
   - **REMOVE** FFmpeg version detection in `commands/system.rs`
   - **REMOVE** imports of `crate::ffmpeg::` modules
   - **UPDATE** error handling to remove `FFmpegError` variants

### **Phase 8: Error Handling Simplification**
8. **Update Error Types**
   - **REMOVE** `FFmpegError` import and variants from `errors.rs`
   - **SIMPLIFY** error conversion functions
   - **REMOVE** shell-specific error handling

   **Commit Point**: Write concise commit message summarizing what was done and commit.

### **Phase 9: Progress Monitor & Process Management Cleanup**

9. **Remove Shell Process Code**
   - **DELETE** `progress_monitor.rs` entirely (shell process monitoring)
   - **REMOVE** process management imports from other modules
   - **REMOVE** `Child` process handling in cleanup modules
   - **REMOVE** `#[cfg(feature = "legacy-adapters")]` blocks

   **Commit Point**: Write concise commit message summarizing what was done and commit.

### **Phase 10: Build System & Integration Cleanup**

10. **Final System Cleanup**
    - **REMOVE** FFmpeg setup scripts from `package.json`
    - **SIMPLIFY** `build.rs` to remove all FFmpeg bundling logic
    - **REMOVE** FFmpeg binary references from `.gitignore` and docs
    - **UPDATE** any remaining documentation references

    **Commit Point**: Write concise commit message summarizing what was done and commit.

### **Phase 11: Post-Nuclear Debug & Validation**

11. **Compilation Error Resolution**
    - **FIX** remaining compilation errors from removed dependencies
    - **UPDATE** imports to use only ffmpeg-next paths
    - **RESOLVE** any remaining shell command references
    - **ENSURE** `cargo build` succeeds

    **Commit Point**: Write concise commit message summarizing what was done and commit.

12. **Test Suite Restoration**
    - **UPDATE** tests to work without shell command mocking
    - **FIX** integration tests that relied on shell FFmpeg
    - **ENSURE** `cargo test` passes with ffmpeg-next only

    **Commit Point**: Write concise commit message summarizing what was done and commit.

13. **Functional Validation**
    - **TEST** audiobook processing works end-to-end
    - **VERIFY** metadata and cover art functionality preserved
    - **CONFIRM** progress tracking and cancellation work
    - **VALIDATE** all UI workflows function correctly

    **Commit Point**: Write concise commit message summarizing what was done and commit.

### **Phase 12: Technical Debt Resolution**

14. **Complete Deferred Features**
    - **IMPLEMENT** native cover art embedding via ffmpeg-next
    - **REPLACE** `#[cfg(feature = "never")]` placeholders with working code
    - **RESEARCH** and implement twoloop AAC enhancement properly
    - **ELIMINATE** Lofty fallback in finalize stage

    **Commit Point**: Write concise commit message summarizing what was done and commit.

## Risk Mitigation

1. **Git Safety**: Each phase creates a commit point for potential rollback
2. **Incremental Validation**: Test compilation after each major change
3. **Focused Debugging**: Single implementation easier to debug than dual-engine
4. **No Production Impact**: Solo development environment

## Success Criteria

- ✅ `cargo build` compiles successfully
- ✅ `cargo test` passes completely
- ✅ Single, clean ffmpeg-next-only codebase
- ✅ No feature flags or conditional compilation
- ✅ No shell dependencies (`which`, `Command`, etc.)
- ✅ No bundled FFmpeg binaries
- ✅ All metadata and cover art functionality preserved
- ✅ UI processing creates valid M4B files
- ✅ Technical debt items resolved
---

**Nuclear Approach Rationale**: The dual-engine complexity is causing more problems than it solves. Better to debug a clean, single-implementation codebase than continue with feature flag spaghetti code.