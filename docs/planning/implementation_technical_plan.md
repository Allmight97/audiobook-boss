# Implementation & Coding Standards Plan

_Last updated: 2025-07-27_  
_Current Status: Post-Phase 4 (All bugs fixed, functions ≤30 lines, zero production unwrap())_  
_Next Focus: Module splitting (5 modules exceed 400-line limit)_

## Purpose
Provide a concrete, step-by-step plan that junior developers **and AI agents** can follow to:
1. Complete the **remaining module refactor** (5 oversized modules exceed 400-line limit).
2. Maintain established **code standards** (≤ 60 lines per function, ≤ 400 lines per module).
3. Apply the proven **facade-API pattern** consistently across all modules.
4. Address remaining UI bugs (3 quick wins from bug tracker).

## Current Module Status

| Module            | Lines | Status     | Priority            | Target Split |
| ----------------- | ----- | ---------- | ------------------- | ------------ |
| `processor.rs`    | 1,455 | 🔴 Critical | P0 - Highest Impact | 7 sub-modules |
| `cleanup.rs`      | 946   | 🔴 Critical | P1                  | 3 sub-modules |
| `context.rs`      | 804   | 🔴 Critical | P2                  | 3 sub-modules |
| `progress.rs`     | 485   | 🟡 High     | P3                  | 2 sub-modules |
| `commands/mod.rs` | 438   | 🟡 High     | P4                  | 3 sub-modules |

**Target**: All modules ≤400 lines (≤300 lines preferred)

---

## Part A: Coding Standards

| Area                   | Rule                                                                 | Rationale                                                   |
| ---------------------- | -------------------------------------------------------------------- | ----------------------------------------------------------- |
| Function length        | ≤ **60** effective lines (exclude comments/blank tests)              | Keeps cognitive load low; encourages single-responsibility. |
| Module length          | ≤ **400** effective lines for all modules                            | Avoids God-modules; simplifies navigation & review.         |
| Critical modules > 400 | **Must be refactored** - 5 modules currently exceed limit           | Active maintenance burden; violates Single Responsibility.  |
| Public Surface         | Each top-level feature exposes a **single façade module** (`mod.rs`) | Encapsulation, flexibility, and clear entry-points.         |
| Tests                  | Inline `#[cfg(test)]` allowed but prefer separate `tests/` trees     | Prevent production LOC inflation.                           |
| Logging                | Use `log` crate, no `eprintln!` in production code                   | Consistent observability.                                   |
| Error handling         | No `.unwrap()` / `.expect()` in production; use `?` or custom error  | Prevent runtime panics.                                     |

### Definitions
• *Effective lines* = lines containing code (strip comments & blank). Use `tokei --output json` for measurement.
• *Façade module* = a tiny `mod.rs` (or `lib.rs`) re-exporting the _minimal_ public API from private sub-modules.

---

## Part B: Enforcement Strategy

**Current Status**: Working `dev-check` alias already exists and is proven effective.

1. **Existing Clippy baseline** (already implemented)
   ```rust
   #![deny(clippy::unwrap_used)]
   #![warn(clippy::too_many_lines)]
   ```

2. **Simple LOC monitoring** (for learning environment)
   ```bash
   # Manual check for oversized modules
   find src-tauri/src -name "*.rs" -exec wc -l {} + | sort -nr | head -10
   ```
   • **No automated enforcement** - keep it simple for Rust beginner
   • Focus on **awareness and gradual improvement**

3. **Proven dev-check workflow** (already working)
   ```bash
   # Current working alias:
   alias dev-check="./scripts/loc_guard.sh && cargo clippy -- -D warnings && cargo test --workspace"
   ```
   • **130 tests passing, zero clippy warnings** - system is working
   • Simple, reliable, appropriate for learning environment

---

## Part C: Current Status & Remaining Work

### ✅ Phases 1-4 Complete (refactoring_debug_plan.md)

**Achievements**:
- ✅ All 7 bugs fixed (sample rate, progress, metadata, file reordering, cover art)
- ✅ All functions ≤30 lines (exceeded 60-line target)
- ✅ Zero `unwrap()` calls in production code
- ✅ 130 tests passing, zero clippy warnings
- ✅ Robust error handling with `AppError` enum
- ✅ Working `dev-check` validation system

### 🔄 Phase 5: Module Refactoring (Current Focus)

**Goal**: Split 5 oversized modules using proven facade pattern to achieve ≤400 lines per module.

---

## Part D: Detailed Implementation Steps

### Phase 5.0: UI Quick Wins (2-4 hours)

**Goal**: Address remaining UI bugs before module refactoring

#### Task 0.1: Fix Cover Art Clear Button Visibility
**Issue**: 'clear cover art' button only visible when loaded via button, not from file selection  
**Action**:
1. Edit `/Users/jstar/Projects/audiobook-boss/src/ui/coverArt.ts`
2. Ensure `showClearButton()` is called when cover art loaded from any source
3. Test: Select file with cover art → verify clear button appears

**Validation**: Clear button visible when any file with cover art is selected

