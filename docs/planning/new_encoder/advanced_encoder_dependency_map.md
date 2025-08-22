# Advanced Encoder UI/Engine — Dependency Map (repo surfaces)

Last updated: 2025-08-22 — See index: `docs/planning/new_encoder/README.md`

Scope: Map every surface that touches audio encoding and the connected frontend/back-end paths to support Outcome 2 (advanced encoder options + UI redesign).

## High-level flow

- Frontend UI collects AudioSettings → invokes Tauri command
- Rust command `process_audiobook_files` builds a `ProcessingContext` and runs ffmpeg-next pipeline
- Pipeline sets up encoder, decodes inputs, resamples, encodes, emits progress, finalizes, embeds metadata/cover art

## Frontend surfaces

- `src/types/audio.ts`
  - Types used by IPC today: `AudioSettings`, `ChannelConfig`, `SampleRateConfig`
  - No concept of encoder selection, AAC coder, afterburner, threads yet
- `src/main.ts`
  - Tauri IPC calls:
    - `validate_audio_settings(settings: AudioSettings)`
    - `process_audiobook_files(filePaths, settings, metadata?)`
- `src/ui/outputPanel.ts`
  - UI controls for bitrate, channels, sample rate, output path
  - Size estimation logic and filename/output-dir rules
  - No advanced encoder controls yet (coder/afterburner/threads/profile placeholder)
- `src/ui/statusPanel.ts`
  - Listens to `processing-progress` events, updates progress and status
- `src/ui/fileImport.ts`, `src/ui/fileList/*`
  - File ingestion and sorting; provides durations for size estimates
- `src/types/events.ts`
  - Event contracts for progress flow from backend → frontend

## Tauri commands / IPC boundary

- `src-tauri/src/commands/audio.rs`
  - `validate_audio_settings(settings: AudioSettings)`
  - `analyze_audio_files(file_paths)`
  - `process_audiobook_files(window, state, file_paths, settings, metadata, preview_seconds)`
    - Constructs `ProcessingContext` with provided `AudioSettings`
    - Calls `audio::processor::process_audiobook_with_context`
- `src-tauri/src/commands/metadata.rs`
  - Metadata read/write commands (Lofty/native embedding coordination)

## Backend core types and validation

- `src-tauri/src/audio/mod.rs`
  - `AudioSettings { bitrate, channels, sample_rate, output_path }` (current contract)
  - `ProcessingStage`, `ProcessingProgress`, `SampleRateConfig`, `ChannelConfig`
- `src-tauri/src/audio/settings.rs`
  - Validation: bitrate 32–128 kbps, sample rates {22050, 32000, 44100, 48000}, `.m4b` output path, writable directory probe
  - Presets: audiobook/high_quality/low_bandwidth
- Gap vs Outcome 2: no `EncoderSettings`/`EncoderType`/`ThreadSetting` yet

## Media pipeline and processor surfaces

- `src-tauri/src/audio/media_pipeline.rs`
  - `MediaProcessingPlan { output_path, settings: AudioSettings, input_file_paths, total_duration }`
  - `FfmpegNextProcessor` implements `MediaProcessor::execute`
- `src-tauri/src/audio/processor/encoder.rs`
  - Creates AAC encoder (fixed to `ff::codec::Id::AAC`)
  - Sets bitrate, rate, channel layout, sample format, time_base
  - Tries `strict=experimental`
  - Optionally sets `aac_coder=twoloop` (env-controlled ABB_DISABLE_TWOOLOOP)
  - Encodes frames, flushes, writes trailer
  - No profile selection (HE-AAC v1/v2), no afterburner, no thread control yet
- `src-tauri/src/audio/processor/streams.rs`
  - Decoder/resampler setup; format negotiation for frame pipeline
- `src-tauri/src/audio/processor/frame_pipeline.rs`
  - Packet processing loop, resampling, frame sizing, PTS handling, progress emission
- `src-tauri/src/audio/processor/prepare.rs` and `finalize.rs`
  - Pre/post hooks (finalize includes trailer and metadata fallbacks if applicable)
- `src-tauri/src/audio/processor/selection.rs`
  - Single-engine alias (`FfmpegNextProcessor`), placeholder for future engines

## Metadata and cover art surfaces

- `src-tauri/src/metadata/mod.rs`
  - Re-exports ffmpeg bridge functions
- `src-tauri/src/metadata/ffmpeg_bridge.rs`
  - `set_container_metadata`, `metadata_to_ffmpeg_dict`
  - Native cover art stream add/write via ffmpeg-next + FFI (ATTACHED_PIC)
  - `validate_metadata_compatibility` warnings, dimension detection
  - Interacts with encoder setup (pre/post header)

## Progress and context surfaces

- `src-tauri/src/audio/context.rs`
  - `ProcessingContext`, cancellation, preview seconds, session id
- `src-tauri/src/audio/progress/*`
  - `ProgressEmitter` emits `processing-progress` to frontend
- `src-tauri/src/audio/session.rs`, `cleanup.rs`
  - Session scoping and output cleanup guard

## Tests touching encoding/processing

- `src-tauri/src/tests_metadata_integration.rs` (unit-ish): ffmpeg metadata dict and cover art pieces
- `src-tauri/tests/*.rs` (integration): cover art embedding, path validation, ffmpegnext integration
- Gaps: No tests for AAC profile selection, channel enforcement for HE-AAC v2, afterburner, threads, or macOS `aac_at` selection/ignores

## External API docs in repo

- `docs/external-apis/ffmpeg-next.md` — ffmpeg-next integration patterns
- `docs/external-apis/lofty.md` — metadata API and safety
- `docs/external-apis/tauri-patterns.md` and `tauri-ts-boundaries.md`
- `docs/external-apis/path-handling.md`

## Summary of change impact zones for Outcome 2

- Frontend: introduce new EncoderSettings types + UI controls, payload wiring
- Commands: expand payload to carry `settings: EncoderSettings` (or augment `AudioSettings`)
- Backend types: add enums/structs for encoder selection, threads
- Encoder setup: map new settings → ffmpeg options (profile, aac_coder, afterburner?, threads)
- Validation: enforce HE-AAC v2 stereo; ignore unsupported options on AAC-AT with logs
- Tests: add unit/integration for new validation and option mappings

---

This map should remain a living document during the Outcome 2 work. Update as new files are introduced. See `README.md` for canonical decisions and cross-links.
