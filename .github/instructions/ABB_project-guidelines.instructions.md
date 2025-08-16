---
applyTo: '**'
---
# Audiobook Boss (ABB) Project Guidelines

## Architecture Overview
This is a Tauri desktop app with TypeScript frontend and Rust backend that converts audio files into audiobooks. After the P4.3/P4.4 "nuclear" transition the codebase runs exclusively on the type-safe `ffmpeg-next` engine (all shell-based FFmpeg and feature flags removed).

### Key Architectural Patterns
- **Single Processing Engine**: `FfmpegNextProcessor` implements `MediaProcessor`
- **Media Abstraction**: `MediaProcessingPlan` → `execute()` keeps future extensibility
- **No Feature Flags**: All former `legacy-adapters` / `safe-ffmpeg` conditionals purged
- **Path Security**: All input paths go through `audio::path_validation::validate_input_audio_path()`

### Critical Data Flows
1. **File Import**: UI drag/drop → `analyze_audio_files` → `audio::file_list::get_file_list_info`
2. **Processing Pipeline**: `process_audiobook_files` → `MediaProcessor::execute` → progress events via Tauri window
3. **Metadata Flow**: Lofty for reading → custom `AudiobookMetadata` → Lofty for writing

## Development Workflows

### Testing Commands (from `src-tauri/`)
```bash
cargo test                 # All tests (unit + integration)
cargo clippy -- -D warnings
cargo test path_validation # Run path security tests subset
```

### Build Commands
```bash
npm run tauri dev                # Full dev mode (port 1420)
RUST_LOG=debug npm run tauri dev # With verbose logging
```

### Coding Style & Naming Conventions
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

### Project-Specific Coding Exceptions
When applying cross-project coding standards, these audiobook-boss specific exceptions are allowed:
- **Tauri command handlers**: May exceed function size limits for complex command orchestration
- **FFmpeg integration code**: Bindings and adapters may exceed limits due to external API constraints
- **Generated protocol code**: Serialization and IPC boundaries may have larger functions

### Collaboration Context
- Maintain a collaborative pair programmer approach when planning and implementing
- User is a junior developer - validate code changes and implementation plans before executing
- Provide clear explanations and rationale for architectural decisions

### Feature Flags
Feature flags related to engine selection have been removed. Any new feature gating should be unrelated to the core processing engine and documented explicitly.

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
- Direct progress calculation from ffmpeg-next PTS/duration (legacy parser removed)
- Events emitted via `window.emit("processing-progress", event)`

### FFmpeg Runtime
The application links to system FFmpeg libraries via `ffmpeg-next`; no bundled binary discovery logic remains.

## Security & Validation Requirements
- **All Inputs**: Must pass through `validate_input_audio_path()` which canonicalizes paths, checks extensions, and prevents directory traversal
- **Symlinks**: Accepted but resolved with warning logs
- **Output Directories**: Probed for write permissions before processing
- **File Extensions**: Validated against `ALLOWED_AUDIO_EXTENSIONS` whitelist

## Post-Migration Context
The migration to ffmpeg-next is complete. Follow-up enhancements (native cover art embedding, AAC encoder option tuning, metadata enrichment) should target the single engine path without reintroducing shell execution.