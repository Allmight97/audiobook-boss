# Outcome 2 — Advanced Encoder Options + UI Redesign (macOS‑first, modular)

Context anchors:
- ffmpeg‑next only, single engine; no shell FFmpeg.
- macOS priority: prefer `aac_at`; fallback to native AAC elsewhere (future).
- Respect repo constraints: small functions, no `unwrap`, feature‑gated enhancements, façade via `audio/processor/*`.
- Keep fast‑path disabled by default during trials.

## Goals
- Expose advanced encoder options to allow manual validation of each toggle.
- Redesign UI to host these controls cleanly, plus a placeholder for a future profile selector (Quality / Balance / Performance).
- Ensure each option propagates to the encoder with graceful fallback and clear logging.

## Advanced options (phase 1)
- Platform encoder selection (implicit): macOS → `aac_at`; others → native AAC (HE-AAC v1 or v2)
- Bitrate: 56k, 64k, 72k, 80k, 88k, 96k (CBR default for HE‑AAC and AAC‑AT).
- Channels: 1 or 2 (always honored independent of sample‑rate mode).
- AAC coder (native AAC only): `twoloop` | `fast` (with fallback if unsupported).
- Afterburner (native AAC only): on/off (fallback if unsupported).
- Threads: `auto` | `off` | specific number (best‑effort on encoder context, fallback gracefully).
- Audio mode: `CBR` (default); VBR hidden for now for HE‑AAC/AAC‑AT compatibility.
- Sample rate: pass‑through from input files; display-only in UI.

## UI redesign (phase 1)
- Encoder panel
  - Encoder AAC-AT/HE-AAC v1/v2 (defaults to 'AAC-AT' on macOS)
  - Bitrate dropdown.
  - Channels dropdown.
  - Advanced section (accordion):
    - AAC coder (twoloop/fast) [disabled on macOS `aac_at`].
    - Afterburner toggle [disabled on macOS `aac_at`].
    - Threads control: `Auto` / `Off` / `N` (numeric input).
  - Profile selector placeholder (disabled): Quality / Balance / Performance.
- Contextual help tooltips for each option (1‑line, concise).

## Encoder modes and constraints (mapping)
- Encoder type semantics
  - **AAC‑AT (macOS)**: Higher quality at low bitrates; private native flags unsupported. Enforce CBR.
  - **HE‑AAC v1 (native)**: Mono or stereo allowed. CBR. Supports `aac_coder`, `afterburner`, `threads`.
  - **HE‑AAC v2 (native)**: Stereo‑only due to Parametric Stereo. CBR. Supports `aac_coder`, `afterburner`, `threads`.
- Invalid combinations and enforcement
  - If `encoderType=he_aac_v2`, the UI locks `channels` to `2`. Backend validates and returns a user‑facing validation error if `channels != 2`.
  - If `encoderType=aac_at`, ignore `aac_coder`/`afterburner` values with INFO log “ignored for aac_at”.
  - VBR is not exposed (and ignored) for all three in phase 1.
- Best‑effort threads: set encoder context `threads` when available; log fallback if unsupported.

## Data contracts (authoritative)

### TypeScript (frontend payload)
```ts
export type EncoderType = 'aac_at' | 'he_aac_v1' | 'he_aac_v2';
export type AacCoder = 'twoloop' | 'fast';
export type ThreadMode = 'auto' | 'off' | 'fixed';

export interface ThreadSetting {
  mode: ThreadMode;
  value?: number; // required iff mode === 'fixed'
}

export interface EncoderSettings {
  encoderType: EncoderType;           // default: 'aac_at' on macOS
  bitrateKbps: 56 | 64 | 72 | 80 | 88 | 96;
  channels: 1 | 2;                    // if he_aac_v2 → coerced to 2
  aacCoder?: AacCoder;                // ignored for aac_at
  afterburner?: boolean;              // ignored for aac_at
  threads: ThreadSetting;             // best‑effort for aac_at
}

export interface ProcessCommandPayload {
  inputFiles: string[];
  outputDir: string;
  settings: EncoderSettings;
}
```

