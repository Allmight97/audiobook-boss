## Tauri command & event surfaces

This guide expands on the lightweight index by summarizing the public Tauri IPC the app exposes, the Rust modules that implement them, and the UI surfaces that depend on each command or event.

## Frontend call path (current contract)

- Runtime calls route through `src/lib/tauri/client.ts` (`tauriClient`).
- Typical flow: `src/App.svelte` -> `src/ui/**` feature modules -> `tauriClient` -> generated bindings (`src/lib/generated/tauri.ts`) -> Rust commands.
- Runtime calls route through the `tauriClient` boundary even where UI features still live in `src/ui/**` modules.

## Contract generation

- Rust contract builder: `src-tauri/src/ipc_contract.rs`
- Generated TypeScript bindings: `src/lib/generated/tauri.ts`
- UI TypeScript boundary adapter: `src/lib/tauri/client.ts`
- Export command: `bun run bindings:generate`
- Strict drift check: `bun run bindings:check`
- Change-aware local drift check: `bun run bindings:check:local`
- Hook sync path: `.githooks/pre-commit` runs `scripts/check-generated-bindings.sh --mode sync --staged` when staged Rust IPC contract files are detected

### Command matrix

| Command | Rust implementation | Primary consumers (via `tauriClient`) |
| --- | --- | --- |
| `ping`, `echo` | `src-tauri/src/commands/system.rs` | Integration smoke tests and ad-hoc debug invocation |
| `validate_files` | `src-tauri/src/commands/audio.rs` → `audio::path_validation` | Integration tests and QA diagnostics |
| `analyze_audio_files` | `src-tauri/src/commands/audio.rs` → `audio::file_list` | Drag/drop and picker flows in `src/ui/fileImport.ts` |
| `validate_encoder_settings` | `src-tauri/src/commands/audio.rs` → `audio::settings_encoder` | Reserved for advanced encoder UI; no current UI caller |
| `process_audiobook_files` | `src-tauri/src/commands/audio.rs` (async) | `src/ui/statusPanel/processing.ts` start/preview flows |
| `cancel_processing` | `src-tauri/src/commands/audio.rs` → `JobRegistry` | `src/ui/statusPanel/logic.ts` cancel-all and per-job cancel |
| `list_available_encoders` | `src-tauri/src/commands/audio.rs` | Used by UI to surface encoder guidance |
| `get_max_concurrent_jobs` | `src-tauri/src/commands/audio.rs` → `JobRegistry` | `src/ui/jobControls.ts` |
| `set_max_concurrent_jobs` | `src-tauri/src/commands/audio.rs` → `JobRegistry` | `src/ui/jobControls.ts` |
| `read_audio_metadata` | `src-tauri/src/commands/metadata.rs` → `metadata::reader` (mp4ameta for MP4/M4B, ffmpeg fallback) | File list metadata pane, cover-art thumbnail refresh |
| `save_metadata_to_file` | `src-tauri/src/commands/metadata.rs` (mp4ameta for MP4/M4B, ffmpeg for others) | Metadata-only editing (Cmd+S workflow) |
| `write_cover_art` | `src-tauri/src/commands/metadata.rs` (mp4ameta for MP4/M4B, ffmpeg for others) | Console/testing only |
| `load_cover_art_file` | `src-tauri/src/commands/metadata.rs` → filesystem load + validation | `src/ui/coverArt.ts` "Load Cover Art" action |
| `load_cover_art_from_url` | `src-tauri/src/commands/metadata.rs` | `src/ui/coverArt.ts`, `src/ui/metadataLookup.ts` |
| `search_online_metadata` | `src-tauri/src/commands/metadata_lookup/mod.rs` | `src/ui/metadataLookup.ts` |

### Command payloads & returns

- `analyze_audio_files`
  - Args: `{ filePaths: string[] }`
  - Returns: `FileListInfo` (`src/types/audio.ts:15`)

- `process_audiobook_files`
  - Args: `{ payload, metadata?, previewSeconds? }`
    - `payload: { inputFiles: string[]; outputDir: string; settings: EncoderSettings; sampleRate?: SampleRateConfig; jobType?: 'merge' | 'batch'; outputNaming?: OutputNamingConfig }`
      - `OutputNamingConfig: { absCompatible: boolean; includeYear: boolean }`
    - `settings: EncoderSettings` (`src/types/audio.ts:62`)
    - `metadata?: AudiobookMetadataMap` (`src/types/metadata.ts`)
    - `previewSeconds?: number` (optional short preview)
  - Returns: `ProcessCommandResult` (`{ message: string; previewFilePath?: string; previewActualSeconds?: number; jobId: string }`)
  - Notes:
    - `metadata` is keyed by input path; merge uses the first input path as the metadata key.
    - Threads mapping: `{mode:'auto'|'off'|'fixed'; value?}` → `threads=0|1|n`
    - Emits `processing-progress` events with `job_id` and optional `input_index` (backward compatible)
    - Emits `processing-queue` queue snapshots for batch ordering

- `cancel_processing`
  - Args: `{ job_id?: string }`
    - With `job_id`: cancels that job
    - Without: cancels all active jobs
  - Returns: `string` confirmation

- `get_max_concurrent_jobs`
  - Args: none
  - Returns: `number` (current cap)

- `set_max_concurrent_jobs`
  - Args: `{ max_concurrent?: number }`
    - `null/undefined` → reset to auto (`num_cpus/2`, clamped 1–8)
  - Returns: `number` (effective cap)

- `read_audio_metadata`
  - Args: `{ filePath: string }`
  - Returns: `AudiobookMetadata`

- `load_cover_art_file`
  - Args: `{ filePath: string }`
  - Returns: `number[]` (bytes); basic header validation for JPG/PNG/WebP

- `search_online_metadata`
  - Args: `{ query: string, sources?: MetadataSource[], limit?: number }`
  - Returns: `OnlineMetadataResult[]`

### Contract sources

- Generated source of truth: `src/lib/generated/tauri.ts`
- UI boundary adapter: `src/lib/tauri/client.ts`
- UI compatibility types: `src/types/audio.ts`, `src/types/metadata.ts`, `src/types/events.ts`

### Backend → frontend events

- `processing-progress` (emitted from `src-tauri/src/audio/progress/*`) drives the StatusPanel state machine via `src/types/events.ts` contracts and listeners installed through `tauriClient.listen`.
  - Emission throttling currently uses `PROGRESS_EMIT_INTERVAL_MS=1000` in `src-tauri/src/audio/processor/frame_pipeline.rs`.
- `processing-queue` (emitted from `src-tauri/src/commands/audio_processing.rs`) provides queue snapshots consumed by `src/ui/statusPanel/events.ts`.

### Frontend harness for verification

- Harness runtime is mounted via `src/harness-main.ts` and `src/HarnessApp.svelte`; production runtime remains `src/main.ts` + `src/App.svelte`.
- Use `bun run harness:verify --scenario <name>` or `bun run harness:verify --changed` as the verification entrypoint for UI-affecting work.
- Harness verification is expected to emit local screenshot/assertion/runtime artifacts rather than relying on manual “looks good” checks.

### Notes on scope

- Backend path validation accepts `wav`/`flac` alongside the UI-visible formats; the importer currently filters to MP3/M4A/M4B/AAC. Any future expansion should coordinate the UI filter with `audio::path_validation`.
- The processing command maps advanced `EncoderSettings` into the existing processing pipeline; when the dedicated encoder panel is enabled, it should call `validate_encoder_settings` before invoking processing.
