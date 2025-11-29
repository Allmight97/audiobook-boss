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

import type {
  EncoderSettings,
  EncoderType,
  ThreadSetting,
  BitrateKbps,
  BitrateMode,
  EncoderChannelConfig,
} from "./audio";
import { defaultEncoderSettings, VALID_ENCODER_BITRATES } from "./audio";

export type EncoderFlavor =
  | "auto"
  | "aac_at"
  | "fdk_he_aac"
  | "native_aac";
type VbrLevel = Extract<BitrateMode, { mode: "vbr" }>["level"];

export interface VbrSetting {
  enabled: boolean;
  level?: number; // Reserved; disabled now
}

export interface EncoderSettingsV2 {
  flavor: EncoderFlavor;
  bitrateKbps: BitrateKbps;
  bitrateMode?: BitrateMode;
  channels?: EncoderChannelConfig;
  vbr?: VbrSetting;
  fdkAfterburner?: boolean;
  threads?: ThreadSetting;
}

export const defaultEncoderSettingsV2 = (
  isMac: boolean
): EncoderSettingsV2 => ({
  flavor: isMac ? "aac_at" : "native_aac",
  bitrateKbps: 64,
  bitrateMode: isMac ? { mode: "cvbr" } : { mode: "cbr" },
  channels: "auto",
  vbr: { enabled: false },
  fdkAfterburner: false,
});

// VALID_ENCODER_BITRATES imported from audio.ts (single source of truth)

export type EncoderSettingsLike =
  | Partial<EncoderSettingsV2>
  | EncoderSettings
  | null
  | undefined;

const defaultBitrateModeFor = (encoderType: EncoderType): BitrateMode => {
  switch (encoderType) {
    case "fdk_he_aac":
    case "auto":
      return { mode: "vbr", level: 3 };
    case "aac_at":
      return { mode: "cvbr" };
    case "native_aac":
    default:
      return { mode: "cbr" };
  }
};

const isBoundaryEncoderSettings = (
  value: unknown
): value is EncoderSettings => {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Record<string, unknown>;
  const channels = candidate.channels;
  const bitrateMode = candidate.bitrateMode;
  return (
    typeof candidate.encoderType === "string" &&
    typeof candidate.bitrateKbps === "number" &&
    (channels === "auto" || channels === 1 || channels === 2) &&
    typeof bitrateMode === "object" &&
    bitrateMode !== null &&
    typeof (bitrateMode as BitrateMode).mode === "string"
  );
};

const sanitizeBitrate = (
  value: unknown,
  fallback: EncoderSettings["bitrateKbps"]
): EncoderSettings["bitrateKbps"] => {
  if (
    typeof value === "number" &&
    VALID_ENCODER_BITRATES.includes(value as EncoderSettings["bitrateKbps"])
  ) {
    return value as EncoderSettings["bitrateKbps"];
  }
  return fallback;
};

const sanitizeChannels = (
  value: unknown,
  fallback: EncoderSettings["channels"]
): EncoderSettings["channels"] => {
  if (value === "auto" || value === 1 || value === 2) {
    return value;
  }
  return fallback;
};

const sanitizeBitrateMode = (
  value: unknown,
  encoderType: EncoderType,
  fallback: BitrateMode
): BitrateMode => {
  if (
    typeof value === "object" &&
    value !== null &&
    "mode" in (value as BitrateMode)
  ) {
    const candidate = value as BitrateMode;
    if (candidate.mode === "cbr" || candidate.mode === "cvbr") {
      return { mode: candidate.mode };
    }
    if (candidate.mode === "vbr") {
      const numeric = Number((candidate as { level?: number }).level ?? 3);
      if (Number.isFinite(numeric)) {
        const clamped = Math.max(
          1,
          Math.min(5, Math.round(numeric))
        ) as VbrLevel;
        return { mode: "vbr", level: clamped };
      }
      return { mode: "vbr", level: 3 };
    }
  }
  return fallback ?? defaultBitrateModeFor(encoderType);
};

const sanitizeThreads = (
  value: ThreadSetting | undefined,
  fallback: ThreadSetting
): ThreadSetting => {
  if (!value) {
    return fallback;
  }
  switch (value.mode) {
    case "auto":
    case "off":
      return value;
    case "fixed": {
      const numeric = Number(value.value);
      if (!Number.isFinite(numeric)) {
        return fallback;
      }
      const clamped = Math.max(1, Math.min(1024, Math.round(numeric)));
      return { mode: "fixed", value: clamped };
    }
    default:
      return fallback;
  }
};

const resolveEncoderType = (
  flavor: EncoderFlavor | undefined,
  fallback: EncoderType
): EncoderType => {
  switch (flavor) {
    case "auto":
      return "auto";
    case "aac_at":
      return "aac_at";
    case "fdk_he_aac":
      return "fdk_he_aac";
    case "native_aac":
      return "native_aac";
    default:
      return fallback;
  }
};

const normalizeBoundary = (
  candidate: EncoderSettings,
  base: EncoderSettings
): EncoderSettings => {
  const encoderType = candidate.encoderType;
  const bitrateKbps = sanitizeBitrate(candidate.bitrateKbps, base.bitrateKbps);
  const bitrateMode = sanitizeBitrateMode(
    candidate.bitrateMode,
    encoderType,
    base.bitrateMode
  );
  const channels = sanitizeChannels(candidate.channels, base.channels);
  const threads = sanitizeThreads(candidate.threads, base.threads);
  const afterburner =
    encoderType === "fdk_he_aac" ? candidate.afterburner : false;

  return {
    encoderType,
    bitrateKbps,
    bitrateMode,
    channels,
    afterburner,
    threads,
  };
};

export const toBoundaryEncoderSettings = (
  source: EncoderSettingsLike,
  defaults: EncoderSettings = defaultEncoderSettings()
): EncoderSettings => {
  const base = normalizeBoundary(defaults, defaultEncoderSettings());

  if (isBoundaryEncoderSettings(source)) {
    return normalizeBoundary(source, base);
  }

  const ui = (source ?? {}) as Partial<EncoderSettingsV2>;
  const encoderType = resolveEncoderType(ui.flavor, base.encoderType);
  const bitrateKbps = sanitizeBitrate(ui.bitrateKbps, base.bitrateKbps);
  const bitrateMode = sanitizeBitrateMode(
    ui.bitrateMode,
    encoderType,
    base.bitrateMode
  );
  const channels = sanitizeChannels(ui.channels, base.channels);
  const threads = sanitizeThreads(ui.threads ?? base.threads, base.threads);
  const afterburner =
    encoderType === "fdk_he_aac" ? !!ui.fdkAfterburner : false;

  return {
    encoderType,
    bitrateKbps,
    bitrateMode,
    channels,
    afterburner,
    threads,
  };
};
