import type { EncoderDefaults } from '../../types/appSettings';
import type {
	EncoderAvailability,
	EncoderSettingsCapabilities,
	EncodingRequestConfig,
	SampleRateConfig,
} from '../../types/audio';
import {
	type EncoderFlavor,
	type EncoderSettingsState,
	toBoundaryEncoderSettings,
} from '../../types/encoder';
import { estimateKbpsFromRequest } from './estimate';

export type BitrateModeSelection = 'vbr' | 'cvbr' | 'cbr';
export type EncodingField =
	| 'encoder'
	| 'bitrateMode'
	| 'quality'
	| 'bitrate'
	| 'sampleRate'
	| 'channels';

export type EncodingOption = {
	readonly value: string;
	readonly label: string;
	readonly disabled?: boolean;
};

export type EncodingView = {
	readonly flavor: string;
	readonly flavorOptions: ReadonlyArray<EncodingOption>;
	readonly flavorDisabled: boolean;
	readonly availabilityHint: string;
	readonly profileDisplay: string;
	readonly bitrateMode: BitrateModeSelection;
	readonly bitrateModeOptions: ReadonlyArray<EncodingOption>;
	readonly bitrateModeDisabled: boolean;
	readonly qualityBitrateLabel: 'Quality' | 'Bitrate';
	readonly showQuality: boolean;
	readonly quality: number;
	readonly qualityOptions: ReadonlyArray<EncodingOption>;
	readonly bitrate: number;
	readonly bitrateOptions: ReadonlyArray<EncodingOption>;
	readonly estimatedBitrateText: string;
	readonly sampleRate: string;
	readonly sampleRateOptions: ReadonlyArray<EncodingOption>;
	readonly sampleRateDisabled: boolean;
	readonly sampleRateHint: string;
	readonly channels: string;
	readonly channelOptions: ReadonlyArray<EncodingOption>;
	readonly channelsDisabled: boolean;
	readonly channelsHint: string;
	readonly afterburner: boolean;
};

export type EncodingBag = {
	flavor: EncoderFlavor;
	bitrateMode: BitrateModeSelection;
	quality: number;
	bitrate: number;
	sampleRate: string;
	channels: NonNullable<EncoderSettingsState['channels']>;
	afterburner: boolean;
	capabilities: EncoderSettingsCapabilities | null;
	availability: EncoderAvailability | null;
	sampleRateHint: string;
	channelsHint: string;
};

const DEFAULT_SAMPLE_RATE_HINT = 'Auto -> source audio';
const DEFAULT_CHANNELS_HINT = 'Auto -> source audio';
const DEFAULT_AVAILABILITY_HINT = 'Checking encoder availability…';
const NATIVE_AAC_WARNING =
	'Native AAC (FFmpeg) may sound degraded on speech (known issue; prefer Auto/Apple/FDK).';
const ENCODER_PROFILES: Record<EncoderFlavor, string> = {
	auto: 'HE-AAC v1',
	fdk_he_aac: 'HE-AAC v1',
	aac_at: 'AAC-LC',
	native_aac: 'AAC-LC',
};

export function createDefaultBag(): EncodingBag {
	return {
		flavor: 'auto',
		bitrateMode: 'vbr',
		quality: 3,
		bitrate: 64,
		sampleRate: 'auto',
		channels: 'auto',
		afterburner: true,
		capabilities: null,
		availability: null,
		sampleRateHint: DEFAULT_SAMPLE_RATE_HINT,
		channelsHint: DEFAULT_CHANNELS_HINT,
	};
}

function rangeOptions(min: number, max: number): number[] {
	const values: number[] = [];
	for (let value = min; value <= max; value += 1) {
		values.push(value);
	}
	return values;
}

function bitrateModeFromKind(kind: string): BitrateModeSelection {
	return kind === 'cvbr' || kind === 'cbr' ? kind : 'vbr';
}

