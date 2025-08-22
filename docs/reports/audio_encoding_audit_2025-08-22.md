# Audio Encoding Surface Audit – 2025-08-22

## Overview
This report surveys the repository for all components involved in audio encoding. Both backend (Rust/Tauri) and frontend (TypeScript) surfaces are cataloged, along with the APIs that bridge them. The goal is to illuminate where encoder logic resides to guide future work such as enforcing HE-AAC or AAC-AT codecs and expanding UI support.

## Backend (Rust/Tauri)
### Encoder configuration and packet writing
- `src-tauri/src/audio/processor/encoder.rs` configures the AAC encoder, enables experimental frame size and optional twoloop optimization, and sanitizes samples before writing packets.
  - `try_configure_variable_frame_size` sets `strict=experimental` to allow variable frame sizes.
  - `try_enable_twoloop_aac` toggles the `aac_coder=twoloop` option for improved psychoacoustics.
  - `create_audio_encoder` applies bitrate, sample rate, channels, and optional global header before opening the encoder.
  - `encode_and_write_frame` clamps samples to [-1,1], replaces non‑finite values, and writes packets.
  - `finalize_encoding` and `finalize_encoding_after_preview` flush remaining packets and write the trailer.

### Frame processing pipeline
- `src-tauri/src/audio/processor/frame_pipeline.rs` decodes packets, resamples when needed, accumulates samples to match encoder frame sizes, and invokes `encode_and_write_frame`.
- `src-tauri/src/audio/processor/streams.rs` opens input files, sets up decoders, and builds resamplers aligned with encoder parameters.

### Buffering utilities
- `src-tauri/src/audio/buffer.rs` provides `SampleAccumulator` to build exact encoder-sized frames with sample sanitization.
- `src-tauri/src/audio/frame_accumulator.rs` offers an alternative accumulator used during earlier refactors.

### Orchestration and settings
- `src-tauri/src/audio/media_pipeline.rs` and `src-tauri/src/audio/processor/mod.rs` orchestrate validation, execution, and finalization of the merge process using the ffmpeg-next engine.
- `src-tauri/src/audio/settings.rs` defines `AudioSettings`, validates bitrate, sample rate, channels, and output path.
- `src-tauri/src/audio/context.rs` carries `ProcessingContext` and `PreviewConfig` for early-stop preview encodes.

### Metadata and cover art
- `src-tauri/src/metadata/ffmpeg_bridge.rs` builds FFmpeg dictionaries for metadata and embeds cover art streams prior to header writing, ensuring proper `attached_pic` disposition.

### API commands
- `src-tauri/src/commands/audio.rs` exposes Tauri commands including `process_audiobook_files`, which constructs a `ProcessingContext` and runs the pipeline with optional preview seconds.
- `src-tauri/src/commands/metadata.rs` provides metadata read/write and cover art helpers.

## Frontend (TypeScript)
### Types and presets
- `src/types/audio.ts` defines `AudioSettings`, `SampleRateConfig`, and presets used by the UI.

### Output configuration
- `src/ui/outputPanel.ts` gathers user selections for bitrate, sample rate, channels, and output directory. `getCurrentAudioSettings()` returns these settings for backend use.

### Processing orchestration
- `src/ui/statusPanel/logic.ts` validates selected files and settings, then invokes `process_audiobook_files` via Tauri. It also listens for progress events.

### File import
- `src/ui/fileImport.ts` filters dropped/selected files to supported formats (MP3, M4A/M4B, AAC) before analysis.

## Backend–Frontend Connection
- The frontend calls Tauri commands (e.g., `process_audiobook_files`) defined in `src-tauri/src/commands/audio.rs` and `src-tauri/src/commands/metadata.rs` using `invoke()`.
- Progress events emitted by the backend follow the contract in `src/types/events.ts` and are handled in `statusPanel/logic.ts`.

## Guidance and Next Steps
1. **Introduce codec selection**
   - Extend `AudioSettings` (both Rust and TS) with an encoder type (e.g., AAC-LC, HE-AAC, AAC-AT).
   - Update `outputPanel.ts` UI to expose codec choices and persist them in `getCurrentAudioSettings()`.
   - Modify `create_audio_encoder` to select appropriate FFmpeg codec IDs or options based on the new setting.
   - Adjust validation logic in `settings.rs` to ensure supported combinations of codec, bitrate, and sample rate.

2. **UI/UX enhancements**
   - Add dropdown or radio buttons for encoder selection and possibly advanced options like variable bitrate.
   - Surface warnings for unsupported presets (e.g., the low-bandwidth preset in `types/audio.ts` uses a 16 kHz rate not accepted by backend validation).

3. **Testing and integration**
   - Add unit and integration tests covering new encoder types and preview flows.
   - Verify sample sanitization and frame accumulation remain correct under different codecs.

4. **Documentation**
   - Update developer guides and API docs (`docs/external-apis/encoder_options.md`, etc.) once new codecs are supported.

This audit should provide sufficient context to begin implementing alternate AAC encoders and corresponding UI controls.
