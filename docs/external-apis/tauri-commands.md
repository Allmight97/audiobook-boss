## Tauri command & event surfaces

This guide expands on the lightweight index by summarizing the public Tauri IPC the app exposes, the Rust modules that implement them, and the UI surfaces that depend on each command or event.

### Command matrix

| Command | Rust implementation | Primary consumers |
| --- | --- | --- |
| `ping`, `echo` | `src-tauri/src/commands/system.rs` | Console smoke tests via `window.testCommands` |
| `validate_files` | `src-tauri/src/commands/audio.rs` → `audio::path_validation` | Integration tests and console harness |
| `analyze_audio_files` | `src-tauri/src/commands/audio.rs` → `audio::file_list` | Drag/drop and picker flows in `src/ui/fileImport` |
| `validate_encoder_settings_cmd` | `src-tauri/src/commands/audio.rs` → `audio::settings_encoder` | Reserved for advanced encoder UI; no current UI caller |
| `process_audiobook_files_v2` | `src-tauri/src/commands/audio.rs` (async) | `StatusPanel` start/preview flows, providing EncoderSettings v2 |
| `cancel_processing` | `src-tauri/src/commands/audio.rs` → shared `ProcessingState` + `JobRegistry` | StatusPanel cancel-all and per-job cancel |
| `list_available_encoders` | `src-tauri/src/commands/audio.rs` | Used by UI to surface encoder guidance |
| `get_max_concurrent_jobs` | `src-tauri/src/commands/audio.rs` → `JobRegistry` | StatusPanel “Max concurrent conversions” selector |
| `set_max_concurrent_jobs` | `src-tauri/src/commands/audio.rs` → `JobRegistry` | StatusPanel “Max concurrent conversions” selector |
| `read_audio_metadata` | `src-tauri/src/commands/metadata.rs` → `metadata::reader` | File list metadata pane, cover-art thumbnail refresh |
| `save_metadata_to_file` | `src-tauri/src/commands/metadata.rs` | Metadata-only editing (Cmd+S workflow) |
| `write_audio_metadata` | `src-tauri/src/commands/metadata.rs` → `metadata::ffmpeg_bridge::rewrite_metadata_with_ffmpeg` | Console/testing only |
| `write_cover_art` | `src-tauri/src/commands/metadata.rs` → `metadata::ffmpeg_bridge::rewrite_metadata_with_ffmpeg` | Console/testing only |
| `load_cover_art_file` | `src-tauri/src/commands/metadata.rs` → filesystem load + validation | `src/ui/coverArt` "Load Cover Art" button |

### Command payloads & returns

- `analyze_audio_files`
  - Args: `{ filePaths: string[] }`
  - Returns: `FileListInfo` (`src/types/audio.ts:15`)

- `process_audiobook_files_v2`
  - Args: `{ payload, metadata?, previewSeconds? }`
    - `payload: { inputFiles: string[]; outputDir: string; settings: EncoderSettings; sampleRate?: SampleRateConfig; jobType?: 'Merge' | 'Batch'; useSubdirPattern?: boolean; filenamePattern?: 'title_year' | 'author_title' }`
    - `settings: EncoderSettings` (`src/types/audio.ts:62`)
    - `metadata?: AudiobookMetadata` (`src/types/metadata.ts`)
    - `previewSeconds?: number` (optional short preview)
  - Returns: `ProcessCommandResult` (`{ message: string; previewFilePath?: string; previewActualSeconds?: number; jobId: string }`)
  - Notes:
    - Threads mapping: `{mode:'auto'|'off'|'fixed'; value?}` → `threads=0|1|n`
    - Emits `processing-progress` events with `job_id` (optional in payload for backward compatibility)

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

- `write_audio_metadata`
  - Args: `{ filePath: string; metadata: AudiobookMetadata }`
  - Returns: `void`

- `load_cover_art_file`
  - Args: `{ filePath: string }`
  - Returns: `number[]` (bytes); basic header validation for JPG/PNG/WebP

### Contract sources

- Command and data types: `src/types/audio.ts`, `src/types/metadata.ts`
- Event contracts: `src/types/events.ts`

### Backend → frontend events

- `processing-progress` (emitted from `src-tauri/src/audio/progress/reporter.rs`) drives the StatusPanel state machine via `src/types/events.ts` contracts and the listener installed in `src/ui/statusPanel`. Payload includes optional `job_id` to support multiple concurrent jobs.
  - Emission throttling (~200ms) originates in `src-tauri/src/audio/processor/frame_pipeline.rs`.

### Frontend harness for QA

- `window.testCommands` in `src/main.ts` exposes select commands for manual QA (ping/echo/validation/metadata); production UI flows depend on the modules listed above rather than the harness itself.

### Notes on scope

- Backend path validation accepts `wav`/`flac` alongside the UI-visible formats; the importer currently filters to MP3/M4A/M4B/AAC. Any future expansion should coordinate the UI filter with `audio::path_validation`.
- The v2 processing command maps advanced `EncoderSettings` into the existing processing pipeline; when the dedicated encoder panel is enabled, it should call `validate_encoder_settings_cmd` before invoking processing.
