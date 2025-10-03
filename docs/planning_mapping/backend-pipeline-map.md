# Backend Pipeline Map (Rust)

## Command Layer
- `src-tauri/src/commands/audio.rs`
  - `process_audiobook_files_v2` (async) – v2 entrypoint; validates `EncoderSettings`, converts to legacy `AudioSettings`, attaches v2 settings to `ProcessingContext`; handles preview seconds, metadata payload, session state, preview output naming.
  - `process_audiobook_files` – legacy path (same pipeline without v2 context).
  - Shared helpers: `derive_v1_settings_from_v2`, `validate_encoder_settings_cmd`, `validate_audio_settings`, `validate_files`, `analyze_audio_files`.
  - Dependencies: `audio::get_file_list_info`, `audio::session`, `ProcessingState`, `tauri::Window` for progress.

## Core Types & Validation
- `audio::AudioSettings`, `ChannelConfig`, `SampleRateConfig` (whitelists, `.m4b` check via `audio::settings`).
- `audio::settings_encoder::EncoderSettings` (bitrate whitelist 56–96 kbps, stereo-only HE-AAC v2, thread range 1–1024).
- `audio::path_validation` – canonicalizes inputs, enforces extension whitelist (mp3/m4a/m4b/aac/wav/flac), symlink logging.
- `audio::file_list` – builds `AudioFile` structs using Lofty for metadata.

## ProcessingContext & Workflow
- `audio::context::ProcessingContext` stores window, session, v1 settings, optional `encoder_settings_v2`, preview config.
- `audio::processor::ProcessingWorkflow` (temp dir + total duration) handed through stages.
- Session & cleanup: `audio::session::ProcessingSession`, `audio::cleanup::CleanupGuard`.

## Stage Breakdown
1. **prepare (`audio/processor/prepare.rs`)**
   - `validate_and_prepare` → `validate_inputs_with_progress` (progress stage=Analyzing) + workspace setup.
   - Detects total duration, ensures temp dir exists, re-validates cancellation.

2. **execute (`audio/processor/execute.rs`)**
   - `execute_processing` → `merge_audio_files_with_context`.
   - Builds `MediaProcessingPlan` (carries v2 settings) and invokes `create_default_processor()` (ffmpeg-next engine).
   - Handles progress stage=Converting, cancellation early exit.

3. **finalize (`audio/processor/finalize.rs`)**
   - `write_metadata_stage` (emit progress, native cover art check via Lofty fallback).
   - `complete_processing` (move temp → final path, cleanup temp dir, emit completion).
   - Preview mode: writes `<stem>.preview.m4b` and skips metrics summary.

## Media Pipeline (`audio/media_pipeline.rs`)
- `MediaProcessingPlan` (output path, AudioSettings, input paths, total duration, optional encoder v2).
- `MediaProcessor` trait; only implementation is `FfmpegNextProcessor`.
- `execute` flow:
  1. Initialize ffmpeg (once).
  2. `encoder::setup_encoder` → returns output context, encoder, stream index, time base, target sample rate. Maps v2 settings:
     - `EncoderType::AacAt` → `aac_at` if available else `aac`.
     - HE-AAC profiles via `profile` opt (ffmpeg constants).
     - `aac_coder` option; logs if unsupported.
     - Threads (`ThreadSetting::Auto|Off|Fixed`).
     - Afterburner logged (ignored unless future FDK).
     - Legacy env `ABB_DISABLE_TWOOLOOP` fallback if no v2 settings.
  3. Build `FramePipelineCtx` for progress/resample state.
  4. For each input file: `streams::setup_decoder_and_resampler` (ffmpeg contexts, logs format) → `frame_pipeline::process_input_packets`.
  5. Accumulation & encoding: `buffer::SampleAccumulator` ensures frame size, clamps samples; `frame_pipeline::process_decoded_frames` handles fast-path vs resample, sets PTS, regards preview early-stop and cancellation.
  6. `encoder::finalize_encoding_after_preview` flushes encoder, writes trailer.
  7. Cleanup guard removes path on success.

## Progress & Metrics
- `audio::progress::ProgressEmitter` – emits `processing-progress` events (Analyzing, Converting, WritingMetadata, Completed, Cancelled).
- `audio::processor::ProgressReporter` (newtype) used for stage transitions + final `complete()`.
- `audio::metrics::ProcessingMetrics` collects duration + bytes (used in orchestrator log; suppressed for preview).

## Metadata Integration
- `metadata::ffmpeg_bridge` – container metadata + native cover art attachments (pre/post header) used inside `encoder::setup_encoder` and finalize stage.
- `metadata::writer` – Lofty fallback for cover art, invoked if native embedding absent.
- `metadata::AudiobookMetadata` shared across commands.

## Cancellation & Preview
- Commands set `ProcessingState` flags; `ProcessingContext::is_cancelled()` checked in prepare, execute, frame loops.
- Preview requests (command env or explicit) configure `context.preview`; `frame_pipeline` stops when elapsed seconds ≥ preview target; finalize writes preview file.

## Tests & Tooling
- Integration tests: `src-tauri/tests/*` (path validation, cover art, ffmpeg pipeline, preview).
- Unit tests: `settings_encoder.rs`, `path_validation.rs`, `buffer.rs`, etc.
- Required checks pre-commit: `cargo fmt --all -- --check`, `cargo clippy -- -D warnings`, `cargo test` (per `AGENTS.md`).

## External Dependencies
- `ffmpeg-next` 7 – encode/decode/resample.
- `lofty` 0.20 – metadata parsing/writing.
- `tauri` 2 – IPC, window events.
- Logging: `log`, `env_logger`.

## Future Hooks
- FDK support: would require external binary invocation or bundling; current pipeline expects ffmpeg-next encoders; consider `MediaProcessor` abstraction for shell fallback if needed.
- VBR/afterburner: existing v2 types allow storing intent; encoder mapping needs extension once feature toggles enabled.
- Thread control: `threads=0|1|n` already wired; confirm ffmpeg-next exposes multi-thread benefits for AAC.
- Merge/chapters: script’s chapter metadata hints at future pipeline extension (today pipeline concatenates via decode/encode, not ffmpeg concat metadata).

