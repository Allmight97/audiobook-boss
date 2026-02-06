# Rust Backend Guidelines

Inherits principles from root `AGENTS.md`. This file covers Rust-specific architecture, conventions, and testing.

---

## Architecture Fundamentals

- **Single engine**: `FfmpegNextProcessor` via ffmpeg-next bindings (no shell FFmpeg, no engine feature flags)
- **Concurrency surface**: `JobRegistry` (semaphore-backed) is the **exclusive source of truth** for active jobs.
  - **Parallelism**: Multiple jobs can run concurrently (up to `max_concurrent`).
  - **Blocking I/O**: CPU-bound encoding tasks MUST be offloaded using `tokio::task::spawn_blocking` or `tokio::task::block_in_place` to prevent async runtime starvation.
  - **Cancellation**: Managed via `CancellationChecker` (per-job) or global signal.
- **Path security**: all inputs → `audio::path_validation::validate_input_audio_path()` (canonicalize, whitelist extensions, traverse-safe, symlink warnings)
- **Progress system**: ffmpeg-next timestamps → `processing-progress` Tauri events → UI (`src/ui/statusPanel`)
- **Metadata**: ffmpeg-next read/write via custom `AudiobookMetadata` structure

## Critical Flows

- **Import**: UI drag/drop → `analyze_audio_files` → `audio::file_list::get_file_list_info`
- **Processing**: `process_audiobook_files_v2` → `MediaProcessor::execute` → progress events

## Integration Touchpoints

| Location | Purpose |
|----------|---------|
| `src-tauri/src/commands/` | All user actions via `#[tauri::command]` handlers; use `ProcessingState` for cancellation |
| `src-tauri/src/audio/processor/selection.rs` | Engine selection (single engine) |
| `src-tauri/src/audio/progress/reporter.rs` | Progress emission to window |

## Architectural Invariants

- **Type-Safe Encoder**: Encoder setup must consume `EncoderSettings` directly.
- **Logic Location**: New processing logic belongs in `audio/processor/{encoder/,streams.rs,frame_pipeline.rs}`.
- **Sanitization**: Finite/clamp sanitization must happen in `audio/buffer.rs`.
- **Primary Target**: macOS (Apple Silicon).

## Interface Boundaries

- **Command Surface**: UI must call `process_audiobook_files_v2` exclusively.
- **Contract Guard**: Maintain TS ↔ Rust command parity (`scripts/ensure-contract.sh`) until typesafe codegen is adopted.
- **Pointers**: `docs/external-apis/ffmpeg-next.md` (encoder/progress patterns), `docs/external-apis/tauri-commands.md` (command matrix).

---

## Code Conventions

- `#![deny(clippy::unwrap_used)]`; prefer `Result<T, AppError>` and `?`
- Keep internals non-`pub` unless required across modules
- Format with rustfmt defaults
- Map external errors → `AppError` (`src-tauri/src/errors.rs`)
- Don't leak raw paths in user-facing errors
- No wildcard re-exports in module files

---

## Testing

### Strategy: External Testing

- **Default**: tests live in `src-tauri/tests/` (flat structure).
- **Exception 1**: tiny private helpers (<50 LOC, no I/O, no FFmpeg, no TempDir) may keep inline tests.
  - Mark with `// EXCEPTION: tiny helper inline test`.
- **Exception 2**: tests requiring non-pub API access may be inline **only** if they are unit-only and avoid I/O/FFmpeg/TempDir.
  - Mark with `// EXCEPTION: requires private API access`.
- **Anti-pattern**: large integration suites (FFmpeg/filesystem/network) under `src-tauri/src`.
- **Location**: `src-tauri/tests/` (flat structure)
  - **Naming convention** (strict):
    - `unit_*_tests.rs` - Fast tests for a single module; may use TempDir, but avoid FFmpeg/filesystem-heavy flows
    - `integration_*_tests.rs` - Cross-module flows, real resources (files/FFmpeg/filesystem)
  - **Why flat?** Cargo auto-discovers tests only in top-level `tests/`, not subdirectories.

**Tiered checks**: Follow the repo-wide Standard/Release tiers in `AGENTS.md`. Use `scripts/standard-checks.sh` (the default go-to) and `scripts/release-checks.sh` from the repo root.

**Workspace note**: Run cargo commands from the repo root (workspace). No need to `cd src-tauri`.

### Running Tests

```bash
cargo test                                  # All tests
cargo test --tests                          # All external test binaries
cargo test --test unit_audio_buffer_tests   # Specific unit test file
cargo test --test integration_metadata_tests # Specific integration test file
```
