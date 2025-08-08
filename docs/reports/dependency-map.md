# Function & Dependency Map

## Overview
- App type: Tauri desktop app with a TypeScript/Vite frontend and a Rust backend.
- Primary flow: UI (TS) -> Tauri `invoke(...)` -> Rust `#[tauri::command]` -> domain modules (`audio`, `metadata`, `ffmpeg`).
- External runtime: FFmpeg binary (bundled or system); metadata via Lofty.

## Backend (Rust) Modules
- `src-tauri/src/lib.rs`
  - `run()`: Initializes logging, plugins, shared `ProcessingState`, and registers commands.
  - Manages `ProcessingState` (processing/cancel/progress flags) used by long-running tasks.
- `src-tauri/src/commands/mod.rs`
  - `ping`, `echo`, `validate_files`, `get_ffmpeg_version`, `merge_audio_files`.
  - Metadata: `read_audio_metadata`, `write_audio_metadata`, `write_cover_art`, `load_cover_art_file`.
  - Audio: `analyze_audio_files` -> `audio::get_file_list_info`; `validate_audio_settings`; `process_audiobook_files` (async) -> processing pipeline with progress events; `cancel_processing`.
- `src-tauri/src/audio/*`
  - `mod.rs`: Types (`AudioFile`, `AudioSettings`, `ChannelConfig`, `SampleRateConfig`, `ProcessingProgress`, `ProcessingStage`) and re-exports.
  - `file_list.rs`: `get_file_list_info(&[PathBuf]) -> FileListInfo` (validation + size/duration summary).
  - `settings.rs`: `validate_audio_settings(&AudioSettings)` + helpers for bitrate, sample rate, and output path.
  - `processor.rs` and `media_pipeline.rs`: merge/transcode pipeline; `process_audiobook_with_events(...)` emits progress through Tauri window.
  - `progress.rs`, `progress_monitor.rs`, `metrics.rs`, `context.rs`, `session.rs`, `cleanup.rs`: progress emission, ffmpeg process management, cancellation, ETA.
- `src-tauri/src/metadata/*`
  - `reader.rs`: `read_metadata(path)` uses Lofty `Probe`, reads primary/first tag, extracts title/author/album/narrator/year/genre/description/cover.
  - `writer.rs`: `write_metadata(path, &AudiobookMetadata)` updates tags; `write_cover_art(path, &[u8])` writes front cover picture.
- `src-tauri/src/ffmpeg/*`
  - `mod.rs`: `locate_ffmpeg()` search order (bundled, binaries/, PATH, macOS locations); helpers `escape_ffmpeg_path`, `format_concat_file_line`.
  - `command.rs`: `FFmpegCommand` builder: `.add_input`, `.set_output`, `.execute()` dispatches to concat/single; `version()`; internal `parse_version`.
- `src-tauri/src/errors.rs`
  - Central `AppError`/`Result` (mapped from IO/Lofty/FFmpeg errors) used across commands and domains.

## Frontend (TypeScript) Modules
- `src/main.ts`: Wires UI init (`fileImport`, `outputPanel`, `statusPanel`, `coverArt`) and exposes `window.testCommands` mapping to Tauri commands.
- `src/ui/fileImport.ts`: Drag/drop and file selection; calls `analyze_audio_files`; passes results to `displayFileList`.
- `src/ui/fileList.ts`: Renders file list, supports sorting and clearing; may read per-file metadata via `read_audio_metadata`.
- `src/ui/outputPanel.ts`: Manages `AudioSettings`; exports `getCurrentAudioSettings`, and change triggers.
- `src/ui/statusPanel.ts`: Orchestrates processing lifecycle; invokes `process_audiobook_files` and `cancel_processing`; displays progress.
- `src/ui/coverArt.ts`: UI for cover selection; integrates with `write_cover_art` and `load_cover_art_file`.
- `src/types/*`: TS versions of core types (`AudioSettings`, `FileListInfo`, `AudiobookMetadata`, events).

## Cross-Boundary Contracts
- Tauri commands (Rust) are called with `invoke(...)` from TS; data types are serialized with camelCase in TS and serde rename rules in Rust where applicable.
- Long-running tasks emit progress via Tauri window events consumed by `statusPanel`.

## External Dependencies
- Rust: `tauri`, `tauri-plugin-opener`, `tauri-plugin-dialog`, `lofty`, `tokio`, `which`, `anyhow`, `uuid`, `log`, `env_logger`, `thiserror`.
- TS: `vite`, `typescript`, `@tauri-apps/api` and plugins.
- Binary: FFmpeg (bundled in `src-tauri/binaries` for macOS packaging, or discovered via PATH).

## Build/Run Notes
- Dev: `npm run tauri dev` launches Vite (port 1420) and Rust backend.
- Build: `npm run build` for frontend; `tauri build` for app packaging.
- FFmpeg bundling: `npm run setup-ffmpeg` populates `src-tauri/binaries/ffmpeg-universal`; referenced by `tauri.conf.json` `bundle.externalBin`.
