# PLAN B: Systematic Module Splitting Plan

_Prerequisites: Plan A (Emergency Stabilization) must be completed first_  
_Previous: `docs/planning/plan_a_emergency_stabilization.md`_  
_Next: `docs/planning/plan_c_quality_enhancement.md`_  
_Timeline: 3-4 weeks after Plan A completion_  
_Complexity: MEDIUM - following established patterns_

## Prerequisites Check

Before starting Plan B, verify Plan A success:
- ✅ `processor.rs` reduced to ≤800 lines
- ✅ Progress tracking centralized in utilities  
- ✅ All 130+ tests still passing
- ✅ Zero clippy warnings
- ✅ UI functionality preserved

## Plan B Overview

**Goal**: Split remaining oversized modules using proven facade pattern from Plan A

**Modules to Split**:
1. `cleanup.rs` (946 lines) → 3 sub-modules
2. `context.rs` (804 lines) → 3 sub-modules  
3. `progress.rs` (485 lines) → 2 sub-modules
4. `commands/mod.rs` (438 lines) → 3 sub-modules

**Key Difference from Plan A**: These modules are more self-contained and have fewer DRY violations, making them safer to split.

---

## Phase B1: cleanup.rs Splitting (Week 1)

### B1.1: Analysis - cleanup.rs Structure
**Current**: 946 lines of RAII cleanup guards

**Target Sub-modules**:
```
src-tauri/src/audio/cleanup/
├── guards.rs        # CleanupGuard struct (~300 lines)
├── processes.rs     # ProcessGuard struct (~250 lines)  
├── strategies.rs    # Cleanup algorithms (~200 lines)
└── mod.rs          # Public facade (~50 lines)
```

### B1.2: Extraction Strategy
**Step 1**: Create directory structure
```bash
cd src-tauri/src/audio/
mkdir cleanup_new
cd cleanup_new
touch guards.rs processes.rs strategies.rs mod.rs
```

**Step 2**: Move CleanupGuard implementation
```rust
// NEW: guards.rs
pub struct CleanupGuard {
    paths: HashSet<PathBuf>,
    session_id: String,
    enabled: bool,
}

impl CleanupGuard {
    // Move all CleanupGuard methods here
}

impl Drop for CleanupGuard {
    // Move Drop implementation
}
```

**Step 3**: Move ProcessGuard implementation  
```rust
// NEW: processes.rs
pub struct ProcessGuard {
    // Move ProcessGuard implementation
}
```

**Step 4**: Create facade
```rust
// NEW: mod.rs
pub use guards::CleanupGuard;
pub use processes::ProcessGuard;
// Re-export minimal public API
```

**Step 5**: Update imports throughout codebase
```bash
# Find all uses of cleanup types:
rg "use.*cleanup" src-tauri/src/
# Update imports to use new facade
```

### B1.3: Validation
```bash
cargo test --lib
cargo clippy -- -D warnings
# Check that temp directory cleanup still works correctly
```

---

## Phase B2: context.rs Splitting (Week 2)

### B2.1: Analysis - context.rs Structure  
**Current**: 804 lines of context builders

**Target Sub-modules**:
```
src-tauri/src/audio/context/
├── processing.rs    # ProcessingContext + builder (~300 lines)
├── progress.rs      # ProgressContext + builder (~250 lines)
├── types.rs         # Shared types and constants (~150 lines)
└── mod.rs          # Public facade (~50 lines)
```

### B2.2: Extraction Strategy
**Lower Risk**: Context objects are mostly data structures with builders

**Step 1**: Move ProcessingContext
```rust
// NEW: processing.rs
pub struct ProcessingContext {
    // Move struct definition and impl
}

pub struct ProcessingContextBuilder {
    // Move builder pattern implementation
}
```

**Step 2**: Move ProgressContext
```rust  
// NEW: progress.rs
pub struct ProgressContext {
    // Move struct definition and impl
}

pub struct ProgressContextBuilder {
    // Move builder pattern implementation  
}
```

**Step 3**: Extract shared types
```rust
// NEW: types.rs
pub enum ContextError {
    // Move context-specific error types
}

// Move any shared constants or utilities
```

### B2.3: Low-Risk Validation
Context splitting is lower risk because contexts are mainly data containers.

---

## Phase B3: progress.rs Splitting (Week 3)

### B3.1: Analysis - progress.rs Structure
**Current**: 485 lines of progress parsing and emission

**Target Sub-modules**:
```
src-tauri/src/audio/progress/
├── parsing.rs       # FFmpeg output parsing (~250 lines)
├── emission.rs      # Progress event emission (~200 lines)  
└── mod.rs          # Public facade (~50 lines)
```

### B3.2: Extraction Strategy
**Higher Risk**: Progress logic is critical for UI updates

**Step 1**: Move parsing functions
```rust
// NEW: parsing.rs  
pub fn parse_ffmpeg_time(time_str: &str) -> Result<f64>
pub fn parse_ffmpeg_progress(line: &str) -> Result<f32>
pub fn parse_speed_multiplier(line: &str) -> Option<f64>
```

**Step 2**: Move emission logic
```rust
// NEW: emission.rs
pub struct ProgressEmitter {
    // Move event emission implementation
}

impl ProgressEmitter {
    pub fn emit_analyzing_progress(...)
    pub fn emit_converting_progress(...)
    // Move all emit_* methods
}
```

