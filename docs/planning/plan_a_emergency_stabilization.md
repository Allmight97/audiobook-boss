# Implementation & Coding Standards Plan
_Next: `docs/planning/plan_b_systematic_module_splitting.md`_
_Final: `docs/planning/plan_c_quality_enhancement.md`_

## Revised Plan Purpose
Provide realistic, sequential plans to address:
1. **Phase A**: High-priority refactoring of `processor.rs` and DRY violations.
2. **Phase B**: Systematic module splitting with DRY remediation.
3. **Phase C**: Final quality improvements and standards compliance.

---

## Plan Breakdown
The refactoring is split into three distinct plans:

### **Phase A**: Stabilization (This Plan)
- **Focus**: `processor.rs` refactoring and progress tracking DRY violations.

### **Phase B**: Systematic Module Splitting (Future)
- **Focus**: `cleanup.rs`, `context.rs`, `progress.rs`, `commands/mod.rs`.

### **Phase C**: Quality Enhancement (Future)
- **Focus**: Test DRY violations, naming consistency, final polish.

---

## High-Priority Issues Assessment

### Issues Blocking Feature Development

| Issue                                      | Priority      | Impact                       |
| ------------------------------------------ | ------------- | ---------------------------- |
| `processor.rs` (1,455 lines)               | P0 (Critical) | Blocks all audio features    |
| Progress tracking DRY violations           | P0 (Critical) | Breaks progress UI updates   |
| Function length violations (50-100+ lines) | P1 (High)     | Hinders code understanding   |
| Test setup DRY violations                  | P2 (Medium)   | Complicates new test writing |

### `processor.rs` Rationale
- **Size**: 1,455 lines with over 60 functions.
- **Violations**: Multiple functions exceed 50-100 lines; progress logic is scattered and duplicated.
- **Impact**: Changes to any audio processing logic are risky and difficult. Debugging is inefficient.

---

## PLAN A: STABILIZATION

### Phase A1: DRY Violation Remediation

#### A1.1: Progress Tracking Consolidation (P0)
**Problem**: Progress calculation logic is duplicated in 4+ locations, making UI changes error-prone.

**Approach**:
```rust
// NEW: src-tauri/src/audio/progress_utils.rs
pub struct ProgressCalculator {
    stage: ProcessingStage,
    total_files: usize,
    current_file: usize,
}

impl ProgressCalculator {
    pub fn calculate_percentage(&self, file_progress: f64) -> f32 {
        // Centralized logic, single source of truth
    }
    
    pub fn format_eta(&self, speed: Option<f64>) -> Option<String> {
        // Centralized ETA calculation
    }
}
```

**Files to consolidate**:
- Extract from `processor.rs` (progress calculation functions).
- Extract from `progress.rs` (parsing logic).
- Update callers to use the centralized utility.

**Validation**: All progress events must emit correctly, and UI updates should remain unchanged.

#### A1.2: Test Setup Utilities (IMMEDIATE QUICK WIN)
**Problem**: The `TempDir::new().unwrap()` pattern is repeated 15+ times in tests.

**Approach**:
```rust
// NEW: src-tauri/src/test_utils.rs
#[cfg(test)]
pub mod test_utils {
    use tempfile::TempDir;
    use std::path::PathBuf;
    use std::fs;
    use crate::metadata::types::AudiobookMetadata;
    
    pub fn create_test_audio_file(filename: &str, content: &[u8]) -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join(filename);
        fs::write(&file_path, content).unwrap();
        (temp_dir, file_path)
    }
    
    pub fn create_test_m4b() -> (TempDir, PathBuf) {
        create_test_audio_file("test.m4b", b"test audiobook data")
    }
    
    pub fn create_test_metadata() -> AudiobookMetadata {
        AudiobookMetadata {
            title: Some("Test Audiobook".to_string()),
            author: Some("Test Author".to_string()),
            narrator: Some("Test Narrator".to_string()),
            ..Default::default()
        }
    }
    
    pub fn create_test_file_list(count: usize) -> (TempDir, Vec<PathBuf>) {
        let temp_dir = TempDir::new().unwrap();
        let files: Vec<PathBuf> = (0..count)
            .map(|i| {
                let path = temp_dir.path().join(format!("file_{:02}.mp3", i));
                fs::write(&path, format!("test data {}", i)).unwrap();
                path
            })
            .collect();
        (temp_dir, files)
    }
}
```

