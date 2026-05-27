// UI-only encoder settings types (frontend)
//
// Important:
// - This file models richer UI state for future encoder options.
// - The canonical Tauri boundary type is `EncoderSettings` in `src/types/audio.ts`.
// - When invoking backend commands, map this UI shape to the boundary type to
//   avoid contract drift with Rust (`src-tauri/src/audio/settings_encoder.rs`).
// - Do not serialize this shape directly across the IPC boundary.
//
import type {
	EncoderSettings,
	EncoderType,
	ExternalToolchainPreference,
	ThreadSetting,
	BitrateKbps,
	BitrateMode,
	EncoderChannelConfig,
	EncoderSettingsCapabilities,
} from './audio';
import { defaultEncoderSettings } from './audio';

export type EncoderFlavor = EncoderType;
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

export type EncoderSettingsLike =
	| Partial<EncoderSettingsState>
	| EncoderSettings
	| null
	| undefined;

type EncoderCapabilities = EncoderSettingsCapabilities | null | undefined;

const defaultBitrateModeFor = (
	encoderType: EncoderType,
	capabilities: EncoderCapabilities,
	fallback: BitrateMode,
): BitrateMode =>
	capabilities?.bitrateModesByEncoder.find((entry) => entry.encoderType === encoderType)
		?.defaultMode ?? fallback;

const supportedBitrateModeKindsFor = (
	encoderType: EncoderType,
	capabilities: EncoderCapabilities,
): readonly string[] =>
	capabilities?.bitrateModesByEncoder.find((entry) => entry.encoderType === encoderType)
		?.allowedModes ?? [];

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

const isBoundaryEncoderType = (
	value: unknown,
	capabilities: EncoderCapabilities,
): value is EncoderType =>
	typeof value === 'string' &&
	(!capabilities || capabilities.encoderTypes.includes(value as EncoderType));

const isBoundaryEncoderSettings = (
	value: unknown,
	capabilities: EncoderCapabilities,
): value is EncoderSettings => {
	if (!value || typeof value !== 'object') {
		return false;
	}
	const candidate = value as Record<string, unknown>;
	const channels = candidate.channels;
	const bitrateMode = candidate.bitrateMode;
	return (
		isBoundaryEncoderType(candidate.encoderType, capabilities) &&
		typeof candidate.bitrateKbps === 'number' &&
		typeof channels === 'string' &&
		(!capabilities || capabilities.channelOptions.includes(channels as EncoderChannelConfig)) &&
		typeof candidate.afterburner === 'boolean' &&
		isThreadSetting(candidate.threads) &&
		isBitrateMode(bitrateMode) &&
		(candidate.twoloop === undefined || typeof candidate.twoloop === 'boolean')
	);
};

const sanitizeBitrate = (
	value: unknown,
	fallback: EncoderSettings['bitrateKbps'],
	capabilities: EncoderCapabilities,
): EncoderSettings['bitrateKbps'] => {
	if (
		typeof value === 'number' &&
		(!capabilities || capabilities.bitrateKbpsOptions.includes(value))
	) {
		return value as BitrateKbps;
	}
	return fallback;
};

const sanitizeChannels = (
	value: unknown,
	fallback: EncoderSettings['channels'],
	capabilities: EncoderCapabilities,
): EncoderSettings['channels'] => {
	if (
		typeof value === 'string' &&
		(!capabilities || capabilities.channelOptions.includes(value as EncoderChannelConfig))
	) {
		return value as EncoderChannelConfig;
	}
	return fallback;
};

const sanitizeBitrateMode = (
	value: unknown,
	encoderType: EncoderType,
	fallback: BitrateMode,
	capabilities: EncoderCapabilities,
): BitrateMode => {
	if (isBitrateMode(value)) {
		const supportedKinds = supportedBitrateModeKindsFor(encoderType, capabilities);
		const supportsMode = capabilities ? supportedKinds.includes(value.mode) : true;
		if (!supportsMode) {
			return defaultBitrateModeFor(encoderType, capabilities, fallback);
		}
		if (value.mode === 'cbr' || value.mode === 'cvbr') return { mode: value.mode };

		const numeric = Number(value.value ?? capabilities?.vbrLevelDefault ?? 3);
		if (Number.isFinite(numeric)) {
			const rounded = Math.round(numeric);
			const level = capabilities
				? Math.max(capabilities.vbrLevelMin, Math.min(capabilities.vbrLevelMax, rounded))
				: rounded;
			return { mode: 'vbr', value: level };
		}
		return { mode: 'vbr', value: capabilities?.vbrLevelDefault ?? 3 };
	}
	return fallback ?? defaultBitrateModeFor(encoderType, capabilities, fallback);
};

const sanitizeThreads = (
	value: unknown,
	fallback: ThreadSetting,
	capabilities: EncoderCapabilities,
): ThreadSetting => {
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
			const rounded = Math.round(numeric);
			const clamped = capabilities
				? Math.max(capabilities.threadFixedMin, Math.min(capabilities.threadFixedMax, rounded))
				: rounded;
			return { mode: 'fixed', value: clamped };
		}
		default:
			return fallback;
	}
};

const resolveEncoderType = (
	flavor: EncoderFlavor | undefined,
	fallback: EncoderType,
	capabilities: EncoderCapabilities,
): EncoderType => (isBoundaryEncoderType(flavor, capabilities) ? flavor : fallback);

const normalizeBoundary = (
	candidate: EncoderSettings,
	base: EncoderSettings,
	capabilities: EncoderCapabilities,
): EncoderSettings => {
	const encoderType = candidate.encoderType;
	const bitrateKbps = sanitizeBitrate(candidate.bitrateKbps, base.bitrateKbps, capabilities);
	const bitrateMode = sanitizeBitrateMode(
		candidate.bitrateMode,
		encoderType,
		base.bitrateMode,
		capabilities,
	);
	const channels = sanitizeChannels(candidate.channels, base.channels, capabilities);
	const threads = sanitizeThreads(candidate.threads, base.threads, capabilities);
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
	capabilities?: EncoderSettingsCapabilities | null,
): EncoderSettings => {
	const base = normalizeBoundary(defaults, defaultEncoderSettings(), capabilities);

	if (isBoundaryEncoderSettings(source, capabilities)) {
		return normalizeBoundary(source, base, capabilities);
	}

	const ui = (source ?? {}) as Partial<EncoderSettingsState>;
	const encoderType = resolveEncoderType(ui.flavor, base.encoderType, capabilities);
	const bitrateKbps = sanitizeBitrate(ui.bitrateKbps, base.bitrateKbps, capabilities);
	const bitrateMode = sanitizeBitrateMode(
		ui.bitrateMode,
		encoderType,
		base.bitrateMode,
		capabilities,
	);
	const channels = sanitizeChannels(ui.channels, base.channels, capabilities);
	const threads = sanitizeThreads(ui.threads ?? base.threads, base.threads, capabilities);
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
