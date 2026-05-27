# API Map

This file is the lightweight human and agent index for Audiobook Boss runtime boundaries.

It is intentionally not a full contract dump. Use it to find the owning code quickly, then verify behavior in code and tests before making changes.

## Source Of Truth

- Rust IPC contract export: `src-tauri/src/ipc_contract.rs`
- Frontend runtime boundary: `src/lib/tauri/client.ts`
- Generated TypeScript bindings: `src/lib/generated/tauri.ts`
- Event types and names: `src/types/events.ts`
- Proof routing: `scripts/proof.sh --help`
- Main review proof: `scripts/proof.sh standard`

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

### App Settings

- `get_app_settings`, `update_app_settings`, `reset_app_settings`
  - Rust: `src-tauri/src/commands/app_settings.rs`
  - Core owner: `src-tauri/src/app_settings/`
  - Frontend: `src/ui/appSettings/` through `src/lib/tauri/client.ts`
  - Use: durable preference hydration and persistence for existing controls.
    Runtime-coupled values, such as max concurrency, are accepted by their
    runtime owner first and then persisted as settings truth.

### Audio Analysis And Processing

- `validate_files`, `analyze_audio_files`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/audio/path_validation.rs`, `src-tauri/src/audio/file_list.rs`
  - Frontend callers route through `tauriClient`

- `get_supported_audio_import_metadata`, `discover_audio_import_paths`, `take_opened_audio_files`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/audio/imports.rs`, `src-tauri/src/audio/path_validation.rs`, `src-tauri/src/opened_audio.rs`
  - Frontend: `src/ui/fileImport/importAnalysisWorkflow.ts` through `src/lib/tauri/client.ts`
  - Use: local audio import metadata, recursive folder/file discovery, and OS-opened audio drain for the Local Audio Import Boundary

- `validate_encoder_settings`, `list_available_encoders`, `refresh_external_toolchain`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/audio/settings_encoder.rs`, `src-tauri/src/audio/toolchain.rs`
  - Frontend: encoder and toolchain status flows via `src/lib/tauri/client.ts`

- `get_runtime_settings_capabilities`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/audio/settings_capabilities.rs`, `src-tauri/src/processing/job_registry/`
  - Frontend: runtime settings controls via `src/lib/tauri/client.ts`
  - Use: backend-owned selectable encoder and concurrency capability facts for UI controls

- `preview_output_path`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helper: `src-tauri/src/output_artifact/`
  - Use: backend-owned naming preview without collision suffixing

- `get_max_concurrent_jobs`, `set_max_concurrent_jobs`, `process_audiobook_files`, `cancel_processing`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/processing/run.rs`, `src-tauri/src/processing/plan.rs`, `src-tauri/src/processing/job_registry/`
  - Lifecycle helpers: `src-tauri/src/processing/lifecycle.rs`, `src-tauri/src/processing/progress/`
  - Audio execution: call the `crate::audio` public strip; native `ffmpeg-next` and external FFmpeg/FDK adapter selection remain private under `src-tauri/src/audio/processor/`
  - Use: queueing, processing, cancellation, batch orchestration

### Metadata

- `read_audio_metadata`, `save_metadata_to_file`, `save_metadata_batch`, `write_cover_art`, `load_cover_art_file`, `load_cover_art_from_url`
  - Rust: `src-tauri/src/commands/metadata.rs`
  - Core helpers: `src-tauri/src/metadata/`, `src-tauri/src/audio/path_validation.rs`
  - Lifecycle reporting: metadata batch save emits processing-owned queue/progress events with `operation_kind: metadataSave`
  - Note: metadata writes use intent patches at the boundary, not raw ad hoc object mutation; `track`/`disk` stay read-compatible only

- `search_online_metadata`
  - Rust: `src-tauri/src/commands/metadata_lookup/`
  - Frontend: `src/lib/tauri/client.ts`
  - Use: external metadata lookup and normalization before UI application

## Events

- `processing-progress`
- `processing-queue`
- `opened-audio-files`

Source files:

- Rust event export: `src-tauri/src/ipc_contract.rs`
- Rust event types: `src-tauri/src/processing/progress/mod.rs`, `src-tauri/src/opened_audio.rs`
- Rust event names: `src-tauri/src/processing/progress/mod.rs`, `src-tauri/src/opened_audio.rs`
- Rust operation vocabulary: `src-tauri/src/processing/lifecycle.rs`
- Frontend event contract: `src/types/events.ts`
- Frontend listener boundary: `src/lib/tauri/client.ts`
- Main UI consumers: `src/ui/statusPanel/events.ts`, `src/ui/fileImport/handlers.ts`

Lifecycle event payloads carry `operation_kind` so Status Panel can distinguish
merge processing, batch processing, and metadata save from backend truth instead
of caller-only choreography. Shared terminal counts use
`OperationResultSummary` in generated bindings.

`opened-audio-files` is an OS-opened local audio signal. The frontend drains the
backend queue through `tauriClient.takeOpenedAudioFiles()` and then routes those
paths through the same Local Audio Import Boundary as picker and drag/drop
imports.

## Tauri Plugins Used At Runtime

- Raw dialog open: `@tauri-apps/plugin-dialog` via `tauriClient.open`
- Typed dialog helpers: `tauriClient.openFile`, `tauriClient.openFiles`, and
  `tauriClient.openDirectory`
- Path opener: `@tauri-apps/plugin-opener` via `tauriClient.openPath`
- URL opener: `@tauri-apps/plugin-opener` via `tauriClient.openUrl`
- Generic event listen channel: `@tauri-apps/api/event` via `tauriClient.listen` for events outside the typed processing event pair.

```text
UI caller
  -> tauriClient
     -> getSupportedAudioImportMetadata -> Rust audio import metadata
     -> discoverAudioImportPaths        -> Rust audio import discovery
     -> takeOpenedAudioFiles            -> Rust OS-opened audio queue drain
     -> openFile/openFiles/openDirectory -> plugin-dialog open
     -> openPath                       -> plugin-opener open_path
     -> openUrl                        -> plugin-opener open_url (HTTPS only)
```

## Verification Pointers

- Contract and boundary regressions: `src/lib/behavior-contract.test.ts`, `src/lib/tauri-client.test.ts`
- UI integration and event behavior: `src/ui/**/__tests__`, `src/test/setup.ts`
- Runtime boundary proof: `scripts/proof.sh runtime`
- Full review proof: `scripts/proof.sh standard`

## Maintenance Rule

Keep this file short. Update it when a command family, event family, or owning source-of-truth path changes. Do not turn it back into a second full API specification.
