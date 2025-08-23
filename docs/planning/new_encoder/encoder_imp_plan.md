# Outcome 2 — Advanced Encoder Options + UI Redesign (Expanded Plan)

Last updated: 2025-08-22

This plan consolidates UI and encoder work in one place and reflects the current codebase state, mapping goals to concrete, atomized steps with phases and guardrails.

References:
- Index and canonical decisions: `docs/planning/new_encoder/README.md`
- PR strategy: `docs/planning/new_encoder/PR_strategy.md`
- Dependency map: `docs/planning/new_encoder/advanced_encoder_dependency_map.md`
- External API patterns: `docs/external-apis/ffmpeg-next.md`, `docs/external-apis/lofty.md`, `docs/external-apis/tauri-patterns.md`, `docs/external-apis/tauri-ts-boundaries.md`

## Current state vs goals (delta summary)

- Frontend:
  - Has basic `AudioSettings` (bitrate, channels, sample rate, output path) and output panel controls
  - Lacks encoder selection, coder, afterburner, threads, and profile placeholder UI
- Backend:
  - Uses single engine (ffmpeg-next) with AAC codec
  - Encoder setup sets bitrate/rate/channels and optionally `aac_coder=twoloop` via env opt out
  - No HE-AAC v1/v2 profile selection, no threads control, no AAC-AT selection, no validation for HE-AAC v2 stereo
- Commands/IPC:
  - `process_audiobook_files(window, state, file_paths, settings, metadata, preview_seconds)` expects `AudioSettings`
  - No payload for advanced encoder settings

Conclusion: We need new types and wiring on both sides, staged to avoid breaking existing UI.

## Contracts to introduce (authoritative, phase 1)

TypeScript (frontend):
- Encoder types and payload:
  - `EncoderType = 'aac_at' | 'he_aac_v1' | 'he_aac_v2'`
  - `AacCoder = 'twoloop' | 'fast'`
  - `ThreadMode = 'auto' | 'off' | 'fixed'`
  - `ThreadSetting = { mode: ThreadMode; value?: number }`
  - `EncoderSettings = { encoderType, bitrateKbps: 56|64|72|80|88|96, channels: 1|2, aacCoder?, afterburner?, threads }`
  - `ProcessCommandPayload = { inputFiles: string[]; outputDir: string; settings: EncoderSettings }`

Rust (backend):
- `EncoderType { AacAt, HeAacV1, HeAacV2 }`
- `AacCoder { Twoloop, Fast }`
- `ThreadSetting { Auto, Off, Fixed(u16) }`
- `EncoderSettings { encoder_type, bitrate_kbps, channels, aac_coder: Option<AacCoder>, afterburner: Option<bool>, threads }`

Notes:
- Back-compat: keep existing `AudioSettings` and v1 command intact; add a new v2 command that accepts `EncoderSettings`.
- macOS default: `encoderType = aac_at`.

## Phased implementation plan
Agents: Create a new PR when each phase is complete.

### Phase 0 — Analysis and scaffolding (this PR)
- [x] Dependency map of repo surfaces
- [x] Expanded plan with phases and contracts
- [x] Test gaps report (see `docs/planning/new_encoder/encoding_test_gaps.md`)
- [x] Decide on API path: dual-command v1/v2 vs. extending `AudioSettings` with optional encoder settings
  Decision (canonical): adopt a multi‑PR path with a new v2 command that accepts `EncoderSettings`, keeping the existing v1 command intact initially. Migrate UI to v2 in a later PR, then deprecate v1. See `PR_strategy.md` and `README.md`.

### Phase 1 — Types and validation (no behavior change for v1)
Backend:
- [ ] Add `src-tauri/src/audio/settings_encoder.rs` (new) with enums/structs + serde
- [ ] Add validation helpers: bitrate whitelist, HE-AAC v2 stereo enforcement
- [ ] Implement a small FFI helper to select encoder by name (for `aac_at`); fall back to `Id::AAC`
- [ ] Expose `validate_encoder_settings` tauri command returning `Result<String>`
 - [ ] Feature flag for FDK afterburner: accept `afterburner` in settings but treat as no-op unless FDK encoder is active; return INFO "ignored" log when not active