function encoderFlavorLabel(flavor: EncoderFlavor): string {
	switch (flavor) {
		case 'fdk_he_aac':
			return 'FDK AAC';
		case 'aac_at':
			return 'Apple AAC';
		case 'native_aac':
			return 'Native AAC (FFmpeg)';
		default:
			return 'Auto';
	}
}

function compactToolchainPath(path: string | null | undefined): string | null {
	if (!path) return null;
	if (path.length <= 38) return path;
	const normalized = path.replace(/\\/g, '/');
	const parts = normalized.split('/').filter(Boolean);
	const parent = parts[parts.length - 2];
	const filename = parts[parts.length - 1];
	if (!parent || !filename) return path;
	if (normalized.startsWith('/opt/homebrew/')) {
		return `/opt/homebrew/.../${parent}/${filename}`;
	}
	if (normalized.startsWith('/usr/local/')) {
		return `/usr/local/.../${parent}/${filename}`;
	}
	const root = normalized.startsWith('/') ? `/${parts[0]}` : (parts[0] ?? '');
	return root ? `${root}/.../${parent}/${filename}` : `.../${parent}/${filename}`;
}

function effectiveEncoder(bag: EncodingBag): EncoderFlavor {
	if (bag.flavor !== 'auto') return bag.flavor;
	return bag.availability?.autoEncoder ?? 'auto';
}

function disabledEncoderOptions(availability: EncoderAvailability | null) {
	return {
		fdk_he_aac: !availability?.fdkAvailable,
		aac_at: availability ? !availability.aacAtAvailable : false,
		native_aac: availability ? !availability.nativeAacAvailable : false,
	};
}

function flavorIsUnavailable(bag: EncodingBag): boolean {
	if (!bag.availability || bag.flavor === 'auto') return false;
	const disabled = disabledEncoderOptions(bag.availability);
	if (bag.flavor === 'fdk_he_aac') return disabled.fdk_he_aac;
	if (bag.flavor === 'aac_at') return disabled.aac_at;
	if (bag.flavor === 'native_aac') return disabled.native_aac;
	return false;
}

function sampleRateFromBag(bag: EncodingBag): SampleRateConfig {
	if (bag.sampleRate === 'auto') return 'auto';
	const parsed = Number.parseInt(bag.sampleRate, 10);
	if (!Number.isFinite(parsed)) return 'auto';
	if (!bag.capabilities) return { explicit: parsed };
	return bag.capabilities.explicitSampleRates.includes(parsed) ? { explicit: parsed } : 'auto';
}

function uiSettingsFromBag(bag: EncodingBag): EncoderSettingsState {
	return {
		flavor: bag.flavor,
		channels: bag.channels,
		bitrateKbps: bag.bitrate as EncoderSettingsState['bitrateKbps'],
		bitrateMode:
			bag.bitrateMode === 'vbr' ? { mode: 'vbr', value: bag.quality } : { mode: bag.bitrateMode },
		fdkAfterburner: bag.afterburner,
	};
}

export function bagRequest(bag: EncodingBag): EncodingRequestConfig {
	return {
		encoderSettings: toBoundaryEncoderSettings(uiSettingsFromBag(bag), undefined, bag.capabilities),
		sampleRate: sampleRateFromBag(bag),
	};
}

export function bagDefaults(bag: EncodingBag): EncoderDefaults {
	const request = bagRequest(bag);
	return {
		settings: request.encoderSettings,
		sampleRate: request.sampleRate,
	};
}

export function bagEstimateKbps(bag: EncodingBag): number {
	return estimateKbpsFromRequest(bagRequest(bag));
}

