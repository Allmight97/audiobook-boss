# API Map

This file is the lightweight human and agent index for Audiobook Boss runtime boundaries.

It is intentionally not a full contract dump. Use it to find the owning code quickly, then verify behavior in code and tests before making changes.

## Lifecycle Classification

ABB processes audiobook work through two distinct lifecycle paths. This
classification is canonical and must not drift.

### Background Operations (WorkRuntime)

WorkRuntime (`src-tauri/src/work_runtime/`) owns all accepted final background
processing. It is the single source of truth for:

- Operation identity (`OperationId`)
- Immutable accepted submissions (operation input/kind/source identity is
  stable across the full lifecycle)
- Backend-authored `OperationSnapshot` truth with in-flight progress,
  child-job detail, cancellability, lanes, and terminal summaries
- Operation-scoped cancellation (`cancel_work_operation` per `OperationId`)
- Terminal status derived from the canonical
  `abb_processing_core::classify_run_terminal` classifier

Work Center (`src/ui/workCenter/`) renders WorkRuntime-authored snapshot
events (`work-operation-snapshot`, `work-operation-list-snapshot`) as the
sole progress source for accepted background operations. Work Center does
**not** subscribe to `processing-progress` events or apply client-authored progress
overlays.

Submission path: `submitProcessingOperation` (Tauri command
`submit_processing_operation`) — used only for final (non-preview) work
where `previewSeconds` is `None` / not set.

### Foreground / Direct Adapters (Status Panel)

The Status Panel (`src/ui/statusPanel/`) is **retained** as a foreground/direct
adapter. It is **not** a WorkRuntime consumer for background operations. It
owns the **preview lane only**:

- **Preview execution**: preview runs use `processAudiobookFiles` (Tauri command
  `process_audiobook_files`) with `previewSeconds` set. These are direct
  execution — they do not enter WorkRuntime. The Status Panel renders in-flight
  preview progress and the terminal verdict, consuming the backend-owned
  `RunTerminalClass` on `ProcessCommandResult` (it does not re-classify terminal
  precedence from per-job rows). `openGeneratedPreviewIfSingle` opens the result.
- **Metadata batch save**: **not** owned by the Status Panel. `save_metadata_batch`
  runs as a WorkRuntime `MetadataSave` operation rendered by the Work Center; the
  command returns the per-file `MetadataSaveBatchResult` for draft clearing.
- **Cancellation**: operation-scoped only — `cancel_work_operation` per
  `OperationId` (Work Center / WorkRuntime). The retained preview lane has **no**
  backend cancel command; the Status Panel cancel button settles the local render
  only and does not reach the backend.

### Event Scoping

`processing-progress` and `processing-queue` events are **foreground/direct
only**. Background WorkRuntime operations (batch/merge processing, metadata save)
do not emit these window events — they emit `work-operation-snapshot` /
`work-operation-list-snapshot`, which the Work Center consumes. There is no
`operation_id` discriminator: the Status Panel consumes every
`processing-progress`/`processing-queue` event it receives (all foreground
preview), and the two event channels (window progress vs. work-operation
snapshots) are the only scoping.

## Source Of Truth

- Rust IPC contract export: `src-tauri/src/ipc_contract.rs`
- Frontend runtime boundary: `src/lib/tauri/client.ts`
- Generated TypeScript bindings: `src/lib/generated/tauri.ts`
- Event types and names: `src/types/events.ts`

## Boundary Model

- Frontend UI code should go through `tauriClient` in `src/lib/tauri/client.ts`.
- Rust commands are registered in `src-tauri/src/ipc_contract.rs` and implemented under `src-tauri/src/commands/`.
- Generated bindings in `src/lib/generated/tauri.ts` are committed for drift detection, not used as the main hand-authored runtime seam.
- Progress and queue events flow Rust -> frontend through tauri-specta events, normalized at the `tauriClient` boundary.

## Tauri Commands

### App Settings

- `get_app_settings`, `update_app_settings`, `reset_app_settings`
  - Rust: `src-tauri/src/commands/app_settings.rs`
  - Core owner: `src-tauri/src/app_settings/`
  - Frontend: `src/ui/appSettings/` through `src/lib/tauri/client.ts`
  - Use: durable preference hydration and persistence for existing controls.
    Runtime-coupled values, such as max concurrency, are accepted by their
    runtime owner first and then persisted as settings truth.