### Rust (backend settings)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncoderType { AacAt, HeAacV1, HeAacV2 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AacCoder { Twoloop, Fast }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreadSetting { Auto, Off, Fixed(u16) }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderSettings {
  pub encoder_type: EncoderType,   // default per target_os
  pub bitrate_kbps: u32,
  pub channels: u16,
  pub aac_coder: Option<AacCoder>,
  pub afterburner: Option<bool>,
  pub threads: ThreadSetting,
}
```

## Backend mapping (how settings apply)
- `encoder_type`
  - `AacAt` (macOS): select `aac_at` codec; do not set `aac_coder`/`afterburner`. Set bitrate, channels. Attempt `threads` if supported.
  - `HeAacV1`: select native `aac` and set profile to HE (if available), or rely on `-profile:a aac_he` via `av_opt_set_int("profile", FF_PROFILE_AAC_HE)`. Apply `aac_coder` and `afterburner` via `av_opt_set` (best‑effort); set bitrate, channels, threads.
  - `HeAacV2`: same as v1 but set profile HEv2 (`FF_PROFILE_AAC_HE_V2`) and require `channels=2` (coerce and log if needed).
- `bitrate_kbps`: set CBR target on codec context.
- `channels`: always honored; for HE‑AAC v2 enforce 2.
- `threads`: `ThreadSetting::Auto` → `threads=0`; `Off` → `threads=1`; `Fixed(n)` → `threads=n` (best‑effort log if ignored).
- Logging: one INFO summary line with resolved params; DEBUG entries for each `av_opt_set` attempt and any fallbacks.

## UI behavior rules
- Default encoder on macOS: `AAC‑AT`.
- On macOS, when switching away from `AAC‑AT` to a native HE‑AAC option, show a confirmation dialog: “AAC‑AT is recommended for best quality at low bitrates on macOS. Continue switching?” If cancelled, revert to `AAC‑AT`.
- When selecting `HE‑AAC v2`, lock `channels` to `2` (disable the control) and show inline helper text: “HE‑AAC v2 improves stereo compression and is not intended for mono.”
- When selecting `HE‑AAC v1`, default `channels` to `1` (mono) if not previously set; keep the control editable.
- Disable `AAC coder` and `Afterburner` controls when `AAC‑AT` is selected; show tooltip “Not applicable to AAC‑AT on macOS”.
- Threads: allow Auto/Off/Fixed; validate `Fixed` numeric input ≥ 1.
- Profile selector placeholder visible but disabled.

## Atomized implementation checklist

### Backend (Rust)
1) `src-tauri/src/audio/settings.rs`
   - Add `EncoderType`, `AacCoder`, `ThreadSetting`, `EncoderSettings` with serde derives and defaults (macOS → `AacAt`).
2) `src-tauri/src/commands/audio.rs`
   - Extend command payload to include `settings: EncoderSettings`.
   - Validate invalid combos (e.g., `HeAacV2` + `channels!=2`) and return a user‑facing validation error; do not coerce.
3) `src-tauri/src/audio/processor/selection.rs`
   - Resolve final `EncoderSettings` (apply platform defaults if missing).
4) `src-tauri/src/audio/processor/encoder.rs`
   - Select codec by `encoder_type`.
   - Apply bitrate/channels on codec context.
   - `HeAacV1/V2`: try `profile` via `av_opt_set_int`; apply `aac_coder` and `afterburner` via `av_opt_set`; apply `threads` best‑effort; INFO summary log.
   - `AacAt`: ignore native flags; attempt threads; INFO summary log.
5) `src-tauri/src/audio/processor/prepare.rs`
   - Ensure channel setting is honored regardless of sample‑rate mode; sample rate pass‑through preserved.
6) Tests (`src-tauri/tests/unit/ffmpeg/` or integration)
   - Verify validation error for HE‑AAC v2 with `channels != 2`.
   - Verify INFO summary contains resolved params.

### Frontend (TypeScript)
1) `src/types/audio.ts`
   - Add `EncoderType`, `AacCoder`, `ThreadSetting`, `EncoderSettings`, `ProcessCommandPayload` types.
2) `src/ui/outputPanel.ts` (or new `src/ui/encoderPanel.ts`)
   - Add encoder selector (AAC‑AT / HE‑AAC v1 / HE‑AAC v2).
   - Add bitrate and channels dropdowns.
   - Add Advanced accordion: coder, afterburner, threads.
   - Wire change handlers to update a module‑level `currentEncoderSettings` and command payload.
   - Disable/enable controls per rules above and show tooltips.
3) `src/main.ts`
   - Ensure the processing command includes `settings` from UI state.
4) UX polish
   - Show inline helper text for HE‑AAC v2 stereo requirement; channel control disabled when v2 is active.

## Validation plan
- Manual A/B using preview or short clips
  - twoloop vs fast at 64/72/80k, mono/stereo.
  - afterburner on/off.
  - threads auto vs fixed.
- Logs confirm applied settings and any ignored flags.

## Acceptance criteria
- Each advanced toggle changes output as expected on macOS where applicable; ignored gracefully on `aac_at`.
- Channels and bitrate honored; HE‑AAC v2 enforces stereo with clear UX.
- UI shows disabled states/tooltips appropriately; profile placeholder visible.
 - On macOS, switching away from `AAC‑AT` triggers a confirmation warning.

## Open questions
- On macOS, do we allow users to select HE‑AAC v1/v2 (native) explicitly, or should we warn that AAC‑AT is generally preferred? Plan currently allows explicit selection.
- For HE‑AAC v2 mono requests, do you prefer auto‑coercion to stereo (current plan) or a blocking validation error?
