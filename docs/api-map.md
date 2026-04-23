# API Map

This file is the lightweight human and agent index for Audiobook Boss runtime boundaries.

It is intentionally not a full contract dump. Use it to find the owning code quickly, then verify behavior in code and tests before making changes.

## Source Of Truth

- Rust IPC contract export: `src-tauri/src/ipc_contract.rs`
- Frontend runtime boundary: `src/lib/tauri/client.ts`
- Generated TypeScript bindings: `src/lib/generated/tauri.ts`
- Event types and names: `src/types/events.ts`
- Main quality gate: `scripts/checks.sh standard`

## Boundary Model

- Frontend UI code should go through `tauriClient` in `src/lib/tauri/client.ts`.
- Rust commands are registered in `src-tauri/src/ipc_contract.rs` and implemented under `src-tauri/src/commands/`.
- Generated bindings in `src/lib/generated/tauri.ts` are committed for drift detection, not used as the main hand-authored runtime seam.
- Progress and queue events flow Rust -> frontend through tauri-specta events, normalized at the `tauriClient` boundary.

## Tauri Commands

### System

- `ping`, `echo`
  - Rust: `src-tauri/src/commands/system.rs`
  - Frontend: `src/lib/tauri/client.ts`
  - Use: smoke/debug wiring

### Audio Analysis And Processing

- `validate_files`, `analyze_audio_files`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/audio/path_validation.rs`, `src-tauri/src/audio/file_list.rs`
  - Frontend callers route through `tauriClient`

- `validate_encoder_settings`, `list_available_encoders`, `refresh_external_toolchain`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/audio/settings_encoder.rs`, `src-tauri/src/audio/toolchain.rs`
  - Frontend: encoder and toolchain status flows via `src/lib/tauri/client.ts`

- `preview_output_path`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helper: `src-tauri/src/audio/output_path/`
  - Use: backend-owned naming preview without collision suffixing

- `get_max_concurrent_jobs`, `set_max_concurrent_jobs`, `process_audiobook_files`, `cancel_processing`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/commands/audio_processing.rs`, `src-tauri/src/commands/audio_processing/plan.rs`, `src-tauri/src/audio/job_registry/`
  - Use: queueing, processing, cancellation, batch orchestration

### Metadata

- `read_audio_metadata`, `save_metadata_to_file`, `write_cover_art`, `load_cover_art_file`, `load_cover_art_from_url`
  - Rust: `src-tauri/src/commands/metadata.rs`
  - Core helpers: `src-tauri/src/metadata/`, `src-tauri/src/audio/path_validation.rs`
  - Note: metadata writes use intent patches at the boundary, not raw ad hoc object mutation; `track`/`disk` stay read-compatible only

- `search_online_metadata`
  - Rust: `src-tauri/src/commands/metadata_lookup/`
  - Frontend: `src/lib/tauri/client.ts`
  - Use: external metadata lookup and normalization before UI application

## Events

- `processing-progress`
- `processing-queue`

Source files:

- Rust event export: `src-tauri/src/ipc_contract.rs`
- Rust event types: `src-tauri/src/audio/progress/mod.rs`
- Rust event names: `src-tauri/src/audio/constants.rs`
- Frontend event contract: `src/types/events.ts`
- Frontend listener boundary: `src/lib/tauri/client.ts`
- Main UI consumer: `src/ui/statusPanel/events.ts`

## Tauri Plugins Used At Runtime

- Dialog open: `@tauri-apps/plugin-dialog` via `tauriClient.open`
- External opener: `@tauri-apps/plugin-opener` via `tauriClient.openExternal`
- Raw event listen fallback: `@tauri-apps/api/event` via `tauriClient.listen`

## Verification Pointers

- Contract and boundary regressions: `src/lib/behavior-contract.test.ts`, `src/lib/tauri-client.test.ts`
- UI integration and event behavior: `src/ui/**/__tests__`, `src/test/setup.ts`, `src/harness/mockTauri.ts`
- Full gate: `scripts/checks.sh standard`

## Maintenance Rule

Keep this file short. Update it when a command family, event family, or owning source-of-truth path changes. Do not turn it back into a second full API specification.
