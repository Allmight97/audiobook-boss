## Tauri command & event surfaces

This guide expands on the lightweight index by summarizing the public Tauri IPC the app exposes, the Rust modules that implement them, and the UI surfaces that depend on each command or event.

### Command matrix

| Command | Rust implementation | Primary consumers |
| --- | --- | --- |
| `ping`, `echo` | `src-tauri/src/commands/system.rs` | Console smoke tests via `window.testCommands` |
| `validate_files` | `src-tauri/src/commands/audio.rs` → `audio::path_validation` | Integration tests and console harness |
| `analyze_audio_files` | `src-tauri/src/commands/audio.rs` → `audio::file_list` | Drag/drop and picker flows in `src/ui/fileImport` |
| `validate_audio_settings` | `src-tauri/src/commands/audio.rs` → `audio::validate_audio_settings` | Diagnostics through `window.testCommands`; UI does inline validation |
| `validate_encoder_settings_cmd` | `src-tauri/src/commands/audio.rs` → `audio::settings_encoder` | Reserved for advanced encoder UI; no current UI caller |
| `process_audiobook_files` | `src-tauri/src/commands/audio.rs` → `audio::processor::process_audiobook_with_context` | Legacy console fallback; superseded by v2 payload |
| `process_audiobook_files_v2` | `src-tauri/src/commands/audio.rs` (async) | `StatusPanel` start/preview flows, providing EncoderSettings v2 |
| `cancel_processing` | `src-tauri/src/commands/audio.rs` → shared `ProcessingState` | StatusPanel cancel button |
| `read_audio_metadata` | `src-tauri/src/commands/metadata.rs` → `metadata::reader` | File list metadata pane, cover-art thumbnail refresh |
| `write_audio_metadata` | `src-tauri/src/commands/metadata.rs` → `metadata::writer` | Console/testing only |
| `write_cover_art` | `src-tauri/src/commands/metadata.rs` → `metadata::writer::write_cover_art` | Console/testing only |
| `load_cover_art_file` | `src-tauri/src/commands/metadata.rs` → filesystem load + validation | `src/ui/coverArt` "Load Cover Art" button |

### Command payloads & returns

- `analyze_audio_files`
  - Args: `{ filePaths: string[] }`
  - Returns: `FileListInfo` (`src/types/audio.ts:15`)

- `process_audiobook_files_v2`
  - Args: `{ payload, metadata?, previewSeconds? }`
    - `payload: { inputFiles: string[]; outputDir: string; settings: EncoderSettings }`
    - `settings: EncoderSettings` (`src/types/audio.ts:62`)
    - `metadata?: AudiobookMetadata` (`src/types/metadata.ts`)
    - `previewSeconds?: number` (optional short preview)
  - Returns: `{ message: string; previewFilePath?: string; previewActualSeconds?: number }` (`src-tauri/src/commands/audio.rs:215`)
  - Notes:
    - `he_aac_v2` requires `channels: 2` (stereo)
    - Threads mapping: `{mode:'auto'|'off'|'fixed'; value?}` → `threads=0|1|n`

- `cancel_processing`
  - Args: none
  - Returns: `string` confirmation

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

- `processing-progress` (emitted from `src-tauri/src/audio/progress/reporter.rs`) drives the StatusPanel state machine via `src/types/events.ts` contracts and the listener installed in `src/ui/statusPanel`.
  - Emission throttling (~200ms) originates in `src-tauri/src/audio/processor/frame_pipeline.rs`.

### Frontend harness for QA

- `window.testCommands` in `src/main.ts` mirrors each command for manual QA and automated console testing; production UI flows depend on the modules listed above rather than the harness itself.

### Notes on scope

- Backend path validation accepts `wav`/`flac` alongside the UI-visible formats; the importer currently filters to MP3/M4A/M4B/AAC. Any future expansion should coordinate the UI filter with `audio::path_validation`.
- The v2 processing command maps advanced `EncoderSettings` into the existing processing pipeline; when the dedicated encoder panel is enabled, it should call `validate_encoder_settings_cmd` before invoking processing.