- `log_frontend`
  - Rust: `src-tauri/src/commands/frontend_log.rs`
  - Frontend bridge: `src/lib/frontendLogBridge.ts` (installed from
    `src/lib/frontendLogBridge.install.ts` before `App.svelte` in `src/main.ts`)
  - Use: bounded local dev-log forwarding for webview `error` /
    `unhandledrejection` events; not remote telemetry

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

### Remote Source Acquisition

- `list_remote_source_providers`, `get_remote_source_account_state`,
  `start_remote_source_auth`, `complete_remote_source_auth`,
  `logout_remote_source_account`
  - Rust: `src-tauri/src/commands/remote_source.rs`
  - Core owner: `src-tauri/src/remote_source/`
  - Frontend: `src/ui/remoteSource/` through `src/lib/tauri/client.ts`
  - Use: provider-neutral source availability and backend-owned auth/account lifecycle. Frontend state must not hold provider credentials, tokens, cookies, license blobs, or raw provider responses.

- `load_remote_source_library`, `start_remote_source_acquisition`,
  `get_remote_source_acquisition_status`, `cancel_remote_source_acquisition`,
  `purge_remote_source_session`
  - Rust: `src-tauri/src/commands/remote_source.rs`
  - Core owner: `src-tauri/src/remote_source/`
  - Private provider cluster: `src-tauri/src/remote_source/providers/audible/`
  - Frontend: `src/ui/remoteSource/` through `src/lib/tauri/client.ts`
  - Use: library scan, acquisition job status, local materialized audio handoff
    into the Local Audio Import Boundary, Supplemental Asset tracking, and
    cleanup/purge of ABB-owned acquisition session storage. Processing receives
    explicit Supplemental Asset maps by file-list `inputId`; it does not query
    `RemoteSourceRuntime`.

- `validate_encoder_settings`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/audio/settings_encoder.rs`, `src-tauri/src/audio/toolchain.rs`
  - Frontend: encoder validation via `src/lib/tauri/client.ts`

- `get_runtime_settings_capabilities`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/audio/settings_capabilities.rs`, `src-tauri/src/processing/job_registry/`
  - Frontend: runtime settings controls via `src/lib/tauri/client.ts`
  - Use: backend-owned selectable encoder and concurrency capability facts for UI controls

- `preview_output_path`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helper: `src-tauri/src/output_artifact/`
  - Use: backend-owned naming preview without collision suffixing

