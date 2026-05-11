# AudioBook Boss — Architecture Audit: Deep Modules & Flow

> Generated from codebase audit on 2026-05-11. No code changes were made.  
> [HTML companion &rarr;](grey-arch-audit.html)

## 1. Architecture Overview

- **UI:** Svelte 5 + Tailwind 4, Vite 8 bundler
- **Shell:** Tauri 2 (desktop)
- **Backend:** Rust 2021, ffmpeg-next 8.1 audio engine
- **Backend source:** ~13,500 LOC (src-tauri/src)
- **Backend tests:** ~10,800 LOC (~45% of backend total)
  - Integration tests: ~6,500 LOC (29 files)
  - Inline tests: ~4,300 LOC (23% of src files)
- **Frontend source:** ~12,000 LOC (src/)
- **Frontend tests:** ~5,200 LOC (~30% of frontend total)
- **Commands:** 17 Tauri commands
- **Events:** 2 specta-typed events

## 2. Backend Deep Modules

### 2.1 `audio/processor` — Deepest Module (~3,864 LOC, 12 files)
The audio processing pipeline. Hides the full lifecycle of audiobook creation behind a tiny public API.

**Public API:**
- `process_audiobook_with_context(ProcessingContext, Vec<AudioFile>, Option<AudiobookMetadata>, bool) -> Result<String>`
- `detect_input_sample_rate(&Path) -> Result<u32>`
- `resolve_processor_adapter(...) -> ResolvedProcessorAdapter`
- `FfmpegNextProcessor` (implements `MediaProcessor`)

**Cluster — Stage Orchestration:**
- `prepare.rs` — validates inputs, detects sample rates, temp workspace
- `execute.rs` — runs FFmpeg merge/encode (native or external FDK)
- `finalize.rs` — writes metadata, moves temp to destination, cleans up
- `staging.rs` — destination-adjacent temp directories
- `plan.rs` — `MediaProcessingPlan` trait and processor selection

**Cluster — Engine & Adapter:**
- `engine.rs` — in-process ffmpeg-next orchestrator
- `adapter.rs` — resolves native AAC / Apple AAC / external FDK HE-AAC
- `external_fdk.rs` — shells out to user-provided FFmpeg with libfdk_aac
- `frame_pipeline.rs` — preview/full-run frame accumulation and chapter slicing
- `streams.rs` — stream setup and codec context
- `selection.rs`, `preview_state.rs`

**Cluster — Encoder Subsystem:**
- `encoder/mod.rs`, `write.rs`, `context.rs`, `common.rs`
- `encoder/options/mod.rs`, `native.rs`, `fdk.rs`, `apple.rs`

**Downstream contracts:**
- Emits `ProgressEvent` / `QueueEvent` through Tauri window emitter
- Reads `CancellationChecker` from `job_registry`
- Calls `metadata::save_metadata_with_plan` during finalize
- Uses `output_path` planning for destination paths

---

### 2.2 `metadata` — Metadata Boundary (~2,269 LOC, 12 files)
Owns read/write behavior for audiobook tags. Chooses `mp4ameta` (direct atom editing) vs FFmpeg remux based on actual container classification.

**Public API:**
- `read_metadata(path) -> Result<AudiobookMetadata>`
- `save_metadata_with_plan(path, &MetadataWritePlan) -> Result<()>`
- `set_container_metadata(...)`, `validate_metadata_compatibility(...)`
- `MetadataIntentPatch` → `MetadataWritePlan` conversion
- Re-exports: `CoverFormat`, cover-art stream helpers

**Cluster — Read/Write Engine:**
- `reader.rs`, `container.rs`, `remux.rs`, `mp4ameta_bridge.rs`, `ffmpeg_dict.rs`, `tag_registry.rs`

**Cluster — Intent & Cover Art:**
- `intent.rs`, `intent_plan.rs`, `passthrough.rs`
- `cover_art/mod.rs`, `format.rs`, `embedding.rs`, `ffi.rs`

**Downstream contracts:**
- Consumed by `commands::metadata` (Tauri commands)
- Consumed by `audio::processor::finalize` (post-encode metadata)
- Types are specta-typed and cross the TS↔Rust boundary

---

### 2.3 `audio/output_path` — Path Planning (~1,971 LOC, 7 files)
Hides template-based naming, collision suffixing, and safe temp→final moves.

**Public API:**
- `build_output_path_preview(...) -> Result<PathBuf>`
- `derive_output_artifact_path(&Path, OutputKind) -> Result<PathBuf>`
- `enforce_output_plan_review(...) -> Result<()>`
- `OutputPlanReview`, `ResolvedOutputPlan`, `CollisionPolicy`

**Cluster:**
- `naming.rs`, `plan.rs`, `review.rs`, `collision.rs`, `artifact.rs`, `commit.rs`, `types.rs`

**Downstream contracts:**
- Called by `commands::audio::preview_output_path`
- Called by `commands::audio_processing::plan`
- Used inside `processor::finalize`

---

### 2.4 `audio/job_registry` — Concurrency Lifecycle (~342 LOC, 3 files)
Semaphore-backed registry with per-job cancellation and global cancel-all.