**Implementation Notes for AI Agent**:
- Create this module FIRST as it helps test all subsequent changes
- Update ALL test files to use these utilities
- This is a SAFE change that won't affect production code
- Run `cargo test` after each file update to ensure tests still pass

**Priority**: P0 (IMMEDIATE) - Do this alongside A1.1 for maximum efficiency.

#### A1.3: Long Parameter List Fixes (QUICK WIN)
**Problem**: Functions with 7+ parameters are hard to understand and maintain.

**Approach**:
```rust
// Example: process_progress_update_context has 7 parameters
// BEFORE:
pub fn process_progress_update_context(
    app_handle: &AppHandle,
    session_id: String,
    stage: ProcessingStage,
    current: usize,
    total: usize,
    percentage: f32,
    message: String,
) -> Result<()>

// AFTER:
pub struct ProgressUpdateRequest {
    pub session_id: String,
    pub stage: ProcessingStage,
    pub current: usize,
    pub total: usize,
    pub percentage: f32,
    pub message: String,
}

pub fn process_progress_update_context(
    app_handle: &AppHandle,
    request: ProgressUpdateRequest,
) -> Result<()>
```

**Files to fix**:
- `processor.rs`: `process_progress_update_context()` (7 params)
- Look for any other functions with 5+ parameters

**Priority**: P1 (High) - Makes code more maintainable immediately.

### Phase A2: `processor.rs` Surgical Splitting

**Strategy**: Extract 3 sub-modules first, keeping the rest of the file intact for now.

#### A2.1: Extract Sample Rate Detection
```rust
// NEW: src-tauri/src/audio/processor/detection.rs
pub fn detect_input_sample_rate(file_paths: &[PathBuf]) -> Result<u32>
pub fn get_file_sample_rate(path: &Path) -> Result<u32>
// ~50 lines extracted
```

**Priority**: P3 (Low) - Pure functions with no side effects.

#### A2.2: Extract Validation Logic
```rust
// NEW: src-tauri/src/audio/processor/validation.rs
pub fn validate_processing_inputs(files: &[AudioFile], settings: &AudioSettings) -> Result<()>
pub fn validate_inputs_with_progress(...) -> Result<()>
// ~80 lines extracted
```

**Priority**: P2 (Medium) - Validates inputs; could affect error handling.

#### A2.3: Refactor Core Processing Function
```rust
// NEW: src-tauri/src/audio/processor/mod.rs (facade)
pub use detection::detect_input_sample_rate;
pub use validation::validate_processing_inputs;

// Keep main processing function HERE for now
pub async fn process_audiobook(...) -> Result<String> {
    // Orchestration only, delegate to extracted modules
}
```

**Priority**: P1 (High) - Main entry point; could affect progress events and overall functionality.

### Phase A3: Validation & Stabilization

#### A3.1: Integration Testing
```bash
# Must pass after each extraction:
cd src-tauri
cargo test --lib  # All 130+ tests
cargo clippy -- -D warnings  # Zero warnings
npm run tauri dev  # UI loads and works
```

#### A3.2: Progress Event Validation
- Load test files and verify that progress events emit correctly.
- Check that the UI progress bar updates smoothly.
- Verify ETA calculations match previous behavior.

#### A3.3: Performance Regression Testing
- Time a typical audiobook merge operation.
- Verify that memory usage has not increased.
- Check that error messages are still clear.

---

## AI AGENT WORK ORGANIZATION GUIDE

### Parallel Work Streams
To maximize efficiency, organize AI agents to work on non-conflicting files:

**Stream 1: Test Utilities (Safe, Independent)**
- Agent 1 creates `test_utils.rs`
- Agent 2 updates test files to use utilities
- No risk of breaking production code

**Stream 2: Progress Consolidation (Critical Path)**
- Agent 3 creates `progress_utils.rs`
- Agent 4 updates callers after utilities are ready
- Must be done carefully with validation

**Stream 3: Parameter Refactoring (Medium Risk)**
- Agent 5 creates request structs for long parameter lists
- Can be done independently of other changes

### Sequential Work (Must be done in order)
1. Complete Streams 1-3
2. Begin processor.rs extraction (A2.1, A2.2, A2.3)
3. Validate and stabilize

### Agent Instructions Template
When deploying agents, use this template:
```
Task: [Specific task from plan]
File(s): [Exact files to modify]
Dependencies: [What must be complete first]
Validation: Run `cargo test` after changes
Constraints:
- Do NOT modify [list of off-limits files]
- Preserve all public APIs
- Add tests for new utilities
```

