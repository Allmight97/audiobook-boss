# Copilot Instructions for Audiobook Boss

## Architecture Overview
This is a Tauri desktop app with TypeScript frontend and Rust backend that converts audio files into audiobooks. The app is currently migrating from shell-based FFmpeg to type-safe `ffmpeg-next` bindings via feature flags.

### Key Architectural Patterns
- **Dual Processing Engines**: `ShellFFmpegProcessor` (default) vs `FfmpegNextProcessor` (feature = "safe-ffmpeg")
- **MediaProcessor Trait**: Unified interface (`MediaProcessingPlan` → `execute()`) abstracts FFmpeg execution
- **Feature-Gated Migration**: Use `#[cfg(any(test, feature = "safe-ffmpeg"))]` for new engine components
- **Path Security**: All input paths go through `audio::path_validation::validate_input_audio_path()`

### Critical Data Flows
1. **File Import**: UI drag/drop → `analyze_audio_files` → `audio::file_list::get_file_list_info`
2. **Processing Pipeline**: `process_audiobook_files` → `MediaProcessor::execute` → progress events via Tauri window
3. **Metadata Flow**: Lofty for reading → custom `AudiobookMetadata` → Lofty for writing

## Development Workflows

### Testing Commands (from `src-tauri/`)
```bash
cargo test                           # All tests (unit + integration)  
cargo test --features safe-ffmpeg   # Test new FFmpeg-Next engine
cargo clippy -- -D warnings         # Lint checks (must pass)
cargo test path_validation           # Run path security tests
```

### Build Commands
```bash
npm run tauri dev                    # Full dev mode (port 1420)
RUST_LOG=debug npm run tauri dev     # With verbose logging
npm run setup-ffmpeg                # Bundle FFmpeg for packaging
cargo clippy --features safe-ffmpeg # Check feature-gated code
```

## Project-Specific Patterns

### Module Size Enforcement
- **Hard Limits**: Files ≤400 lines, functions ≤60 lines
- **When Violated**: Extract helpers, but avoid >3 parameters or circular deps
- **Check Compliance**: Use `scripts/sg/size_budget.sh`

# Coding Style & Naming Conventions
- TypeScript: strict mode; prefer explicit types, avoid `any`. Files typically camelCase (e.g., `fileList.ts`), types/interfaces PascalCase.
- Rust: follow Rust conventions (snake_case modules, CamelCase types). Lints: `#![deny(clippy::unwrap_used)]` and `#![warn(clippy::too_many_lines)]` are enforced—avoid `unwrap`; use `Result` and `?`.
- Formatting: use default rustfmt; for TS, rely on `tsc` + Vite. Keep functions small and focused.
- Visibility: keep private/internal items non-`pub` unless required by cross-module use.

### Test Organization
- **External Tests**: `src-tauri/tests/unit/{audio,metadata,ffmpeg,commands}/` for public APIs
- **Inline Tests**: Only for private/`pub(crate)` internals that can't be tested externally
- **Integration Tests**: `src-tauri/tests/*.rs` for end-to-end workflows
- **Frontend Testing**: Currently manual via `window.testCommands` in `main.ts` - consider adding automated tests for UI state management and Tauri command integration

### Error Handling Patterns
- **Banned**: `unwrap()` and `expect()` in production code (`#![deny(clippy::unwrap_used)]`)
- **Required**: Return `Result<T, AppError>` and use `?` operator
- **Custom Errors**: Use `AppError` enum from `src-tauri/src/errors.rs`

### Feature Flag Conventions
```rust
#[cfg(any(test, feature = "safe-ffmpeg"))]  // New engine components
#[cfg(not(feature = "safe-ffmpeg"))]        // Legacy shell-based code
#[cfg(feature = "safe-ffmpeg")]             // FFmpeg-Next exclusive
```

### Frontend Patterns (TypeScript)
- **Class-Based UI Components**: Each UI module (`StatusPanel`, `FileList`) uses classes with private state and DOM element caching
- **Global State via Modules**: Use module-level variables (e.g., `currentFileList`) for shared state between components
- **Event-Driven Architecture**: Components communicate via `listen()` for Tauri events and custom callbacks (e.g., `onFileListChange`)
- **DOM Element Caching**: Cache frequently-accessed elements in constructor/init (e.g., `this.progressBar!: HTMLElement`)
- **Type Safety**: Define interfaces for all data structures (`ProgressEvent`, `ProcessingStatus`) that cross Rust/TS boundary

## Critical Integration Points

### Tauri Commands (`src-tauri/src/commands/mod.rs`)
- All user actions flow through `#[tauri::command]` handlers
- Always validate inputs via path validation before processing
- Use `ProcessingState` for cancellation and progress tracking

### Progress System
- **Legacy**: `progress_monitor.rs` with shell FFmpeg parsing
- **New**: Direct progress calculation from ffmpeg-next PTS/duration
- **Events**: Emit via `window.emit("processing-progress", event)`

### FFmpeg Path Discovery (`src-tauri/src/ffmpeg/mod.rs`)
1. Bundled binary (`src-tauri/binaries/ffmpeg-universal`)
2. Development symlink to Homebrew
3. System PATH lookup
4. macOS-specific locations (`/opt/homebrew/bin`, `/usr/local/bin`)

## Security & Validation Requirements
- **All Inputs**: Must pass through `validate_input_audio_path()` which canonicalizes paths, checks extensions, and prevents directory traversal
- **Symlinks**: Accepted but resolved with warning logs
- **Output Directories**: Probed for write permissions before processing
- **File Extensions**: Validated against `ALLOWED_AUDIO_EXTENSIONS` whitelist

## Migration Context
The codebase is actively migrating from shell FFmpeg to ffmpeg-next. When working on audio processing:
- Prefer `FfmpegNextProcessor` for new features (when safe-ffmpeg enabled)
- Keep `ShellFFmpegProcessor` working as fallback
- Test both engines: `cargo test` and `cargo test --features safe-ffmpeg`
- Follow P0/P1/P2 priority tasks from `docs/planning/consolidated-roadmap.md`
```
Always apply [general coding guidelines](instructions/coding-guidelines.instructions.md)