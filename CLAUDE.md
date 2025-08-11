# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---
alwaysApply: true
---

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
## Project Structure & Module Organization
- `src/`: Frontend (Vite + TypeScript). UI modules in `src/ui/`, shared types in `src/types/`, assets in `src/assets/`.
- `src-tauri/`: Rust backend for Tauri. Code in `src-tauri/src/` with domains like `audio/`, `metadata/`, `ffmpeg/`, `commands/`.
- `docs/`, `media/`: Documentation and media assets. `dist/`: Vite build output.
- `src-tauri/binaries/`: FFmpeg bundling helpers. `scripts/`: project scripts.

## Build, Test, and Development Commands
- `npm run tauri dev`: Start the full desktop app (frontend + Rust).
- `RUST_LOG=debug npm run tauri dev`: Full dev mode with verbose logging.
- `npm run dev`: Frontend dev server only (port 1420).
- `npm run build`: Type-check and build frontend to `dist/`.
- `npm run build-macos` / `npm run package-macos`: Prepare FFmpeg and build/package macOS app.
- `npm run setup-ffmpeg`: Download/bundle FFmpeg for macOS.

### Testing Commands (from `src-tauri/`)
- `cargo test`: All tests (unit + integration).
- `cargo test --features safe-ffmpeg`: Test new FFmpeg-Next engine.
- `cargo clippy -- -D warnings`: Lint checks (must pass).
- `cargo clippy --features safe-ffmpeg -- -D warnings`: Check feature-gated code.
- `cargo test path_validation`: Run path security tests.
- **Clippy verification**: Both standard and safe-ffmpeg feature clippy must pass - IF THEY DON'T report to the user after a single round of root cause 5-whys analysis.

## Commit & Pull Request Guidelines
- Commits: concise, imperative, and scoped (e.g., "Refactor FFmpeg path handling"). Emojis are fine; keep subject ≤ 72 chars.
- PRs: include a clear summary, linked issues, steps to test, and screenshots/GIFs for UI changes.
- Checks: `cargo test` passes, `npm run build` succeeds, and app runs via `npm run tauri dev`. Avoid committing large binaries—use `src-tauri/binaries/` scripts.

## Security & Configuration Tips
- Logging: set `RUST_LOG` (e.g., `RUST_LOG=info npm run tauri dev`).
- FFmpeg: prefer `npm run setup-ffmpeg`; the bundled binary is referenced in `tauri.conf.json` (`bundle.externalBin`).

### Security & Validation Requirements
- **All Inputs**: Must pass through `validate_input_audio_path()` which canonicalizes paths, checks extensions, and prevents directory traversal
- **Symlinks**: Accepted but resolved with warning logs
- **Output Directories**: Probed for write permissions before processing
- **File Extensions**: Validated against `ALLOWED_AUDIO_EXTENSIONS` whitelist

### Feature Flag Conventions
```rust
#[cfg(any(test, feature = "safe-ffmpeg"))]  // New engine components
#[cfg(not(feature = "safe-ffmpeg"))]        // Legacy shell-based code
#[cfg(feature = "safe-ffmpeg")]             // FFmpeg-Next exclusive
```

### Migration Context
The codebase is actively migrating from shell FFmpeg to ffmpeg-next. When working on audio processing:
- Prefer `FfmpegNextProcessor` for new features (when safe-ffmpeg enabled)
- Keep `ShellFFmpegProcessor` working as fallback
- Test both engines: `cargo test` and `cargo test --features safe-ffmpeg`

## Testing Guidelines
- Rust: place external tests under `src-tauri/tests/unit/**` by domain (e.g., `audio/`, `commands/`, `ffmpeg/`, `metadata/`). Use inline tests inside modules only when required to test non-`pub` internals (including `pub(crate)`, `pub(super)`, `pub(in ...)`). Run with `cargo test` in `src-tauri/`.
- Frontend: no formal test runner yet; use `window.testCommands` in `src/main.ts` for manual validation paths.
- Coverage focus: `audio` pipeline, Tauri command handlers, and metadata read/write; name tests by behavior and module.
- Tests: prefer external tests in `src-tauri/tests/unit/**` for public APIs/behavior; use inline tests only when needed to cover private/internal items that aren’t accessible externally.
  - Use inline tests only when testing items that are:
  - Private (`fn` without `pub`)
  - Module-scoped (`pub(crate)`, `pub(super)`, `pub(in path)`) or
  - Cannot be meaningfully tested through public APIs

