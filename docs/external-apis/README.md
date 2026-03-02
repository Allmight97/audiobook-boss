# External API Architecture

This index categorizes the software boundaries in **Audiobook Boss**, mapping the external API documentation to the specific code surfaces where they are implemented.

---

## 🏗️ 1. Frontend IPC Boundary (`tauriClient`)
**API Class**: **IPC (Inter-Process Communication)**
**Interface**: Frontend (JS) ↔ Backend (Rust)
**Mechanism**: Command Invocations (Request/Response)

These APIs define the "Menu" of actions available to the user interface.

- **[Tauri Commands](./tauri-commands.md)**
    - **Command Entry Points**: `src-tauri/src/commands/*`
    - **Frontend Call Path**: `src/App.svelte` -> `src/ui/**` feature modules -> `src/lib/tauri/client.ts`
- **[Type Safety & Boundaries](./tauri-ts-boundaries.md)**
    - **Shared Contract**: `src/types/` (TS) ↔ `src-tauri/src/commands` (Rust) via generated `src/lib/generated/tauri.ts`
    - **Note**: `process_audiobook_files` accepts per-file metadata keyed by input path.

---

## 🛰️ 2. Telemetry (The Observer)
**API Class**: **Pub/Sub / Event Streaming**
**Interface**: Backend (Rust) → Frontend (JS)
**Mechanism**: Event Streaming (Unidirectional)

Real-time feedback as background jobs progress.

- **[Event Patterns](./tauri-patterns.md)**
    - **Emission Logic**: `src-tauri/src/audio/progress/*` + `src-tauri/src/audio/processor/frame_pipeline.rs`
    - **Frontend Listeners**: installed via `tauriClient.listen` (primarily `src/ui/statusPanel/events.ts`)
    - **Event Types**: `src/types/events.ts`
    - **Note**: `processing-progress` includes optional `job_id` + `input_index` for batch mapping; `processing-queue` carries queue snapshots for batch ordering.

---

## ⚙️ 3. Computation (The Engine)
**API Class**: **FFI (Foreign Function Interface)**
**Interface**: Application Code ↔ Native System Libraries
**Mechanism**: Low-level bindings via `ffmpeg-next` + MP4/M4B metadata via `mp4ameta`

The specialized "heavy lifting" layer for audio processing.

- **[Audio Processing (FFmpeg)](./ffmpeg-next.md)**
    - **Core Pipeline**: `src-tauri/src/audio/processor/{encoder/, frame_pipeline.rs, streams.rs}`
    - **Buffer Logic**: `src-tauri/src/audio/buffer.rs`
    - **Metadata (non-MP4)**: `src-tauri/src/metadata/ffmpeg_bridge.rs`
- **[MPEG-4 Metadata (mp4ameta)](./mp4ameta.md)**
    - **MP4/M4B Manager**: `src-tauri/src/metadata/mp4ameta_bridge.rs`
    - **Tag Extraction**: `src-tauri/src/metadata/reader.rs`

---

## 🛡️ 4. Guardrails (Security & I/O)
**API Class**: **System / OS Interface**
**Interface**: Application Code ↔ Operating System
**Mechanism**: Path normalization and validation

Ensuring system safety and data integrity at the boundaries of the local machine.

- **[Path Handling](./path-handling.md)**
    - **Validation**: `src-tauri/src/audio/path_validation.rs`
    - **Analysis**: `src-tauri/src/audio/file_list.rs`
    - **Pre-execution Check**: `src-tauri/src/audio/processor/prepare.rs`

---

## 🏗️ Automated Contract Validation

To maintain IPC contract integrity, this repo treats generated bindings and the `tauriClient` boundary as the source-of-truth stack.

- **What it does**: `scripts/check-generated-bindings.sh --mode verify` regenerates `src/lib/generated/tauri.ts` and fails on drift; local gate mode (`--mode local`) skips regeneration when no contract-related files changed.
- **Why it matters**: It prevents silent TS↔Rust drift and keeps boundary behavior deterministic for UX-critical flows.
- **Usage**: Run `scripts/checks.sh standard` from the repository root (includes frontend format checks via `bun run fmt:check` and change-aware local drift checks), `CHECK_BINDINGS_STRICT=1 scripts/checks.sh standard` for strict-in-gate verification, or `bun run bindings:check` for strict drift-only verification.

---

## Migration Status Snapshot (Branch: `feat/zero-legacy-svelte-cutover`)

- **Completed**:
  - `src/lib/bridge.ts` is retired.
  - Frontend command/event boundary is centralized in `src/lib/tauri/client.ts`.
  - Svelte app shell exists (`src/App.svelte` + `src/main.ts`).
- **Partial**:
  - Runtime remains hybrid: Svelte islands plus imperative DOM modules under `src/ui/**`.
- **Current guardrails**:
  - `scripts/check-no-bridge-imports.sh` blocks bridge resurrection.
  - Runtime DOM guardrails are being ratcheted to block new imperative patterns in migrated runtime entry paths.

---

## 📈 Performance System

Contract-sensitive perf surfaces (status panel event/render flow, metadata lookup path, audio processing path) are tracked with repo-native scripts under `scripts/perf/`.

- **Quick start** (package.json aliases):
  - `bun run perf` — full synthetic sweep (default go-to)
  - `bun run perf:audio` — real audio encode test
  - `bun run perf:all` — synthetic + real, combined matrix
  - `bun run perf:list` — list benchmarks with user impact descriptions
- **Advanced** (manual runner): `bun scripts/perf/run.mjs`
  - `bun scripts/perf/run.mjs --list`
  - `bun scripts/perf/run.mjs --all --mode synthetic --runs 9 --compare-baseline --append-history`
- Baselines:
  - `scripts/perf/baselines/synthetic-main.json`
  - `scripts/perf/baselines/real-main.json`
- Results:
  - `scripts/perf/results/latest.md` — combined matrix (includes app-vs-encoder attribution), encoder breakdown, trends; triage notes live here
  - `scripts/perf/results/latest.json` — latest run payload
  - `scripts/perf/results/latest-{mode}.json` — per-mode snapshots
  - `scripts/perf/results/history.ndjson` — full history
- Threshold semantics:
  - `warn`: >15% regression vs baseline
  - `improved`: >15% improvement vs baseline
