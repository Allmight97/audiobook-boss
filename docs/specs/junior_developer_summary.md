# Audiobook Boss - Junior Developer Summary

## 1. Languages & Frameworks

**Frontend:**
- **TypeScript** - Primary frontend language (`.ts` files in `src/`)
- **Vite** - Build tool and dev server (`vite.config.ts`)
- **HTML/CSS** - UI structure and styling
- **Tauri API** - Bridge between frontend and Rust backend

**Backend:**
- **Rust** - Core backend language (`src-tauri/`)
- **Tauri 2** - Desktop application framework
- **FFmpeg** - Audio processing (via command execution)
- **Cargo** - Rust package manager and build system

## 2. Overall Architecture

The application follows a typical Tauri architecture with clear separation between frontend and backend:

```
┌─────────────────────────────────────────────────────┐
│                    Frontend (UI)                     │
│                 TypeScript + Vite                    │
│  - src/ui/       (UI components)                    │
│  - src/types/    (TypeScript interfaces)            │
│  - src/main.ts   (Entry point)                      │
└──────────────────┬──────────────────────────────────┘
                   │ Tauri Commands (IPC)
                   │ via events.ts contract
┌──────────────────▼──────────────────────────────────┐
│              Backend (Rust + Tauri)                  │
│                                                      │
│  ┌─────────────────────────────────────────────┐   │
│  │         Commands Module (mod.rs)             │   │
│  │    - Entry point for all frontend calls      │   │
│  └─────────────────┬───────────────────────────┘   │
│                    │                                 │
│  ┌─────────────────▼───────────────────────────┐   │
│  │           Core Audio Modules                 │   │
│  │  - processor.rs (main processing logic)     │   │
│  │  - context.rs   (context management)        │   │
│  │  - cleanup.rs   (resource cleanup)          │   │
│  │  - progress.rs  (progress tracking)         │   │
│  └─────────────────┬───────────────────────────┘   │
│                    │                                 │
│  ┌─────────────────▼───────────────────────────┐   │
│  │         Support Modules                      │   │
│  │  - ffmpeg/    (FFmpeg command wrapper)      │   │
│  │  - metadata/  (Audio metadata handling)     │   │
│  │  - errors.rs  (Centralized error handling)  │   │
│  └─────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────┘
```

## 3. Design Patterns

**Currently Used:**

1. **Tauri Command Pattern** - Functions in `commands/mod.rs` are exposed to frontend
2. **Facade Pattern** - Well-implemented in `ffmpeg/` and `metadata/` modules:
   - Public interface in `mod.rs` hides internal complexity
   - Clean API boundaries for module consumers
3. **RAII (Resource Acquisition Is Initialization)** - Cleanup guards with Drop trait in `cleanup.rs`
4. **Centralized Error Handling** - All errors use `Result<T, AppError>` pattern
5. **Event-Driven Progress** - Multiple progress reporter implementations

**Missing Patterns (noted for future implementation):**
- Builder Pattern (for complex configurations)
- Strategy Pattern (for different audio processing approaches)
- Factory Pattern (for creating processor types)

## 4. Testing Strategy

**Current State:**
- ✅ **130+ tests passing** - Comprehensive test coverage
- ✅ **CI/CD validation** - Tests run automatically
- ✅ **Integration tests** - Located in `tests_integration.rs`
- ⚠️ **Test setup duplication** - Repeated patterns need extraction
- ⚠️ **Test artifacts** - Some tests create actual filesystem artifacts

**Test Commands (run from `src-tauri/`):**
- `cargo test` - Run all tests
- `cargo clippy -- -D warnings` - Lint code (must pass with zero warnings)

## 5. Code Audit & Onboarding Guide

### Code Smells

**Major Issues to Be Aware Of:**

1. **God Objects** - `processor.rs` is a massive 1,455-line file handling too many responsibilities
2. **Long Parameter Lists** - Some functions have 5-7 parameters (e.g., `process_progress_update_context()`)
3. **Complex Conditional Logic** - Nested if/else chains, especially in progress tracking
4. **Shotgun Surgery Risk** - Changes to progress calculation require updates across multiple modules
5. **DRY Violations** - Significant code duplication in:
   - Progress tracking logic
   - Error handling patterns
   - Temporary directory management
   - Test setup code

### Modules to Watch

