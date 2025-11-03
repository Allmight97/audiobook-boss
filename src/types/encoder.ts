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

import type { EncoderSettings, EncoderType, ThreadSetting } from './audio';
import { defaultEncoderSettings } from './audio';

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

const VALID_ENCODER_BITRATES: ReadonlyArray<EncoderSettings['bitrateKbps']> = [
  56,
  64,
  72,
  80,
  88,
  96,
];

export type EncoderSettingsLike = Partial<EncoderSettingsV2> | EncoderSettings | null | undefined;

const isBoundaryEncoderSettings = (value: unknown): value is EncoderSettings => {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.encoderType === 'string' &&
    typeof candidate.bitrateKbps === 'number' &&
    typeof candidate.channels === 'number'
  );
};

const sanitizeBitrate = (
  value: unknown,
  fallback: EncoderSettings['bitrateKbps'],
): EncoderSettings['bitrateKbps'] => {
  if (typeof value === 'number' && VALID_ENCODER_BITRATES.includes(value as EncoderSettings['bitrateKbps'])) {
    return value as EncoderSettings['bitrateKbps'];
  }
  return fallback;
};

const sanitizeChannels = (
  encoderType: EncoderType,
  value: unknown,
  fallback: EncoderSettings['channels'],
): EncoderSettings['channels'] => {
  if (encoderType === 'he_aac_v2') {
    return 2;
  }
  if (value === 1 || value === 2) {
    return value;
  }
  return fallback;
};

const sanitizeThreads = (value: ThreadSetting | undefined, fallback: ThreadSetting): ThreadSetting => {
  if (!value) {
    return fallback;
  }
  switch (value.mode) {
    case 'auto':
    case 'off':
      return value;
    case 'fixed': {
      const numeric = Number(value.value);
      if (!Number.isFinite(numeric)) {
        return fallback;
      }
      const clamped = Math.max(1, Math.min(1024, Math.round(numeric)));
      return { mode: 'fixed', value: clamped };
    }
    default:
      return fallback;
  }
};

const resolveEncoderType = (
  flavor: EncoderFlavor | undefined,
  profile: AacProfile | undefined,
  fallback: EncoderType,
): EncoderType => {
  switch (flavor) {
    case 'aac_at':
      return 'aac_at';
    case 'external_fdk':
    case 'native_aac':
      if (profile === 'he_v2') {
        return 'he_aac_v2';
      }
      return 'he_aac_v1';
    case 'auto':
    default:
      if (profile === 'he_v2') {
        return 'he_aac_v2';
      }
      if (profile === 'he') {
        return fallback === 'aac_at' ? 'he_aac_v1' : fallback;
      }
      return fallback;
  }
};

const normalizeBoundary = (
  candidate: EncoderSettings,
  base: EncoderSettings,
): EncoderSettings => {
  const encoderType = candidate.encoderType;
  const bitrateKbps = sanitizeBitrate(candidate.bitrateKbps, base.bitrateKbps);
  const channels = sanitizeChannels(encoderType, candidate.channels, base.channels);
  const threads = sanitizeThreads(candidate.threads, base.threads);
  const aacCoder = encoderType === 'aac_at' ? undefined : candidate.aacCoder ?? base.aacCoder;
  const afterburner = encoderType === 'aac_at' ? undefined : candidate.afterburner ?? base.afterburner;

  return {
    encoderType,
    bitrateKbps,
    channels,
    aacCoder,
    afterburner,
    threads,
  };
};

export const toBoundaryEncoderSettings = (
  source: EncoderSettingsLike,
  defaults: EncoderSettings = defaultEncoderSettings(),
): EncoderSettings => {
  const base = normalizeBoundary(defaults, defaultEncoderSettings());

  if (isBoundaryEncoderSettings(source)) {
    return normalizeBoundary(source, base);
  }

  const ui = (source ?? {}) as Partial<EncoderSettingsV2>;
  const encoderType = resolveEncoderType(ui.flavor, ui.profile, base.encoderType);
  const bitrateKbps = sanitizeBitrate(ui.bitrateKbps, base.bitrateKbps);
  const channels = sanitizeChannels(encoderType, ui.channels, base.channels);
  const threads = sanitizeThreads(base.threads, base.threads);
  const aacCoder = encoderType === 'aac_at' ? undefined : base.aacCoder;
  const afterburner = encoderType === 'aac_at' ? undefined : ui.fdkAfterburner ?? base.afterburner;

  return {
    encoderType,
    bitrateKbps,
    channels,
    aacCoder,
    afterburner,
    threads,
  };
};
