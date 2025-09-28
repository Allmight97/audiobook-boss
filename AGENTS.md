# AGENTS.md

This file guides AI coding agents working on Audiobook Boss. For the philosophy and precedence rules of AGENTS.md, see the public spec at [agents.md](https://agents.md/). The closest AGENTS.md to an edited file wins; explicit user chat prompts override; agents may auto-run listed checks; treat this as living documentation.

## What to do before editing
- Validate your plan with the repository owner before implementing non-trivial changes or refactors (prefer smallest safe diffs).
- Run quick checks (must pass before and after changes):
  - Rust (from `src-tauri/`):
    - `cargo test`
    - `cargo clippy -- -D warnings`
    - `cargo fmt --all -- --check`
  - Frontend (from repo root):
    - `tsc --noEmit`
    - `npm run build` (or `npm run dev` during iteration)
- Read these first:
  - `AGENTS.md` (this file)
  - `README.md` (human-facing overview + links)
  - `src-tauri/src/commands/*` and `src-tauri/src/audio/*` (integration points)
  - `docs/external-apis/*.md` for ffmpeg-next, lofty, tauri, and path handling

## Build, run, and logs
- Frontend dev: `npm run dev`
- App dev: `npm run tauri dev`
- App dev (verbose backend logs): `RUST_LOG=debug npm run tauri dev`
- Production build (Tauri): `npm run app:build`
- Rust tests/lints (from `src-tauri/`):
  - `cargo test`
  - `cargo clippy -- -D warnings`
  - Name-filtered subset: `cargo test path_validation`

## Architecture and rules of the road
- Single engine: `FfmpegNextProcessor` via ffmpeg-next bindings; no shell FFmpeg usage or engine feature flags.
- Path security: all input paths must pass `audio::path_validation::validate_input_audio_path()` (canonicalize, whitelist extensions, traverse-safe, symlink warnings).
- Progress system: progress is computed from ffmpeg-next timestamps; backend emits `processing-progress` Tauri events; frontend listens in `src/ui/statusPanel`.
- Metadata: Lofty for read/write; custom `AudiobookMetadata` structure in between.

### Critical flows
- File import: UI drag/drop → `analyze_audio_files` → `audio::file_list::get_file_list_info`
- Processing pipeline: `process_audiobook_files` (and v2) → `MediaProcessor::execute` → progress events
- Metadata: Lofty read → custom model → Lofty write (with native cover art path as available)

### Integration touchpoints
- Tauri commands: `src-tauri/src/commands/` (all user actions go through `#[tauri::command]` handlers; use `ProcessingState` for cancellation).
- Engine selection: `src-tauri/src/audio/processor/selection.rs` (single engine).
- Progress emission: `src-tauri/src/audio/progress/reporter.rs` (emits to window).
- Input validation occurs in `audio::path_validation` and must be respected in any new code.

## Coding standards and constraints
- TypeScript:
  - Strict mode; explicit types; avoid `any`.
  - File names camelCase; types/interfaces PascalCase.
  - Class-based UI modules with DOM caching; event-driven via `listen()`; strong boundary types for Rust/TS crossing (`src/types/*`).
- Rust:
  - `#![deny(clippy::unwrap_used)]`; prefer `Result<T, AppError>` and `?`.
  - Keep internals non-`pub` unless required across modules.
  - Format with rustfmt defaults.
- Size/complexity limits (cultural gate):
  - File ≤ 400 LOC; function ≤ 55 LOC; ≤ 7 params; ≤ 4 nesting depth.
  - Prefer guard clauses; DRY; high cohesion; single responsibility; clear separation of concerns and enforce orthogonality.
  - If exceeding limits for protocol/adapter/generated code, annotate with `// EXCEPTION: [reason]`.
- Imports: group std | third-party | local; no wildcard re-exports.
- Errors/logging: map external errors into `AppError` (`src-tauri/src/errors.rs`); don’t leak raw paths in user-facing errors.

## Security & validation
- Only accept input files with allowed extensions (see whitelist).
- Resolve symlinks with warnings; canonicalize to prevent traversal.
- Probe/validate output directories for write perms before processing.

## Platform and environment
- Primary development target: macOS (Apple Silicon) only.
- ffmpeg-next links against system libraries; no bundled ffmpeg binary discovery logic.

## Testing guidance
- Prefer external tests in `src-tauri/tests/` (public APIs). Inline tests are okay for private/`pub(crate)` items that are otherwise unreachable.
- Useful subsets:
  - Path security-only: `cargo test path_validation`
- Manual UI testing via `window.testCommands` in `src/main.ts`.

## Change management expectations
- Minimize diffs; prefer smallest safe change; avoid broad refactors unless asked.
- Do not reintroduce shell-based FFmpeg usage or engine feature flags.
- Keep progress emission behavior intact and reflected in UI types.
- Validate inputs with `validate_input_audio_path()` in any new code paths.
- Keep TS/Rust boundaries type-safe and explicit; update shared types if events change.

## Current state and near-term constraints
- ffmpeg-next migration is complete. Remove any lingering shell-based artifacts when discovered.
- Avoid adding new logic to `media_pipeline.rs`; new code should live under `audio/processor/{encoder.rs,streams.rs,frame_pipeline.rs}`.
- Keep finite/clamp sanitization centralized in `audio/buffer.rs`.
- Consider adding debug-only frame contract validation around encoder boundaries (nb_samples/alignment/PTS).
- Fix the “output settings not honored” issue before touching fast-path optimizations.

## Event contracts
- Event name: `processing-progress`
- Rust source of truth: `src-tauri/src/audio/progress/reporter.rs::ProgressEvent`
- TS source of truth: `src/types/events.ts::ProcessingProgressEvent`
- Backward-compat policy:
  - Additive fields should be optional in TS and defaulted in Rust if needed.
  - Do not rename/remove existing fields without updating all listeners and UI surfaces.
- Verification:
  - Run: `RUST_LOG=debug npm run tauri dev`, process a short sample, confirm stage transitions, percentage progression, and UI renders.
  - Then: `cargo test && cargo clippy -- -D warnings`.

## Pre-submit checklist
- `cargo test`
- `cargo clippy -- -D warnings`
- `cargo fmt --all -- --check`
- `tsc --noEmit`
- `npm run build`
