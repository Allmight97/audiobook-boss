// UI-only encoder v2 types (frontend)
//
// Important:
// - This file models richer UI state for future encoder options.
// - The canonical Tauri boundary type is `EncoderSettings` in `src/types/audio.ts`.
// - When invoking backend commands, map this UI shape to the boundary type to
//   avoid contract drift with Rust (`src-tauri/src/audio/settings_encoder.rs`).
// - Do not serialize this shape directly across the IPC boundary.
//
// VBR/FDK fields remain reserved/disabled in this phase.
// FEATURE_TOGGLE:VBR  VBR_DISABLED_MARKER
// FEATURE_TOGGLE:FDK  FDK_PLACEHOLDER

export type EncoderFlavor = 'auto' | 'aac_at' | 'external_fdk' | 'native_aac';
export type AacProfile = 'lc' | 'he' | 'he_v2';

export interface VbrSetting {
  enabled: boolean;
  level?: number; // Reserved; disabled now
}

export interface EncoderSettingsV2 {
  flavor: EncoderFlavor;
  bitrateKbps: 64 | 72 | 80 | 88 | 96;
  channels: 1 | 2;
  profile?: AacProfile;                 // hidden/ignored for native
  vbr?: VbrSetting;                     // Reserved; disabled now; backend ignores
  fdkAfterburner?: boolean;             // Reserved; disabled now
  optimizeLcLowBitrate?: boolean;       // Native only; suggests 32 kHz at ≤ 64 kbps
  externalFfmpegPath?: string;          // Future FDK only
}

export const defaultEncoderSettingsV2 = (isMac: boolean): EncoderSettingsV2 => ({
  flavor: isMac ? 'aac_at' : 'native_aac',
  bitrateKbps: 64,
  channels: 1,
  profile: isMac ? 'he' : undefined,
  vbr: { enabled: false },
  fdkAfterburner: false,
  optimizeLcLowBitrate: true,
});

