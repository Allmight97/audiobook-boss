# Technical Reference

Canonical architecture/runtime reference. For docs routing and proof-of-done, start in `docs/README.md` and `docs/verification.md`.

Architecture and implementation details for contributors and AI agents.

## Overview

- Backend: Rust with `ffmpeg-next` for audio processing (decode → resample → encode → mux) and `mp4ameta` for MP4/M4B metadata read/write
- Frontend: TypeScript + Svelte + Tauri 2
- Metadata: `mp4ameta` for MP4/M4B tags + cover art; `ffmpeg-next` for non-MP4 metadata
- Audio Processing Engine: Single engine (`FfmpegNextProcessor`); no shell-based FFmpeg
- AAC decode contract: the engine may select a named AAC decoder (`aac_at`, `libfdk_aac`) at runtime when the default AAC decoder cannot process an AAC-family stream; this is part of the macOS product contract, not a shell fallback

Internal docs:
- `docs/browser-harness.md` — required scenario verification versus optional interactive browser review
- `docs/workloop.md` — Workloop task-runner contract and `.agent-work/` temporary-state rules
- `docs/external-apis/ffmpeg-next.md` — audio/PTS/time_base, encoder, progress
- `docs/external-apis/tauri-patterns.md` — event lifecycle & IPC patterns
- `docs/external-apis/path-handling.md` — macOS-focused path validation and atomic moves

## Architecture & Key Patterns

- Single Processing Engine: `FfmpegNextProcessor` implements `MediaProcessor`
- Media Abstraction: `MediaProcessingPlan` → `execute()`
- Path Security: All input paths must pass `audio::path_validation::validate_input_audio_path()` (canonicalizes, checks whitelist, resolves symlinks with warnings)
- Progress System: Based on ffmpeg-next timestamps; UI updates via Tauri events (`processing-progress`, `processing-queue`). Supports multiple concurrent jobs (events include an optional `job_id`) and a UI max-concurrency selector (Auto = `num_cpus/2`, clamped 1–8).

### Frontend Runtime Ownership

- `src/App.svelte` is a thin shell; init/save/preview orchestration lives in `src/ui/core/bootstrap.ts`
- Canonical UI truth for the current migrated surfaces lives in typed reactive state modules rather than DOM nodes or `document` event buses:
  - file/session state in `src/ui/fileList/state.svelte.ts`
  - cover-art state in `src/ui/coverArt/state.svelte.ts`
  - metadata lookup queue state in `src/ui/metadataLookup/state.svelte.ts`
  - encoder state
  - metadata form state
  - output panel state, including preview request coordination
  - selected-file inspector state
- `src/ui/fileList/metadataPanel.ts` now guards late metadata and auto-cover completions so stale selection loads do not overwrite the current form or newer custom cover art
- Remaining imperative DOM usage is only acceptable when it is operational browser/platform behavior such as focus placement, native drag/drop geometry checks, or temporary drag-state classes

Operational code in this repo should mean physical UI behavior only. If a module owns workflow state, business truth, or cross-feature coordination, that code belongs in canonical reactive state and typed actions rather than DOM helpers, module globals, or singleton controllers.

### Remaining Mixed Ownership Seam

This is the main remaining frontend seam rather than a completed steady state:

- `src/ui/statusPanel/controller.ts` owns processing lifecycle through a typed `StatusPanelController`
- `src/ui/statusPanel/logic.ts` is now a thin singleton shell around that controller

The surrounding surfaces are less hybrid than before:

- file/session truth no longer lives in `src/ui/fileList/state.ts`; that path is now a thin compatibility re-export over `state.svelte.ts`
- cover-art truth no longer lives in module-global variables
- metadata lookup queue/workflow no longer lives in module-global controller state
- output preview race coordination no longer lives in module-local counters inside `src/ui/outputPanel/dom.ts`

The target standard is stricter than “remove DOM usage.” The real rule is:

- DOM code may stay for browser/platform mechanics
- runtime truth should live in typed reactive state and store-owned actions
- no CSS class, `data-*`, element ID, module-global controller, or singleton accessor should act as a hidden app-level contract