**Public API:**
- `JobRegistry::new(max) / ::auto()`
- `register_job() -> (JobId, OwnedSemaphorePermit)`
- `scheduler() -> BatchScheduler`
- `cancel_all()`, `cancel_job(JobId)`
- `update_max_concurrent(max) -> Result<usize>`
- `CancellationChecker`

**Cluster:**
- `mod.rs`, `types.rs`, `cancel.rs`

**Downstream contracts:**
- Managed as Tauri state (`ManagedJobRegistry = Arc<JobRegistry>`)
- Injected into `process_audiobook_files` and `cancel_processing`
- `BatchScheduler::run_batch` used by `commands::audio_processing::run`

---

### 2.5 `commands` — Tauri Command Surface (~696 LOC, 5 files)
Thin wrappers. No heavy logic; delegates to `audio` and `metadata` domains.

**Cluster:**
- `audio.rs`, `metadata.rs`, `system.rs`
- `audio_processing/run.rs` (761 LOC), `plan.rs` (707 LOC), `terminal_outcomes.rs`
- `metadata_lookup/mod.rs`, `service.rs`, `parse.rs`, `mapping.rs`, `providers/*.rs`

**Commands (17):**
ping, echo, validate_files, analyze_audio_files, read_audio_metadata, write_cover_art, load_cover_art_file, load_cover_art_from_url, save_metadata_to_file, search_online_metadata, validate_encoder_settings, list_available_encoders, refresh_external_toolchain, preview_output_path, preflight_processing_plan, get/set_max_concurrent_jobs, process_audiobook_files, cancel_processing

---

### 2.6 `audio` — Domain Umbrella (~2,311 LOC total)
Re-exports subsystems and defines shared types (`AudioFile`, `ProcessingProgress`, `ProcessingStage`, `SampleRateConfig`).

**Sub-clusters:**
| Submodule | LOC | Role |
|-----------|-----|------|
| context | 333 | ProcessingContext builder & OutputConfig |
| progress | 380 | ProgressEvent/QueueEvent emission + formatting |
| settings | ~120 | Encoder settings validation |
| settings_encoder | ~180 | Encoder option resolution |
| file_list | ~160 | Input file probing & FileListInfo |
| metrics | ~80 | ProcessingMetrics accumulation |
| path_validation | ~120 | Input audio/image path validation |
| toolchain | ~200 | Encoder availability detection |
| cleanup | 213 | CleanupGuard + temp resource removal |
| buffer | ~60 | Sample buffer helpers |
| extensions | ~40 | File extension constants |
| preview_config | ~60 | Preview duration & chapter slicing |
| session | ~40 | Session ID generation |
| constants | ~30 | Event name constants |

---

## 3. Frontend Deep Modules

### 3.1 `ui/statusPanel` (~1,699 LOC, 14 files)
Most complex frontend module. Manages job lifecycle state, progress aggregation, queue rendering, and Tauri event connection.

**Cluster — Domain:**
- `domain/stateMachine.ts`, `stateMachineTypes.ts`, `stateMachineHelpers.ts`
- `domain/queueState.ts`, `domain/aggregate.ts`, `domain/jobKeys.ts`

**Cluster — Services & Rendering:**
- `services/progressSubscription.ts`, `services/coverArtTracker.ts`, `services/fileLookup.ts`
- `render.ts`, `preview.ts`, `controller.ts`, `feedback.ts`, `events.ts`, `processing.ts`, `runtimeApi.ts`

**Contracts:**
- Subscribes to `processing-progress` and `processing-queue` via `tauriClient.listen`
- Calls `tauriClient.processAudiobookFiles`, `cancelProcessing`, etc.

---

### 3.2 `lib/tauri` — IPC Boundary Adapter (~783 LOC, 4 files)
Every Tauri command and event crosses through here.

**Public API:** `tauriClient` — single object exposing all commands and `listen`.

**Cluster:** `client.ts`, `commands.ts`, `normalizers.ts`, `appError.ts`

**Contracts:**
- Generated from Rust via `tauri-specta` (`ipc_contract.rs`)
- Tests in `src/lib/tauri-client.test.ts`
- Drift check: `scripts/check-generated-bindings.sh`

---

### 3.3 `ui/fileList` (~1,482 LOC, 8 files)
Drag-and-drop file list, reordering, selection, inspector panel.

**Cluster:** `viewState.svelte.ts`, `inspectorState.svelte.ts`, `FileListIsland.svelte`, `FileListItem.svelte`, `FileListReorder.svelte`, `FileListSelection.svelte`, `FileListEmpty.svelte`, `FileListToolbar.svelte`

---

### 3.4 `ui/encoderPanel` (~1,036 LOC, 4 files)
Encoder settings UI. Syncs with backend toolchain detection.

**Cluster:** `EncoderPanelIsland.svelte`, `state.svelte.ts`, `EncoderSettings.svelte`, `EncoderAvailability.svelte`

---

### 3.5 `ui/outputPanel` (~774 LOC, 6 files)
Output directory selection, naming template preview, collision policy UI.

**Cluster:** `OutputPanelIsland.svelte`, `state.svelte.ts`, `pathBuilder.ts`, `actions.ts`, `preview.ts`, `index.ts`

