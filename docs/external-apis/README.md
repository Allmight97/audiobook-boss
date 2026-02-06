# External API Architecture

This index categorizes the software boundaries in **Audiobook Boss**, mapping the external API documentation to the specific code surfaces where they are implemented.

---

## 🏗️ 1. Orchestration (The Bridge)
**API Class**: **IPC (Inter-Process Communication)**
**Interface**: Frontend (JS) ↔ Backend (Rust)
**Mechanism**: Command Invocations (Request/Response)

These APIs define the "Menu" of actions available to the user interface.

- **[Tauri Commands](./tauri-commands.md)**
    - **Command Entry Points**: `src-tauri/src/commands/*`
    - **UI Integration**: `src/ui/{fileImport, statusPanel, coverArt, metadataLookup}`
- **[Type Safety & Boundaries](./tauri-ts-boundaries.md)**
    - **Shared Contract**: `src/types/` (TS) ↔ `src-tauri/src/commands` (Rust)
    - **Note**: `process_audiobook_files_v2` now accepts per-file metadata keyed by input path.

---

## 🛰️ 2. Telemetry (The Observer)
**API Class**: **Pub/Sub / Event Streaming**
**Interface**: Backend (Rust) → Frontend (JS)
**Mechanism**: Event Streaming (Unidirectional)

Real-time feedback as background jobs progress.

- **[Event Patterns](./tauri-patterns.md)**
    - **Emission Logic**: `src-tauri/src/audio/progress/reporter.rs`
    - **Frontend Listeners**: `src/ui/statusPanel`
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

To maintain the integrity of "The Bridge", we use `scripts/ensure-contract.sh` to ensure the JS and Rust sides of the API stay in sync.

- **What it does**: It extracts all `invoke()` calls from the TypeScript source and compares them against the registered commands in the Rust `generate_handler!` block.
- **Why it matters**: It prevents "Dead End Invitations" where the UI tries to call a command that no longer exists in Rust, or where a Rust command is added but forgotten in the UI.
- **Usage**: Run `scripts/ensure-contract.sh` from the repository root. It will exit with an error if there's a mismatch.

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
  - `scripts/perf/results/latest.md` — combined matrix, encoder breakdown, trends
  - `scripts/perf/results/latest.json` — latest run payload
  - `scripts/perf/results/latest-{mode}.json` — per-mode snapshots
  - `scripts/perf/results/history.ndjson` — full history
- Threshold semantics:
  - `warn`: >15% regression vs baseline
  - `improved`: >15% improvement vs baseline