## SUCCESS CRITERIA FOR PLAN A

### Must Be Met Before Starting Plan B
- [ ] `processor.rs` reduced to ≤800 lines (from 1,455).
- [ ] Progress tracking logic is centralized (no DRY violations).
- [ ] Test utilities extracted (15+ duplications eliminated).
- [ ] Functions with 7+ parameters refactored to use structs.
- [ ] All 130+ tests are still passing.
- [ ] Zero clippy warnings.
- [ ] UI functionality is unchanged.
- [ ] No performance regressions.

### What NOT to Do in Plan A
- **Do not touch** `cleanup.rs` or `context.rs` (reserved for Plan B).
- **Do not refactor** for function length violations yet (except parameter lists).
- **Do not optimize**—focus on extraction only.
- **Do not change** public APIs; preserve all existing interfaces.
- **Do not fix** file system artifacts in tests (save for Phase C).

---

## DRY VIOLATION REMEDIATION STRATEGY

### P0 Priority (Plan A)
1. **Progress calculation**: 4+ similar functions → 1 utility.
2. **Test setup patterns**: 15+ repetitions → shared utilities.
3. **Error message formatting**: inconsistent patterns → standardized helpers.

### P1 Priority (Plan B)
4. **Temp directory management**: 3+ similar functions.
5. **File validation patterns**: repeated across modules.
6. **FFmpeg command building**: similar patterns in multiple places.

### P2 Priority (Plan C)
7. **Path checking logic**: minor repetitions.
8. **Logging patterns**: inconsistent formatting.
9. **Configuration validation**: repeated parameter checks.

---

## SAFETY & VALIDATION GUARDRAILS

### Before Each Change
1. **Understand the function completely**.
2. **Identify all callers**.
3. **Run tests before extraction** to establish a baseline.
4. **Extract incrementally**—one function at a time.
5. **Test after each extraction**; do not batch changes.

### Risk Mitigation
1. **Commit** after each successful extraction.
2. **Keep old code commented** until the new code is proven.
3. Have a **rollback plan** (`git reset --hard`).
4. **Ask for help** if an extraction seems too complex.
5. **Stop at the first sign of trouble**.

### Warning Signs to Stop
- Tests start failing.
- Clippy warnings appear.
- UI behavior changes.
- Performance degrades noticeably.
- Code becomes more complex, not simpler.

---

## MEASUREMENTS & VALIDATION

### Line Count Tracking
```bash
# Before changes:
find src-tauri/src -name "*.rs" -exec wc -l {} + | sort -nr

# Target after Plan A:
# processor.rs: ≤800 lines (from 1,455)
# New modules: ≤200 lines each
# progress_utils.rs: ~100 lines
```

### Function Length Tracking
```bash
# Use ripgrep to find long functions:
rg -A 50 "^pub.*fn.*\(" src-tauri/src/ | wc -l
# Manually count for functions 50 lines
```

### DRY Violation Tracking
```bash
# Check for remaining repeated patterns:
rg "TempDir::new\(\)\.unwrap\(\)" src-tauri/src/  # Should be 0 after Plan A
rg "progress.*calculation" src-tauri/src/  # Should be centralized
```

---

## NEXT STEPS

1. **Create a git branch** for Plan A work.
2. **Deploy parallel work streams**:
   - Stream 1: Test utilities (immediate, safe)
   - Stream 2: Progress consolidation (critical)
   - Stream 3: Parameter structs (quality of life)
3. **Validate after each extraction** with full test suite.

**For Junior Developer**:
- Start with Stream 1 (test utilities) - it's the safest and helps everything else
- Use the AI agent instructions template for each task
- Always validate with `cargo test` and `cargo clippy`
- Commit after each successful change

**Remember**: This is stabilization. The goal is to make the codebase safe for feature development, not perfect. Perfection is the goal of Plans B and C.

## APPENDIX: File System Test Artifacts

**Current Issue**: Some tests create real file system artifacts that aren't cleaned up.

**When to Fix**: Phase C (pre-beta)

**Why Wait**:
- Low risk during development
- Would complicate current refactoring
- Better addressed with comprehensive test overhaul

**Tracking**:
```bash
# To find tests creating artifacts:
rg "File::create|fs::write" src-tauri/src --type rust | grep -E "#\[cfg\(test\)\]" -A 10
```

**Future Fix** (Phase C):
- Use in-memory mocks where possible
- Ensure all TempDir instances are properly dropped
- Add test categories (unit vs integration)
- Set up proper test isolation