## Critical Data Flows

1. File Import: UI drag/drop → `analyze_audio_files` → `audio::file_list::get_file_list_info`
2. Processing Pipeline: `process_audiobook_files` → input decoder arbitration/open → `MediaProcessor::execute` → progress events via Tauri window
3. Metadata Flow: MP4/M4B read/write via `mp4ameta` (ffmpeg fallback for gaps) → `AudiobookMetadata` → `mp4ameta` write during metadata-only edits and finalize; non-MP4 stays on ffmpeg-next
4. Metadata Edit Intent Flow (frontend): metadata form edits are modeled as canonical patch ops (`set | clear | noop`) and compiled at the `tauriClient` boundary (`src/lib/tauri/client.ts` + `src/lib/tauri/normalizers.ts`) to current Rust payload semantics (`''`, `0`, `[]` clear sentinels).

## Commands & Integration Points

- Tauri Commands module: `src-tauri/src/commands/`
- `validate_files`, `analyze_audio_files`, `process_audiobook_files`, `validate_encoder_settings`, `cancel_processing`, plus metadata read/write commands
- Processing Runtime
  - Engine selection is trivial: `FfmpegNextProcessor` only (see `audio/processor/selection.rs`)
  - ffmpeg-next initialized once per process (`ff::init()`)
  - AAC-family input streams use runtime decoder arbitration at the engine boundary so validation, probing, and full processing agree on what counts as processable input
- Progress Emission
  - Backend: `audio/progress/reporter.rs` emits via `window.emit("processing-progress", event)`
  - Backend: `commands/audio_processing.rs` emits `window.emit("processing-queue", event)` snapshots for batch ordering
  - Frontend: listeners live in `src/ui/statusPanel/events.ts` (types in `src/types/events.ts`)

## Development Workflows

### Toolchain Setup

```bash
# Install JS/TS dependencies
bun install

# Rust toolchain is pinned via rust-toolchain.toml (auto-installed by cargo)
```

Notes:
- `rust-toolchain.toml` pins Rust channel/components for `cargo`, `rustfmt`, and `clippy`.

### Testing (run from repo root)

```bash
scripts/checks.sh standard            # Primary pre-PR quality gate (Rust + frontend format + TS + change-aware contract drift + build)
CHECK_BINDINGS_STRICT=1 scripts/checks.sh standard # Full gate with strict binding drift verification
scripts/check-fallback-policy.sh      # Fallback governance check (marker + sunset + issue metadata)
cargo test                              # All tests (unit + integration)
cargo test --tests                      # All external test binaries
cargo test --test unit_audio_buffer_tests   # Specific unit test file
cargo test --test integration_metadata_tests # Specific integration test file
cargo clippy -- -D warnings             # Lint checks (must pass)
bun run fmt:check                       # Frontend format checks only (Biome format + Prettier for Svelte)
bun run lint:check                      # Frontend lint checks only (Biome lint; explicit any fails)
cargo test path_validation              # Path security subset by name filter
scripts/checks.sh package               # Packaging gate + macOS AAC decoder contract verification
```

UI verification posture:

```bash
bun run harness:verify --changed
bun run harness:verify --scenario file-management
bun run harness:verify --scenario metadata-edit
bun run harness:verify --scenario status-processing
bun run harness:verify --scenario output-preview
```

- Use the harness verification path for UI-affecting work in addition to targeted tests.
- The current scenario registry covers:
  - `file-management` for import, selection, reorder, clear, and input-lane state
  - `metadata-edit` for metadata form, lookup, and cover-art flows
  - `status-processing` for queue/progress rendering and processing lock behavior
  - `output-preview` for encoder/output naming controls and preview health
- Harness runs should emit local artifacts (screenshots + assertion/runtime summaries) for the verified scenario set.
- Keep `harness:agent` as an optional interactive desktop browser-review lane for layout and control-affordance inspection. It should not replace `harness:verify` or be added to `scripts/checks.sh standard`.
- Audiobook Boss is desktop-only, so alternate viewport review is for explicit diagnostics only.

Local task-runner posture:

