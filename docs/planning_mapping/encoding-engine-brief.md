# Encoding Engine Brief (new_encoder branch)

Last reviewed: 2025-10-11

## Current Focus
- Branch: `new_encoder`
- Goal: ship advanced AAC encoder controls while keeping single ffmpeg-next engine.
- Rust targets: expand `EncoderSettings` plumbing, honor AAC profiles, keep path/metadata/progress guarantees.
- TS targets: surface advanced panel, send v2 payloads, respect platform affordances.

## System Overview
- UI collects files + output settings → `process_audiobook_files_v2` (Tauri) → `ProcessingContext`.
- `MediaProcessingPlan` passes advanced encoder data (`encoder_settings_v2`) to `FfmpegNextProcessor::execute`.
- Processor stages (`prepare`, `execute`, `finalize`) use shared modules (`streams`, `frame_pipeline`, `encoder`, `buffer`, `progress`).
- Supports preview runs (30s) and metadata embedding (native + Lofty fallback).

## Key Contracts
- **Frontend payloads**: `EncoderSettings` (TS) mirrors Rust `EncoderSettings` (bitrate whitelist 56–96 kbps, channel counts 1/2, `EncoderType` `aac_at|he_aac_v1|he_aac_v2`, optional `aac_coder`, `afterburner`, `threads`).
- **Rust validation**: `validate_encoder_settings` enforces bitrate whitelist, HE-AAC v2 stereo, threads range. `resolve_encoder_name` prefers `aac_at` on macOS.
- **Processing flow**: `process_audiobook_files_v2` → map v2 settings to legacy `AudioSettings` (for pipeline) + stash full encoder settings on `ProcessingContext`.
- **Engine execution**: `encoder::create_audio_encoder` maps v2 options to ffmpeg context (profile, coder, threads); fallback for legacy twoloop via env.
- **Progress**: `ProgressEmitter` emits `processing-progress`; frontend listens via `StatusPanel`.

## Supporting Modules
- Validation: `audio::path_validation`, `audio::settings` (bitrate range, .m4b check).
- Buffer safety: `audio::buffer::SampleAccumulator` sanitizes floats, clamps [-1,1].
- Cleanup: `audio::cleanup::CleanupGuard` removes temp dirs.
- Metadata: `metadata::ffmpeg_bridge` (native cover art) + `metadata::writer` fallback.
- Tests: `src-tauri/tests/*` cover path validation, pipeline, cover art, preview.

## External Script Alignment (`shrink.sh`)
- Purpose: evaluate libfdk_aac vs `aac_at`, control VBR/CVBR, chapters, metadata.
- Features: auto merge MP3 dirs, preview toggles, DRY runs, FDK quality levels, Apple CVBR fallback, per-file skip logic for already-low-bitrate mono.
- Params of interest: `ENCODER=auto|fdk|apple`, `FDK_VBR`, `BITRATE`, `CHANNELS`, `THREADS`, preview length, metadata preservation via ffprobe.
- Takeaways for app roadmap:
  - Need encoder availability detection (libfdk_aac vs aac_at) with graceful fallback.
  - VBR (FDK) and CVBR (aac_at) toggles with quality level messaging.
  - Per-title metadata + chapter handling; consider future integration for merge operations.
  - Debug hooks (DRY runs, preview) useful for advanced users; could inspire advanced UI toggles/logging.
  - Thread control and ffmpeg probing to remain best-effort; current pipeline already exposes `ThreadSetting` but doesn’t open multiple jobs.

## Open Questions for Next Agent
1. When should v2 encoder settings graduate from metadata-only to real pipeline behavior (i.e., respect `threads`, `afterburner`, FDK support)?
2. How should UI expose FDK discovery/install guidance (align with script fallback prompt)?
3. What normalization do we want between per-file script workflow and in-app batching (chapter metadata, preview output naming)?
4. Testing: once profiles change actual encode, define sample assets + golden outputs.

