# Audiobook Boss — Repository Guidance

## Overview
- Backend: Rust with `ffmpeg-next` for all audio processing (decode → resample → encode → mux)
- Frontend: TypeScript (vanilla) + Tauri 2
- Metadata: Lofty for MP4/M4B tag writes and cover art fallback
- Audio Processing Engine: Single engine (`FfmpegNextProcessor`); no shell-based FFmpeg and no feature flags (see note end of this doc for updates to audio processing pipeline)

Internal docs:
- `docs/external-apis/ffmpeg-next.md` — audio/PTS/time_base, encoder, progress
- `docs/external-apis/lofty.md` — MP4/M4B tags, cover art, atomic write options
- `docs/external-apis/tauri-patterns.md` — event lifecycle & IPC patterns
- `docs/external-apis/path-handling.md` — macOS-focused path validation and atomic moves

## Architecture & Key Patterns
- Single Processing Engine: `FfmpegNextProcessor` implements `MediaProcessor`
- Media Abstraction: `MediaProcessingPlan` → `execute()`
- Path Security: All input paths must pass `audio::path_validation::validate_input_audio_path()` (canonicalizes, checks whitelist, resolves symlinks with warnings)
- Progress System: Based on ffmpeg-next timestamps; UI updates via a single Tauri event channel

## Critical Data Flows
1. File Import: UI drag/drop → `analyze_audio_files` → `audio::file_list::get_file_list_info`
2. Processing Pipeline: `process_audiobook_files` (or v2) → `MediaProcessor::execute` → progress events via Tauri window
3. Metadata Flow: Lofty read → custom `AudiobookMetadata` → Lofty write (and native embedding when available)

## Commands & Integration Points
- Tauri Commands module: `src-tauri/src/commands/`
  - `validate_files`, `analyze_audio_files`, `validate_audio_settings`, `process_audiobook_files`, `process_audiobook_files_v2`, `cancel_processing`, plus metadata read/write commands
- Processing Runtime
  - Engine selection is trivial: `FfmpegNextProcessor` only (see `audio/processor/selection.rs`)
  - ffmpeg-next initialized once per process (`ff::init()`)
- Progress Emission
  - Backend: `audio/progress/reporter.rs` emits via `window.emit("processing-progress", event)`
  - Frontend: listen to `processing-progress` (see `src/ui/statusPanel/logic.ts` and `src/types/events.ts`)

## Development Workflows
### Testing (run from `src-tauri/`)
```bash
cargo test                        # All tests (unit + integration)
cargo clippy -- -D warnings       # Lint checks (must pass)
cargo test path_validation        # Path security subset by name filter
```

### Build & Run
```bash
npm run tauri dev                 # Full dev mode (port 1420)
RUST_LOG=debug npm run tauri dev  # With verbose logging
```

### Logging (Rust)
Configure via `RUST_LOG` environment variable. Examples:
```bash
RUST_LOG=debug npm run tauri dev
RUST_LOG=audiobook_boss=debug npm run tauri dev
RUST_LOG=warn,audiobook_boss=debug npm run tauri dev
```

## Coding Standards
- TypeScript: strict mode; explicit types; avoid `any`; camelCase filenames; PascalCase types
- Rust: idiomatic; `#![deny(clippy::unwrap_used)]`, `#![warn(clippy::too_many_lines)]`; use `Result<T, AppError>` and `?`
- Formatting: `rustfmt` defaults; TS via `tsc`/Vite; keep functions small and focused
- Visibility: keep internals non-`pub` unless cross-module use requires it

### Repository-Specific Expectations
- Single responsibility, high cohesion, guard clauses, DRY
- Exceptions allowed for:
  - Tauri command handlers (orchestration)
  - FFmpeg integration bindings/adapters
  - Generated protocol code
  Add `// EXCEPTION: [reason]` and consider follow-up refactor when exceeded.

### Test Organization
- External unit tests: `src-tauri/tests/unit/{audio,metadata,commands}/`
- Integration tests: `src-tauri/tests/*.rs`
- Inline tests: only for private/`pub(crate)` internals not testable externally
- Frontend: manual testing via `window.testCommands` (see `src/main.ts`)

## Security & Validation
- Inputs: must pass `validate_input_audio_path()`
  - Rejects invalid chars (CR/LF/NUL), enforces allowed extensions, canonicalizes path, logs symlink resolution
- Output Directories: probed for write permissions before processing
- File Extensions: validated against `ALLOWED_AUDIO_EXTENSIONS` whitelist
- Cancellation: via `ProcessingState` and `cancel_processing` command

## Engine & Feature Flags
- Engine: `FfmpegNextProcessor` only
- Feature flags: none for engine selection; no shell-based FFmpeg fallback remains

## Post-Migration Context
- Migration to `ffmpeg-next` is complete. Remove any discovered shell-based artifacts (code, tests, docs) opportunistically.
- Enhance within the single engine path (e.g., encoder options, metadata enrichment, native cover art embedding).

## Frontend Patterns (TypeScript)
- Class-based UI modules with private state and DOM element caching (`StatusPanel`, `FileList`)
- Event-driven communication via `listen()` and Tauri events
- Strongly-typed boundaries for cross-language data (`ProgressEvent`, `ProcessingStatus`, `AudiobookMetadata`)

## External References
- ffmpeg-next (Rust crate): [docs.rs – ffmpeg-next](https://docs.rs/ffmpeg-next/latest/ffmpeg_next/)
- Tauri 2: [Tauri v2 Documentation](https://tauri.app/v2/)
- TypeScript: [TypeScript Handbook](https://www.typescriptlang.org/docs/)
- FFmpeg: [Official FFmpeg Documentation](https://ffmpeg.org/documentation.html)
- Lofty (metadata): [docs.rs – lofty](https://docs.rs/lofty/latest/lofty/)

## Quick Reference
```bash
# Run dev with verbose backend logging
RUST_LOG=debug npm run tauri dev

# All tests + lints
cargo test
cargo clippy -- -D warnings

# Path validation-focused tests
cargo test path_validation
```

## Platform Notes
- Repo Branches
  - 'main' (https://github.com/Allmight97/audiobook-boss.git) is the current stable branch.
  - 'feat/new_encoder' (https://github.com/Allmight97/audiobook-boss.git) is the current work-in-progress audio processing pipeline update that will support Apple AAC (aac_at) and HE-AAC v1/v2 and libfdk_aac (external call to local FDK binary only) encoders. And expose new 'advanced' settings panel for the user to configure more encoder and profile options.
- Primary development target: macOS (Apple Silicon). Out of scope: Intel Macs, Linux, Windows.