**Critical Violations (>400 lines):**
| Module | Lines | Issue |
|--------|-------|-------|
| `processor.rs` | **1,455** | Massive monolith with 60+ functions |
| `cleanup.rs` | **946** | RAII guards with heavy duplication |
| `context.rs` | **804** | Complex context builders |
| `progress.rs` | **485** | Progress tracking with parsing logic |
| `commands/mod.rs` | **438** | All Tauri commands in one file |

### Functions to Watch

**Functions exceeding the 50-line limit:**
- `process_audiobook_with_context()` - ~70-80 lines
- `execute_with_progress_context()` - ~60-70 lines
- `merge_audio_files_with_context()` - ~50-60 lines
- `process_progress_update_context()` - ~50+ lines

**Note:** The codebase claims "functions ≤30 lines" but this is FALSE - many functions are 50-100+ lines.

### Key "Gotchas"

**⚠️ Critical Warnings for Feature Development:**

1. **Don't Trust Function Length Claims** - Documentation says ≤30 lines but reality is very different
2. **Module Boundaries Unclear** - Large modules make it hard to predict change impacts
3. **Progress Logic Fragility** - Multiple overlapping progress implementations can break easily
4. **Sample Rate Detection** - Logic is scattered across multiple functions
5. **Error Propagation** - Complex error handling chains in large functions
6. **Cleanup Dependencies** - RAII guards have complex dependency chains
7. **Progress Event Timing** - Multiple overlapping progress reporting mechanisms
8. **Test Dependencies** - Some tests create real filesystem artifacts

**Safe Development Approach:**
1. Start with well-structured modules (`ffmpeg/`, `metadata/`) as examples
2. Run `cargo test` after EVERY small change
3. Follow the facade pattern when creating new modules
4. ALWAYS use `Result<T, AppError>`, NEVER `unwrap()` in production code
5. Check the refactoring plans before making major changes:
   - Plan A: Emergency Stabilization
   - Plan B: Systematic Module Splitting
   - Plan C: Quality Enhancement

**Important:** The codebase is in active refactoring phase. Always check `docs/planning/README.md` for current refactoring status before making changes.
===
## Tauri Version Confirmation

Yes, this application uses **Tauri 2**. This is confirmed by:
- `Cargo.toml`: `tauri = { version = "2", features = [] }`
- `package.json`: `"@tauri-apps/api": "^2"`
- Configuration uses Tauri 2 schema

## Architecture Analysis

### Current Architecture Strengths:
1. **Clean separation** between TypeScript frontend and Rust backend
2. **Well-implemented facade pattern** in `ffmpeg/` and `metadata/` modules
3. **Centralized error handling** with `Result<T, AppError>`
4. **RAII pattern** for resource cleanup
5. **Minimal frontend** - No heavy frameworks, just TypeScript + Vite + Tauri APIs

### Major Issues:
1. **Massive god objects** - `processor.rs` has 1,455 lines with 60+ functions
2. **Function bloat** - Many functions are 50-100+ lines (despite documentation claiming ≤30 lines)
3. **Module violations** - 5 modules exceed the 400-line limit
4. **Significant code duplication** across modules

### Is the Tech Stack Common?

The frontend is **unusually minimal** compared to typical modern apps - and that's actually **good**! Most apps use heavy frameworks (React/Vue), state management, UI libraries, etc. This app's vanilla TypeScript + Vite approach is refreshingly efficient.

The Rust + Tauri backend is becoming increasingly popular for desktop apps due to better performance and lower memory usage than Electron.

### Architecture Improvements:

1. **Split the god object** (`processor.rs`) into a pipeline pattern with stages:
   - Validation, Preparation, Conversion, Merging, Finalization
   - Each stage ~200 lines with single responsibility

2. **Implement design patterns**:
   - **Pipeline Pattern** for audio processing stages
   - **Builder Pattern** for complex configurations
   - **Repository Pattern** for file operations
   - **Observer Pattern** for progress tracking
   - **Strategy Pattern** for different audio formats

3. **Better module organization**:
   ```
   audio/
   ├── pipeline/
   │   ├── stages/
   │   └── orchestrator.rs
   ├── services/
   │   ├── processor_service.rs
   │   └── metadata_service.rs
   └── repositories/
       └── audio_file_repo.rs
   ```

4. **Keep the minimal frontend** - it's a strength, not a weakness!

The architecture is conceptually solid but needs the systematic refactoring you've planned (Plans A→B→C) to address the implementation issues. The minimal tech stack is actually more efficient than most modern applications.