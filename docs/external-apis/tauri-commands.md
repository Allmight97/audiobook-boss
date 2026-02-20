## Tauri command & event surfaces

This guide expands on the lightweight index by summarizing the public Tauri IPC the app exposes, the Rust modules that implement them, and the UI surfaces that depend on each command or event.

## Contract generation

- Rust contract builder: `src-tauri/src/ipc_contract.rs`
- Generated TypeScript bindings: `src/lib/generated/tauri.ts`
- UI TypeScript boundary adapter: `src/lib/tauri/client.ts`
- Export command: `bun run bindings:generate`
- Drift check: `bun run bindings:check`

### Command matrix

| Command | Rust implementation | Primary consumers |
| --- | --- | --- |
| `ping`, `echo` | `src-tauri/src/commands/system.rs` | Console smoke tests via `window.testCommands` |
| `validate_files` | `src-tauri/src/commands/audio.rs` → `audio::path_validation` | Integration tests and console harness |
| `analyze_audio_files` | `src-tauri/src/commands/audio.rs` → `audio::file_list` | Drag/drop and picker flows in `src/ui/fileImport` |
| `validate_encoder_settings_cmd` | `src-tauri/src/commands/audio.rs` → `audio::settings_encoder` | Reserved for advanced encoder UI; no current UI caller |
| `process_audiobook_files_v2` | `src-tauri/src/commands/audio.rs` (async) | `StatusPanel` start/preview flows, providing EncoderSettings v2 |
| `cancel_processing` | `src-tauri/src/commands/audio.rs` → `JobRegistry` | StatusPanel cancel-all and per-job cancel |
| `list_available_encoders` | `src-tauri/src/commands/audio.rs` | Used by UI to surface encoder guidance |
| `get_max_concurrent_jobs` | `src-tauri/src/commands/audio.rs` → `JobRegistry` | StatusPanel “Max concurrent conversions” selector |
| `set_max_concurrent_jobs` | `src-tauri/src/commands/audio.rs` → `JobRegistry` | StatusPanel “Max concurrent conversions” selector |
| `read_audio_metadata` | `src-tauri/src/commands/metadata.rs` → `metadata::reader` (mp4ameta for MP4/M4B, ffmpeg fallback) | File list metadata pane, cover-art thumbnail refresh |
| `save_metadata_to_file` | `src-tauri/src/commands/metadata.rs` (mp4ameta for MP4/M4B, ffmpeg for others) | Metadata-only editing (Cmd+S workflow) |
| `write_cover_art` | `src-tauri/src/commands/metadata.rs` (mp4ameta for MP4/M4B, ffmpeg for others) | Console/testing only |
| `load_cover_art_file` | `src-tauri/src/commands/metadata.rs` → filesystem load + validation | `src/ui/coverArt` "Load Cover Art" button |
| `load_cover_art_from_url` | `src-tauri/src/commands/metadata.rs` | `src/ui/coverArt`, `src/ui/metadataLookup` |
| `search_online_metadata` | `src-tauri/src/commands/metadata_lookup/mod.rs` | `src/ui/metadataLookup` |

### Command payloads & returns

- `analyze_audio_files`
  - Args: `{ filePaths: string[] }`
  - Returns: `FileListInfo` (`src/types/audio.ts:15`)

- `process_audiobook_files_v2`
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

- `processing-progress` (emitted from `src-tauri/src/audio/progress/reporter.rs`) drives the StatusPanel state machine via `src/types/events.ts` contracts and the listener installed in `src/ui/statusPanel`. Payload includes optional `job_id` and `input_index` to support multiple concurrent jobs and stable file mapping.
  - Emission throttling (~200ms) originates in `src-tauri/src/audio/processor/frame_pipeline.rs`.
- `processing-queue` (emitted from `src-tauri/src/commands/audio_processing.rs`) provides queue snapshots consumed by `src/ui/statusPanel`.

### Frontend harness for QA

- `window.testCommands` in `src/main.ts` exposes select commands for manual QA (ping/echo/validation/metadata); production UI flows depend on the modules listed above rather than the harness itself.

### Notes on scope

- Backend path validation accepts `wav`/`flac` alongside the UI-visible formats; the importer currently filters to MP3/M4A/M4B/AAC. Any future expansion should coordinate the UI filter with `audio::path_validation`.
- The v2 processing command maps advanced `EncoderSettings` into the existing processing pipeline; when the dedicated encoder panel is enabled, it should call `validate_encoder_settings_cmd` before invoking processing.