function availabilityHint(bag: EncodingBag): string {
	if (!bag.availability) return DEFAULT_AVAILABILITY_HINT;
	const selected = bag.flavor;
	const effective = effectiveEncoder(bag);
	if (selected === 'auto') {
		if (effective === 'fdk_he_aac') return fdkAvailabilityHint(bag);
		if (effective === 'aac_at') return 'Auto will use Apple AAC.';
		return `Auto will use Native AAC (FFmpeg). ${NATIVE_AAC_WARNING}`;
	}
	if (effective === 'native_aac') {
		if (!bag.availability.nativeAacAvailable) {
			return 'Native AAC (FFmpeg) is unavailable in this build.';
		}
		return NATIVE_AAC_WARNING;
	}
	if (effective === 'fdk_he_aac') {
		if (!bag.availability.fdkAvailable) {
			return 'FDK AAC needs an auto-detectable FFmpeg with libfdk_aac.';
		}
		return fdkAvailabilityHint(bag);
	}
	if (!bag.availability.aacAtAvailable) return 'Apple AAC is unavailable in this build.';
	return 'Apple AAC available';
}

function fdkAvailabilityHint(bag: EncodingBag): string {
	const path = compactToolchainPath(bag.availability?.detectedToolchainPath);
	const pathSegment = path ? ` via ${path}` : '';
	const afterburnerSegment = bag.afterburner ? 'Afterburner on.' : 'Afterburner off.';
	return `Using external FDK AAC${pathSegment}. ${afterburnerSegment}`;
}

function autoOptionLabel(bag: EncodingBag): string {
	if (bag.flavor === 'auto' && bag.availability) {
		return `Auto (${encoderFlavorLabel(effectiveEncoder(bag))})`;
	}
	return 'Auto';
}

function encoderLabel(bag: EncodingBag, value: string): string {
	if (value === 'auto') return autoOptionLabel(bag);
	if (value === 'fdk_he_aac' || value === 'aac_at' || value === 'native_aac') {
		return encoderFlavorLabel(value);
	}
	return value;
}

function qualityLabel(bag: EncodingBag, value: number): string {
	if (value === bag.capabilities?.vbrLevelMin) return `${value} (Smallest)`;
	if (value === bag.capabilities?.vbrLevelDefault) return `${value} (Recommended)`;
	if (value === bag.capabilities?.vbrLevelMax) return `${value} (Largest)`;
	return String(value);
}

function sampleRateLabel(value: string): string {
	return value === 'auto' ? 'Auto' : `${value} Hz`;
}

function channelLabel(value: string): string {
	switch (value) {
		case 'auto':
			return 'Auto';
		case 'mono':
			return 'Mono';
		case 'stereo':
			return 'Stereo';
		default:
			return value;
	}
}

function sampleRateDetail(bag: EncodingBag): string {
	if (bag.sampleRate === 'auto') return bag.sampleRateHint;
	return `Using ${sampleRateLabel(bag.sampleRate)}.`;
}

function channelsDetail(bag: EncodingBag): string {
	if (bag.channels === 'auto') return bag.channelsHint;
	return `Using ${channelLabel(bag.channels)}.`;
}