### B3.3: Critical Validation
Progress changes risk breaking UI updates:
```bash
cargo test --lib
npm run tauri dev
# Manually test: load files, verify progress bar updates correctly
# Check that ETA calculations still work
```

---

## Phase B4: commands/mod.rs Splitting (Week 4)

### B4.1: Analysis - commands/mod.rs Structure
**Current**: 438 lines of all Tauri commands in one file

**Target Sub-modules**:
```
src-tauri/src/commands/
├── audio.rs         # Audio processing commands (~150 lines)
├── metadata.rs      # Metadata commands (~150 lines)
├── files.rs         # File management commands (~100 lines)
└── mod.rs          # Re-exports all commands (~50 lines)
```

### B4.2: Extraction Strategy
**Lowest Risk**: Commands are independent functions

**Step 1**: Group by functionality
```rust
// NEW: audio.rs
#[tauri::command]
pub fn merge_audio_files(...) -> Result<String>

#[tauri::command]  
pub fn analyze_audio_files(...) -> Result<FileListInfo>

#[tauri::command]
pub fn process_audiobook_files(...) -> Result<String>
```

**Step 2**: Metadata commands
```rust
// NEW: metadata.rs
#[tauri::command]
pub fn read_audio_metadata(...) -> Result<AudiobookMetadata>

#[tauri::command]
pub fn write_audio_metadata(...) -> Result<()>

#[tauri::command]
pub fn write_cover_art(...) -> Result<()>
```

**Step 3**: File management
```rust
// NEW: files.rs
#[tauri::command]
pub fn validate_files(...) -> Result<String>

#[tauri::command]
pub fn validate_audio_settings(...) -> Result<String>
```

**Step 4**: Update Tauri registration
```rust
// UPDATE: src-tauri/src/lib.rs
.invoke_handler(tauri::generate_handler![
    commands::audio::merge_audio_files,
    commands::metadata::read_audio_metadata,
    commands::files::validate_files,
    // Update all command paths
])
```

### B4.3: Frontend Integration Check
Commands splitting could break frontend calls:
```bash
npm run tauri dev
# Test all UI functions that call backend commands
# Verify no command registration errors in console
```

---

## Remaining DRY Violations (Address During Plan B)

### Moderate DRY Violations to Fix

**1. Temp Directory Management** (during cleanup.rs split)
- Extract common temp directory creation patterns
- Consolidate cleanup retry logic
- Standardize error handling for directory operations

**2. File Validation Patterns** (during commands split)  
- Extract common file existence checking
- Consolidate audio file validation logic
- Standardize error messages for file operations

**3. Command Parameter Validation** (during commands split)
- Extract common parameter checking patterns
- Consolidate input sanitization logic
- Standardize validation error responses

### Implementation During Splitting
```rust
// NEW: src-tauri/src/utils/mod.rs (created during Plan B)
pub mod file_utils;
pub mod validation_utils;
pub mod temp_utils;

// file_utils.rs
pub fn validate_audio_file_exists(path: &Path) -> Result<()> {
    // Centralized file validation logic
}

// validation_utils.rs  
pub fn validate_non_empty_string(value: &str, field_name: &str) -> Result<()> {
    // Centralized string validation
}

// temp_utils.rs
pub fn create_session_temp_dir(session_id: &str) -> Result<PathBuf> {
    // Centralized temp directory creation
}
```

---

## Success Criteria for Plan B

### ✅ Must Pass Before Plan C
- [ ] All 4 modules split successfully
- [ ] All modules ≤400 lines (target: ≤300 lines)
- [ ] Facade pattern consistently applied
- [ ] Moderate DRY violations eliminated
- [ ] All 130+ tests still passing
- [ ] Zero clippy warnings
- [ ] UI functionality unchanged
- [ ] No performance regressions

### 📊 Line Count Targets
| Module            | Before Plan B | After Plan B      | Sub-modules         |
| ----------------- | ------------- | ----------------- | ------------------- |
| `cleanup.rs`      | 946 lines     | ≤300 lines facade | 3 modules ≤300 each |
| `context.rs`      | 804 lines     | ≤300 lines facade | 3 modules ≤300 each |
| `progress.rs`     | 485 lines     | ≤200 lines facade | 2 modules ≤250 each |
| `commands/mod.rs` | 438 lines     | ≤100 lines facade | 3 modules ≤150 each |

---

## Risk Management

### Lower Risk Modules (start here)
1. **commands/mod.rs** - Independent functions, easy to split
2. **context.rs** - Mostly data structures, low coupling
3. **cleanup.rs** - RAII patterns, well-encapsulated

### Higher Risk Module (do last)  
4. **progress.rs** - Critical for UI updates, test thoroughly

### Rollback Strategy
- Commit after each successful module split
- Keep old files as `.bak` until validation complete
- Have working UI test procedure for each change

### Junior Developer Safety
- Split one module completely before starting the next
- Test UI functionality after each module split
- Ask for help if module seems more complex than expected
- Stop immediately if tests start failing

---

## Preparation for Plan C

After Plan B completion, the codebase will be ready for Plan C:
- All major modules will be properly sized
- Facade pattern will be consistently applied
- Major DRY violations will be resolved
- Foundation set for final quality improvements

**Next**: Plan C Quality Enhancement (`docs/planning/plan_c_quality_enhancement.md`) for final polish phase. 