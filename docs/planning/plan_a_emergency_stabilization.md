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

#### A1.2: Test Setup Utilities
**Problem**: The `TempDir::new().unwrap()` pattern is repeated 15+ times in tests.

**Approach**:
```rust
// NEW: src-tauri/src/test_utils.rs
pub fn create_test_audio_file(content: &[u8]) -> (TempDir, PathBuf) {
    // Centralized test file creation
}

pub fn create_test_metadata() -> AudiobookMetadata {
    // Standard test metadata
}
```

**Priority**: P3 (Low) - Test-only changes.

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

## SUCCESS CRITERIA FOR PLAN A

### Must Be Met Before Starting Plan B
- [ ] `processor.rs` reduced to ≤800 lines (from 1,455).
- [ ] Progress tracking logic is centralized (no DRY violations).
- [ ] All 130+ tests are still passing.
- [ ] Zero clippy warnings.
- [ ] UI functionality is unchanged.
- [ ] No performance regressions.

### What NOT to Do in Plan A
- **Do not touch** `cleanup.rs` or `context.rs` (reserved for Plan B).
- **Do not refactor** for function length violations yet.
- **Do not optimize**—focus on extraction only.
- **Do not change** public APIs; preserve all existing interfaces.

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
# Manually count for functions >30 lines
```

### DRY Violation Tracking
```bash
# Check for remaining repeated patterns:
rg "TempDir::new\(\)\.unwrap\(\)" src-tauri/src/  # Should be 0 after Plan A
rg "progress.*calculation" src-tauri/src/  # Should be centralized
```

---

## NEXT STEPS

1. **Review this corrected plan** with any questions.
2. **Start with A1.1** (progress tracking consolidation).
3. **Create a git branch** for Plan A work.

**Remember**: This is stabilization. The goal is to make the codebase safe for feature development, not perfect. Perfection is the goal of Plans B and C. 