export function projectView(bag: EncodingBag): EncodingView {
	const estimate = bagEstimateKbps(bag);
	const showQuality = bag.bitrateMode === 'vbr';
	const disabled = disabledEncoderOptions(bag.availability);
	const flavorOptions: EncodingOption[] =
		bag.capabilities === null
			? [{ value: 'auto', label: 'Loading…', disabled: true }]
			: bag.capabilities.encoderTypes.map((flavor) => ({
					value: flavor,
					label: encoderLabel(bag, flavor),
					disabled: flavor !== 'auto' && Boolean(disabled[flavor as keyof typeof disabled]),
				}));
	const bitrateModeOptions: EncodingOption[] = (
		bag.capabilities
			? [
					...new Set(
						bag.capabilities.bitrateModesByEncoder.flatMap((entry) =>
							entry.allowedModes.map(bitrateModeFromKind),
						),
					),
				]
			: []
	).map((mode) => ({
		value: mode,
		label: mode.toUpperCase(),
		disabled: !bitrateModeAllowed(bag, mode),
	}));
	const qualityOptions = bag.capabilities
		? rangeOptions(bag.capabilities.vbrLevelMin, bag.capabilities.vbrLevelMax).map((value) => ({
				value: String(value),
				label: qualityLabel(bag, value),
			}))
		: [];
	const bitrateOptions = (bag.capabilities?.bitrateKbpsOptions ?? []).map((value) => ({
		value: String(value),
		label: `${value} kbps`,
	}));
	const sampleRateOptions = bag.capabilities
		? [
				...(bag.capabilities.sampleRateAuto ? ['auto'] : []),
				...bag.capabilities.explicitSampleRates.map(String),
			].map((value) => ({ value, label: sampleRateLabel(value) }))
		: [];
	const channelOptions = (bag.capabilities?.channelOptions ?? []).map((value) => ({
		value,
		label: channelLabel(value),
	}));

	return {
		flavor: bag.flavor,
		flavorOptions,
		flavorDisabled: bag.capabilities === null,
		availabilityHint: availabilityHint(bag),
		profileDisplay: ENCODER_PROFILES[effectiveEncoder(bag)] ?? 'AAC-LC',
		bitrateMode: bag.bitrateMode,
		bitrateModeOptions,
		bitrateModeDisabled: bitrateModeOptions.length === 0,
		qualityBitrateLabel: showQuality ? 'Quality' : 'Bitrate',
		showQuality,
		quality: bag.quality,
		qualityOptions,
		bitrate: bag.bitrate,
		bitrateOptions,
		estimatedBitrateText: showQuality ? `Est: ~${estimate} kbps` : `Target: ${estimate} kbps`,
		sampleRate: bag.sampleRate,
		sampleRateOptions,
		sampleRateDisabled: sampleRateOptions.length === 0,
		sampleRateHint: sampleRateDetail(bag),
		channels: bag.channels,
		channelOptions,
		channelsDisabled: channelOptions.length === 0,
		channelsHint: channelsDetail(bag),
		afterburner: bag.afterburner,
	};
}

function bitrateModeAllowed(bag: EncodingBag, mode: BitrateModeSelection): boolean {
	const capability = bag.capabilities?.bitrateModesByEncoder.find(
		(entry) => entry.encoderType === effectiveEncoder(bag),
	);
	const allowed = capability?.allowedModes.map(bitrateModeFromKind) ?? [];
	return allowed.includes(mode);
}

export type SyncResult = {
	readonly flavorReset: boolean;
};

export function syncPolicy(bag: EncodingBag): SyncResult {
	const flavorReset = flavorIsUnavailable(bag);
	if (flavorReset) {
		bag.flavor = 'auto';
	}
	const capability = bag.capabilities?.bitrateModesByEncoder.find(
		(entry) => entry.encoderType === effectiveEncoder(bag),
	);
	const allowed = capability?.allowedModes.map(bitrateModeFromKind) ?? [];
	const defaultMode = capability
		? bitrateModeFromKind(capability.defaultMode.mode)
		: bag.bitrateMode;
	if (allowed.length > 0 && !allowed.includes(bag.bitrateMode)) {
		bag.bitrateMode = defaultMode;
	}
	return { flavorReset };
}

export function applyCapabilities(
	bag: EncodingBag,
	capabilities: EncoderSettingsCapabilities | null,
): void {
	const usable =
		capabilities && Array.isArray(capabilities.bitrateKbpsOptions) ? capabilities : null;
	bag.capabilities = usable;
	bag.availability = usable?.availability ?? capabilities?.availability ?? null;
	if (!usable) return;
	if (!usable.bitrateKbpsOptions.includes(bag.bitrate)) {
		bag.bitrate = usable.bitrateKbpsOptions[0] ?? bag.bitrate;
	}
	bag.quality = Math.min(usable.vbrLevelMax, Math.max(usable.vbrLevelMin, bag.quality));
	const sampleRates = [
		...(usable.sampleRateAuto ? ['auto'] : []),
		...usable.explicitSampleRates.map(String),
	];
	if (!sampleRates.includes(bag.sampleRate)) {
		bag.sampleRate = usable.sampleRateAuto ? 'auto' : (sampleRates[0] ?? 'auto');
	}
	if (!usable.channelOptions.includes(bag.channels)) {
		bag.channels = usable.channelOptions[0] ?? bag.channels;
	}
}

