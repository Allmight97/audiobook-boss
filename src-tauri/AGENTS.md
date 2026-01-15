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

- **Rule**: **No inline tests** (`mod tests`) in `src-tauri/src` except for tiny private helpers.
- **Location**: `src-tauri/tests/` (flat structure)
  - **Naming convention** (strict):
    - `unit_*_tests.rs` - Pure logic tests, no I/O (files/FFmpeg/network)
    - `integration_*_tests.rs` - Real resources (files/FFmpeg/filesystem)
  - **Why flat?** Cargo auto-discovers tests only in top-level `tests/`, not subdirectories.
  - **Disabled tests**: Some files have placeholder tests due to private API dependencies. Move these into `#[cfg(test)]` modules or expose test utilities as needed.

### Running Tests

```bash
cargo test                              # All tests
cargo test unit_                        # Unit tests only (pattern match)
cargo test integration_                 # Integration tests only (pattern match)
cargo test --test unit_audio_buffer_tests  # Specific test file
```