- Use `docs/workloop.md` plus root `WORKFLOW.md` as the current policy surface for local Workloop execution.
- Treat `.agent-work/` as temporary runtime state rather than durable project history, and keep it gitignored.
- Promote any durable conclusion out of task files or run logs into code, canonical docs, or `docs/decisions/DECISIONS.md`.
- The executable workflow contract lives in root `WORKFLOW.md`; use it together with `docs/workloop.md` when queue/frontmatter details or cleanup semantics matter.

### Build & Run

```bash
bun run tauri dev                 # Full dev mode (port 1420)
RUST_LOG=debug bun run tauri dev  # With verbose logging
```

### IPC Contract Generation (Rust ↔ TS)

```bash
bun run bindings:generate  # Regenerate src/lib/generated/tauri.ts from Rust commands/events
bun run bindings:check     # Strict verify: regenerate and fail on drift
bun run bindings:check:local # Change-aware local drift check
bun run bindings:sync      # Regenerate and stage bindings (hook-friendly)
```

Optional hook workflow:
- `git config core.hooksPath .githooks`
- pre-commit auto-syncs/stages generated bindings when staged Rust IPC contract files are detected.

Source of truth:
- Rust contract builder: `src-tauri/src/ipc_contract.rs`
- Generated bindings: `src/lib/generated/tauri.ts`
- UI compatibility adapter (thin boundary): `src/lib/tauri/client.ts` + `src/lib/tauri/normalizers.ts`
  - Invariant: clear intent must never be dropped by frontend emptiness heuristics.
  - Metadata clear mapping today: `string -> ''`, `date -> 0`, `cover_art -> []`.

### Formatting

```bash
bun run fmt          # Apply frontend formatting (Biome + Prettier for .svelte)
bun run fmt:check    # Check-only formatting gate
bun run lint:check   # Check-only frontend lint gate
bun run fmt:changed  # Changed-files cleanup loop (agent/human workflow)
```

- Biome owns TS/JS/CSS/JSON and config file formatting.
- Biome linting is a separate gate; repo-authored explicit `any` now fails there rather than surfacing as background noise during formatting.
- Prettier is intentionally scoped to `.svelte` files (Biome does not yet support Svelte).
- Frontend formatting in this repo uses tabs.
- Optional blame hygiene for the baseline commit:
  - `git config blame.ignoreRevsFile .git-blame-ignore-revs`

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
- Testability: prefer typed controllers/services and public behavioral seams over reaching into class internals with casts

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
- Frontend: targeted Vitest coverage plus harness verification for UI-facing flows

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

- `src/App.svelte` is a thin shell; init/save/preview orchestration lives in `src/ui/core/bootstrap.ts`
- Svelte islands with canonical reactive state modules (`.svelte.ts`) for encoder, metadata form, output preview, and inspector flows
- Event-driven communication via `tauriClient.listen()` and Tauri events
- Strongly-typed boundaries for cross-language data (`ProgressEvent`, `ProcessingStatus`, `AudiobookMetadata`)
- Remaining imperative DOM usage should stay limited to operational seams such as focus management, cover-art drag-drop hit-testing, and file-list drag-state classes rather than cross-module state ownership

## External References

- ffmpeg-next (Rust crate): [docs.rs – ffmpeg-next](https://docs.rs/ffmpeg-next/latest/ffmpeg_next/)
- Tauri 2: [Tauri v2 Documentation](https://tauri.app/v2/)
- TypeScript: [TypeScript Handbook](https://www.typescriptlang.org/docs/)
- FFmpeg: [Official FFmpeg Documentation](https://ffmpeg.org/documentation.html)
- Metadata via mp4ameta for MP4/M4B (read/write)
- Metadata via ffmpeg-next for non-MP4 (read/write)

## Platform Notes

- Repo Branches
  - 'main' (https://github.com/Allmight97/audiobook-boss.git) is the current stable branch.
- Audio pipeline uses a single encoder configuration surface (`EncoderSettings` + `SampleRateConfig`) via the `process_audiobook_files` command (sole IPC entrypoint).
- Primary development target: macOS (Apple Silicon). Out of scope: Intel Macs, Linux, Windows.