### 3.6 `ui/metadataForm` (~701 LOC, 3 files)
Captures and validates metadata intent before crossing to backend.
**Cluster:** `MetadataFormFieldsIsland.svelte`, `state.svelte.ts`, `previewState.svelte.ts`

### 3.7 `ui/fileImport` (~599 LOC, 4 files)
Drag-and-drop file import, cover art drop handling, and supported audio type detection.
**Cluster:** `FileImportIsland.svelte`, `handlers.ts`, `state.svelte.ts`, `supportedAudio.ts`

### 3.8 `ui/coverArt` (~487 LOC, 3 files)
Cover art display, load from file/URL, and embedding orchestration.
**Cluster:** `CoverArtIsland.svelte`, `state.svelte.ts`, tests

### 3.9 `ui/metadataLookup` (~392 LOC, 2 files)
Online metadata search UI and result mapping.
**Cluster:** `MetadataLookupIsland.svelte`, `state.svelte.ts`

### 3.10 `ui/collisionDialog` (~360 LOC, 3 files)
Output collision review dialog and state management.
**Cluster:** `CollisionDialogIsland.svelte`, `state.svelte.ts`, tests

---

## 4. IPC Boundary

- **Generated by:** specta (Rust) → tauri-specta (TypeScript)
- **Source of truth:** `src-tauri/src/ipc_contract.rs`
- **Generated file:** `src/lib/generated/tauri.ts`
- **Adapter:** `src/lib/tauri/client.ts` (must be used; direct generated calls forbidden)

**Events:**
| Event | Rust type | Frontend normalizer |
|-------|-----------|---------------------|
| processing-progress | `ProgressEvent` | `normalizeProgressEvent` |
| processing-queue | `QueueEvent` | `normalizeQueueEvent` |

**Error contract:** `Result<T, AppErrorEnvelope>` with `code`, `category`, `message`, `detail`.

---

## 5. High-Level Flow

### A. User Initiates Processing
```
UI (App.svelte)
  → tauriClient (lib/tauri)
    → process_audiobook_files (commands::audio)
      → ProcessingRun::execute (commands/audio_processing/run)
        → prepare_execution_plan (commands/audio_processing/plan)
          → dispatch_merge_job or dispatch_batch_jobs
            → JobRegistry (register_job + scheduler)
              → process_audiobook_with_context (audio::processor)
```

### B. Inside the Processor (3-Stage)
```
prepare (validate + detect + temp)
  → execute (engine/adapter encode + mux)
    → finalize (metadata write + move + cleanup)
```

### C. Metadata-Only Save (Cmd+S)
```
UI (MetadataFormFieldsIsland)
  → saveMetadataFromUI (ui/core/actions)
    → tauriClient.saveMetadataIntentToFile
      → save_metadata_to_file (commands::metadata)
        → MetadataIntentPatch → MetadataWritePlan
          → save_metadata_with_plan (metadata::mod)
            → mp4ameta_bridge or remux
```

### D. Progress & Cancellation Loop
```
processor emits progress
  → Tauri Window (ProgressEvent)
    → normalizers.ts
      → statusPanel state machine update

UI Cancel
  → cancelProcessing
    → JobRegistry (global_cancel = true)
      → CancellationChecker (processor reads flag)
```

---

## 6. Observations

### 6.1 Dependency Categories
| Module | Category | Notes |
|--------|----------|-------|
| audio/processor | Local-substitutable + True external | ffmpeg-next = external; external_fdk = port-and-adapter |
| metadata | Local-substitutable + True external | mp4ameta + FFmpeg remux hidden behind single function |
| audio/job_registry | In-process | Pure tokio; no I/O |
| audio/output_path | Local-substitutable | Filesystem only |
| commands | Boundary glue | Thin Tauri wiring; not for deepening |

### 6.2 False Seams Assessment
- `commands/audio_processing/run.rs` (761 LOC) → **Keep**. Owns merge vs batch dispatch and terminal outcome classification.
- `commands/audio_processing/plan.rs` (707 LOC) → **Keep**. Owns collision review and metadata resolution policy.
- `audio/mod.rs` re-exports → **Keep**. Semantic umbrella; scatters imports otherwise.
- `lib.rs` metadata re-exports for tests → **Bundle later**. Leaks internal symbols into crate root.
- `ui/core/actions.ts` → **Keep**. Expresses domain intent (Cmd+S workflow).

### 6.3 Strengths
- Explicit processor stage boundaries (prepare → execute → finalize)
- Container-aware metadata routing hidden behind single function
- Intent-based metadata edits (Set/Clear/Noop + Recompute)
- Generated IPC bindings with drift checks
- JobRegistry centralizes concurrency and cancellation
- `AGENTS.md` files at module boundaries document local invariants

### 6.4 Friction Points
- `audio/processor` and encoder subtree exceed 400 LOC module target
- `commands/audio_processing/run.rs` at 761 LOC is a large orchestrator
- `ui/statusPanel` at ~1,699 LOC is the deepest frontend module
- Test-only exports in `lib.rs` leak internal metadata symbols