## Coding Style & Naming Conventions
- TypeScript: strict mode; prefer explicit types, avoid `any`. Module files follow kebab-case (e.g., `file-list.ts`), types/interfaces PascalCase.
- Rust: follow Rust conventions (snake_case modules, CamelCase types). Lints: `#![deny(clippy::unwrap_used)]` and `#![warn(clippy::too_many_lines)]` are enforced—avoid `unwrap`; use `Result` and `?`.
- **Strategic allows**: Use `#[cfg_attr(not(any(test, feature = "safe-ffmpeg")), allow(unused_imports))]` for feature-gated infrastructure vs broad `#[allow()]`.
- Formatting: use default rustfmt; for TS, rely on `tsc` + Vite. Keep functions small and focused.
- Visibility: keep private/internal items non-`pub` unless required by cross-module use.

### Error Handling Patterns
- **Banned**: `unwrap()` and `expect()` in production code (`#![deny(clippy::unwrap_used)]`)
- **Required**: Return `Result<T, AppError>` and use `?` operator
- **Custom Errors**: Use `AppError` enum from `src-tauri/src/errors.rs`

### Frontend Patterns (TypeScript)
- **Class-Based UI Components**: Each UI module (`StatusPanel`, `FileList`) uses classes with private state and DOM element caching
- **Global State via Modules**: Use module-level variables (e.g., `currentFileList`) for shared state between components
- **Event-Driven Architecture**: Components communicate via `listen()` for Tauri events and custom callbacks (e.g., `onFileListChange`)
- **DOM Element Caching**: Cache frequently-accessed elements in constructor/init (e.g., `this.progressBar!: HTMLElement`)
- **Type Safety**: Define interfaces for all data structures (`ProgressEvent`, `ProcessingStatus`) that cross Rust/TS boundary

## Code Size & Modularity Standard
Principles:
- Uphold Single Responsibility, high cohesion, and orthogonality (independent components).
- Prefer low cyclomatic/cognitive complexity via simple control flow and early returns.
- Do Not Repeat Yourself (DRY)

Hard limits:
- Module/file: ≤ 400 lines of code (exclude comments/blank).
- Function/method: ≤ 55 lines of code (exclude comments/blank).
- Parameters ≤ 7 for single-purpose functions.

Agent behavior:
- During generation: structure code to meet limits; extract helpers without harming cohesion.
  - Avoid extracting helpers when it would:
    - Require passing >3 parameters between functions
    - Split logically atomic operations (e.g., validate-then-act patterns)
    - Create circular dependencies between modules
- During review: flag any violation and propose concrete refactors (what to extract, names, inputs/outputs, test seams).
- Only exceed limits for justified generated/protocol glue; annotate with a comment with `// EXCEPTION:` and create a refactor ticket
  - Exceeding limits allowed only for:
    - Generated code (proc macros, build scripts)
    - Protocol implementations (Tauri command handlers, serialization)
    - Third-party interface adapters (FFmpeg bindings, external APIs)

Checklist before returning code:
- [ ] Each function ≤ 55 LOC, single-purpose, ≤ 7 Parameters
- [ ] File ≤ 400 LOC
- [ ] Complexity is low; boundaries clean; helpers testable
- [ ] DRY - Avoid Code duplication
- [ ] Any exception annotated + ticketed

Maintain a collaborative pair programmer role when planning and executing/implementing. And keep in mind user is a junior dev and may have limited ability to address questions, but will certainly do their best.
Validate (with user) code changes and implementation plans before executing.