export function applyDefaultsToBag(bag: EncodingBag, defaults: EncoderDefaults): void {
	const settings = toBoundaryEncoderSettings(defaults.settings, undefined, bag.capabilities);
	bag.flavor = settings.encoderType;
	bag.bitrateMode = bitrateModeFromKind(settings.bitrateMode.mode);
	bag.quality =
		settings.bitrateMode.mode === 'vbr'
			? settings.bitrateMode.value
			: (bag.capabilities?.vbrLevelDefault ?? bag.quality);
	bag.bitrate = settings.bitrateKbps;
	bag.channels = settings.channels;
	bag.afterburner = settings.afterburner;
	if (defaults.sampleRate === 'auto') {
		bag.sampleRate = 'auto';
	} else if (
		!bag.capabilities ||
		bag.capabilities.explicitSampleRates.includes(defaults.sampleRate.explicit)
	) {
		bag.sampleRate = String(defaults.sampleRate.explicit);
	} else {
		bag.sampleRate = 'auto';
	}
}

export function selectField(bag: EncodingBag, field: EncodingField, value: string): boolean {
	switch (field) {
		case 'encoder': {
			if (
				value !== 'auto' &&
				value !== 'fdk_he_aac' &&
				value !== 'aac_at' &&
				value !== 'native_aac'
			) {
				return false;
			}
			if (bag.flavor === value) return false;
			bag.flavor = value;
			return true;
		}
		case 'bitrateMode': {
			if (value !== 'vbr' && value !== 'cvbr' && value !== 'cbr') return false;
			if (bag.bitrateMode === value) return false;
			bag.bitrateMode = value;
			return true;
		}
		case 'quality': {
			const parsed = Number.parseInt(value, 10);
			if (!Number.isFinite(parsed)) return false;
			const min = bag.capabilities?.vbrLevelMin ?? parsed;
			const max = bag.capabilities?.vbrLevelMax ?? parsed;
			const next = Math.min(max, Math.max(min, parsed));
			if (bag.quality === next) return false;
			bag.quality = next;
			return true;
		}
		case 'bitrate': {
			const parsed = Number.parseInt(value, 10);
			if (!Number.isFinite(parsed)) return false;
			if (bag.capabilities && !bag.capabilities.bitrateKbpsOptions.includes(parsed)) {
				return false;
			}
			if (bag.bitrate === parsed) return false;
			bag.bitrate = parsed;
			return true;
		}
		case 'sampleRate': {
			const options = bag.capabilities
				? [
						...(bag.capabilities.sampleRateAuto ? ['auto'] : []),
						...bag.capabilities.explicitSampleRates.map(String),
					]
				: ['auto'];
			const next = options.includes(value) ? value : 'auto';
			if (bag.sampleRate === next) return false;
			bag.sampleRate = next;
			return true;
		}
		case 'channels': {
			if (
				value !== 'auto' &&
				value !== 'mono' &&
				value !== 'stereo' &&
				!(bag.capabilities?.channelOptions.includes(value as EncodingBag['channels']) ?? false)
			) {
				return false;
			}
			if (
				bag.capabilities &&
				!bag.capabilities.channelOptions.includes(value as EncodingBag['channels'])
			) {
				return false;
			}
			if (bag.channels === value) return false;
			bag.channels = value as EncodingBag['channels'];
			return true;
		}
	}
}