- `get_max_concurrent_jobs`, `set_max_concurrent_jobs`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/processing/job_registry/`
  - Use: backend-owned concurrency management for JobRegistry

- `preflight_processing_plan`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Core helpers: `src-tauri/src/processing/plan.rs`
  - Frontend: `src/ui/outputPanel/outputPlanWorkflow.ts` through `src/lib/tauri/client.ts`
  - Use: shared, side-effect-free preflight planning used by the output-plan review step that precedes both the direct `process_audiobook_files` and WorkRuntime `submit_processing_operation` paths

### Work Runtime (Work Center) — Background Operations

- `submit_processing_operation`, `list_work_operations`, `get_work_operation`, `cancel_work_operation`
  - Rust: `src-tauri/src/commands/work_runtime.rs`
  - Owner: `src-tauri/src/work_runtime/` (operation identity, immutable accepted submissions, operation snapshots, operation-scoped cancellation, Work Center event truth)
  - Executor boundary: wraps `crate::processing::run`; operation terminal status is derived from the canonical `abb_processing_core::classify_run_terminal` classifier, not a parallel rule
  - Frontend: `src/ui/workCenter/` through `src/lib/tauri/client.ts`
  - Use: accept long-running batch/merge processing as background operations, return the file list to drafting immediately after acceptance, and surface multiple operations with independent cancellation
  - Classification: **background**. Final (non-preview) submissions only. Preview uses `process_audiobook_files` (direct).

### Audio Processing — Retained Direct Execution

- `process_audiobook_files`, `preview_output_path`
  - Rust: `src-tauri/src/commands/audio.rs`
  - Classification: **retained direct / foreground**.
    - `process_audiobook_files` is used for **preview** execution only (with `previewSeconds`). Final
      processing goes through `submit_processing_operation` (WorkRuntime).
    - Cancellation is operation-scoped only (`cancel_work_operation`). There is no
      foreground/direct cancel command; the preview lane has none.
  - Frontend: `src/ui/statusPanel/` through `src/lib/tauri/client.ts`

### Metadata

- `read_audio_metadata`, `read_audio_cover_thumbnail`, `validate_metadata_intent_patch`, `save_metadata_to_file`, `save_metadata_batch`, `write_cover_art`, `load_cover_art_file`, `load_cover_art_from_url`
  - Rust: `src-tauri/src/commands/metadata.rs`
  - Core helpers: `src-tauri/src/metadata/`, `src-tauri/src/audio/path_validation.rs`
  - Classification: `save_metadata_batch` is a **WorkRuntime `MetadataSave` operation**
    rendered by the Work Center. The command still returns `MetadataSaveBatchResult`
    synchronously (the frontend clears pending drafts only for files that succeeded),
    while progress + terminal counts flow through the operation snapshot. Cancellation
    is operation-scoped (`cancel_work_operation`). The Status Panel no longer handles
    metadata save.
  - Note: metadata validation/normalization is Rust-owned. Frontend code compiles explicit intent patches and asks this command for field errors/normalized intent instead of owning publication-date or series-sequence rule tables. Metadata writes use intent patches at the boundary, not raw ad hoc object mutation; `track`/`disk` stay read-compatible only

- `search_online_metadata`
  - Rust: `src-tauri/src/commands/metadata_lookup/`
  - Frontend: `src/lib/tauri/client.ts`
  - Use: external metadata lookup and normalization before UI application.
    Returns results plus typed diagnostics for provider degradation:
    ASIN detail unavailable with text search used, selected source failed with
    partial results, and Audnexus detail unavailable with Audible-only provenance.

## Events

### Background Operation Events (WorkRuntime → Work Center)

- `work-operation-snapshot`: emitted per-operation on lifecycle transitions
  (accepted, running, progress, cancelling, terminal). Carries a full
  `OperationSnapshot` with progress, children, terminal summary.
- `work-operation-list-snapshot`: emitted after any operation state change.
  Carries the full sorted operation list.

Source: `src-tauri/src/work_runtime/` (types in `types.rs`, emission in
`runtime.rs`). Consumer: `src/ui/workCenter/state.svelte.ts` only.

### Foreground / Direct Progress Events (Status Panel)

- `processing-progress`: per-job stage/percentage/message updates, carrying
  `operation_kind`. Emitted only by direct/foreground `process_audiobook_files`
  (preview) execution — background WorkRuntime operations emit no window progress
  events. There is no `operation_id` discriminator; the Status Panel consumes
  every `processing-progress` event (all preview).
- `processing-queue`: preview queue snapshots with ordered item list. Same
  foreground-only emission as `processing-progress`.

Source: `src-tauri/src/processing/progress/` (types in `mod.rs`, emission via
`ProgressEmitter` in `emitter.rs`). Consumer: `src/ui/statusPanel/events.ts`
and `src/ui/statusPanel/domain/stateMachine.ts`.

### OS Signal Events

- `opened-audio-files`: OS-opened local audio signal.

Source: `src-tauri/src/opened_audio.rs`. Consumer: `src/ui/fileImport/handlers.ts`.

Rust event export: `src-tauri/src/ipc_contract.rs`.
Frontend event contract: `src/types/events.ts`, `src/types/workRuntime.ts`.
Frontend listener boundary: `src/lib/tauri/client.ts`.

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
- Runtime boundary checks: `bash scripts/check-generated-bindings.sh --mode local` and targeted `bun run test -- src/lib/...`
- Broader review: escalate through the owner-scoped command menu in `scripts/AGENTS.md` only when the changed surface crosses owners or a concrete safety/data/contract invariant requires it.

## Maintenance Rule

Keep this file short. Update it when a command family, event family, or owning source-of-truth path changes. Do not turn it back into a second full API specification.
