# Audio Encoding Surface Audit

## Overview
This report surveys all code paths and modules in the repository involved in audio encoding for Audiobook Boss.

## Backend (Rust)
- **`src-tauri/src/audio/processor/encoder.rs`** – configures the AAC encoder, applies `aac_coder=twoloop`, and exposes helpers to set up the output context, send frames, and finalize encoding.
- **`src-tauri/src/audio/processor/frame_pipeline.rs`** – drives decoding, resampling, accumulation and calls `encode_and_write_frame` for each ready chunk, updating progress from encoder timestamps.
- **`src-tauri/src/audio/processor/streams.rs`** – opens decoders and resamplers using the target encoder format and rate.
- **`src-tauri/src/audio/media_pipeline.rs`** – orchestrates per-file processing, delegating to the frame pipeline and encoder helpers.
- **`src-tauri/src/audio/context.rs`** – includes `PreviewConfig` to early‑stop encodes after N seconds.
- **`src-tauri/src/audio/settings.rs`** – validates bitrate, channels, sample rate and output paths; these settings feed encoder creation.
- **`src-tauri/src/commands/audio.rs`** – Tauri command `process_audiobook_files` constructs the processing context, invokes the processor, and returns preview information.

## Frontend (TypeScript)
- **`src/ui/outputPanel.ts`** – collects bitrate, sample rate, and channel selections from the user; current UI has no encoder-type selector.
- **`src/types/audio.ts`** – defines `AudioSettings` (bitrate, channels, sampleRate, outputPath) and presets used by the UI.
- **`src/types/events.ts`** – documents progress event ranges; conversion progress is mapped from encoder timestamps.

## Backend/Frontend Connection
- `process_audiobook_files` command (Rust) accepts `AudioSettings` from the frontend and streams progress events back to the UI.
- Progress percentages in `events.ts` mirror the backend’s `ProcessingStage` and encoder PTS tracking.

## Guidance & Next Steps
1. **Expose encoder choice in settings/UI**
   - Extend `AudioSettings` in Rust and TypeScript with an `encoder_type` enum (e.g., `aac_at`, `he_aac_v1`, `he_aac_v2`).
   - Add selection controls in `outputPanel.ts` and persist through presets.
2. **Backend encoder selection**
   - In `encoder.rs`, choose codec based on `encoder_type` and platform (`aac_at` on macOS, native AAC/HE-AAC elsewhere).
   - Enforce profile-specific constraints (e.g., HE-AAC v2 requires stereo; AAC-AT ignores `aac_coder`).
3. **Validation & fallback**
   - Update `settings.rs` to validate incompatible combinations (channels vs. encoder type, unsupported sample rates).
   - Log clear fallbacks when requested features are unavailable.
4. **Progress and preview**
   - Ensure progress calculations remain based on encoder PTS after adding new encoder variants.
   - Confirm `PreviewConfig` stops encodes cleanly across all encoder types.

## References
- Backend encoder configuration【F:src-tauri/src/audio/processor/encoder.rs†L41-L158】
- Frame pipeline invoking encoder【F:src-tauri/src/audio/processor/frame_pipeline.rs†L59-L146】
- Processing command linking UI to backend【F:src-tauri/src/commands/audio.rs†L55-L135】
- Output panel gathering audio settings【F:src/ui/outputPanel.ts†L1-L119】
- TypeScript audio settings definition【F:src/types/audio.ts†L23-L33】
- Progress event mapping from encoder timestamps【F:src/types/events.ts†L158-L173】