Frontend:
- [ ] Add `EncoderSettings` types to `src/types/audio.ts` or `src/types/encoder.ts`
- [ ] Lightweight helper to produce default settings per platform

- [ ] Collaborate with user on testing appropriate for the phase.
- [ ] When phase is complete, create a new PR for review and merge.

### Phase 2 — Command upgrade & plumbing
Backend:
- [ ] Add a new v2 command that accepts the payload with `EncoderSettings` and routes into the same pipeline
- [ ] Derive `MediaProcessingPlan` from the v2 payload
- [ ] Wire validation errors for invalid combinations

Frontend:
- [ ] Update `src/main.ts` to send the v2 payload shape
- [ ] Ensure cancellation and progress listeners are unchanged

- [ ] Collaborate with user on testing appropriate for the phase.
- [ ] When phase is complete, create a new PR for review and merge.

### Phase 3 — Encoder mapping in ffmpeg-next
- [ ] In `processor/encoder.rs`:
  - [ ] Choose encoder: `aac_at` when requested (macOS), else native `aac`
  - [ ] Set bitrate (CBR), rate, channels
  - [ ] For HE-AAC v1/v2: `av_opt_set_int(profile=FF_PROFILE_AAC_HE or HE_V2)` best-effort
  - [ ] `aac_coder` mapping: `twoloop` or `fast` via `av_opt_set("aac_coder", ...)` for native AAC
  - [ ] Threads: `ThreadSetting::Auto => threads=0, Off => 1, Fixed(n) => n` best-effort
  - [ ] Afterburner: only applicable to `libfdk_aac` encoders; if unavailable, log INFO "ignored". Guard behind a future feature flag; not part of formal default options.
- [ ] Log one INFO summary line with resolved params and DEBUG for each option set

Caveat: "afterburner" is FDK-specific; native `aac` and `aac_at` won’t honor it. Keep UI gated accordingly.

- [ ] Collaborate with user on testing appropriate for the phase.
- [ ] When phase is complete, create a new PR for review and merge.

### Phase 4 — UI redesign and UX rules
- [ ] New `EncoderPanel` with:
  - [ ] Encoder selection (AAC-AT / HE-AAC v1 / HE-AAC v2)
  - [ ] Bitrate, Channels controls
  - [ ] Advanced accordion: coder, afterburner, threads
  - [ ] Disabled states and tooltips for unsupported combos
  - [ ] Profile selector placeholder (disabled)
- [ ] macOS warning dialog when switching away from AAC-AT
- [ ] HE-AAC v2 forces `channels=2` with inline helper text and a toast explaining: "HE-AAC v2 is stereo-only (Parametric Stereo). For mono, use HE-AAC v1"
- [ ] Size estimate uses chosen bitrate and channels (pass-through sample rate display-only)
- [ ] Collaborate with user on testing appropriate for the phase.
- [ ] When phase is complete, create a new PR for review and merge.

### Phase 5 — Tests and docs
- [ ] Unit tests: validation (HEv2 stereo), default resolution, option mapping returns
- [ ] Integration tests: verify profiles, channel enforcement, logs contain summary
- [ ] Update docs: `external-apis/ffmpeg-next.md` with profile/threads specifics; UI help text

## Risk register / open questions
- Afterburner availability depends on `libfdk_aac` encoder; treat as a future optional feature flag. If not present, mark as N/A/ignored with clear UX. Implementation note: include commented-out stubs for FDK afterburner option so there’s zero runtime impact for now.
- Selecting `aac_at` requires encoder-by-name; confirm `ffmpeg-next` name-based encoder lookup or add FFI wrapper.
- HE-AAC profile flags: confirm integer constants for FF_PROFILE_AAC_HE and HE_V2 in the linked FFmpeg headers.
- Cross-platform support: AAC-AT is macOS-only; ensure the UI defaults and backend guards reflect this.

## Acceptance criteria (phase 1-3)
- Validation rejects HE-AAC v2 with mono channels and surfaces a user-facing error
- Encoder summary log prints selected encoder, bitrate, channels, profile (if any), threads, and ignored/unsupported flags
- On macOS, choosing AAC-AT results in native `aac_at` encoder being used
