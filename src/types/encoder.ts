// UI-only encoder settings types (frontend)
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
	ExternalToolchainPreference,
	ThreadSetting,
	BitrateKbps,
	BitrateMode,
	EncoderChannelConfig,
} from './audio';
import { defaultEncoderSettings, VALID_ENCODER_BITRATES } from './audio';

export type EncoderFlavor = EncoderType;
type VbrLevel = 1 | 2 | 3 | 4 | 5;

const BOUNDARY_ENCODER_TYPES = {
	auto: true,
	aac_at: true,
	fdk_he_aac: true,
	native_aac: true,
} satisfies Record<EncoderType, true>;

export interface VbrSetting {
	enabled: boolean;
	level?: number; // Reserved; disabled now
}

export interface EncoderSettingsState {
	flavor: EncoderFlavor;
	bitrateKbps: BitrateKbps;
	bitrateMode?: BitrateMode;
	channels?: EncoderChannelConfig;
	externalToolchain?: ExternalToolchainPreference;
	vbr?: VbrSetting;
	fdkAfterburner?: boolean;
	threads?: ThreadSetting;
	twoloop?: boolean;
}

export const defaultEncoderSettingsState = (_isMac: boolean): EncoderSettingsState => ({
	flavor: 'auto',
	bitrateKbps: 64,
	bitrateMode: { mode: 'vbr', value: 3 },
	channels: 'auto',
	vbr: { enabled: true, level: 3 },
	fdkAfterburner: true,
	twoloop: true,
});

// VALID_ENCODER_BITRATES imported from audio.ts (single source of truth)

export type EncoderSettingsLike =
	| Partial<EncoderSettingsState>
	| EncoderSettings
	| null
	| undefined;

const DEFAULT_BITRATE_MODE_BY_ENCODER = {
	auto: (): BitrateMode => ({ mode: 'vbr', value: 3 }),
	aac_at: (): BitrateMode => ({ mode: 'cvbr' }),
	fdk_he_aac: (): BitrateMode => ({ mode: 'vbr', value: 3 }),
	native_aac: (): BitrateMode => ({ mode: 'cbr' }),
} satisfies Record<EncoderType, () => BitrateMode>;

const defaultBitrateModeFor = (encoderType: EncoderType): BitrateMode =>
	DEFAULT_BITRATE_MODE_BY_ENCODER[encoderType]();

const isBitrateMode = (value: unknown): value is BitrateMode => {
	if (!value || typeof value !== 'object') {
		return false;
	}
	const candidate = value as Record<string, unknown>;
	if (candidate.mode === 'cbr' || candidate.mode === 'cvbr') {
		return true;
	}
	if (candidate.mode !== 'vbr') {
		return false;
	}
	return typeof candidate.value === 'number';
};

const isThreadSetting = (value: unknown): value is ThreadSetting => {
	if (!value || typeof value !== 'object') {
		return false;
	}
	const candidate = value as Record<string, unknown>;
	if (candidate.mode === 'auto' || candidate.mode === 'off') {
		return true;
	}
	if (candidate.mode !== 'fixed') {
		return false;
	}
	return typeof candidate.value === 'number';
};

const isBoundaryEncoderType = (value: unknown): value is EncoderType =>
	typeof value === 'string' && value in BOUNDARY_ENCODER_TYPES;

const isBoundaryEncoderSettings = (value: unknown): value is EncoderSettings => {
	if (!value || typeof value !== 'object') {
		return false;
	}
	const candidate = value as Record<string, unknown>;
	const channels = candidate.channels;
	const bitrateMode = candidate.bitrateMode;
	return (
		isBoundaryEncoderType(candidate.encoderType) &&
		typeof candidate.bitrateKbps === 'number' &&
		(channels === 'auto' || channels === 'mono' || channels === 'stereo') &&
		typeof candidate.afterburner === 'boolean' &&
		isThreadSetting(candidate.threads) &&
		isBitrateMode(bitrateMode) &&
		(candidate.twoloop === undefined || typeof candidate.twoloop === 'boolean')
	);
};

const sanitizeBitrate = (
	value: unknown,
	fallback: EncoderSettings['bitrateKbps'],
): EncoderSettings['bitrateKbps'] => {
	if (typeof value === 'number' && VALID_ENCODER_BITRATES.includes(value as BitrateKbps)) {
		return value as BitrateKbps;
	}
	return fallback;
};

const sanitizeChannels = (
	value: unknown,
	fallback: EncoderSettings['channels'],
): EncoderSettings['channels'] => {
	if (value === 'auto' || value === 'mono' || value === 'stereo') {
		return value;
	}
	return fallback;
};

const sanitizeBitrateMode = (
	value: unknown,
	encoderType: EncoderType,
	fallback: BitrateMode,
): BitrateMode => {
	if (isBitrateMode(value)) {
		if (value.mode === 'cbr' || value.mode === 'cvbr') {
			return { mode: value.mode };
		}
		const numeric = Number(value.value ?? 3);
		if (Number.isFinite(numeric)) {
			const clamped = Math.max(1, Math.min(5, Math.round(numeric))) as VbrLevel;
			return { mode: 'vbr', value: clamped };
		}
		return { mode: 'vbr', value: 3 };
	}
	return fallback ?? defaultBitrateModeFor(encoderType);
};

const sanitizeThreads = (value: unknown, fallback: ThreadSetting): ThreadSetting => {
	if (!isThreadSetting(value)) {
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
	fallback: EncoderType,
): EncoderType => (isBoundaryEncoderType(flavor) ? flavor : fallback);

const normalizeBoundary = (candidate: EncoderSettings, base: EncoderSettings): EncoderSettings => {
	const encoderType = candidate.encoderType;
	const bitrateKbps = sanitizeBitrate(candidate.bitrateKbps, base.bitrateKbps);
	const bitrateMode = sanitizeBitrateMode(candidate.bitrateMode, encoderType, base.bitrateMode);
	const channels = sanitizeChannels(candidate.channels, base.channels);
	const threads = sanitizeThreads(candidate.threads, base.threads);
	const afterburner =
		encoderType === 'fdk_he_aac' || encoderType === 'auto'
			? typeof candidate.afterburner === 'boolean'
				? candidate.afterburner
				: base.afterburner
			: false;

	return {
		encoderType,
		bitrateKbps,
		bitrateMode,
		channels,
		afterburner,
		threads,
		...(candidate.twoloop === undefined ? {} : { twoloop: candidate.twoloop }),
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

	const ui = (source ?? {}) as Partial<EncoderSettingsState>;
	const encoderType = resolveEncoderType(ui.flavor, base.encoderType);
	const bitrateKbps = sanitizeBitrate(ui.bitrateKbps, base.bitrateKbps);
	const bitrateMode = sanitizeBitrateMode(ui.bitrateMode, encoderType, base.bitrateMode);
	const channels = sanitizeChannels(ui.channels, base.channels);
	const threads = sanitizeThreads(ui.threads ?? base.threads, base.threads);
	const afterburner =
		encoderType === 'fdk_he_aac' || encoderType === 'auto'
			? (ui.fdkAfterburner ?? base.afterburner)
			: false;

	return {
		encoderType,
		bitrateKbps,
		bitrateMode,
		channels,
		afterburner,
		threads,
		twoloop: ui.twoloop ?? true,
	};
};