#### Task 0.2: Add File List Clear Button  
**Issue**: No way to clear loaded files from file list  
**Action**:
1. Edit `/Users/jstar/Projects/audiobook-boss/src/ui/fileList.ts`
2. Add minimal "Clear All" button using existing UI patterns
3. Wire to backend command or local state clear
4. Test: Load files → click clear → verify list empty

**Validation**: Files can be cleared from UI with single button click

#### Task 0.3: Fix FFmpeg Merge Message
**Issue**: Terminal shows "Starting FFmpeg merge" for single files  
**Action**:
1. Review progress message logic in `src-tauri/src/audio/processor.rs`
2. Determine if message is placeholder or indicates actual behavior
3. Fix message to reflect actual operation (single file vs merge)

**Validation**: Terminal messages accurately reflect operation type

### Phase 5.1: Split processor.rs (1,455 → ~300 lines each)

**Priority**: P0 - Highest impact module  
**Target**: 7 focused sub-modules under `processor/` facade

#### Analysis: processor.rs Function Groups

1. **Sample Rate Detection** (25 functions) - `detect_input_sample_rate`, `get_file_sample_rate`
2. **File Validation** (4 functions) - `validate_processing_inputs`, `validate_inputs_with_progress` 
3. **Workspace Management** (6 functions) - `create_temp_directory*`, `prepare_workspace`, `cleanup_temp_directory*`
4. **FFmpeg Command Building** (3 functions) - `build_merge_command`, `move_to_final_location`
5. **Progress Processing** (12 functions) - All `*progress*` functions, `parse_speed_multiplier`
6. **Process Execution** (8 functions) - `monitor_process*`, `setup_process_execution`, etc.
7. **Session Management** (4 functions) - `create_session_from_legacy_state`, cancellation functions

#### Implementation Steps

**Step 1**: Create new module structure
```bash
# Run from: /Users/jstar/Projects/audiobook-boss/src-tauri/src/audio/
mkdir processor
touch processor/detection.rs     # Sample rate detection
touch processor/validation.rs    # Input validation  
touch processor/workspace.rs     # Temp directory management
touch processor/command.rs       # FFmpeg command building
touch processor/progress.rs      # Progress monitoring
touch processor/execution.rs     # Process execution
touch processor/session.rs       # Session management
touch processor/mod.rs           # Main facade
```

**Step 2**: Move functions to appropriate modules
- **detection.rs**: `detect_input_sample_rate`, `get_file_sample_rate`
- **validation.rs**: `validate_processing_inputs`, `validate_inputs_with_progress`, `validate_and_prepare`
- **workspace.rs**: `create_temp_directory*`, `prepare_workspace`, `cleanup_temp_directory*`
- **command.rs**: `build_merge_command`, `move_to_final_location`
- **progress.rs**: All progress parsing and monitoring functions
- **execution.rs**: `monitor_process*`, `setup_process_execution`, `finalize_process_execution`
- **session.rs**: Session creation and cancellation functions

**Step 3**: Create facade in `processor/mod.rs`
```rust
// Re-export all public functions to maintain API compatibility
pub use detection::detect_input_sample_rate;
pub use validation::validate_processing_inputs;
// ... etc for all public functions

// Keep main processing function here
pub async fn process_audiobook(
    files: Vec<AudioFile>,
    settings: AudioSettings, 
    metadata: Option<AudiobookMetadata>,
) -> Result<String> {
    // Orchestration logic only - delegate to sub-modules
}
```

### Phase 5.2: Split cleanup.rs (946 → ~300 lines each)

**Priority**: P1  
**Target**: 3 focused sub-modules under `cleanup/` facade

#### Implementation Steps

**Step 1**: Create sub-module structure
```bash
mkdir cleanup
touch cleanup/guard.rs           # CleanupGuard implementation
touch cleanup/process.rs         # ProcessGuard implementation  
touch cleanup/strategies.rs      # Cleanup strategies and utilities
touch cleanup/mod.rs             # Facade and common types
```

**Step 2**: Module responsibilities
- **guard.rs**: Core CleanupGuard struct and Drop implementation
- **process.rs**: ProcessGuard struct and process-specific cleanup
- **strategies.rs**: Cleanup algorithms and utility functions
- **mod.rs**: Public API facade, shared types and traits

### Phase 5.3: Split context.rs (804 → ~300 lines each)

**Priority**: P2  
**Target**: 3 focused sub-modules under `context/` facade

#### Implementation Steps

**Step 1**: Create sub-module structure  
```bash
mkdir context
touch context/processing.rs     # ProcessingContext + builder
touch context/progress.rs       # ProgressContext + builder  
touch context/types.rs          # Shared types and traits
touch context/mod.rs            # Facade
```

**Step 2**: Module responsibilities
- **processing.rs**: ProcessingContext and ProcessingContextBuilder
- **progress.rs**: ProgressContext and ProgressContextBuilder  
- **types.rs**: Shared enums, constants, utility types
- **mod.rs**: Public API facade

