# Audiobook Boss — Repository Guidance

## Overview

- Backend: Rust with `ffmpeg-next` for audio processing (decode → resample → encode → mux) and `mp4ameta` for MP4/M4B metadata read/write
- Frontend: TypeScript (vanilla) + Tauri 2
- Metadata: `mp4ameta` for MP4/M4B tags + cover art; `ffmpeg-next` for non-MP4 metadata
- Audio Processing Engine: Single engine (`FfmpegNextProcessor`); no shell-based FFmpeg and no feature flags (see note end of this doc for updates to audio processing pipeline)

Internal docs:

- `docs/external-apis/ffmpeg-next.md` — audio/PTS/time_base, encoder, progress
- `docs/external-apis/tauri-patterns.md` — event lifecycle & IPC patterns
- `docs/external-apis/path-handling.md` — macOS-focused path validation and atomic moves

## For AI agents

If you are an AI coding agent, start with the project’s agent guide in `AGENTS.md`. It defines setup, checks to run automatically, architectural boundaries (single ffmpeg-next engine, path validation, progress events), and coding standards tailored for agents. See the AGENTS.md spec at [agents.md](https://agents.md/) for precedence rules.

## Architecture & Key Patterns

- Single Processing Engine: `FfmpegNextProcessor` implements `MediaProcessor`
- Media Abstraction: `MediaProcessingPlan` → `execute()`
- Path Security: All input paths must pass `audio::path_validation::validate_input_audio_path()` (canonicalizes, checks whitelist, resolves symlinks with warnings)
- Progress System: Based on ffmpeg-next timestamps; UI updates via Tauri events (`processing-progress`, `processing-queue`). Supports multiple concurrent jobs (events include an optional `job_id`) and a UI max-concurrency selector (Auto = `num_cpus/2`, clamped 1–8).

## Critical Data Flows

1. File Import: UI drag/drop → `analyze_audio_files` → `audio::file_list::get_file_list_info`
2. Processing Pipeline: `process_audiobook_files_v2` → `MediaProcessor::execute` → progress events via Tauri window
3. Metadata Flow: MP4/M4B read/write via `mp4ameta` (ffmpeg fallback for gaps) → `AudiobookMetadata` → `mp4ameta` write during metadata-only edits and finalize; non-MP4 stays on ffmpeg-next

## Commands & Integration Points

- Tauri Commands module: `src-tauri/src/commands/`
- `validate_files`, `analyze_audio_files`, `process_audiobook_files_v2`, `validate_encoder_settings_cmd`, `cancel_processing`, plus metadata read/write commands
- Processing Runtime
  - Engine selection is trivial: `FfmpegNextProcessor` only (see `audio/processor/selection.rs`)
  - ffmpeg-next initialized once per process (`ff::init()`)
- Progress Emission
  - Backend: `audio/progress/reporter.rs` emits via `window.emit("processing-progress", event)`
  - Backend: `commands/audio_processing.rs` emits `window.emit("processing-queue", event)` snapshots for batch ordering
  - Frontend: listeners live in `src/ui/statusPanel/events.ts` (types in `src/types/events.ts`)

## Development Workflows

### Toolchain Setup

```bash
# Install pinned toolchain versions for this repo
mise install
```

Notes:
- `mise.toml` pins Bun for reproducible JS/TS tooling.
- `rust-toolchain.toml` pins Rust channel/components for `cargo`, `rustfmt`, and `clippy`.

### Testing (run from repo root)

```bash
scripts/checks.sh standard            # Primary pre-PR quality gate (Rust + frontend format + TS + contract drift + build)
scripts/check-fallback-policy.sh      # Fallback governance check (marker + sunset + issue metadata)
cargo test                              # All tests (unit + integration)
cargo test --tests                      # All external test binaries
cargo test --test unit_audio_buffer_tests   # Specific unit test file
cargo test --test integration_metadata_tests # Specific integration test file
cargo clippy -- -D warnings             # Lint checks (must pass)
bun run fmt:check                       # Frontend format checks (Biome + Prettier for Svelte)
cargo test path_validation              # Path security subset by name filter
```

### Build & Run

```bash
bun run tauri dev                 # Full dev mode (port 1420)
RUST_LOG=debug bun run tauri dev  # With verbose logging
```

### IPC Contract Generation (Rust ↔ TS)

```bash
bun run bindings:generate  # Regenerate src/lib/generated/tauri.ts from Rust commands/events
bun run bindings:check     # Verify generated bindings are up to date
```

Source of truth:
- Rust contract builder: `src-tauri/src/ipc_contract.rs`
- Generated bindings: `src/lib/generated/tauri.ts`
- UI compatibility adapter: `src/lib/bridge.ts`

### Formatting

```bash
bun run fmt          # Apply frontend formatting (Biome + Prettier for .svelte)
bun run fmt:check    # Check-only formatting gate
bun run fmt:changed  # Changed-files cleanup loop (agent/human workflow)
```

- Biome owns TS/JS/CSS/JSON and config file formatting.
- Prettier is intentionally scoped to `.svelte` files during release prep.
- Frontend formatting in this repo uses tabs.

### Find & Kill Stale Dev Sessions

```bash
# Find running Vite/Tauri processes
pgrep -fl "vite|tauri"

# See what's using port 1420 (Vite dev server)
lsof -i :1420

# Kill Vite on port 1420
kill $(lsof -t -i:1420)

# Kill all Vite and Tauri dev processes
pkill -f vite && pkill -f "tauri dev"
```

### Logging (Rust)

Configure via `RUST_LOG` environment variable. Examples:

```bash
RUST_LOG=debug bun run tauri dev
RUST_LOG=audiobook_boss=debug bun run tauri dev
RUST_LOG=warn,audiobook_boss=debug bun run tauri dev
```

### Performance Benchmarks

Run the perf system from `scripts/perf` for local and manual CI trend checks.

**Quick start (using package.json scripts):**

```bash
bun run perf            # Full synthetic sweep (the default go-to)
bun run perf:audio      # Real audio encode test
bun run perf:real       # All benchmarks, real mode
bun run perf:all        # Full sweep: synthetic + real, combined matrix
bun run perf:quick      # Fast 3-run gut check
bun run perf:list       # What benchmarks exist + what they measure
```

**Advanced (manual invocation):**

```bash
# List all benches
bun scripts/perf/run.mjs --list

# Run one bench
bun scripts/perf/run.mjs --bench statuspanel-render-lookup --mode synthetic --runs 9

# Run all phase-1 benches with baseline compare + history append
bun scripts/perf/run.mjs --all --mode synthetic --runs 9 --compare-baseline --append-history

# Run app end-to-end audio throughput attribution bench (real mode)
bun scripts/perf/run.mjs --bench audio-processing-app-e2e --mode real --runs 3 --compare-baseline --append-history
```

**Details:**
- Modes: `synthetic` (deterministic) and `real` (workload-shaped).
- Baselines: `scripts/perf/baselines/synthetic-main.json` and `scripts/perf/baselines/real-main.json`.
- Output: `scripts/perf/results/latest.md` (combined matrix + encoder breakdown + attribution matrix + trends), `latest-{mode}.json` (per-mode snapshots), `history.ndjson` (full history).
- Attribution matrix (real mode): when both `audio-processing-throughput` (encoder CLI layer) and `audio-processing-app-e2e` (app pipeline layer) run, `latest.md` includes `rtf_cli`, `rtf_app`, and `overhead_ratio = (rtf_cli - rtf_app) / rtf_cli` per encoder.
- Threshold policy: `warn` when metric regresses by more than 15% versus baseline.
- CI policy: manual/non-blocking via `.github/workflows/perf.yml`.

## Coding Standards

- TypeScript: strict mode; explicit types; avoid `any`; camelCase filenames; PascalCase types
- Rust: idiomatic; `#![deny(clippy::unwrap_used)]`, `#![warn(clippy::too_many_lines)]`; use `Result<T, AppError>` and `?`
- Formatting: `rustfmt` for Rust; Biome for TS/JS/CSS/JSON; Prettier for `.svelte`; keep functions small and focused
- Visibility: keep internals non-`pub` unless cross-module use requires it

### Repository-Specific Expectations

- Single responsibility, high cohesion, guard clauses, DRY
- Exceptions allowed for:
  - Tauri command handlers (orchestration)
  - FFmpeg integration bindings/adapters
  - Generated protocol code
    Add `// EXCEPTION: [reason]` and consider follow-up refactor when exceeded.

### Test Organization

- External tests: `src-tauri/tests/` (flat structure)
  - `unit_*_tests.rs` for fast, single-module tests
  - `integration_*_tests.rs` for cross-module or FFmpeg/filesystem tests
- Inline tests: only for private/`pub(crate)` internals not testable externally
- Frontend: manual testing via `window.testCommands` (see `src/main.ts`)

## Security & Validation

Cover art URL loading is treated as untrusted input. The app only fetches HTTPS URLs, enforces strict size, format, and dimension limits, uses timeouts and redirect limits, and blocks private, loopback, and link-local IPs at DNS resolution to prevent SSRF and DNS rebinding attacks. Requests send only a fixed user-agent and no cookies or credentials. This preserves the "paste a URL" UX while minimizing internal network and resource exhaustion risk.

- Note: Some hosts (e.g., Reddit preview/CDN and some Google Images links) block hotlinking and return 403. In those cases, download the image locally and use "Load Cover Art" from file.

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
- Metadata via mp4ameta for MP4/M4B (read/write)
- Metadata via ffmpeg-next for non-MP4 (read/write)

## Quick Reference

```bash
# Run dev with verbose backend logging
RUST_LOG=debug bun run tauri dev

# Full quality gate
scripts/checks.sh standard

# All tests + lints
cargo test
cargo test --tests
cargo test --test unit_audio_buffer_tests
cargo test --test integration_metadata_tests
cargo clippy -- -D warnings
bun run fmt:check

# Path validation-focused tests
cargo test path_validation
```

## Platform Notes

- Repo Branches
  - 'main' (https://github.com/Allmight97/audiobook-boss.git) is the current stable branch.
- Audio pipeline uses a single encoder configuration surface (`EncoderSettings` + `SampleRateConfig`) via the `process_audiobook_files_v2` command (sole IPC entrypoint).
- Primary development target: macOS (Apple Silicon). Out of scope: Intel Macs, Linux, Windows.
