# P1 – Output Settings Not Honored (diagnostic plan)

## Context
- Symptom: User-selected encoder settings (bitrate/encoder/profile/channels) reportedly not reflected in output.
- Data path: Frontend builds `EncoderSettings` → `process_audiobook_files_v2` → `ProcessingContext.encoder_settings_v2` → `MediaProcessingPlan.encoder_settings_v2` → `audio::processor::encoder::create_audio_encoder`.
- Fast path: only skips resample when decoder output matches encoder format (`ABB_DISABLE_FASTPATH=1` to force resample). Not a shell-FFmpeg artifact; lives in the ffmpeg-next frame pipeline.

## Repro plan (manual)
1. Launch with diagnostics: `ABB_DISABLE_FASTPATH=1 RUST_LOG=audiobook_boss=debug npm run tauri dev`.
2. Use a short fixture (e.g., `media/media_20sec.mp3`), set distinctive settings: encoder `he_aac_v1`, `bitrate=96 kbps`, `channels=2`, `aac_coder=twoloop` (native), threads `auto`.
3. Run full encode (or preview). Watch log line from `encoder_setup resolved: ... bitrate=... encoder_settings_v2=...`.
4. Inspect output: `ffprobe -v error -show_entries format=bit_rate -of default=nw=1:nk=1 /path/to/output.m4b` and confirm channel count/profile if possible.

## Hypotheses
- UI is sending coerced defaults (TS mapping clamps or overrides settings).
- Encoder setup ignores v2 payload (bitrate/channels overwritten by legacy `AudioSettings`).
- FFmpeg normalizes bitrate/profile causing mismatch (expected ~target but not exact).

## Next diagnostics
- Add a short Rust integration test to assert `encoder_setup resolved` matches given `EncoderSettings`.
- Add TS-level unit to ensure `toBoundaryEncoderSettings` echoes the requested bitrate/channels/profile.
- If mismatch is from FFmpeg rate control, document expected deltas (e.g., HE-AAC VBR-ish results).

## Exit criteria
- Given settings appear in encoder setup logs and verified via `ffprobe` (bitrate within tolerated delta, channels/profile correct).
- UI contract documented for any coerced values (e.g., HE-AAC v2 forces stereo).