### Phase 5.4: Split progress.rs and commands/mod.rs

**Priority**: P3-P4  
**Target**: 2-3 focused modules each

#### Split progress.rs (485 lines)
```bash
mkdir progress
touch progress/emitter.rs       # ProgressEmitter and event emission
touch progress/tracker.rs       # Progress calculation and tracking
touch progress/mod.rs           # Facade and shared types
```

#### Split commands/mod.rs (438 lines)
```bash
# In commands directory
touch audio.rs                  # Audio processing commands
touch metadata.rs               # Metadata manipulation commands  
touch files.rs                  # File management commands
# mod.rs stays as facade
```

---

## Part E: Validation Protocols

### Per-Phase Validation

**After each module split**:
1. **Build validation**: 
   ```bash
   cd /Users/jstar/Projects/audiobook-boss/src-tauri
   cargo check
   cargo clippy -- -D warnings  
   cargo test
   ```

2. **Line count validation**:
   ```bash
   find src -name "*.rs" -exec wc -l {} + | sort -nr
   # Verify no file >400 lines
   ```

3. **API compatibility**:
   ```bash
   # Test frontend integration
   npm run dev  # Verify UI loads
   # Test core functions via browser console
   ```

### Integration Testing Strategy

**After each phase**:
1. **Smoke test**: Load sample audio files
2. **Processing test**: Complete audio merge operation  
3. **Progress test**: Verify progress events emit correctly
4. **Error test**: Test with invalid inputs
5. **Cleanup test**: Verify temp files cleaned up

---

## Part F: Adding New Features – Project-Specific Guide

**For post-Phase 5 codebase**:

1. **Start with a Story** – describe behavior and acceptance criteria
2. **Choose appropriate module**:
   • If existing module < 350 LOC, extend it
   • Otherwise create new facade module following `ffmpeg/` or `metadata/` pattern
3. **Write tests first** - leverage existing test infrastructure (130 passing tests)
4. **Implement with established patterns**:
   • Functions ≤60 lines (current: all ≤30 lines)
   • Use `Result<T, AppError>` error handling
   • Follow facade pattern for public APIs
5. **Hook into UI/Tauri** – add minimal command wrapper
6. **Validate with `dev-check`** – proven working validation
7. **Test frontend integration** – ensure UI functionality preserved

---

## Part G: FAQ / Why Facade API?

**Proven benefits in this codebase**:

* **Encapsulation** – `ffmpeg/` and `metadata/` modules demonstrate clean separation
* **Maintainability** – Split large modules without breaking public APIs
* **Beginner-friendly** – Clear entry points instead of 1,455-line files
* **Testing** – Easier to test focused modules (130 tests already passing)
* **AI Safety** – Agents can work on internals without touching public APIs
* **Incremental refactoring** – Proven path from monolith to clean modules

---

## Implementation Notes

### For AI Agents
1. **One module at a time**: Complete each phase fully before starting next
2. **Preserve all imports**: Maintain existing dependency relationships  
3. **Test after each move**: Don't batch file operations
4. **Use facade pattern**: Follow existing `ffmpeg/` and `metadata/` patterns
5. **Maintain line limits**: Each new module must be ≤400 lines

### For Human Developers
1. **Start with Phase 5.0**: UI quick wins provide immediate value
2. **Use `dev-check` liberally**: Run after each significant change
3. **Keep old code commented**: Until new structure proven working
4. **Document module boundaries**: Update module-level docs
5. **Test frontend integration**: UI must continue working

### Risk Mitigation
1. **Backup current state**: Git commit before each phase
2. **Incremental approach**: One logical group at a time
3. **Rollback strategy**: Keep old code until new code tested
4. **Integration focus**: Verify frontend/backend connection
5. **Performance monitoring**: Watch for degradation

---

## Next Actions Checklist

### ✅ Completed
- ✅ Phases 1-4 of refactoring_debug_plan.md complete
- ✅ All bugs fixed, functions ≤30 lines, zero production unwrap()
- ✅ 130 tests passing, zero clippy warnings
- ✅ Working `dev-check` validation system
- ✅ Proven facade pattern in `ffmpeg/` and `metadata/` modules

### 🔄 In Progress (Phase 5)
- [ ] Phase 5.0: UI quick wins (3 bugs) - **Next Up**
- [ ] Phase 5.1: Split processor.rs (1,455 lines) into 7 sub-modules
- [ ] Phase 5.2: Split cleanup.rs (946 lines) into 3 sub-modules
- [ ] Phase 5.3: Split context.rs (804 lines) into 3 sub-modules
- [ ] Phase 5.4: Split progress.rs (485 lines) and commands/mod.rs (438 lines)

### 🎯 Success Criteria
- [ ] All modules ≤400 lines
- [ ] Public APIs preserved (zero breaking changes)  
- [ ] All tests still passing
- [ ] Frontend integration maintained
- [ ] Facade pattern consistently applied
- [ ] UI bugs resolved

**Development approach**: Use proven `dev-check` workflow, implement one module at a time, test thoroughly. 