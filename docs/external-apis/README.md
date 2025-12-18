# External API Architecture

This index categorizes the software boundaries in **Audiobook Boss**, mapping the external API documentation to the specific code surfaces where they are implemented.

---

## 🏗️ 1. Orchestration (The Bridge)
**Interface**: Frontend (JS) ↔ Backend (Rust)
**Mechanism**: Command Invocations (Request/Response)

These APIs define the "Menu" of actions available to the user interface.

- **[Tauri Commands](./tauri-commands.md)**
    - **Command Entry Points**: `src-tauri/src/commands/*`
    - **UI Integration**: `src/ui/{fileImport, statusPanel, coverArt}`
- **[Type Safety & Boundaries](./tauri-ts-boundaries.md)**
    - **Shared Contract**: `src/types/` (TS) ↔ `src-tauri/src/commands` (Rust)

---

## 🛰️ 2. Telemetry (The Observer)
**Interface**: Backend (Rust) → Frontend (JS)
**Mechanism**: Event Streaming (Unidirectional)

Real-time feedback as background jobs progress.

- **[Event Patterns](./tauri-patterns.md)**
    - **Emission Logic**: `src-tauri/src/audio/progress/reporter.rs`
    - **Frontend Listeners**: `src/ui/statusPanel`
    - **Event Types**: `src/types/events.ts`

---

## ⚙️ 3. Computation (The Engine)
**Interface**: Application Code ↔ Native System Libraries
**Mechanism**: Low-level bindings via `ffmpeg-next`

The specialized "heavy lifting" layer for audio processing.

- **[Audio Processing (FFmpeg)](./ffmpeg-next.md)**
    - **Core Pipeline**: `src-tauri/src/audio/processor/{encoder/, frame_pipeline.rs, streams.rs}`
    - **Buffer Logic**: `src-tauri/src/audio/buffer.rs`
    - **Metadata Engine**: `src-tauri/src/metadata/ffmpeg_bridge.rs`

---

## 🛡️ 4. Guardrails (Security & I/O)
**Interface**: Application Code ↔ Operating System
**Mechanism**: Path normalization and validation

Ensuring system safety and data integrity at the boundaries of the local machine.

- **[Path Handling](./path-handling.md)**
    - **Validation**: `src-tauri/src/audio/path_validation.rs`
    - **Analysis**: `src-tauri/src/audio/file_list.rs`
    - **Pre-execution Check**: `src-tauri/src/audio/processor/prepare.rs`

---

---

## 🏗️ Automated Contract Validation

To maintain the integrity of "The Bridge", we use `scripts/ensure-contract.sh` to ensure the JS and Rust sides of the API stay in sync.

- **What it does**: It extracts all `invoke()` calls from the TypeScript source and compares them against the registered commands in the Rust `generate_handler!` block.
- **Why it matters**: It prevents "Dead End Invitations" where the UI tries to call a command that no longer exists in Rust, or where a Rust command is added but forgotten in the UI.
- **Usage**: Run `scripts/ensure-contract.sh` from the repository root. It will exit with an error if there's a mismatch.
